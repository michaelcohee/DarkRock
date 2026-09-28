//! Lossless DXF byte adapter. Polynomial keys select candidates; raw bytes remain authoritative.
use crate::polynomial::{canonicalize, Term};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ops::Range;
use crate::representation::{choose,decode,Encoding};

#[derive(Clone, Debug)]
pub struct Coverage {
    pub total_bytes: usize,
    pub parsed_bytes: usize,
    pub raw_bytes: usize,
    pub matched_bytes: usize,
    pub eligible_entities: usize,
    pub raw_entities: usize,
    pub literal_runs: usize,
    pub lookup_hits: usize,
    pub verified_matches: usize,
    pub exact_hash_matches: usize,
    pub edited_matches: usize,
}

#[derive(Clone, Debug)]
struct Tag { code: i32, value: Vec<u8>, start: usize }
#[derive(Clone, Debug)]
struct Span { range: Range<usize>, key: Option<String> }
#[derive(Clone, Debug)]
struct Locator { object: usize, range: Range<usize>, hash: [u8;32] }
#[derive(Clone, Debug)]
pub enum Item {
    Literal(usize),
    Matched { base: usize, start: usize, end: usize, prefix: usize, suffix: usize, middle: Vec<u8>, len: usize, hash: [u8;32] },
}
#[derive(Clone, Debug)]
pub struct StoredFile { pub items: Vec<Item>, pub coverage: Coverage, pub hash: [u8;32] }
#[derive(Default)]
pub struct AdapterStore { pub objects: Vec<Vec<u8>>, pub encoded_objects: Vec<Vec<u8>>, pub files: Vec<StoredFile>, index: HashMap<String, Vec<Locator>> }

#[derive(Clone,Copy,Debug)]
pub enum LedgerMode { N, Z }
#[derive(Default,Clone,Debug)]
pub struct LedgerParts { pub literal_objects: usize, pub edits: usize, pub references: usize, pub key_hash_index: usize, pub file_manifests: usize, pub framing: usize }
impl LedgerParts { pub fn total(&self)->usize{self.literal_objects+self.edits+self.references+self.key_hash_index+self.file_manifests+self.framing} }
fn put_u32(out:&mut Vec<u8>,v:usize){out.extend_from_slice(&(v as u32).to_le_bytes())}
fn put_u64(out:&mut Vec<u8>,v:usize){out.extend_from_slice(&(v as u64).to_le_bytes())}
struct Reader<'a>{input:&'a [u8],pos:usize}
impl<'a> Reader<'a>{
    fn take(&mut self,n:usize)->Result<&'a [u8],String>{let end=self.pos.checked_add(n).ok_or("ledger offset overflow")?;let x=self.input.get(self.pos..end).ok_or("truncated ledger")?;self.pos=end;Ok(x)}
    fn u8(&mut self)->Result<u8,String>{Ok(self.take(1)?[0])}
    fn u32(&mut self)->Result<usize,String>{Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize)}
    fn u64(&mut self)->Result<usize,String>{usize::try_from(u64::from_le_bytes(self.take(8)?.try_into().unwrap())).map_err(|_|"length overflow".into())}
    fn hash(&mut self)->Result<[u8;32],String>{Ok(self.take(32)?.try_into().unwrap())}
}

fn digest(bytes: &[u8]) -> [u8;32] { Sha256::digest(bytes).into() }
pub fn validate_ascii_dxf(input:&[u8])->Result<(),String>{
    if input.starts_with(b"AutoCAD Binary DXF") || input.contains(&0){return Err("binary DXF is unsupported; ASCII DXF required".into())}
    Ok(())
}
fn trim_ascii(s: &[u8]) -> &[u8] { s.strip_suffix(b"\r").unwrap_or(s).trim_ascii() }
fn lines(input: &[u8]) -> Vec<(usize,usize,&[u8])> {
    let mut out=Vec::new(); let mut p=0;
    while p<input.len() { let e=input[p..].iter().position(|&c|c==b'\n').map(|x|p+x+1).unwrap_or(input.len());
        let no_lf=if e>p && input[e-1]==b'\n' {e-1} else {e}; out.push((p,e,&input[p..no_lf])); p=e; }
    out
}
fn tags(input: &[u8]) -> Option<Vec<Tag>> {
    let ls=lines(input); if ls.len()%2!=0 {return None} let mut out=Vec::with_capacity(ls.len()/2);
    for pair in ls.chunks_exact(2) { let code=std::str::from_utf8(trim_ascii(pair[0].2)).ok()?.parse().ok()?;
        out.push(Tag{code,value:trim_ascii(pair[1].2).to_vec(),start:pair[0].0}); }
    Some(out)
}
fn entity_type_id(name: &[u8]) -> Option<i64> { Some(match name {
    b"LINE"=>1,b"ARC"=>2,b"CIRCLE"=>3,b"LWPOLYLINE"=>4,b"POLYLINE"=>5,
    b"POINT"=>6,b"TEXT"=>7,b"MTEXT"=>8,b"INSERT"=>9,b"ELLIPSE"=>10,_=>return None }) }
