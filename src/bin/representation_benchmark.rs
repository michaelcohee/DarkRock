use darkrock_storage::representation::{CHUNK_SIZE, METADATA_BYTES_PER_CHUNK, EntropyGateConfig, choose_with_config, inspect_gate, restore};
use darkrock_storage::storage::{BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS, encode_stripe, reconstruct};
use sha2::{Digest, Sha256};
use std::time::Instant;

const MIB: usize = 1024 * 1024;

fn random_bytes(len: usize) -> Vec<u8> {
    let mut x = 0x1234_5678_9abc_def0_u64;
    (0..len).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect()
}

fn logs(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        out.extend_from_slice(format!("2026-09-26 INFO tenant=darkrock block={} repair=ok latency_ms=21\n", i % 1000).as_bytes());
        i += 1;
    }
    out.truncate(len); out
}

fn encoded_size(bytes: &[u8]) -> (usize, f64) {
    let start = Instant::now();
    let mut physical = 0;
    for chunk in bytes.chunks(DATA_SHARDS * BLOCK_SIZE) {
        let stripe = encode_stripe(chunk).unwrap();
        physical += stripe.shards.iter().map(Vec::len).sum::<usize>();
    }
    (physical, start.elapsed().as_secs_f64() * 1000.0)
}

fn run(name: &str, input: &[u8], config: &EntropyGateConfig) {
    let (raw_coded, raw_encode_ms) = encoded_size(input);
    let start = Instant::now();
    let choices: Vec<_> = input.chunks(CHUNK_SIZE).map(|c| choose_with_config(c, config).unwrap()).collect();
    let selection_ms = start.elapsed().as_secs_f64() * 1000.0;
    let baseline_config = EntropyGateConfig { threshold_bits_per_byte: 8.0, ..config.clone() };
    let start = Instant::now();
    let baseline: Vec<_> = input.chunks(CHUNK_SIZE).map(|c| choose_with_config(c, &baseline_config).unwrap()).collect();
    let baseline_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert!(baseline.iter().all(|c| !c.skipped_zstd));
    let packed: Vec<u8> = choices.iter().flat_map(|c| c.payload.iter().copied()).collect();
    let (selected_coded, selected_encode_ms) = encoded_size(&packed);

    let start = Instant::now();
    let mut raw_verified = 0;
    for chunk in input.chunks(CHUNK_SIZE) { raw_verified ^= Sha256::digest(chunk)[0]; }
    std::hint::black_box(raw_verified);
    let raw_read_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    assert_eq!(restore(&choices, &packed).unwrap(), input);
    let selected_read_ms = start.elapsed().as_secs_f64() * 1000.0;

    let encoded: Vec<_> = packed.chunks(DATA_SHARDS * BLOCK_SIZE).map(|c| encode_stripe(c).unwrap()).collect();
    let mut repaired = Vec::new();
    let start = Instant::now();
    for stripe in &encoded {
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0] = None;
        repaired.extend(reconstruct(shares, &stripe.manifest).unwrap());
    }
    let repair_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(restore(&choices, &repaired).unwrap(), input);
    let raw_count = choices.iter().filter(|c| matches!(c.encoding, darkrock_storage::representation::Encoding::Raw)).count();
    let compressed_count = choices.len() - raw_count;
    let skipped_count = choices.iter().filter(|c| c.skipped_zstd).count();
    let metadata = choices.len() * METADATA_BYTES_PER_CHUNK;
    let receipt_metadata = skipped_count + (choices.len() - skipped_count) * 37;
    println!("| {} | {} / {} / {} | {} | {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} / {:.1} | {:.1} |", name, compressed_count, raw_count, skipped_count, packed.len(), raw_coded, selected_coded, metadata + receipt_metadata, raw_encode_ms, baseline_ms, selection_ms, selected_encode_ms, raw_read_ms, selected_read_ms, repair_ms);
}

fn env_f64(name: &str, default: f64) -> f64 { std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default) }
fn env_usize(name: &str, default: usize) -> usize { std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default) }
fn env_lags(default: &[usize]) -> Vec<usize> {
    std::env::var("DARKROCK_PERIOD_LAGS").ok().map(|value| {
        value.split(',').map(|x| x.trim().parse().expect("period lags must be positive integers")).collect()
    }).unwrap_or_else(|| default.to_vec())
}

fn pattern_checks(config: &EntropyGateConfig) {
    println!("\n## Periodic-pattern checks\n");
    println!("| Period | Sample entropy | Matching lag | Chosen encoding | Selected bytes |");
    println!("|---:|---:|---:|---|---:|");
    for period in [2usize, 4, 256, 4096, 6144, 7168, 7680, 8192, 16384] {
        let motif: Vec<u8> = if period >= 4096 {
            random_bytes(period)
        } else { (0..period).map(|i| (i * 83) as u8).collect() };
        let data: Vec<u8> = (0..CHUNK_SIZE).map(|i| motif[i % period]).collect();
        let choice = choose_with_config(&data, config).unwrap();
        println!("| {} | {:.3} | {} | {:?} | {} |", period, choice.first_sample_entropy,
            choice.matching_period_lag.map(|n| n.to_string()).unwrap_or_else(|| "—".into()), choice.encoding, choice.payload.len());
    }
}

