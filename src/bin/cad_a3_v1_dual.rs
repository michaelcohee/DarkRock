use darkrock_storage::dxf_adapter::{AdapterStore, LedgerMode};
use darkrock_storage::storage::{encode_stripe, reconstruct, STRIPE_SIZE};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn gather(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            gather(&path, files)?
        } else if path.extension().is_some_and(|e| e == "dxf") {
            files.push(path)
        }
    }
    Ok(())
}
fn protected(bytes: &[u8]) -> Result<(usize, usize, usize), String> {
    let mut catalog = Vec::new();
    catalog.extend_from_slice(b"DRMC1");
    catalog.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    catalog.extend_from_slice(&(bytes.len().div_ceil(STRIPE_SIZE) as u64).to_le_bytes());
    let mut shares = 0;
    for chunk in bytes.chunks(STRIPE_SIZE) {
        let stripe = encode_stripe(chunk)?;
        shares += stripe.shards.iter().map(Vec::len).sum::<usize>();
        let m = &stripe.manifest;
        catalog.extend_from_slice(&(m.original_len as u64).to_le_bytes());
        catalog.extend_from_slice(&(m.share_len as u64).to_le_bytes());
        catalog.extend_from_slice(&m.original_hash);
        for hash in &m.shard_hashes {
            catalog.extend_from_slice(hash)
        }
        for a in 0..6 {
            for b in a..6 {
                let mut ss: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                ss[a] = None;
                if a != b {
                    ss[b] = None
                }
                if reconstruct(ss, m)? != chunk {
                    return Err("data stripe loss mismatch".into());
                }
            }
        }
    }
    let mut catalog_shares = 0;
    let mut catalog_stripes = 0;
    for chunk in catalog.chunks(STRIPE_SIZE) {
        let stripe = encode_stripe(chunk)?;
        catalog_shares += stripe.shards.iter().map(Vec::len).sum::<usize>();
        catalog_stripes += 1;
        for a in 0..6 {
            for b in a..6 {
                let mut ss: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                ss[a] = None;
                if a != b {
                    ss[b] = None
                }
                if reconstruct(ss, &stripe.manifest)? != chunk {
                    return Err("catalog stripe loss mismatch".into());
                }
            }
        }
    }
    let root = Sha256::digest(&catalog);
    if root.len() != 32 {
        return Err("bad catalog root".into());
    }
    Ok((shares, catalog_shares, 6 * (32 + 240 * catalog_stripes)))
}
fn run_generic(dir: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    gather(dir, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err("no DXFs in requested directory".into());
    }
    let input: Vec<Vec<u8>> = paths.iter().map(fs::read).collect::<Result<_, _>>()?;
    let mut store = AdapterStore::default();
    for raw in &input {
        store.ingest(raw)?;
    }
    store.verify_index_rebuild()?;
    let mut out = String::from(
        "arm\tmode\tview\tfiles\tsource_bytes\tprotected_bytes\tall_21_loss_patterns_exact\n",
    );
    for (view, protect) in [("Protected", true), ("Derived", false)] {
        let (blob, _) = store.export_ledger_with_index(LedgerMode::Z, protect)?;
        AdapterStore::verify_ledger(&blob, &input)?;
        let (data, catalog, boot) = protected(&blob)?;
        let total = data + catalog + boot;
        out.push_str(&format!(
            "A3_v1\tZ\t{view}\t{}\t{}\t{total}\ttrue\n",
            input.len(),
            input.iter().map(Vec::len).sum::<usize>()
        ));
        println!("A3 v1 {} files {view}: {total}", input.len());
    }
    fs::write(output, out)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 {
        return run_generic(Path::new(&args[1]), Path::new(&args[2]));
    }
    if args.len() != 1 {
        return Err("usage: cad_a3_v1_dual [DXF_DIR OUTPUT_TSV]".into());
    }
    let dir = Path::new("canon-test/cad-corpus-v1/dev");
    let mut paths = Vec::new();
    gather(dir, &mut paths)?;
    paths.sort();
    if paths.len() != 44 {
        return Err("expected 44 dev files".into());
    }
    let input: Vec<Vec<u8>> = paths.iter().map(fs::read).collect::<Result<_, _>>()?;
    let mut store = AdapterStore::default();
    for raw in &input {
        store.ingest(raw)?;
    }
    let (keys, verified) = store.verify_index_rebuild()?;
    let mut report=String::from("# A3 v1 dual-index development report\n\n44 Tier A development DXFs only. The original candidate index was deleted for the Derived view; all files were restored from durable literal/edit/reference data and re-ingested. The complete per-file lookup and verify-pass vector matched. Every serialized ledger and its catalog survived all 21 one/two-share loss patterns under RedTail-X variable-tail 4+2.\n\n| Mode | View | Ledger bytes | Index bytes | Data shares | Catalog shares | Bootstrap | Protected bytes |\n|---|---|---:|---:|---:|---:|---:|---:|\n");
    let mut totals = Vec::new();
    for (name, mode) in [("N", LedgerMode::N), ("Z", LedgerMode::Z)] {
        for (view, protect_index) in [("Protected", true), ("Derived", false)] {
            let (blob, parts) = store.export_ledger_with_index(mode, protect_index)?;
            AdapterStore::verify_ledger(&blob, &input)?;
            let (data, catalog, boot) = protected(&blob)?;
            let total = data + catalog + boot;
            report.push_str(&format!(
                "| {name} | {view} | {} | {} | {data} | {catalog} | {boot} | {total} |\n",
                blob.len(),
                parts.key_hash_index
            ));
            totals.push(total);
        }
    }
    report.push_str(&format!("\nRebuild test: {keys} geometry keys; {verified} verified entity matches; per-file lookup, exact-match and edited-match counts identical after replay. Cache RAM and rebuild CPU are not included in physical bytes. A3 v1 Protected Z = {} bytes, versus C2 Z = 2,286,900 bytes on the same dev set: A3 v1 still loses.\n",totals[2]));
    fs::write("canon-test/cad-a3-v1-dual-dev.md", report)?;
    println!(
        "A3 v1 dual dev: N={},{} Z={},{} rebuilt_keys={} verified={}",
        totals[0], totals[1], totals[2], totals[3], keys, verified
    );
    Ok(())
}