fn numeric_geometry(code:i32)->bool { matches!(code,10..=18|20..=28|30..=38|40..=48|50..=58|210..=218|220..=228|230..=238) }
fn harmless_meta(code:i32)->bool { matches!(code,0|1|2|3|6|7|8|39|62|66|67|70..=78|90..=99|100|102|105|210..=238|280..=289|290..=299|300..=309|330..=369|370..=399|410|420..=429|430..=439|440..=449|999) || code==5 }
fn candidate_key(entity:&[Tag], kind:&[u8])->Option<String> {
    let id=entity_type_id(kind)?; let mut counts=HashMap::<(usize,i32),i64>::new(); let mut terms=Vec::new(); let mut child=0usize;
    for t in entity {
        if t.code==0 && t.value==b"VERTEX" { child+=1; }
        if t.code>=1000 || t.code==102 || t.code==360 || t.code==101 || t.code==310 {return None}
        if numeric_geometry(t.code) {
            let v=std::str::from_utf8(&t.value).ok()?.parse::<f64>().ok()?; if !v.is_finite(){return None}
            let occurrence=counts.entry((child,t.code)).or_default();
            terms.push(Term{coefficient:v,exponents:vec![id,child as i64,t.code as i64,*occurrence]}); *occurrence+=1;
        } else if !harmless_meta(t.code) {return None}
    }
    if terms.is_empty(){return None}
    let canon=canonicalize(&terms).ok()?;
    Some(format!("cad-v1:{id}:{}",canon.hash))
}

/// Scan only the raw DXF tag stream. Unsupported entities and sections remain literal bytes.
fn scan(input:&[u8])->Vec<Span> {
    let Some(tags)=tags(input) else {return vec![Span{range:0..input.len(),key:None}]};
    let mut spans=Vec::new(); let mut section:Vec<u8>=Vec::new(); let mut i=0; let mut cursor=0;
    while i<tags.len() {
        if tags[i].code==0 && tags[i].value==b"SECTION" {
            section=if i+1<tags.len() && tags[i+1].code==2 {tags[i+1].value.clone()}else{Vec::new()}; i+=1; continue;
        }
        if tags[i].code==0 && tags[i].value==b"ENDSEC" {section.clear(); i+=1; continue}
        if (section==b"ENTITIES" || section==b"BLOCKS") && tags[i].code==0 {
            let start=i; let kind=tags[i].value.clone(); let mut j=i+1;
            if kind==b"POLYLINE" {
                while j<tags.len() && !(tags[j].code==0 && tags[j].value==b"SEQEND") {j+=1}
                if j<tags.len() {j+=1; while j<tags.len() && tags[j].code!=0 {j+=1} }
                else {j=start+1; while j<tags.len() && tags[j].code!=0 {j+=1}}
            } else {while j<tags.len() && tags[j].code!=0 {j+=1}}
            let begin=tags[start].start; let end=if j<tags.len(){tags[j].start}else{input.len()};
            if cursor<begin {spans.push(Span{range:cursor..begin,key:None})}
            let valid_poly=kind!=b"POLYLINE" || tags[start..j].iter().any(|t|t.code==0 && t.value==b"SEQEND");
            let key=if valid_poly {candidate_key(&tags[start..j],&kind)}else{None};
            spans.push(Span{range:begin..end,key}); cursor=end; i=j;
        } else {i+=1}
    }
    if cursor<input.len(){spans.push(Span{range:cursor..input.len(),key:None})}
    debug_assert_eq!(spans.iter().map(|s|s.range.len()).sum::<usize>(),input.len());
    spans
}