fn repeated_random_block(period: usize) -> Vec<u8> {
    let motif = random_bytes(period);
    (0..CHUNK_SIZE).map(|i| motif[i % period]).collect()
}

fn gate_median_us(chunk: &[u8], config: &EntropyGateConfig) -> (darkrock_storage::representation::GateDecision, f64) {
    let mut times = Vec::with_capacity(101);
    let mut decision = inspect_gate(chunk, config).unwrap();
    for _ in 0..101 {
        let start = Instant::now();
        decision = std::hint::black_box(inspect_gate(std::hint::black_box(chunk), config).unwrap());
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }
    times.sort_by(f64::total_cmp);
    (decision, times[times.len() / 2])
}

fn gate_diagnostics(config: &EntropyGateConfig) {
    println!("\n## Gate-only cost and missed compression\n");
    println!("Each row is one 256 KiB chunk. Gate-only cost is the median of 101 calls and excludes copying, hashing, and zstd. Oracle bytes come from always trying zstd level 3, solely to expose opportunities the gate skips.\n");
    println!("| Pattern | Entropy | Gate action | Lag | Gate-only µs | Raw bytes | Oracle zstd bytes | Missed saving bytes |");
    println!("|---|---:|---|---:|---:|---:|---:|---:|");
    let random = random_bytes(CHUNK_SIZE);
    let mut random_head_zeros_tail = random.clone();
    random_head_zeros_tail[config.sample_bytes.min(CHUNK_SIZE)..].fill(0);
    let cases = [
        ("pseudorandom", random),
        ("logs", logs(CHUNK_SIZE)),
        ("period-256", (0..CHUNK_SIZE).map(|i| (i % 256) as u8).collect()),
        ("period-6144", repeated_random_block(6144)),
        ("period-8192", repeated_random_block(8192)),
        ("random-head/zero-tail", random_head_zeros_tail),
    ];
    for (label, chunk) in cases {
        let (decision, gate_us) = gate_median_us(&chunk, config);
        let compressed = zstd::stream::encode_all(chunk.as_slice(), 3).unwrap();
        let missed = if decision.try_zstd { 0 } else { chunk.len().saturating_sub(compressed.len()) };
        println!("| {} | {:.3} | {} | {} | {:.1} | {} | {} | {} |", label,
            decision.sample_entropy, if decision.try_zstd { "try zstd" } else { "skip" },
            decision.matching_period_lag.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
            gate_us, chunk.len(), compressed.len(), missed);
    }
}

fn main() {
    let random = random_bytes(16 * MIB);
    let text = logs(16 * MIB);
    let mixed = [random[..8 * MIB].as_ref(), text[..8 * MIB].as_ref()].concat();
    let defaults = EntropyGateConfig::default();
    let config = EntropyGateConfig {
        threshold_bits_per_byte: env_f64("DARKROCK_ENTROPY_THRESHOLD", defaults.threshold_bits_per_byte),
        sample_bytes: env_usize("DARKROCK_ENTROPY_SAMPLE_BYTES", defaults.sample_bytes),
        period_lags: env_lags(&defaults.period_lags),
        min_match_fraction: env_f64("DARKROCK_PERIOD_MATCH_FRACTION", defaults.min_match_fraction),
    };
    config.validate().expect("valid entropy gate configuration");
    println!("# Byte representation benchmark\n");
    println!("Three 16 MiB deterministic corpora, 256 KiB chunks, zstd level 3, 4+2 RS after selection; release build, single pass. Gate: entropy threshold {:.2} bits/byte, one initial {}-byte sample, lag set {:?}, match cutoff {:.1}%. Timings are local wall time in ms. Metadata and receipt sizes are fixed-format estimates, not serialized files.\n", config.threshold_bits_per_byte, config.sample_bytes, config.period_lags, config.min_match_fraction * 100.0);
    println!("| Corpus | zstd/raw/skipped chunks | selected payload bytes | raw RS bytes | selected RS bytes | metadata+receipt bytes | raw RS encode ms | ungated select ms | gated select ms | selected RS encode ms | raw verify / selected decode+verify ms | selected one-share repair ms |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    run("pseudorandom", &random, &config);
    run("logs", &text, &config);
    run("mixed", &mixed, &config);
    pattern_checks(&config);
    gate_diagnostics(&config);
    println!("\nSelected total write time is select+verify plus selected RS encode; raw write time is raw RS encode. Selected stored bytes are selected RS bytes plus metadata+receipt bytes. Partial final stripes now use shares sized to ceil(payload bytes / 4); at most three data-padding bytes remain per final stripe. Repair times are reconstruction only. There is no live network, disk, encryption, or persistent manifest. A single initial sample can miss compressible data later in the chunk.\n");
    let _ = PARITY_SHARDS;
}
