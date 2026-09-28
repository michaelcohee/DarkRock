#!/usr/bin/env python3
"""Read-only candidate-byte census for rigid 90-degree pose plus uniform scale.

This is deliberately not a CAD adapter or storage encoder. It never merges files.
"""
from __future__ import annotations
from collections import Counter,defaultdict
from pathlib import Path
import hashlib,json,math,re

ROOT=Path(__file__).resolve().parent
DXF=ROOT/'converted-dxf'/'oda-27.1-r2018-full'
MANIFEST=json.loads((DXF/'conversion-manifest.json').read_text())
assert len(MANIFEST['files'])==47
KINDS={'LINE','CIRCLE','ARC','POINT','LWPOLYLINE','POLYLINE'}

def family(name:str)->str:
    low=name.lower()
    if 'tx9200' in low:return '084243 TX9200 sidelights'
    if 'versamax' in low:return '084243 VersaMax'
    if 'elite-freestanding' in low:return '102313 Elite track'
    if 'solare' in low:return '102313 Solare'
    if low.startswith('083921'):return '083921 series'
    lead=re.match(r'[0-9.]+',name)
    return lead.group(0).rstrip('.') if lead else name.removesuffix('.dxf')

def tags(raw:bytes):
    if raw.startswith(b'AutoCAD Binary DXF') or b'\0' in raw:raise ValueError('binary DXF')
    lines=raw.splitlines(keepends=True)
    if len(lines)%2:raise ValueError('odd line count')
    out=[];offset=0
    for i in range(0,len(lines),2):
        code=int(lines[i].strip());value=lines[i+1].strip().decode('latin1')
        out.append((code,value,offset));offset+=len(lines[i])+len(lines[i+1])
    assert offset==len(raw)
    return out

def numeric(records,code:int):
    for c,v,_ in records:
        if c==code:
            try:
                f=float(v)
                if math.isfinite(f):return f
            except ValueError:pass
    return None

def point(records,xcode:int,ycode:int):
    x=numeric(records,xcode);y=numeric(records,ycode)
    return (x,y) if x is not None and y is not None else None

def geometry(kind:str,records):
    pts=[];radius=None
    if kind=='LINE':pts=[point(records,10,20),point(records,11,21)]
    elif kind=='POINT':pts=[point(records,10,20)]
    elif kind=='CIRCLE':pts=[point(records,10,20)];radius=numeric(records,40)
    elif kind=='ARC':
        center=point(records,10,20);radius=numeric(records,40);a=numeric(records,50);b=numeric(records,51)
        if center and radius is not None and a is not None and b is not None:
            pts=[center]+[(center[0]+radius*math.cos(math.radians(t)),center[1]+radius*math.sin(math.radians(t))) for t in (a,b)]
    elif kind=='LWPOLYLINE':
        x=None
        for code,value,_ in records:
            if code==10:
                try:x=float(value)
                except ValueError:x=None
            elif code==20 and x is not None:
                try:pts.append((x,float(value)))
                except ValueError:pass
                x=None
    elif kind=='POLYLINE':
        group=[]
        for record in records:
            if record[0]==0 and record[1]=='VERTEX':
                if group and group[0][1]=='VERTEX':
                    p=point(group,10,20)
                    if p:pts.append(p)
                group=[record]
            else:group.append(record)
        if group and group[0][1]=='VERTEX':
            p=point(group,10,20)
            if p:pts.append(p)
    if not pts or any(p is None or not all(math.isfinite(v) for v in p) for p in pts):return None
    if radius is not None and (not math.isfinite(radius) or radius<=0):return None
    return (kind,pts,radius)

def entities(raw:bytes):
    ts=tags(raw);out=[];section='';i=0
    while i<len(ts):
        code,value,start=ts[i]
        if code==0 and value=='SECTION':
            section=ts[i+1][1] if i+1<len(ts) and ts[i+1][0]==2 else '';i+=1;continue
        if code==0 and value=='ENDSEC':section='';i+=1;continue
        if section in ('BLOCKS','ENTITIES') and code==0:
            kind=value;j=i+1
            if kind=='POLYLINE':
                while j<len(ts) and not(ts[j][0]==0 and ts[j][1]=='SEQEND'):j+=1
                if j<len(ts):
                    j+=1
                    while j<len(ts) and ts[j][0]!=0:j+=1
            else:
                while j<len(ts) and ts[j][0]!=0:j+=1
            end=ts[j][2] if j<len(ts) else len(raw)
            if kind in KINDS:
                shape=geometry(kind,ts[i:j])
                if shape:out.append((*shape,end-start))
            i=j
        else:i+=1
    return out