/// Candidate-only fingerprint for choosing files to compare as delta bases.
/// No key authorizes a merge; the patch decoder still restores exact bytes.
pub fn geometry_keys(input:&[u8])->Result<Vec<String>,String>{validate_ascii_dxf(input)?;Ok(scan(input).into_iter().filter_map(|s|s.key).collect())}
pub fn geometry_coverage(input:&[u8])->Result<(usize,usize,usize),String>{
    validate_ascii_dxf(input)?;
    let spans=scan(input);let parsed=spans.iter().filter(|s|s.key.is_some()).map(|s|s.range.len()).sum();
    let entities=spans.iter().filter(|s|s.key.is_some()).count();Ok((parsed,input.len()-parsed,entities))
}

fn edit(base:&[u8],target:&[u8])->(usize,usize,Vec<u8>) {
    let mut p=0; while p<base.len().min(target.len()) && base[p]==target[p] {p+=1}
    let mut s=0; while s<base.len()-p && s<target.len()-p && base[base.len()-1-s]==target[target.len()-1-s] {s+=1}
    (p,s,target[p..target.len()-s].to_vec())
}
impl AdapterStore {
    /// Exact, self-contained ledger. The index is counted and validated on decode.
    pub fn export_ledger(&self,mode:LedgerMode)->Result<(Vec<u8>,LedgerParts),String>{
        self.export_ledger_with_index(mode,true)
    }
    /// The derived view omits the candidate index; reads depend only on durable objects and references.
    pub fn export_ledger_with_index(&self,mode:LedgerMode,protect_index:bool)->Result<(Vec<u8>,LedgerParts),String>{
        let mut out=Vec::new();let mut parts=LedgerParts::default();out.extend_from_slice(if protect_index {b"DRA3L1"}else{b"DRA3D1"});out.push(match mode{LedgerMode::N=>0,LedgerMode::Z=>1});parts.framing+=7;
        put_u32(&mut out,self.objects.len());parts.framing+=4;
        for raw in &self.objects {
            let (tag,payload)=match mode {LedgerMode::N=>(0,raw.clone()),LedgerMode::Z=>{
                let c=choose(raw)?;let tag=if c.encoding==Encoding::Zstd{1}else{0};
                if decode(c.encoding,&c.payload)?!=*raw{return Err("literal selector mismatch".into())}(tag,c.payload)
            }};
            let start=out.len();put_u64(&mut out,raw.len());out.extend_from_slice(&digest(raw));out.push(tag);put_u64(&mut out,payload.len());out.extend_from_slice(&payload);parts.literal_objects+=out.len()-start;
        }
        put_u32(&mut out,self.files.len());parts.framing+=4;
        for file in &self.files {
            let start=out.len();put_u64(&mut out,file.coverage.total_bytes);out.extend_from_slice(&file.hash);put_u32(&mut out,file.items.len());parts.file_manifests+=out.len()-start;
            for item in &file.items {match item {
                Item::Literal(id)=>{let s=out.len();out.push(0);put_u32(&mut out,*id);parts.references+=out.len()-s},
                Item::Matched{base,start,end,prefix,suffix,middle,len,hash}=>{
                    let s=out.len();out.push(1);put_u32(&mut out,*base);for n in [*start,*end,*prefix,*suffix,*len]{put_u64(&mut out,n)}
                    out.extend_from_slice(hash);put_u64(&mut out,middle.len());parts.references+=out.len()-s;
                    out.extend_from_slice(middle);parts.edits+=middle.len();
                }
            }}
        }
        if protect_index {
            let mut keys:Vec<_>=self.index.iter().collect();keys.sort_by(|a,b|a.0.cmp(b.0));
            let s=out.len();put_u32(&mut out,keys.len());
            for (key,locs) in keys {put_u32(&mut out,key.len());out.extend_from_slice(key.as_bytes());put_u32(&mut out,locs.len());
                for loc in locs {put_u32(&mut out,loc.object);put_u64(&mut out,loc.range.start);put_u64(&mut out,loc.range.end);out.extend_from_slice(&loc.hash)}
            }parts.key_hash_index=out.len()-s;
        }
        if parts.total()!=out.len(){return Err("ledger accounting mismatch".into())}Ok((out,parts))
    }
    /// Reconstruct from serialized bytes only; no ingest-time raw cache is consulted.
    pub fn verify_ledger(bytes:&[u8],expected:&[Vec<u8>])->Result<(),String>{
        let mut r=Reader{input:bytes,pos:0};let magic=r.take(6)?;let protect_index=match magic {b"DRA3L1"=>true,b"DRA3D1"=>false,_=>return Err("wrong ledger magic".into())};
        let mode=r.u8()?;if mode>1{return Err("unknown ledger mode".into())}
        let object_count=r.u32()?;if object_count>1_000_000{return Err("too many objects".into())}
        let mut objects=Vec::with_capacity(object_count);
        for _ in 0..object_count {let raw_len=r.u64()?;let hash=r.hash()?;let tag=r.u8()?;let payload_len=r.u64()?;
            if raw_len>64*1024*1024 || payload_len>64*1024*1024{return Err("oversized object".into())}
            let payload=r.take(payload_len)?;let raw=match tag{0=>payload.to_vec(),1=>zstd::stream::decode_all(payload).map_err(|e|e.to_string())?,_=>return Err("unknown object tag".into())};
            if raw.len()!=raw_len || digest(&raw)!=hash{return Err("literal hash mismatch".into())}objects.push(raw);
        }
        let files=r.u32()?;if files!=expected.len(){return Err("file count mismatch".into())}
        let mut restored_files=Vec::with_capacity(files);
        for original in expected {let len=r.u64()?;let hash=r.hash()?;let count=r.u32()?;let mut out=Vec::with_capacity(len);
            if count>10_000_000{return Err("too many references".into())}
            for _ in 0..count {match r.u8()?{
                0=>{let id=r.u32()?;out.extend_from_slice(objects.get(id).ok_or("missing literal")?)},
                1=>{let id=r.u32()?;let start=r.u64()?;let end=r.u64()?;let prefix=r.u64()?;let suffix=r.u64()?;let target_len=r.u64()?;let target_hash=r.hash()?;let middle_len=r.u64()?;
                    if middle_len>64*1024*1024{return Err("oversized edit".into())}let middle=r.take(middle_len)?;
                    let base=objects.get(id).and_then(|b|b.get(start..end)).ok_or("bad candidate locator")?;
                    if prefix+suffix>base.len(){return Err("bad edit bounds".into())}
                    let mut target=base[..prefix].to_vec();target.extend_from_slice(middle);target.extend_from_slice(&base[base.len()-suffix..]);
                    if target.len()!=target_len || digest(&target)!=target_hash{return Err("edited entity mismatch".into())}out.extend_from_slice(&target);
                },_=>return Err("unknown reference tag".into())
            }}
            if out.len()!=len || digest(&out)!=hash || &out!=original{return Err("file byte/hash mismatch".into())}
            if !protect_index {restored_files.push(out)}
        }
        if protect_index {
            let key_count=r.u32()?;if key_count>10_000_000{return Err("too many index keys".into())}
            for _ in 0..key_count {let key_len=r.u32()?;if key_len>1024{return Err("oversized key".into())}r.take(key_len)?;let count=r.u32()?;
                if count>8{return Err("too many key candidates".into())}
                for _ in 0..count {let id=r.u32()?;let start=r.u64()?;let end=r.u64()?;let hash=r.hash()?;
                    let raw=objects.get(id).and_then(|b|b.get(start..end)).ok_or("bad index locator")?;if digest(raw)!=hash{return Err("index hash mismatch".into())}
                }
            }
        }
        if r.pos!=bytes.len(){return Err("trailing ledger bytes".into())}
        if !protect_index {let mut rebuilt=AdapterStore::default();for raw in &restored_files {rebuilt.ingest(raw)?;}if rebuilt.index.is_empty(){return Err("derived index rebuild produced no keys".into())}}
        Ok(())
    }
    /// Drop all candidate lookup state, restore from stored literals/edits, and replay ingestion.
    /// The per-file verified-match vector is the compareable verify-pass set.
    pub fn verify_index_rebuild(&self)->Result<(usize,usize),String>{
        let expected:Vec<_>=self.files.iter().map(|f|(f.coverage.lookup_hits,f.coverage.verified_matches,f.coverage.exact_hash_matches,f.coverage.edited_matches)).collect();
        let mut rebuilt=AdapterStore::default();
        for id in 0..self.files.len(){rebuilt.ingest(&self.restore(id)?)?;}
        let got:Vec<_>=rebuilt.files.iter().map(|f|(f.coverage.lookup_hits,f.coverage.verified_matches,f.coverage.exact_hash_matches,f.coverage.edited_matches)).collect();
        if got!=expected{return Err("rebuild changed the verify-pass set".into())}
        if rebuilt.index.len()!=self.index.len(){return Err("rebuild changed index key count".into())}
        Ok((rebuilt.index.len(),got.iter().map(|x|x.1).sum()))
    }
    pub fn ingest(&mut self,input:&[u8])->Result<usize,String> {
        validate_ascii_dxf(input)?;
        let spans=scan(input); let mut items=Vec::new(); let mut pending=Vec::new(); let mut offsets=Vec::<(String,Range<usize>)>::new();
        let mut cov=Coverage{total_bytes:input.len(),parsed_bytes:0,raw_bytes:0,matched_bytes:0,eligible_entities:0,raw_entities:0,literal_runs:0,lookup_hits:0,verified_matches:0,exact_hash_matches:0,edited_matches:0};
        for s in spans {
            let raw=&input[s.range.clone()];
            let Some(key)=s.key else {cov.raw_bytes+=raw.len();cov.raw_entities+=1;pending.extend_from_slice(raw);continue};
            cov.parsed_bytes+=raw.len();cov.eligible_entities+=1;
            // Pending candidates have to be flushed before they can be referenced.
            if offsets.iter().any(|(k,_)|k==&key) {self.flush(&mut pending,&mut offsets,&mut items,&mut cov)}
            let mut best:Option<(Locator,usize,usize,Vec<u8>)>=None;
            if let Some(locs)=self.index.get(&key) {cov.lookup_hits+=1;for loc in locs.iter().take(8) {
                let base=&self.objects[loc.object][loc.range.clone()];
                let same_hash=loc.hash==digest(raw);
                if same_hash && base!=raw {return Err("SHA-256 candidate collision".into())}
                let (p,z,m)=edit(base,raw);
                if best.as_ref().is_none_or(|(_,_,_,old)|m.len()<old.len()) {best=Some((loc.clone(),p,z,m));}
            }}
            if let Some((loc,p,z,m))=best.filter(|(_,_,_,m)|m.len()+16<raw.len()) {
                let base=&self.objects[loc.object][loc.range.clone()];
                let mut decoded=base[..p].to_vec();decoded.extend_from_slice(&m);decoded.extend_from_slice(&base[base.len()-z..]);
                if decoded!=raw || digest(&decoded)!=digest(raw){return Err("candidate edit verification failed".into())}
                cov.verified_matches+=1;if m.is_empty() && loc.hash==digest(raw){cov.exact_hash_matches+=1}else{cov.edited_matches+=1}
                self.flush(&mut pending,&mut offsets,&mut items,&mut cov);
                items.push(Item::Matched{base:loc.object,start:loc.range.start,end:loc.range.end,prefix:p,suffix:z,middle:m,len:raw.len(),hash:digest(raw)});
                cov.matched_bytes+=raw.len();
            } else {let at=pending.len();pending.extend_from_slice(raw);offsets.push((key,at..pending.len()));}
        }
        self.flush(&mut pending,&mut offsets,&mut items,&mut cov);
        if cov.parsed_bytes+cov.raw_bytes!=cov.total_bytes{return Err("span accounting mismatch".into())}
        let id=self.files.len();self.files.push(StoredFile{items,coverage:cov,hash:digest(input)});
        if self.restore(id)?!=input {return Err("post-ingest byte verification failed".into())}Ok(id)
    }
    fn flush(&mut self,pending:&mut Vec<u8>,offsets:&mut Vec<(String,Range<usize>)>,items:&mut Vec<Item>,cov:&mut Coverage) {
        if pending.is_empty(){return} let object=self.objects.len();let raw=std::mem::take(pending);
        let compressed=zstd::bulk::compress(&raw,3).expect("zstd literal compression");
        let mut encoded=Vec::with_capacity(raw.len()+1);
        if compressed.len()<raw.len(){encoded.push(1);encoded.extend_from_slice(&compressed)}else{encoded.push(0);encoded.extend_from_slice(&raw)}
        self.encoded_objects.push(encoded);self.objects.push(raw);items.push(Item::Literal(object));cov.literal_runs+=1;
        for (key,range) in offsets.drain(..) {let hash=digest(&self.objects[object][range.clone()]);let locs=self.index.entry(key).or_default();if locs.len()<8 {locs.push(Locator{object,range,hash})}}
    }
    pub fn restore(&self,id:usize)->Result<Vec<u8>,String> {
        let file=self.files.get(id).ok_or("missing file")?;let mut out=Vec::with_capacity(file.coverage.total_bytes);
        let mut decoded=HashMap::<usize,Vec<u8>>::new();
        for item in &file.items {match item {
            Item::Literal(n)=>{if !decoded.contains_key(n){decoded.insert(*n,self.decode_object(*n)?);}out.extend_from_slice(&decoded[n]);},
            Item::Matched{base,start,end,prefix,suffix,middle,len,hash}=>{
                if !decoded.contains_key(base){decoded.insert(*base,self.decode_object(*base)?);}
                let b=decoded[base].get(*start..*end).ok_or("invalid base range")?;
                if *prefix+*suffix>b.len(){return Err("invalid edit".into())}
                let mut t=b[..*prefix].to_vec();t.extend_from_slice(middle);t.extend_from_slice(&b[b.len()-suffix..]);
                if t.len()!=*len || digest(&t)!=*hash{return Err("edit mismatch".into())}out.extend_from_slice(&t);
            }
        }}
        if out.len()!=file.coverage.total_bytes || digest(&out)!=file.hash{return Err("file mismatch".into())}Ok(out)
    }
    fn decode_object(&self,id:usize)->Result<Vec<u8>,String>{
        let encoded=self.encoded_objects.get(id).ok_or("missing literal object")?;
        let (&mode,payload)=encoded.split_first().ok_or("empty encoded object")?;
        let raw=match mode {0=>payload.to_vec(),1=>zstd::bulk::decompress(payload,self.objects.get(id).ok_or("missing object length")?.len()).map_err(|e|e.to_string())?,_=>return Err("unknown encoding".into())};
        if digest(&raw)!=digest(self.objects.get(id).ok_or("missing object")?){return Err("literal mismatch".into())}Ok(raw)
    }
}

