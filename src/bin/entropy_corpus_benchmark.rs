//! Compare one-sample policies on real files, never concatenating files.
use darkrock_storage::representation::{CHUNK_SIZE, EntropyGateConfig, inspect_gate_at};
use darkrock_storage::storage::{BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::{Path, PathBuf}, time::Instant};

#[derive(Clone, Copy)]
enum Policy { Start8, Start16, Middle8, Oracle }
impl Policy {
    fn label(self) -> &'static str {
        match self { Self::Start8 => "8 KiB start", Self::Start16 => "16 KiB start", Self::Middle8 => "8 KiB middle", Self::Oracle => "always zstd" }
    }
    fn config(self) -> EntropyGateConfig {
        EntropyGateConfig { sample_bytes: if matches!(self, Self::Start16) { 16 * 1024 } else { 8 * 1024 }, ..Default::default() }
    }
}

fn collect_rs(path: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() { collect_rs(&path, paths)?; }
        else if path.extension().is_some_and(|x| x == "rs") { paths.push(path); }
    }
    Ok(())
}

fn project_corpus(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = vec![root.join("README.md"), root.join("Cargo.toml")];
    collect_rs(&root.join("src"), &mut paths).map_err(|e| e.to_string())?;
    paths.sort();
    Ok(paths)
}

fn all_regular_files(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') { continue; }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() { continue; }
        if kind.is_dir() { all_regular_files(&entry.path(), paths)?; }
        else if kind.is_file() { paths.push(entry.path()); }
    }
    Ok(())
}