def frame(items):
    points=[p for _,ps,_,_ in items for p in ps]
    if not points:return None
    cx=sum(p[0] for p in points)/len(points);cy=sum(p[1] for p in points)/len(points)
    scale=math.sqrt(sum((p[0]-cx)**2+(p[1]-cy)**2 for p in points)/len(points))
    return (cx,cy,scale) if math.isfinite(scale) and scale>1e-9 else None

def key(item,origin_scale):
    kind,pts,radius,_=item;cx,cy,scale=origin_scale
    variants=[]
    for turns in range(4):
        transformed=[]
        for x,y in pts:
            a=(x-cx)/scale;b=(y-cy)/scale
            for _ in range(turns):a,b=-b,a
            transformed.append((round(a,3),round(b,3)))
        if kind=='LINE':transformed=sorted(transformed)
        if kind in ('POLYLINE','LWPOLYLINE'):
            transformed=min(transformed,list(reversed(transformed)))
        variants.append((kind,tuple(transformed),None if radius is None else round(radius/scale,3)))
    return min(variants)

def selftest():
    base=[('LINE',[(0,0),(2,0)],None,10),('CIRCLE',[(4,3)],1,10)]
    posed=[]
    for kind,points,radius,n in base:
        posed.append((kind,[(7-3*y,11+3*x) for x,y in points],None if radius is None else 3*radius,n))
    a=frame(base);b=frame(posed)
    assert Counter(key(x,a) for x in base)==Counter(key(x,b) for x in posed)

def main():
    selftest();files=[]
    for row in MANIFEST['files']:
        path=DXF/row['output'];raw=path.read_bytes()
        if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError(f'manifest mismatch: {path.name}')
        parsed=entities(raw);f=frame(parsed)
        keyed=[(key(e,f),e[3]) for e in parsed] if f else []
        files.append({'name':path.name,'family':family(path.name),'source_bytes':len(raw),'eligible_bytes':sum(e[3] for e in parsed),'keyed':keyed})
    grouped=defaultdict(list)
    for f in files:grouped[f['family']].append(f)
    out=['# Tier B posed-sibling candidate census','',
         '**Scope:** read-only feasibility count on the 47 ODA-converted DXFs. No stored payload, reference, undo, parity, or v3 adapter was created. All input bytes were checked against the pinned conversion manifest.','',
         'For LINE, CIRCLE, ARC, POINT, LWPOLYLINE and composite POLYLINE records in ENTITIES or BLOCKS, the key uses geometry relative to each file’s centroid and RMS scale, tests all four 90° orientations, rounds normalized values to 0.001, and ignores handles/layers. A record is counted when its key appears in another file of the same declared filename family. These are **candidate bytes**, including possible false matches; no byte-exact restore or net storage saving is implied. Unsupported records are excluded.','',
         '| Family | Files | Source bytes | Eligible entity bytes | Cross-file candidate bytes | Candidate / eligible |','|---|---:|---:|---:|---:|---:|']
    total=[0,0,0,0]
    for fam,members in sorted(grouped.items()):
        lookup=defaultdict(set)
        for i,f in enumerate(members):
            for k,_ in f['keyed']:lookup[k].add(i)
        source=sum(f['source_bytes'] for f in members);eligible=sum(f['eligible_bytes'] for f in members)
        matched=sum(n for f in members for k,n in f['keyed'] if len(lookup[k])>=2)
        total[0]+=len(members);total[1]+=source;total[2]+=eligible;total[3]+=matched
        pct=100*matched/eligible if eligible else 0
        out.append(f'| {fam} | {len(members)} | {source:,} | {eligible:,} | {matched:,} | {pct:.2f}% |')
    out.append(f'| **Total** | **{total[0]}** | **{total[1]:,}** | **{total[2]:,}** | **{total[3]:,}** | **{100*total[3]/total[2] if total[2] else 0:.2f}%** |')
    out.extend(['','**Limits:** filename grouping is a declared heuristic, and per-file centroid/RMS normalization can miss siblings when drawings differ in content or include large unrelated blocks. Rounding and shape-only keys can create false positives. The counts are a filter for whether pose-aware matching is worth later research, not an A3 v3 benchmark.',''])
    (ROOT/'cad-tierb-posed-sibling-census.md').write_text('\n'.join(out))
    print(f'47 files; eligible={total[2]:,}; candidate={total[3]:,}')

if __name__=='__main__':main()
