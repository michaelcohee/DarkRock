//! Raw 4+2 layout comparison: no compression, deduplication, or packing across files.
use darkrock_storage::storage::{
    BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS, STRIPE_SIZE, encode_stripe, reconstruct,
};
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};
use std::{env, fs, path::{Path, PathBuf}, time::Instant};

#[derive(Default)]
struct Totals {
    files: usize,
    source_bytes: u64,
    stripes: u64,
    tails: u64,
    variable_share_bytes: u64,
    fixed_share_bytes: u64,
    variable_encode_ms: f64,
    fixed_encode_ms: f64,
    variable_reconstruct_ms: f64,
    fixed_reconstruct_ms: f64,
}

fn collect_pdfs(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() { continue; }
        if kind.is_dir() { collect_pdfs(&entry.path(), paths)?; }
        else if kind.is_file() && entry.path().extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("pdf")) {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn run(paths: &[PathBuf]) -> Result<Totals, String> {
    let codec = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS).map_err(|e| e.to_string())?;
    let mut totals = Totals::default();
    for path in paths {
        let source = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if source.is_empty() { continue; }
        totals.files += 1;
        totals.source_bytes += source.len() as u64;
        let mut variable_restored = Vec::with_capacity(source.len());
        let mut fixed_restored = Vec::with_capacity(source.len());
        for part in source.chunks(STRIPE_SIZE) {
            totals.stripes += 1;
            if part.len() < STRIPE_SIZE { totals.tails += 1; }

            let start = Instant::now();
            let variable = encode_stripe(part)?;
            totals.variable_encode_ms += start.elapsed().as_secs_f64() * 1000.0;
            totals.variable_share_bytes += variable.shards.iter().map(|s| s.len() as u64).sum::<u64>();
            let mut variable_shares: Vec<_> = variable.shards.into_iter().map(Some).collect();
            variable_shares[0] = None;
            variable_shares[4] = None;
            let start = Instant::now();
            let rebuilt = reconstruct(variable_shares, &variable.manifest)?;
            totals.variable_reconstruct_ms += start.elapsed().as_secs_f64() * 1000.0;
            if rebuilt != part { return Err("variable-tail stripe mismatch".into()); }
            variable_restored.extend_from_slice(&rebuilt);

            let start = Instant::now();
            let mut fixed = vec![vec![0_u8; BLOCK_SIZE]; DATA_SHARDS + PARITY_SHARDS];
            for (dst, src) in fixed[..DATA_SHARDS].iter_mut().zip(part.chunks(BLOCK_SIZE)) {
                dst[..src.len()].copy_from_slice(src);
            }
            codec.encode(&mut fixed).map_err(|e| e.to_string())?;
            let fixed_original_hash = Sha256::digest(part);
            let fixed_shard_hashes: Vec<_> = fixed.iter().map(|s| Sha256::digest(s)).collect();
            totals.fixed_encode_ms += start.elapsed().as_secs_f64() * 1000.0;
            totals.fixed_share_bytes += fixed.iter().map(|s| s.len() as u64).sum::<u64>();
            let mut fixed_shares: Vec<_> = fixed.into_iter().map(Some).collect();
            fixed_shares[0] = None;
            fixed_shares[4] = None;
            let start = Instant::now();
            for (share, expected_hash) in fixed_shares.iter().zip(&fixed_shard_hashes) {
                if let Some(bytes) = share {
                    if Sha256::digest(bytes) != *expected_hash {
                        return Err("fixed surviving share hash mismatch".into());
                    }
                }
            }
            codec.reconstruct_data(&mut fixed_shares).map_err(|e| e.to_string())?;
            let mut fixed_part = Vec::with_capacity(STRIPE_SIZE);
            for (shard, expected_hash) in fixed_shares.into_iter().zip(&fixed_shard_hashes).take(DATA_SHARDS) {
                let shard = shard.ok_or("missing fixed data share")?;
                if Sha256::digest(&shard) != *expected_hash {
                    return Err("fixed reconstructed share hash mismatch".into());
                }
                fixed_part.extend_from_slice(&shard);
            }
            fixed_part.truncate(part.len());
            if Sha256::digest(&fixed_part) != fixed_original_hash {
                return Err("fixed original hash mismatch".into());
            }
            totals.fixed_reconstruct_ms += start.elapsed().as_secs_f64() * 1000.0;
            if fixed_part != part { return Err("fixed-share stripe mismatch".into()); }
            fixed_restored.extend_from_slice(&fixed_part);
        }
        let source_hash = Sha256::digest(&source);
        if variable_restored.len() != source.len() || fixed_restored.len() != source.len()
            || Sha256::digest(&variable_restored) != source_hash
            || Sha256::digest(&fixed_restored) != source_hash {
            return Err(format!("file hash mismatch: {}", path.display()));
        }
    }
    Ok(totals)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let kind = args.next().ok_or("usage: raw_redtail_benchmark --file PATH | --pdf-dir DIR")?;
    let target = PathBuf::from(args.next().ok_or("missing path")?);
    if args.next().is_some() { return Err("unexpected argument".into()); }
    let mut paths = match kind.as_str() {
        "--file" if target.is_file() => vec![target],
        "--pdf-dir" if target.is_dir() => {
            let mut files = Vec::new();
            collect_pdfs(&target, &mut files)?;
            files
        }
        _ => return Err("use --file PATH or --pdf-dir DIR".into()),
    };
    paths.sort();
    let t = run(&paths)?;
    if t.files == 0 { return Err("no nonempty input files".into()); }
    println!("files={} source_bytes={} stripes={} tails={} variable_share_bytes={} fixed_share_bytes={} bytes_saved={} variable_ratio={:.6} fixed_ratio={:.6} variable_encode_ms={:.3} fixed_encode_ms={:.3} variable_reconstruct_ms={:.3} fixed_reconstruct_ms={:.3}",
        t.files, t.source_bytes, t.stripes, t.tails, t.variable_share_bytes,
        t.fixed_share_bytes, t.fixed_share_bytes - t.variable_share_bytes,
        t.variable_share_bytes as f64 / t.source_bytes as f64,
        t.fixed_share_bytes as f64 / t.source_bytes as f64,
        t.variable_encode_ms, t.fixed_encode_ms, t.variable_reconstruct_ms, t.fixed_reconstruct_ms);
    Ok(())
}
