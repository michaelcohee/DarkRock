//! Read-only real-file comparison of gated vs always-try compression with RS 4+2.
use darkrock_storage::representation::{CHUNK_SIZE, EntropyGateConfig, inspect_gate};
use darkrock_storage::storage::{STRIPE_SIZE, encode_stripe, reconstruct};
use std::{fs, io::Read, path::{Path, PathBuf}, time::Instant};

#[derive(Clone, Copy)]
enum Policy { ClampedGate, AlwaysTry }
impl Policy { fn label(self) -> &'static str { match self { Self::ClampedGate => "clamped entropy gate", Self::AlwaysTry => "no entropy / always try" } } }

#[derive(Default)]
struct Totals {
    files: u64, bytes: u64, source_batches: u64, chunks: u64, skipped: u64, zstd_attempts: u64,
    compressed: u64, payload: u64, coded: u64, stripes: u64, non_finite: u64,
    gate_ms: f64, zstd_encode_ms: f64, zstd_verify_ms: f64, rs_encode_ms: f64,
    rs_rebuild_ms: f64, restore_ms: f64,
}

fn files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') { continue; }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() { continue; }
        if kind.is_dir() { files(&entry.path(), out)?; }
        else if kind.is_file() { out.push(entry.path()); }
    }
    Ok(())
}

fn elapsed(start: Instant) -> f64 { start.elapsed().as_secs_f64() * 1000.0 }

fn run(paths: &[PathBuf], policy: Policy) -> Result<Totals, String> {
    let mut totals = Totals::default();
    let config = EntropyGateConfig::default();
    let mut buffer = vec![0u8; STRIPE_SIZE];
    for path in paths {
        totals.files += 1;
        let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        loop {
            let mut filled = 0;
            while filled < buffer.len() {
                let n = file.read(&mut buffer[filled..]).map_err(|e| format!("{}: {e}", path.display()))?;
                if n == 0 { break; }
                filled += n;
            }
            if filled == 0 { break; }
            totals.bytes += filled as u64;
            totals.source_batches += 1;
            let source = &buffer[..filled];
            let mut packed = Vec::with_capacity(filled);
            let mut records = Vec::new();
            for chunk in source.chunks(CHUNK_SIZE) {
                totals.chunks += 1;
                let try_zstd = match policy {
                    Policy::ClampedGate => {
                        let start = Instant::now();
                        let decision = inspect_gate(chunk, &config)?;
                        totals.gate_ms += elapsed(start);
                        if !decision.sample_entropy.is_finite() { totals.non_finite += 1; }
                        decision.try_zstd
                    }
                    Policy::AlwaysTry => true,
                };
                if !try_zstd {
                    totals.skipped += 1;
                    records.push((chunk.len(), chunk.len(), false));
                    packed.extend_from_slice(chunk);
                    continue;
                }
                totals.zstd_attempts += 1;
                let start = Instant::now();
                let compressed = zstd::stream::encode_all(chunk, 3).map_err(|e| e.to_string())?;
                totals.zstd_encode_ms += elapsed(start);
                let start = Instant::now();
                let decoded = zstd::stream::decode_all(compressed.as_slice()).map_err(|e| e.to_string())?;
                if decoded != chunk { return Err(format!("zstd verification failed: {}", path.display())); }
                totals.zstd_verify_ms += elapsed(start);
                if compressed.len() < chunk.len() {
                    totals.compressed += 1;
                    records.push((chunk.len(), compressed.len(), true));
                    packed.extend_from_slice(&compressed);
                } else {
                    records.push((chunk.len(), chunk.len(), false));
                    packed.extend_from_slice(chunk);
                }
            }
            totals.payload += packed.len() as u64;
            let mut recovered = Vec::with_capacity(packed.len());
            for part in packed.chunks(STRIPE_SIZE) {
                let start = Instant::now();
                let stripe = encode_stripe(part)?;
                totals.rs_encode_ms += elapsed(start);
                totals.coded += stripe.shards.iter().map(|s| s.len() as u64).sum::<u64>();
                totals.stripes += 1;
                let mut shares: Vec<_> = stripe.shards.into_iter().map(Some).collect();
                shares[0] = None;
                shares[4] = None;
                let start = Instant::now();
                recovered.extend(reconstruct(shares, &stripe.manifest)?);
                totals.rs_rebuild_ms += elapsed(start);
            }
            if recovered != packed { return Err(format!("RS rebuild differs: {}", path.display())); }
            let start = Instant::now();
            let mut offset = 0;
            let mut original_offset = 0;
            for (original_len, payload_len, compressed) in records {
                let end = offset + payload_len;
                let restored = if compressed {
                    zstd::stream::decode_all(&recovered[offset..end]).map_err(|e| e.to_string())?
                } else { recovered[offset..end].to_vec() };
                if restored != source[original_offset..original_offset + original_len] {
                    return Err(format!("file restoration differs: {}", path.display()));
                }
                offset = end;
                original_offset += original_len;
            }
            totals.restore_ms += elapsed(start);
            if filled < buffer.len() { break; }
        }
    }
    Ok(totals)
}

fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).map(PathBuf::from)
        .ok_or("usage: entropy_rs_lab <local-corpus-directory>")?;
    let mut paths = Vec::new();
    files(&root, &mut paths)?;
    paths.sort();
    println!("# RedTail-X 4+2 entropy lab\n");
    println!("Source: {}. Non-hidden regular files, recursively; symlinks skipped. One release-build pass per policy, clamped gate first. Each file is read in independent up-to-1 MiB source batches; each batch is compressed in 256 KiB chunks and then 4+2 coded. Zstd level 3. Data share 0 and parity share 4 are removed from every stripe, rebuilt, and compared with original bytes. Timings exclude source reads; verification and restore are shown separately. No disk writes, network, or persisted manifest.\n", root.display());
    println!("| Policy | Files | Source bytes | Chunks | Gate skips | Zstd tries | Compressed | Payload bytes | 4+2 bytes | Stripes | Non-finite | Gate ms | Zstd encode ms | Zstd verify ms | RS encode ms | RS rebuild ms | Restore ms |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for policy in [Policy::ClampedGate, Policy::AlwaysTry] {
        let t = run(&paths, policy)?;
        println!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |", policy.label(), t.files, t.bytes, t.chunks, t.skipped, t.zstd_attempts, t.compressed, t.payload, t.coded, t.stripes, t.non_finite, t.gate_ms, t.zstd_encode_ms, t.zstd_verify_ms, t.rs_encode_ms, t.rs_rebuild_ms, t.restore_ms);
    }
    Ok(())
}
