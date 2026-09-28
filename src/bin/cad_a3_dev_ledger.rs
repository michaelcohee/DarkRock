use darkrock_storage::dxf_adapter::{AdapterStore,LedgerMode};
use darkrock_storage::storage::{encode_stripe,reconstruct,STRIPE_SIZE};
use sha2::{Digest,Sha256};
use std::{fs,path::{Path,PathBuf}};

fn gather(dir:&Path,out:&mut Vec<PathBuf>)->std::io::Result<()>{for item in fs::read_dir(dir)?{let p=item?.path();if p.is_dir(){gather(&p,out)?}else if p.extension().is_some_and(|x|x=="dxf"){out.push(p)}}Ok(())}
fn share_bytes(len:usize)->usize{let full=len/STRIPE_SIZE;let tail=len%STRIPE_SIZE;full*6*(STRIPE_SIZE/4)+if tail==0{0}else{6*tail.div_ceil(4)}}
fn protected_bytes(len:usize)->usize{let stripes=len.div_ceil(STRIPE_SIZE);let catalog=21+240*stripes;let catalog_stripes=catalog.div_ceil(STRIPE_SIZE);share_bytes(len)+share_bytes(catalog)+6*(32+240*catalog_stripes)}
fn verify_striped(bytes:&[u8],expected:&[Vec<u8>])->Result<(usize,usize),String>{
    let mut recovered=Vec::with_capacity(bytes.len());let mut catalog=Vec::new();catalog.extend_from_slice(b"DRMC1");catalog.extend_from_slice(&(bytes.len() as u64).to_le_bytes());catalog.extend_from_slice(&(bytes.len().div_ceil(STRIPE_SIZE) as u64).to_le_bytes());
    let mut data_share=0;
    for part in bytes.chunks(STRIPE_SIZE){let stripe=encode_stripe(part)?;data_share+=stripe.shards.iter().map(Vec::len).sum::<usize>();
        let m=&stripe.manifest;catalog.extend_from_slice(&(m.original_len as u64).to_le_bytes());catalog.extend_from_slice(&(m.share_len as u64).to_le_bytes());catalog.extend_from_slice(&m.original_hash);for h in &m.shard_hashes{catalog.extend_from_slice(h)}
        for first in 0..6 {for second in first..6 {let mut s:Vec<_>=stripe.shards.iter().cloned().map(Some).collect();s[first]=None;if second!=first{s[second]=None}if reconstruct(s,&stripe.manifest)?!=part{return Err("ledger stripe mismatch".into())}}}
        recovered.extend_from_slice(part);
    }
    if Sha256::digest(&catalog).len()!=32{return Err("catalog digest error".into())}
    let mut manifest_share=0;for part in catalog.chunks(STRIPE_SIZE){let stripe=encode_stripe(part)?;manifest_share+=stripe.shards.iter().map(Vec::len).sum::<usize>();
        for first in 0..6 {for second in first..6 {let mut s:Vec<_>=stripe.shards.iter().cloned().map(Some).collect();s[first]=None;if second!=first{s[second]=None}if reconstruct(s,&stripe.manifest)?!=part{return Err("catalog stripe mismatch".into())}}}
    }
    AdapterStore::verify_ledger(&recovered,expected)?;Ok((data_share,manifest_share))
}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=Path::new("canon-test/cad-corpus-v1/dev");let mut paths=Vec::new();gather(root,&mut paths)?;paths.sort();if paths.len()!=44{return Err("expected 44 dev DXFs".into())}
    let inputs:Vec<Vec<u8>>=paths.iter().map(fs::read).collect::<Result<_,_>>()?;
    let mut store=AdapterStore::default();let mut snapshots=Vec::new();
    for (i,input) in inputs.iter().enumerate(){store.ingest(input)?;
        let mut row=Vec::new();for mode in [LedgerMode::N,LedgerMode::Z]{let (bytes,parts)=store.export_ledger(mode)?;row.push((bytes.len(),protected_bytes(bytes.len()),parts));}
        snapshots.push(row);eprintln!("dev {}/44",i+1);
    }
    let mut finals=Vec::new();for mode in [LedgerMode::N,LedgerMode::Z]{let (bytes,parts)=store.export_ledger(mode)?;
        let (data,manifest)=verify_striped(&bytes,&inputs)?;let catalog_stripes=(21+240*bytes.len().div_ceil(STRIPE_SIZE)).div_ceil(STRIPE_SIZE);let bootstrap=6*(32+240*catalog_stripes);let physical=data+manifest+bootstrap;
        if physical!=protected_bytes(bytes.len()){return Err("physical accounting mismatch".into())}
        finals.push((bytes.len(),data,manifest,physical,parts));
    }
    let mut report=String::from("# A3 full incremental protected-byte ledger — Tier A dev\n\n44 source DXFs restored byte-for-byte from each fully serialized ledger. Both complete ledgers and their manifest catalogs passed all 21 one/two-share loss patterns. Binary DXF is unsupported. No Tier A test file was read.\n\nAll actual ledger bytes are striped with RedTail-X 4+2. The stripe-manifest catalog is also 4+2 coded. The finite bootstrap is six physical copies of each catalog-stripe manifest (240 bytes) and a 32-byte catalog digest; this is a charged root assumption, not authenticated production metadata.\n\n| Mode | Ledger bytes | Literal objects | Edit bytes | Ordered references | Key/hash index | File manifests | Framing | Data shares | Manifest shares | Bootstrap | Total physical |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for (label,(bytes,data,manifest,physical,p)) in ["N","Z"].iter().zip(&finals){let bootstrap=6*(32+240*(21+240*bytes.div_ceil(STRIPE_SIZE)).div_ceil(STRIPE_SIZE));report.push_str(&format!("| {label} | {bytes} | {} | {} | {} | {} | {} | {} | {data} | {manifest} | {bootstrap} | {physical} |\n",p.literal_objects,p.edits,p.references,p.key_hash_index,p.file_manifests,p.framing));}
    let lookups:usize=store.files.iter().map(|f|f.coverage.lookup_hits).sum();let verified:usize=store.files.iter().map(|f|f.coverage.verified_matches).sum();let exact:usize=store.files.iter().map(|f|f.coverage.exact_hash_matches).sum();let edited:usize=store.files.iter().map(|f|f.coverage.edited_matches).sum();
    let raw:usize=store.files.iter().map(|f|f.coverage.raw_bytes).sum();let total:usize=inputs.iter().map(Vec::len).sum();
    report.push_str(&format!("\nKey lookup hits: {lookups}; verified matched entities: {verified} ({exact} exact-hash, {edited} edited); verified-hit rate among key lookups: {:.2}%. Pass-through: {raw}/{total} bytes ({:.4}%). The only v1 edit that fired was `(prefix_len, suffix_len, literal_middle)`, {edited} times. No canonical key alone caused a merge.\n\n",100.0*verified as f64/lookups.max(1) as f64,100.0*raw as f64/total as f64));
    let mut raw_edit_bytes=0usize;let mut compressed_edit_bytes=0usize;let mut compressed_files=0usize;
    for file in &store.files {let edit_stream:Vec<u8>=file.items.iter().flat_map(|item|match item{darkrock_storage::dxf_adapter::Item::Matched{middle,..}=>middle.as_slice(),_=>&[]}).copied().collect();
        if !edit_stream.is_empty(){let z=zstd::bulk::compress(&edit_stream,3)?;raw_edit_bytes+=edit_stream.len();
            if z.len()<edit_stream.len(){compressed_edit_bytes+=z.len();compressed_files+=1}else{compressed_edit_bytes+=edit_stream.len()}
        }
    }
    report.push_str(&format!("V2 proposal diagnostic only: per-file zstd level-3 compression of edit **middle bytes alone** would change {raw_edit_bytes} raw bytes to {compressed_edit_bytes} selected payload bytes across {compressed_files} compressed files, before tags, offsets, hashes, references, indexes, manifests, and 4+2. These bytes are **not** used in the v1 table.\n\n"));
    report.push_str("| File | Source bytes | Parsed bytes | Raw pass-through | Key lookups | Verified exact | Verified edited | Incremental N physical bytes | Incremental Z physical bytes |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut prev_n=0;let mut prev_z=0;for (i,path) in paths.iter().enumerate(){let c=&store.files[i].coverage;let n=snapshots[i][0].1;let z=snapshots[i][1].1;
        report.push_str(&format!("| `{}` | {} | {} | {} | {} | {} | {} | {} | {} |\n",path.strip_prefix(root)?.display(),c.total_bytes,c.parsed_bytes,c.raw_bytes,c.lookup_hits,c.exact_hash_matches,c.edited_matches,n-prev_n,z-prev_z));prev_n=n;prev_z=z;
    }
    fs::write("canon-test/cad-a3-dev-ledger.md",report)?;
    println!("A3 dev full physical: N={} Z={} bytes; lookup={} verified={} raw_pass={}/{}",finals[0].3,finals[1].3,lookups,verified,raw,total);Ok(())
}
