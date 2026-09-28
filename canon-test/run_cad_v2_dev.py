#!/usr/bin/env python3
"""V2 dev-only mapping: geometry-key candidate ranking, byte-exact file patches."""
from pathlib import Path
from collections import Counter,defaultdict
import csv, hashlib, json, os, subprocess, tempfile

ROOT=Path(__file__).resolve().parent
DEV=ROOT/'cad-corpus-v1'/'dev'
KEYS=ROOT.parent/'target'/'release'/'cad_geometry_keys'
ENV=dict(os.environ,RUSTC=os.environ.get('RUSTC',str(Path.home()/'.cargo/bin/rustc')))
assert KEYS.exists(), 'build cad_geometry_keys first'

def zstd(args,data=None):
    return subprocess.run(['zstd','-q',*args],input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True).stdout

rows=sorted((r for r in csv.DictReader((ROOT/'manifest.csv').open()) if r['split']=='dev'),key=lambda r:r['path'])
assert len(rows)==44
files=[]
for r in rows:
    p=DEV/Path(r['path']).relative_to('dev');raw=p.read_bytes()
    assert hashlib.sha256(raw).hexdigest()==r['sha256']
    keys=Counter(subprocess.check_output([str(KEYS),str(p)],text=True).splitlines())
    parsed_bytes,raw_bytes,eligible_entities=map(int,subprocess.check_output([str(KEYS),'--coverage',str(p)],text=True).split())
    assert parsed_bytes+raw_bytes==len(raw)
    files.append((r,p,raw,keys,(parsed_bytes,raw_bytes,eligible_entities)))

chosen=[];exact=defaultdict(list);methods=defaultdict(int);choices=[];records=[]
with tempfile.TemporaryDirectory(prefix='darkrock-cad-v2-dev-') as td:
    temp=Path(td)
    for i,(r,p,raw,keys,coverage) in enumerate(files):
        h=hashlib.sha256(raw).digest()
        duplicate=next((j for j in exact[h] if files[j][2]==raw),None)
        if duplicate is not None:
            methods['exact_duplicate']+=1;records.append(('exact_duplicate',duplicate,b''))
            choices.append({'file':r['path'],'method':'exact_duplicate','bytes':0,'parsed_bytes':coverage[0],'raw_bytes':coverage[1],'eligible_entities':coverage[2]});continue
        exact[h].append(i)
        standalone=zstd(['-c'],raw);assert zstd(['-d','-c'],standalone)==raw
        selection=(standalone,None,'standalone')
        # Rank previous files by overlapping canonical geometry occurrences.
        scored=[]
        for j,(prev,base,base_raw,base_keys,_) in enumerate(files[:i]):
            shared=sum((keys & base_keys).values())
            denom=max(1,min(sum(keys.values()),sum(base_keys.values())))
            scored.append((shared/denom,shared,j))
        scored.sort(reverse=True)
        candidates=[j for _,_,j in scored[:3]]
        # Always include C2's first family base, so V2 has the same option.
        first=next((j for j,(prev,_,_,_,_) in enumerate(files[:i]) if prev['family']==r['family']),None)
        if first is not None and first not in candidates:candidates.append(first)
        for j in candidates:
            base=files[j][1]
            patch=subprocess.run(['zstd','-q','--patch-from='+str(base),'-c',str(p)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True).stdout
            if len(patch)<len(selection[0]):
                assert zstd(['-d','--patch-from='+str(base),'-c'],patch)==raw
                selection=(patch,j,'patch')
        payload,base_id,method=selection
        chosen.append(payload);records.append((method,base_id,payload));methods[method]+=1
        choices.append({'file':r['path'],'method':method,'base':files[base_id][0]['path'] if base_id is not None else None,
                        'standalone_bytes':len(standalone),'selected_bytes':len(payload),'geometry_candidates':len(candidates),
                        'parsed_bytes':coverage[0],'raw_bytes':coverage[1],'eligible_entities':coverage[2]})
    # Decode from stored payloads and prior decoded bases, never source-base bytes.
    decoded=[]
    for i,(method,base_id,payload) in enumerate(records):
        if method=='exact_duplicate':restored=decoded[base_id]
        elif method=='standalone':restored=zstd(['-d','-c'],payload)
        else:
            base_file=temp/f'decoded-base-{base_id}.dxf';base_file.write_bytes(decoded[base_id])
            restored=zstd(['-d','--patch-from='+str(base_file),'-c'],payload)
        assert restored==files[i][2],files[i][0]['path']
        decoded.append(restored)
    packed=temp/'v2.bin';packed.write_bytes(b''.join(chosen))
    cmd=['cargo','run','--release','--quiet','--bin','canon_control_measure','--',str(packed)]
    out=subprocess.check_output(cmd,cwd=ROOT.parent,env=ENV,text=True).strip()
    measure={k:int(v) if v.isdigit() else v for k,v in (field.split('=',1) for field in out.split(','))}
    meta=232*measure['stripes']+36*len(chosen)+8*len(files)+8*len(files)+len(chosen)+4*methods['patch']
    measure['modeled_metadata_raw_bytes']=meta
    measure['modeled_metadata_protected_bytes']=3*meta//2
    measure['total_protected_bytes']=measure['actual_share_bytes']+measure['modeled_metadata_protected_bytes']
    result={'mapping':'v2 geometry-key-ranked whole-DXF patches; 3 best previous candidates plus first family base',
            'scope':'44 Tier A dev DXFs only','source_bytes':sum(len(f[2]) for f in files),'methods':dict(methods),
            'measure':measure,'choices':choices,'all_files_restored_from_selected_payload_graph':True,
            'limitations':'Improvement over C2, if any, comes from choosing a better zstd patch base; not evidence that polynomial terms compress CAD geometry. Metadata uses the same estimated constants as C2; no persisted object index.'}
    (ROOT/'cad-v2-dev-results.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'methods':result['methods'],'measure':measure},indent=2))
