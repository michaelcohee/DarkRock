use darkrock_storage::representation::{CHUNK_SIZE, choose, restore};
use darkrock_storage::storage::{STRIPE_SIZE, encode_stripe, reconstruct};
use std::{fs, path::{Path, PathBuf}, time::Instant};

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

#[derive(Default)]
struct Totals {
    files: usize, input: usize, selected: usize, coded: usize,
    stripes: usize, tails: usize, compressed_chunks: usize, skipped_chunks: usize,
    selection_ms: f64, encode_ms: f64, repair_restore_ms: f64,
}

fn run(root: &Path) -> Result<Totals, String> {
    let mut paths = Vec::new();
    pdfs(root, &mut paths)?;
    paths.sort();
    let mut t = Totals { files: paths.len(), ..Default::default() };
    for path in paths {
        let original = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if original.is_empty() { continue; }
        t.input += original.len();
        let start = Instant::now();
        let choices: Vec<_> = original.chunks(CHUNK_SIZE).map(choose).collect::<Result<_, _>>()?;
        let packed: Vec<u8> = choices.iter().flat_map(|choice| choice.payload.iter().copied()).collect();
        t.selection_ms += start.elapsed().as_secs_f64() * 1000.0;
        t.selected += packed.len();
        t.compressed_chunks += choices.iter().filter(|c| matches!(c.encoding, darkrock_storage::representation::Encoding::Zstd)).count();
        t.skipped_chunks += choices.iter().filter(|c| c.skipped_zstd).count();
        let mut reconstructed_packed = Vec::with_capacity(packed.len());
        for part in packed.chunks(STRIPE_SIZE) {
            let start = Instant::now();
            let stripe = encode_stripe(part)?;
            t.encode_ms += start.elapsed().as_secs_f64() * 1000.0;
            t.coded += stripe.shards.iter().map(Vec::len).sum::<usize>();
            t.stripes += 1;
            if part.len() < STRIPE_SIZE { t.tails += 1; }
            let mut shares: Vec<_> = stripe.shards.into_iter().map(Some).collect();
            shares[0] = None; shares[4] = None;
            let start = Instant::now();
            reconstructed_packed.extend(reconstruct(shares, &stripe.manifest)?);
            t.repair_restore_ms += start.elapsed().as_secs_f64() * 1000.0;
        }
        let start = Instant::now();
        let restored = restore(&choices, &reconstructed_packed)?;
        t.repair_restore_ms += start.elapsed().as_secs_f64() * 1000.0;
        if restored != original { return Err(format!("round trip differs: {}", path.display())); }
    }
    Ok(t)
}

fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).map(PathBuf::from)
        .ok_or("usage: pdf_pipeline_benchmark <local-pdf-directory>")?;
    let t = run(&root)?;
    println!("# PDF compression gate → variable-final-share RS 4+2\n");
    println!("Source: {} (non-hidden PDFs only). One release-build pass, files independently packed. Default 8 KiB start gate and zstd level 3. Every coded stripe was reconstructed after losing data share 0 and parity share 4; every file was restored and compared byte-for-byte. Manifest and receipts remain in memory and are not counted as persisted bytes. No encryption, network, or disk writes.\n", root.display());
    println!("| Files | Input bytes | Selected payload bytes | Actual RS shard bytes | Coded ratio/input | Compressed chunks | Skipped chunks | Stripes/tails | Select ms | RS encode ms | Repair+restore ms |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    println!("| {} | {} | {} | {} | {:.4}× | {} | {} | {} / {} | {:.1} | {:.1} | {:.1} |", t.files, t.input, t.selected, t.coded, t.coded as f64 / t.input as f64, t.compressed_chunks, t.skipped_chunks, t.stripes, t.tails, t.selection_ms, t.encode_ms, t.repair_restore_ms);
    Ok(())
}