fn corpus(root: &Path, project_only: bool) -> Result<Vec<PathBuf>, String> {
    if project_only { return project_corpus(root); }
    let mut paths = Vec::new();
    all_regular_files(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn for_each_chunk(files: &[PathBuf], mut handle: impl FnMut(usize, &[u8]) -> Result<(), String>) -> Result<(), String> {
    let mut buffer = vec![0_u8; CHUNK_SIZE];
    for (file_index, path) in files.iter().enumerate() {
        let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        loop {
            let mut filled = 0;
            while filled < CHUNK_SIZE {
                let n = file.read(&mut buffer[filled..]).map_err(|e| format!("{}: {e}", path.display()))?;
                if n == 0 { break; }
                filled += n;
            }
            if filled == 0 { break; }
            handle(file_index, &buffer[..filled])?;
            if filled < CHUNK_SIZE { break; }
        }
    }
    Ok(())
}

#[derive(Default, Clone, Copy)]
struct Tally { input: usize, selected: usize, coded_bytes: usize, zstd_chunks: usize, skipped: usize, missed_bytes: usize }

fn fixed_4_2_bytes(payload_len: usize) -> usize {
    if payload_len == 0 { return 0; }
    let data_per_stripe = DATA_SHARDS * BLOCK_SIZE;
    payload_len.div_ceil(data_per_stripe) * (DATA_SHARDS + PARITY_SHARDS) * BLOCK_SIZE
}

fn select_chunk(chunk: &[u8], policy: Policy) -> Result<(usize, bool, bool), String> {
    let try_zstd = if matches!(policy, Policy::Oracle) { true } else {
        let config = policy.config();
        let sample_len = chunk.len().min(config.sample_bytes);
        let offset = if matches!(policy, Policy::Middle8) { (chunk.len() - sample_len) / 2 } else { 0 };
        inspect_gate_at(chunk, &config, offset)?.try_zstd
    };
    let _original_hash = std::hint::black_box(Sha256::digest(chunk));
    if !try_zstd { return Ok((chunk.len(), false, true)); }
    let compressed = zstd::stream::encode_all(chunk, 3).map_err(|e| e.to_string())?;
    let decoded = zstd::stream::decode_all(compressed.as_slice()).map_err(|e| e.to_string())?;
    if decoded != chunk { return Err("zstd round trip failed".into()); }
    let _compressed_hash = std::hint::black_box(Sha256::digest(&compressed));
    Ok((chunk.len().min(compressed.len()), compressed.len() < chunk.len(), false))
}

fn evaluate(files: &[PathBuf], policy: Policy, oracle_sizes: &[usize]) -> Result<Tally, String> {
    let mut result = Tally::default();
    let mut index = 0;
    let mut current_file = None;
    let mut selected_in_file = 0;
    for_each_chunk(files, |file_index, chunk| {
        if current_file != Some(file_index) {
            result.coded_bytes += fixed_4_2_bytes(selected_in_file);
            selected_in_file = 0;
            current_file = Some(file_index);
        }
        let (size, zstd, skipped) = select_chunk(chunk, policy)?;
        result.input += chunk.len();
        result.selected += size;
        selected_in_file += size;
        result.zstd_chunks += usize::from(zstd);
        result.skipped += usize::from(skipped);
        result.missed_bytes += size.saturating_sub(*oracle_sizes.get(index).ok_or("oracle chunk count mismatch")?);
        index += 1;
        Ok(())
    })?;
    result.coded_bytes += fixed_4_2_bytes(selected_in_file);
    if index != oracle_sizes.len() { return Err("oracle chunk count mismatch".into()); }
    Ok(result)
}

fn timed_evaluate(files: &[PathBuf], policy: Policy, oracle_sizes: &[usize], passes: usize) -> Result<(Tally, f64), String> {
    let mut times = Vec::with_capacity(passes);
    let mut first: Option<Tally> = None;
    for _ in 0..passes {
        let start = Instant::now();
        let tally = std::hint::black_box(evaluate(files, policy, oracle_sizes)?);
        if let Some(previous) = first {
            if tally.input != previous.input || tally.selected != previous.selected || tally.coded_bytes != previous.coded_bytes || tally.missed_bytes != previous.missed_bytes {
                return Err("corpus changed during benchmark".into());
            }
        } else { first = Some(tally); }
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    Ok((first.unwrap(), times[times.len() / 2]))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let root = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let pdf_only = args.iter().any(|arg| arg == "--pdf-only");
    let project_only = root == PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = corpus(&root, project_only)?;
    if pdf_only { files.retain(|path| path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("pdf"))); }
    let mut oracle_sizes = Vec::new();
    let mut total_input = 0_usize;
    for_each_chunk(&files, |_, chunk| {
        total_input += chunk.len();
        oracle_sizes.push(select_chunk(chunk, Policy::Oracle)?.0);
        Ok(())
    })?;
    let passes = if total_input > 8 * 1024 * 1024 { 3 } else { 101 };
    let sizes: Vec<_> = files.iter().map(|p| fs::metadata(p).map(|m| m.len() as usize).map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
    let raw_coded_bytes: usize = sizes.iter().map(|&size| fixed_4_2_bytes(size)).sum();
    println!("# Real-file entropy corpus\n");
    println!("Source: `{}`. {} Each file is chunked independently at 256 KiB; file boundaries are not concatenated. One initial or centered sample per chunk. Threshold 6.4 bits/byte with the same lag check. Times are median wall time over {} complete passes and include file reads, entropy/periodicity, zstd where attempted, round-trip verification, and hashing.\n", root.display(), if pdf_only { "PDF files only, recursively; hidden paths and symlinks excluded." } else if project_only { "Rust source, README, and Cargo manifest." } else { "All non-hidden regular files, recursively; symlinks excluded." }, passes);
    println!("Files: {}. Nonempty chunks: {}. Total input: {} bytes. Files at or below 8 KiB: {}. Files over 16 KiB: {}.\n", files.len(), oracle_sizes.len(), total_input, sizes.iter().filter(|&&n| n <= 8192).count(), sizes.iter().filter(|&&n| n > 16384).count());
    println!("Legacy fixed-share 4+2 control, with each file independently striped: {} coded bytes. Each nonempty stripe writes six fixed 256 KiB shares.\n", raw_coded_bytes);
    println!("| Policy | Selected bytes | Saved vs raw | Missed vs oracle | Fixed 4+2 coded bytes | Coded saving vs raw | Zstd chunks | Skipped chunks | Median select+verify ms |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    for policy in [Policy::Start8, Policy::Start16, Policy::Middle8, Policy::Oracle] {
        let (tally, time) = timed_evaluate(&files, policy, &oracle_sizes, passes)?;
        println!("| {} | {} | {} | {} | {} | {} | {} | {} | {:.3} |", policy.label(), tally.selected,
            tally.input - tally.selected, tally.missed_bytes, tally.coded_bytes,
            raw_coded_bytes.saturating_sub(tally.coded_bytes), tally.zstd_chunks, tally.skipped, time);
    }
    println!("\nFiles shorter than a policy's sample size are entirely sampled. Coded bytes model the legacy fixed-share layout per file and exclude metadata, encryption, and transport. They are calculated from selected lengths, not emitted by RS. The current variable-final-share codec is measured separately by variable_stripe_benchmark and pdf_pipeline_benchmark. The folder's file mix may differ from production DarkRock objects.\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_stripe_padding_boundaries() {
        assert_eq!(fixed_4_2_bytes(0), 0);
        assert_eq!(fixed_4_2_bytes(1), 6 * BLOCK_SIZE);
        assert_eq!(fixed_4_2_bytes(DATA_SHARDS * BLOCK_SIZE), 6 * BLOCK_SIZE);
        assert_eq!(fixed_4_2_bytes(DATA_SHARDS * BLOCK_SIZE + 1), 12 * BLOCK_SIZE);
    }
}
