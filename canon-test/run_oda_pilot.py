#!/usr/bin/env python3
"""ODA version-diverse pilot. Copies source DWGs to a temporary input directory."""
from pathlib import Path
from collections import Counter, defaultdict
import csv, hashlib, json, plistlib, shutil, subprocess, tempfile

ROOT=Path(__file__).resolve().parent
DWG=ROOT/'DWG'
APP=Path('/Applications/ODAFileConverter.app')
EXE=APP/'Contents/MacOS/ODAFileConverter'
OUT=ROOT/'oda-pilot-2026-09-27.json'
EXPECTED={r['relative_path']:r for r in csv.DictReader((ROOT/'dwg-input-manifest-2026-09-27.csv').open())}
with (APP/'Contents/Info.plist').open('rb') as f: version=plistlib.load(f)['CFBundleShortVersionString']

def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()

def dxf_check(p):
    b=p.read_bytes()
    s=b.decode('utf-8',errors='replace')
    lines=s.splitlines()
    if len(lines)%2: raise ValueError(f'odd DXF pair count {p.name}')
    pairs=[]
    for i in range(0,len(lines),2):
        code=lines[i].strip()
        if not code.isdecimal(): raise ValueError(f'invalid group code at pair {i//2}: {code!r}')
        pairs.append((int(code),lines[i+1]))
    if not pairs or pairs[-1]!=(0,'EOF'): raise ValueError(f'missing EOF {p.name}')
    section=None; next_section=False; entities=Counter(); layers=0; blocks=0; acadver=None
    for i,(code,value) in enumerate(pairs):
        if (code,value)==(0,'SECTION'): next_section=True; continue
        if next_section and code==2: section=value;next_section=False;continue
        if (code,value)==(0,'ENDSEC'): section=None;continue
        if section=='HEADER' and (code,value)==(9,'$ACADVER') and i+1<len(pairs): acadver=pairs[i+1][1]
        if code==0 and section=='ENTITIES': entities[value]+=1
        if (code,value)==(0,'LAYER') and section=='TABLES': layers+=1
        if (code,value)==(0,'BLOCK') and section=='BLOCKS': blocks+=1
    if acadver!='AC1032' or sum(entities.values())==0: raise ValueError(f'bad version or empty entities {p.name}: {acadver}, {entities}')
    return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'acadver':acadver,'entity_count':sum(entities.values()),'entity_types':dict(entities),'layer_records':layers,'block_records':blocks,'utf8_replacement_chars':s.count('\ufffd')}

candidates=defaultdict(list)
for p in DWG.glob('*.dwg'):
    candidates[p.read_bytes()[:6].decode('ascii')].append(p)
versions=['AC1015','AC1018','AC1021','AC1024','AC1032']
chosen=[min(candidates[v],key=lambda p:p.stat().st_size) for v in versions]
with tempfile.TemporaryDirectory(prefix='darkrock-oda-pilot-') as td:
    root=Path(td); inp=root/'input'; out=root/'output';inp.mkdir();out.mkdir()
    for p in chosen:
        key='DWG/'+p.name
        if key not in EXPECTED or sha(p)!=EXPECTED[key]['sha256']: raise ValueError(f'source hash changed: {p.name}')
        shutil.copy2(p,inp/p.name)
    cmd=[str(EXE),str(inp),str(out),'ACAD2018','DXF','0','0','*.dwg']
    run=subprocess.run(cmd,capture_output=True,text=True,timeout=120)
    results=[]
    for p in chosen:
        target=out/(p.stem+'.dxf')
        if not target.is_file(): raise ValueError(f'no DXF for {p.name}; exit={run.returncode}; stderr={run.stderr}')
        row={'source':p.name,'source_version':p.read_bytes()[:6].decode('ascii'),'source_bytes':p.stat().st_size,'source_sha256':sha(p),'output':target.name}
        row.update(dxf_check(target));results.append(row)
    extra=[p.name for p in out.iterdir() if p.is_file() and p.name not in {r['output'] for r in results}]
    if run.returncode!=0 or extra: raise ValueError(f'converter exit={run.returncode}, extra={extra}, stderr={run.stderr}')
    report={'converter':str(EXE),'converter_version':version,'converter_sha256':sha(EXE),'output_version_option':'ACAD2018','format':'DXF','recursive':'0','audit':'0','filter':'*.dwg','exit_code':run.returncode,'stdout':run.stdout,'stderr':run.stderr,'samples':results}
    OUT.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'converter_version':version,'samples':[{'source_version':r['source_version'],'source_bytes':r['source_bytes'],'dxf_bytes':r['bytes'],'entities':r['entity_count'],'acadver':r['acadver'],'utf8_replacement_chars':r['utf8_replacement_chars']} for r in results]},indent=2))
