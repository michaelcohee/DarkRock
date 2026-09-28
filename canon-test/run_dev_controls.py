#!/usr/bin/env python3
"""A1 and C2 on Tier A dev only; real zstd CLI, read-only source files."""
from pathlib import Path
from collections import defaultdict
import csv, hashlib, json, os, subprocess, tempfile

ROOT=Path(__file__).resolve().parent
DEV=ROOT/'cad-corpus-v1'/'dev'
BLOCK=256*1024
ENV=dict(os.environ,RUSTC=os.environ.get('RUSTC',str(Path.home()/'.cargo/bin/rustc')))

def run_zstd(args, data):
    return subprocess.run(['zstd','-q',*args],input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True).stdout

def measure(name,objects,refs,files,patches,temp):
    path=temp/(name+'.bin');path.write_bytes(b''.join(objects))
    cmd=['cargo','run','--release','--quiet','--bin','canon_control_measure','--',str(path)]
    out=subprocess.check_output(cmd,cwd=ROOT.parent,env=ENV,text=True).strip()
    row={k:int(v) if v.isdigit() else v for k,v in (p.split('=',1) for p in out.split(','))}
    metadata=232*row['stripes']+36*len(objects)+8*refs+8*files+len(objects)+4*patches
    row['modeled_metadata_raw_bytes']=metadata
    row['modeled_metadata_protected_bytes']=3*metadata//2
    row['total_protected_bytes']=row['actual_share_bytes']+row['modeled_metadata_protected_bytes']
    return row

rows=[r for r in csv.DictReader((ROOT/'manifest.csv').open()) if r['split']=='dev']
assert len(rows)==44
source=[]
for r in sorted(rows,key=lambda x:x['path']):
    p=DEV/Path(r['path']).relative_to('dev');data=p.read_bytes()
    assert len(data)==int(r['bytes']) and hashlib.sha256(data).hexdigest()==r['sha256']
    source.append((r,p,data))
assert sum(len(x[2]) for x in source)==21601154

with tempfile.TemporaryDirectory(prefix='darkrock-cad-dev-controls-') as t:
    temp=Path(t)
    seen=defaultdict(list);objects=[];hits=0;refs=0
    for _,_,data in source:
        for i in range(0,len(data),BLOCK):
            chunk=data[i:i+BLOCK];h=hashlib.sha256(chunk).digest();refs+=1
            if any(prior==chunk for prior in seen[h]):hits+=1
            else:seen[h].append(chunk);objects.append(chunk)
    a1_objects=[]
    for obj in objects:
        z=run_zstd(['-c'],obj)
        assert run_zstd(['-d','-c'],z)==obj
        a1_objects.append(z if len(z)<len(obj) else obj)
    a1=measure('a1_dev',a1_objects,refs,len(source),0,temp)

    bases={};exact=defaultdict(list);c2_objects=[];methods=defaultdict(int);choices=[]
    for r,p,data in source:
        h=hashlib.sha256(data).digest()
        if any(prior==data for prior in exact[h]):methods['exact_duplicate']+=1;continue
        exact[h].append(data)
        standalone=run_zstd(['-c'],data)
        assert run_zstd(['-d','-c'],standalone)==data
        chosen=standalone;method='standalone_zstd';fam=r['family']
        if fam in bases:
            base_path,base_data=bases[fam]
            patch=subprocess.run(['zstd','-q','--patch-from='+str(base_path),'-c',str(p)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True).stdout
            assert run_zstd(['-d','--patch-from='+str(base_path),'-c'],patch)==data
            if len(patch)<len(chosen):chosen=patch;method='patch'
        else:bases[fam]=(p,data)
        c2_objects.append(chosen);methods[method]+=1
        choices.append({'file':r['path'],'method':method,'standalone_bytes':len(standalone),'selected_bytes':len(chosen)})
    c2=measure('c2_dev',c2_objects,len(source),len(source),methods['patch'],temp)

    report={'scope':'Tier A dev only; 44 DXFs; sorted manifest path','source_bytes':sum(len(d) for _,_,d in source),
        'zstd_version':subprocess.check_output(['zstd','--version'],text=True).strip(),
        'a1':a1,'a1_unique_chunks':len(objects),'a1_references':refs,'a1_confirmed_hits':hits,
        'c2':c2,'c2_methods':dict(methods),'c2_choices':choices,
        'accounting':'actual 4+2 share bytes; estimated protected metadata via existing baseline constants; zstd CLI real; no source files modified'}
    (ROOT/'cad-dev-controls-results.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['source_bytes','a1','c2','c2_methods']},indent=2))
