use darkrock_storage::dxf_adapter::AdapterStore;
use darkrock_storage::storage::{encode_stripe,reconstruct,STRIPE_SIZE};
use std::{fs,path::{Path,PathBuf}};

fn gather(dir:&Path,out:&mut Vec<PathBuf>)->std::io::Result<()> {
    for ent in fs::read_dir(dir)? {let path=ent?.path();if path.is_dir(){gather(&path,out)?}else if path.extension().is_some_and(|x|x=="dxf"){out.push(path)}}Ok(())
}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let root=Path::new("canon-test/cad-corpus-v1/dev");let mut paths=Vec::new();gather(root,&mut paths)?;paths.sort();
    if paths.len()!=44{return Err(format!("expected 44 dev DXFs, found {}",paths.len()).into())}
    let mut store=AdapterStore::default();let mut inputs=Vec::new();let mut rows=Vec::new();
    for path in &paths {let input=fs::read(path)?;let id=store.ingest(&input).map_err(std::io::Error::other)?;let c=&store.files[id].coverage;
        rows.push(format!("| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",path.strip_prefix(root)?.display(),c.total_bytes,c.parsed_bytes,c.raw_bytes,c.matched_bytes,c.eligible_entities,c.lookup_hits,c.exact_hash_matches,c.edited_matches,c.literal_runs));inputs.push(input);
    }
    for (id,input) in inputs.iter().enumerate(){if store.restore(id).map_err(std::io::Error::other)?!=*input{return Err(format!("restore mismatch: {}",paths[id].display()).into())}}
    // Protect the actual literal-run payloads, not the original source files.
    let mut checked_stripes=0usize;
    for object in &store.encoded_objects {for part in object.chunks(STRIPE_SIZE){
        let stripe=encode_stripe(part).map_err(std::io::Error::other)?;
        for first in 0..6 {for second in first+1..6 {
            let mut shares:Vec<_>=stripe.shards.iter().cloned().map(Some).collect();shares[first]=None;shares[second]=None;
            if reconstruct(shares,&stripe.manifest).map_err(std::io::Error::other)?!=part{return Err("RS two-loss mismatch".into())}
        }}checked_stripes+=1;
    }}
    let raw:usize=inputs.iter().map(Vec::len).sum();let parsed:usize=store.files.iter().map(|f|f.coverage.parsed_bytes).sum();let matched:usize=store.files.iter().map(|f|f.coverage.matched_bytes).sum();
    let stored:usize=store.encoded_objects.iter().map(Vec::len).sum::<usize>()+store.files.iter().flat_map(|f|f.items.iter()).map(|i|match i{darkrock_storage::dxf_adapter::Item::Matched{middle,..}=>middle.len(),_=>0}).sum::<usize>();
    let edits:Vec<u8>=store.files.iter().flat_map(|f|f.items.iter()).flat_map(|i|match i{darkrock_storage::dxf_adapter::Item::Matched{middle,..}=>middle.as_slice(),_=>&[]}).copied().collect();
    let compressed_edits=zstd::bulk::compress(&edits,3)?;
    let literal_bytes:usize=store.encoded_objects.iter().map(Vec::len).sum();
    let packed_lower_bound=literal_bytes+compressed_edits.len().min(edits.len());
    let lookups:usize=store.files.iter().map(|f|f.coverage.lookup_hits).sum();
    let exact:usize=store.files.iter().map(|f|f.coverage.exact_hash_matches).sum();
    let edited:usize=store.files.iter().map(|f|f.coverage.edited_matches).sum();
    let mut report=format!("# CAD adapter v1 — Tier A development run\n\nAll 44 development DXFs restored byte-for-byte. No Tier A test DXF was read in this run; an older exploratory control had already read that original test split. Encoded literal-run payloads passed every two-share loss pair on {checked_stripes} RedTail-X stripes (15 pairs per stripe); one-share loss is separately covered by `storage.rs` tests. **References, edit descriptors and the index were not striped in this check.**\n\nThe only v1 candidate edit is `(prefix_len, suffix_len, literal middle bytes)`; it fired {edited} times. Exact raw-hash references fired {exact} times. There were {lookups} entity-key lookup hits; {exact} exact and {edited} edited matches passed byte and SHA-256 verification. Other lookup hits remained literal. No key equality alone caused a merge.\n\nRaw input bytes: {raw}. Parsed eligible bytes: {parsed}. Matched entity bytes: {matched}. Encoded literal objects plus uncompressed edit middle bytes (excluding references/index): {stored}. Literal encoded bytes: {literal_bytes}. Edit middle bytes: {} raw, {} as one zstd block. The current layout's literal-plus-compressed-edit partial tally is {packed_lower_bound} bytes before references, index, metadata and 4+2. This is an adapter diagnostic, **not** a protected-byte A3 benchmark.\n\n| File | Total bytes | Parsed bytes | Raw bytes | Matched bytes | Eligible entities | Key lookups | Exact refs | Edited refs | Literal runs |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n",edits.len(),compressed_edits.len());
    report.push_str(&rows.join("\n"));report.push('\n');fs::write("canon-test/cad-adapter-dev-results.md",report)?;println!("restored {} dev DXFs; raw={raw} parsed={parsed} matched={matched} objects={}",paths.len(),store.objects.len());Ok(())
}
