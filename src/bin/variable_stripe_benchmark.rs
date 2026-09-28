use darkrock_storage::storage::{BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS, STRIPE_SIZE, encode_stripe, reconstruct};
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::{Path, PathBuf}, time::Instant};

fn pdfs(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') { continue; }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() { continue; }
        if kind.is_dir() { pdfs(&entry.path(), paths)?; }
        else if kind.is_file() && entry.path().extension().is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case("pdf")) {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn fixed_encode(bytes: &[u8], codec: &ReedSolomon) -> Result<(Vec<Vec<u8>>, Vec<[u8; 32]>), String> {
    let mut shards = vec![vec![0_u8; BLOCK_SIZE]; DATA_SHARDS + PARITY_SHARDS];
    for (dst, src) in shards[..DATA_SHARDS].iter_mut().zip(bytes.chunks(BLOCK_SIZE)) {
        dst[..src.len()].copy_from_slice(src);
    }
    codec.encode(&mut shards).map_err(|e| e.to_string())?;
    let _original_hash = std::hint::black_box(Sha256::digest(bytes));
    let hashes = shards.iter().map(|s| Sha256::digest(s).into()).collect();
    Ok((shards, hashes))
}

fn fixed_reconstruct(mut shares: Vec<Option<Vec<u8>>>, hashes: &[[u8; 32]], original: &[u8], codec: &ReedSolomon) -> Result<Vec<u8>, String> {
    for (share, hash) in shares.iter_mut().zip(hashes) {
        if let Some(data) = share {
            if data.len() != BLOCK_SIZE || Sha256::digest(data.as_slice()).as_slice() != hash { *share = None; }
        }
    }
    codec.reconstruct_data(&mut shares).map_err(|e| e.to_string())?;
    let mut output = Vec::with_capacity(STRIPE_SIZE);
    for (share, hash) in shares.into_iter().zip(hashes).take(DATA_SHARDS) {
        let data = share.ok_or("missing fixed data share")?;
        if Sha256::digest(&data).as_slice() != hash { return Err("fixed share hash differs".into()); }
        output.extend_from_slice(&data);
    }
    output.truncate(original.len());
    if output != original { return Err("fixed reconstruction differs".into()); }
    Ok(output)
}

#[derive(Default)]
struct Totals {
    files: usize, input: usize, stripes: usize, tails: usize,
    fixed_bytes: usize, variable_bytes: usize,
    fixed_full_encode_ms: f64, variable_full_encode_ms: f64,
    fixed_tail_encode_ms: f64, variable_tail_encode_ms: f64,
    fixed_tail_repair_ms: f64, variable_tail_repair_ms: f64,
}

fn run(root: &Path) -> Result<Totals, String> {
    let mut paths = Vec::new();
    pdfs(root, &mut paths)?;
    paths.sort();
    let fixed_codec = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS).map_err(|e| e.to_string())?;
    let mut totals = Totals { files: paths.len(), ..Default::default() };
    let mut buffer = vec![0_u8; STRIPE_SIZE];
    for path in paths {
        let mut file = fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        loop {
            let mut filled = 0;
            while filled < STRIPE_SIZE {
                let n = file.read(&mut buffer[filled..]).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                filled += n;
            }
            if filled == 0 { break; }
            let bytes = &buffer[..filled];
            let tail = filled < STRIPE_SIZE;
            let start = Instant::now();
            let (fixed, fixed_hashes) = fixed_encode(bytes, &fixed_codec)?;
            let fixed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = Instant::now();
            let variable = encode_stripe(bytes)?;
            let variable_ms = start.elapsed().as_secs_f64() * 1000.0;
            totals.input += filled;
            totals.stripes += 1;
            totals.fixed_bytes += fixed.iter().map(Vec::len).sum::<usize>();
            totals.variable_bytes += variable.shards.iter().map(Vec::len).sum::<usize>();
            if tail {
                totals.tails += 1;
                totals.fixed_tail_encode_ms += fixed_ms;
                totals.variable_tail_encode_ms += variable_ms;
                let mut fixed_shares: Vec<_> = fixed.into_iter().map(Some).collect();
                fixed_shares[0] = None; fixed_shares[4] = None;
                let start = Instant::now();
                assert_eq!(fixed_reconstruct(fixed_shares, &fixed_hashes, bytes, &fixed_codec)?, bytes);
                totals.fixed_tail_repair_ms += start.elapsed().as_secs_f64() * 1000.0;
                let mut variable_shares: Vec<_> = variable.shards.into_iter().map(Some).collect();
                variable_shares[0] = None; variable_shares[4] = None;
                let start = Instant::now();
                assert_eq!(reconstruct(variable_shares, &variable.manifest)?, bytes);
                totals.variable_tail_repair_ms += start.elapsed().as_secs_f64() * 1000.0;
            } else {
                totals.fixed_full_encode_ms += fixed_ms;
                totals.variable_full_encode_ms += variable_ms;
            }
            if filled < STRIPE_SIZE { break; }
        }
    }
    Ok(totals)
}

fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).map(PathBuf::from)
        .ok_or("usage: variable_stripe_benchmark <local-pdf-directory>")?;
    let t = run(&root)?;
    let ideal = t.input as f64 * (DATA_SHARDS + PARITY_SHARDS) as f64 / DATA_SHARDS as f64;
    println!("# Actual RS 4+2 variable final-share benchmark\n");
    println!("Source: {} (PDFs only). One release-build pass. Files are encoded independently. Tail reconstruction removes data share 0 and parity share 4, verifies hashes and exact output. No disk persistence, encryption, network, or compressed representation in this isolation pass.\n", root.display());
    println!("Files: {}. Input bytes: {}. Stripes: {} (partial tails: {}). Ideal 1.5×: {:.1} bytes.\n", t.files, t.input, t.stripes, t.tails, ideal);
    println!("| Layout | Actual emitted shard bytes | Overhead ratio | Full-stripe encode ms | Tail encode ms | Tail two-loss repair ms |");
    println!("|---|---:|---:|---:|---:|---:|");
    println!("| Fixed 256 KiB shares | {} | {:.4}× | {:.1} | {:.1} | {:.1} |", t.fixed_bytes, t.fixed_bytes as f64 / t.input as f64, t.fixed_full_encode_ms, t.fixed_tail_encode_ms, t.fixed_tail_repair_ms);
    println!("| Variable final shares | {} | {:.4}× | {:.1} | {:.1} | {:.1} |", t.variable_bytes, t.variable_bytes as f64 / t.input as f64, t.variable_full_encode_ms, t.variable_tail_encode_ms, t.variable_tail_repair_ms);
    println!("\nActual emitted-byte saving: {}. Fixed and variable full stripes have identical share lengths. Timings are single-pass local CPU wall time and include RS parity and hashing; they are not robust latency distributions.\n", t.fixed_bytes - t.variable_bytes);
    Ok(())
}
