//! Isolated boundary-value example for byte Shannon entropy.
use darkrock_storage::representation::entropy_bits_per_byte;
use darkrock_storage::representation::CHUNK_SIZE;
use std::{fs, io::Read, path::Path};

const EPSILON: f64 = 1e-12;

fn histogram(sample: &[u8]) -> [usize; 256] {
    let mut counts = [0; 256];
    for &byte in sample {
        counts[byte as usize] += 1;
    }
    counts
}

fn naive_entropy(sample: &[u8]) -> f64 {
    if sample.is_empty() { return 0.0; }
    let size = sample.len() as f64;
    histogram(sample).into_iter().map(|count| {
        let p = count as f64 / size;
        -p * p.log2()
    }).sum()
}

fn clamped_entropy(sample: &[u8]) -> f64 {
    if sample.is_empty() { return 0.0; }
    let size = sample.len() as f64;
    histogram(sample).into_iter().map(|count| {
        let p = (count as f64 / size).clamp(EPSILON, 1.0 - EPSILON);
        -p * p.log2()
    }).sum()
}

#[derive(Default)]
struct Scan {
    files: u64,
    bytes: u64,
    chunks: u64,
    samples: u64,
    non_finite: u64,
    min: f64,
    max: f64,
}

fn scan_path(path: &Path, scan: &mut Scan) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() { return Ok(()); }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with('.') { continue; }
            scan_path(&entry.path(), scan)?;
        }
    } else if metadata.is_file() {
        scan.files += 1;
        let mut file = fs::File::open(path)?;
        let mut buffer = vec![0; CHUNK_SIZE];
        loop {
            let mut size = 0;
            while size < buffer.len() {
                let read = file.read(&mut buffer[size..])?;
                if read == 0 { break; }
                size += read;
            }
            if size == 0 { break; }
            scan.bytes += size as u64;
            scan.chunks += 1;
            for sample in [&buffer[..size.min(8192)], &buffer[..size]] {
                let entropy = entropy_bits_per_byte(sample);
                scan.samples += 1;
                if !entropy.is_finite() {
                    scan.non_finite += 1;
                    eprintln!("non-finite entropy: {}", path.display());
                } else {
                    scan.min = scan.min.min(entropy);
                    scan.max = scan.max.max(entropy);
                }
            }
            if size < buffer.len() { break; }
        }
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    if let Some(root) = std::env::args_os().nth(1) {
        let mut scan = Scan { min: f64::INFINITY, ..Default::default() };
        scan_path(Path::new(&root), &mut scan)?;
        println!("files={} bytes={} chunks={} entropy_checks={} non_finite={} min={:.6} max={:.6}",
            scan.files, scan.bytes, scan.chunks, scan.samples, scan.non_finite, scan.min, scan.max);
        if scan.non_finite != 0 { std::process::exit(1); }
        return Ok(());
    }
    for (name, sample) in [
        ("empty", &[][..]),
        ("constant", &[0u8; 8192][..]),
        ("alternating", &[0u8, 1][..]),
        ("all byte values", &(0u8..=255).collect::<Vec<_>>()[..]),
    ] {
        let naive = naive_entropy(sample);
        let clamped = clamped_entropy(sample);
        let production = entropy_bits_per_byte(sample);
        println!("{name:15} naive={naive:>8.5}  clamped={clamped:>8.5}  production={production:>8.5}");
        assert!(clamped.is_finite() && production.is_finite());
        if !sample.is_empty() && sample.len() < 256 { assert!(naive.is_nan()); }
    }
    Ok(())
}
