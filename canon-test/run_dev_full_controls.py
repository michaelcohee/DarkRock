#!/usr/bin/env python3
"""Full, self-contained control ledgers; defaults to the original dev split."""
from __future__ import annotations
from collections import defaultdict
from pathlib import Path
import csv,hashlib,json,os,struct,subprocess,tempfile
from fastcdc import fastcdc
from dxf_spelling_undo import normalize,restore

ROOT=Path(__file__).resolve().parent
CORPUS=Path(os.environ.get('DARKROCK_CORPUS_ROOT',ROOT/'cad-corpus-v1'))
SPLIT=os.environ.get('DARKROCK_SPLIT','dev')
DEV=CORPUS/SPLIT
ARMS=tuple(os.environ.get('DARKROCK_ARMS','P,A1,A2,C1,C2').split(','))
MODES=tuple(os.environ.get('DARKROCK_MODES','N,Z').split(','))
NO_PREFIX=os.environ.get('DARKROCK_NO_PREFIX')=='1'
OUTPUT=Path(os.environ.get('DARKROCK_OUTPUT',ROOT/'cad-dev-full-controls.json'))
SELECT=ROOT.parent/'target'/'release'/'canon_pack_select'
MEASURE=ROOT.parent/'target'/'release'/'canon_full_ledger_measure'
STRIPE=1048576
BLOCK=262144