#[cfg(test)] mod tests {use super::*;
    fn dxf(body:&str)->Vec<u8>{format!("  0\nSECTION\n  2\nENTITIES\n{body}  0\nENDSEC\n  0\nEOF\n").into_bytes()}
    #[test] fn literal_and_match_roundtrip(){let a=dxf("  0\nLINE\n  5\nA\n  8\nL1\n 10\n1\n 20\n2\n 11\n3\n 21\n4\n");let b=dxf("  0\nLINE\n  5\nB\n  8\nL2\n 10\n1.0\n 20\n2\n 11\n3\n 21\n4\n");let mut s=AdapterStore::default();let x=s.ingest(&a).unwrap();let y=s.ingest(&b).unwrap();assert_eq!(s.restore(x).unwrap(),a);assert_eq!(s.restore(y).unwrap(),b);assert!(s.files[y].coverage.matched_bytes>0);}
    #[test] fn polyline_composite_and_blocks(){let a=b"  0\nSECTION\n  2\nBLOCKS\n  0\nBLOCK\n  2\nB\n  0\nPOLYLINE\n 10\n0\n  0\nVERTEX\n 10\n1\n 20\n2\n  0\nSEQEND\n  0\nENDBLK\n  0\nENDSEC\n  0\nEOF\n";let x=scan(a);assert_eq!(x.iter().filter(|s|s.key.is_some()).count(),1);let mut s=AdapterStore::default();let i=s.ingest(a).unwrap();assert_eq!(s.restore(i).unwrap(),a);}
    #[test] fn unsupported_is_raw(){let a=dxf("  0\nHATCH\n 10\n2\n");let mut s=AdapterStore::default();let i=s.ingest(&a).unwrap();assert_eq!(s.files[i].coverage.parsed_bytes,0);assert_eq!(s.restore(i).unwrap(),a);}
    #[test] fn corrupted_edit_fails(){let a=dxf("  0\nCIRCLE\n 10\n1\n 20\n2\n 40\n3\n");let mut s=AdapterStore::default();s.ingest(&a).unwrap();let i=s.ingest(&a).unwrap();for item in &mut s.files[i].items {if let Item::Matched{middle,..}=item {middle.push(1);break}}assert!(s.restore(i).is_err());}
    #[test] fn complete_whitelist_and_raw_exclusions(){
        for name in ["LINE","ARC","CIRCLE","LWPOLYLINE","POINT","TEXT","MTEXT","INSERT","ELLIPSE"] {
            let a=dxf(&format!("  0\n{name}\n 10\n1\n 20\n2\n"));assert_eq!(scan(&a).iter().filter(|s|s.key.is_some()).count(),1,"{name}");
        }
        for name in ["HATCH","DIMENSION","SPLINE","ACAD_PROXY_ENTITY"]{
            let a=dxf(&format!("  0\n{name}\n 10\n1\n"));assert_eq!(scan(&a).iter().filter(|s|s.key.is_some()).count(),0,"{name}");
        }
    }
    #[test] fn metadata_stays_out_of_key_and_numeric_collisions_are_lossless(){
        let a=dxf("  0\nLINE\n  5\nA\n  8\nred\n 62\n1\n 10\n1e-10\n 20\n2\n");
        let b=dxf("  0\nLINE\n  5\nB\n  8\nblue\n 62\n2\n 10\n0\n 20\n2.0000000000001\n");
        assert_eq!(scan(&a).into_iter().find_map(|s|s.key),scan(&b).into_iter().find_map(|s|s.key));
        let mut store=AdapterStore::default();let x=store.ingest(&a).unwrap();let y=store.ingest(&b).unwrap();assert_eq!(store.restore(x).unwrap(),a);assert_eq!(store.restore(y).unwrap(),b);
    }
    #[test] fn crlf_and_bad_input_roundtrip(){let a=b"  0\r\nSECTION\r\n  2\r\nENTITIES\r\n  0\r\nLINE\r\n 10\r\n1\r\n 20\r\n2\r\n  0\r\nENDSEC\r\n  0\r\nEOF\r\n";let mut s=AdapterStore::default();let i=s.ingest(a).unwrap();assert_eq!(s.restore(i).unwrap(),a);let j=s.ingest(b"broken\n").unwrap();assert_eq!(s.restore(j).unwrap(),b"broken\n");}
    #[test] fn binary_dxf_fails_closed(){let mut store=AdapterStore::default();assert!(store.ingest(b"AutoCAD Binary DXF\r\n\x1a\0test").is_err());assert!(geometry_keys(b"AutoCAD Binary DXF\r\n\x1a\0test").is_err());assert!(store.files.is_empty());}
    #[test] fn unknown_tag_order_and_binary_chunks_are_literal(){
        let a=b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n 10\n1.0000000000001\n  1\nfirst split\n  3\nsecond split\n310\nDEADBEEF\n1004\nABCD\n999\nkeep this comment\n 20\n2\n  0\nENDSEC\n  0\nEOF\n";
        assert_eq!(geometry_coverage(a).unwrap().0,0);let mut s=AdapterStore::default();let i=s.ingest(a).unwrap();assert_eq!(s.restore(i).unwrap(),a);
    }
    #[test] fn full_ledger_serialization_and_corruption(){
        let a=dxf("  0\nCIRCLE\n 10\n1\n 20\n2\n 40\n3\n");let mut store=AdapterStore::default();store.ingest(&a).unwrap();store.ingest(&a).unwrap();
        for mode in [LedgerMode::N,LedgerMode::Z] {let (bytes,parts)=store.export_ledger(mode).unwrap();assert_eq!(parts.total(),bytes.len());
            AdapterStore::verify_ledger(&bytes,&[a.clone(),a.clone()]).unwrap();
            let mut bad=bytes.clone();let n=bad.len()-1;bad[n]^=1;assert!(AdapterStore::verify_ledger(&bad,&[a.clone(),a.clone()]).is_err());
        }
    }
}