def h(data):return hashlib.sha256(data).digest()
def u32(n):return struct.pack('<I',n)
def u64(n):return struct.pack('<Q',n)
def zstd(args,data=None):return subprocess.run(['zstd','-q','-3',*args],input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True).stdout
def physical_size(n):
    def rs(k):return 1572864*(k//STRIPE)+(0 if k%STRIPE==0 else 6*((k%STRIPE+3)//4))
    catalog=21+240*((n+STRIPE-1)//STRIPE)
    return rs(n)+rs(catalog)+6*(32+240*((catalog+STRIPE-1)//STRIPE))

manifest_path=(CORPUS/'manifest.csv') if (CORPUS/'manifest.csv').exists() else (ROOT/'manifest.csv')
manifest=[r for r in csv.DictReader(manifest_path.open()) if r['split']==SPLIT]
assert manifest
sources=[]
for r in sorted(manifest,key=lambda r:r['path']):
    path=DEV/Path(r['path']).relative_to(SPLIT);raw=path.read_bytes()
    assert len(raw)==int(r['bytes']) and h(raw).hex()==r['sha256']
    sources.append((r,path,raw))

def select_zstd(raws,temp):
    framed=temp/'framed';packed=temp/'selected';idx=temp/'selected.idx'
    with framed.open('wb') as f:
        for raw in raws:f.write(u64(len(raw)));f.write(raw)
    subprocess.run([str(SELECT),str(framed),str(packed),str(idx)],check=True,capture_output=True)
    blob=packed.read_bytes();rows=[line.split('\t') for line in idx.read_text().splitlines()]
    assert len(rows)==len(raws)
    out=[];at=0
    for raw,row in zip(raws,rows):
        raw_len,pay_len,kind,sha=row;pay_len=int(pay_len);payload=blob[at:at+pay_len];at+=pay_len
        assert int(raw_len)==len(raw) and sha==h(raw).hex()
        decoded=zstd(['-d','-c'],payload) if kind=='zstd' else payload
        assert decoded==raw
        out.append((1 if kind=='zstd' else 0,payload,None))
    assert at==len(blob);return out

def encode_arm(arm,mode,temp):
    raw_objects=[];refs=[];undo_refs=[];seen=defaultdict(list);patches=0;hits=0;family_first={};a2_counts=[]
    for r,path,raw in sources:
        undo=None
        if arm=='A2':data,undo,counts=normalize(raw);a2_counts.append(counts)
        else:data=raw
        parts=([data[i:i+BLOCK] for i in range(0,len(data),BLOCK)] if arm in ('A1','A2') else
               [c.data for c in fastcdc(raw,min_size=4096,avg_size=16384,max_size=65536,fat=True)] if arm=='C1' else [raw])
        ids=[]
        for part in parts:
            digest=h(part);found=next((i for i in seen[digest] if raw_objects[i]==part),None)
            if arm=='P':found=None
            if found is None:found=len(raw_objects);raw_objects.append(part);seen[digest].append(found)
            else:hits+=1
            ids.append(found)
        assert b''.join(raw_objects[i] for i in ids)==data
        refs.append(ids)
        if undo is not None:
            digest=h(undo);found=next((i for i in seen[digest] if raw_objects[i]==undo),None)
            if found is None:found=len(raw_objects);raw_objects.append(undo);seen[digest].append(found)
            else:hits+=1
            undo_refs.append(found)
        else:undo_refs.append(None)
        if arm=='C2' and r['family'] not in family_first:family_first[r['family']]=ids[0]
    if mode=='N' or arm=='P':selected=[(0,raw,None) for raw in raw_objects]
    elif arm in ('A1','A2','C1'):selected=select_zstd(raw_objects,temp)
    else:
        selected=[]
        object_source_path={}
        for (_,path,_),ids in zip(sources,refs):object_source_path.setdefault(ids[0],path)
        object_family={ids[0]:r['family'] for (r,_,_),ids in zip(sources,refs)}
        for obj_id,raw in enumerate(raw_objects):
            path=object_source_path[obj_id];standalone=zstd(['-c',str(path)]);assert zstd(['-d','-c'],standalone)==raw
            payload=standalone;tag=1;base=None
            first=family_first[object_family[obj_id]]
            if first!=obj_id:
                base_path=object_source_path[first]
                patch=zstd(['--patch-from='+str(base_path),'-c',str(path)])
                assert zstd(['-d','--patch-from='+str(base_path),'-c'],patch)==raw
                if len(patch)<len(payload):payload=patch;tag=2;base=first;patches+=1
            selected.append((tag,payload,base))
    return raw_objects,selected,refs,undo_refs,hits,patches,a2_counts

ARM_ID={'P':0,'A1':1,'A2':2,'A3':3,'C1':4,'C2':5}
def serialize(arm,mode,raw_objects,selected,refs,undo_refs,source_count):
    # Bytes decoded from this format are sufficient to restore every source file.
    out=bytearray(b'DRBL1\0');out+=bytes([ARM_ID[arm],{'N':0,'Z':1}[mode]])+u32(len(raw_objects))
    sections={'literal_payloads':0,'object_headers':0,'ordered_references':0,'file_manifests':0,'hash_index':0,'framing':len(out)}
    for raw,(tag,payload,base) in zip(raw_objects,selected):
        header=u64(len(raw))+h(raw)+bytes([tag])+u32(base if base is not None else 0xffffffff)+u64(len(payload))
        out+=header+payload;sections['object_headers']+=len(header);sections['literal_payloads']+=len(payload)
    out+=u32(source_count);sections['framing']+=4
    for file_index,((_,_,raw),ids) in enumerate(zip(sources[:source_count],refs)):
        head=u64(len(raw))+h(raw)+u32(len(ids));out+=head;sections['file_manifests']+=len(head)
        for i in ids:out+=u32(i);sections['ordered_references']+=4
        if arm=='A2':out+=u32(undo_refs[file_index]);sections['ordered_references']+=4
    out+=u32(len(raw_objects));sections['hash_index']+=4
    for i,raw in enumerate(raw_objects):out+=h(raw)+u32(i);sections['hash_index']+=36
    assert len(out)==sum(sections.values())
    return bytes(out),sections

def verify(blob,arm,mode,expected,temp):
    at=0
    def take(n):
        nonlocal at
        b=blob[at:at+n];assert len(b)==n;at+=n;return b
    def read32():return struct.unpack('<I',take(4))[0]
    def read64():return struct.unpack('<Q',take(8))[0]
    assert take(6)==b'DRBL1\0' and take(1)==bytes([ARM_ID[arm]]) and take(1)==bytes([{'N':0,'Z':1}[mode]])
    objects=[]
    for _ in range(read32()):
        n=read64();sha=take(32);tag=take(1)[0];base=read32();payload=take(read64())
        if tag==0:raw=payload
        elif tag==1:raw=zstd(['-d','-c'],payload)
        else:
            assert tag==2 and base<len(objects)
            basefile=temp/'decode-base';basefile.write_bytes(objects[base]);raw=zstd(['-d','--patch-from='+str(basefile),'-c'],payload)
        assert len(raw)==n and h(raw)==sha;objects.append(raw)
    assert read32()==len(expected)
    for (_,_,original) in expected:
        n=read64();sha=take(32);count=read32();joined=b''.join(objects[read32()] for _ in range(count))
        restored=restore(joined,objects[read32()]) if arm=='A2' else joined
        assert restored==original and len(restored)==n and h(restored)==sha
    assert read32()==len(objects)
    for i,raw in enumerate(objects):assert take(32)==h(raw) and read32()==i
    assert at==len(blob)

results={}
with tempfile.TemporaryDirectory(prefix='darkrock-cad-dev-full-') as td:
    temp=Path(td)
    for arm in ARMS:
        arm_rows={}
        for mode in MODES:
            raw_objects,selected,refs,undo_refs,hits,patches,a2_counts=encode_arm(arm,mode,temp)
            snapshots=[]
            for count in ([] if NO_PREFIX else range(1,len(sources)+1)):
                used=sorted(set(i for ids in refs[:count] for i in ids)|{i for i in undo_refs[:count] if i is not None})
                remap={old:new for new,old in enumerate(used)}
                subset_raw=[raw_objects[i] for i in used];subset_sel=[]
                for old in used:
                    tag,payload,base=selected[old];subset_sel.append((tag,payload,remap[base] if base is not None else None))
                subset_refs=[[remap[i] for i in ids] for ids in refs[:count]]
                subset_undo=[remap[i] if i is not None else None for i in undo_refs[:count]]
                prefix,parts=serialize(arm,mode,subset_raw,subset_sel,subset_refs,subset_undo,count)
                snapshots.append(physical_size(len(prefix)))
            blob,sections=serialize(arm,mode,raw_objects,selected,refs,undo_refs,len(sources));verify(blob,arm,mode,sources,temp)
            packed=temp/f'{arm}-{mode}.bin';packed.write_bytes(blob)
            out=subprocess.check_output([str(MEASURE),str(packed)],text=True).strip()
            physical={k:int(v) if v.isdigit() else v for k,v in (x.split('=',1) for x in out.split(','))}
            if snapshots:assert physical['total_physical_bytes']==snapshots[-1]
            arm_rows[mode]={'physical':physical,'sections':sections,'unique_objects':len(raw_objects),'references':sum(map(len,refs)),
                            'confirmed_hash_hits':hits,'patches':patches,'incremental_physical_bytes':([snapshots[0]]+[b-a for a,b in zip(snapshots,snapshots[1:])]) if snapshots else None,
                            'all_files_byte_and_sha256_restored':True,'a2_normalization_counts':a2_counts if arm=='A2' else None}
            print(arm,mode,physical['total_physical_bytes'],flush=True)
        results[arm]=arm_rows
report={'scope':f'{len(sources)} DXFs from {CORPUS.name}/{SPLIT}; sorted manifest path; {sum(len(raw) for _,_,raw in sources)} source bytes','source_paths':[r['path'] for r,_,_ in sources],
        'modes':'N raw objects; Z production representation selector for A1/A2/C1, zstd standalone/patch for C2; P remains raw in both rows; C2 N is exact whole-file dedup without patches',
        'fastcdc_version':__import__('fastcdc').__version__,'zstd_version':subprocess.check_output(['zstd','--version'],text=True).strip(),
        'root_manifest_policy':'manifest catalog encoded 4+2; each 240-byte catalog-stripe manifest plus 32-byte external trust hash charged as six physical copies',
        'arms':results}
OUTPUT.write_text(json.dumps(report,indent=2)+'\n')
