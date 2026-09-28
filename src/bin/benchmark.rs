use darkrock_storage::storage::{BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS, encode_stripe, reconstruct};
use darkrock_storage::polynomial_code;
use std::process::Command;
use std::time::Instant;

const MIB: f64 = 1024.0 * 1024.0;
const STRIPES: usize = 16; // 16 MiB logical, one MiB per stripe

// Six deliberately different prospective storage nodes. Latency and throughput
// are scenario assumptions, not measurements or observations of Storj.
const NODES: [(&str, f64, f64); 6] = [
    ("us-east", 25.0, 50.0), ("us-west", 75.0, 25.0),
    ("eu-west", 95.0, 20.0), ("eu-central", 110.0, 16.0),
    ("ap-south", 220.0, 10.0), ("ap-east", 170.0, 12.0),
];

fn sysctl(name: &str) -> String {
    Command::new("sysctl").args(["-n", name]).output().ok()
        .filter(|x| x.status.success())
        .map(|x| String::from_utf8_lossy(&x.stdout).trim().to_string())
        .unwrap_or_else(|| "unavailable".into())
}

fn sample_input(bytes: usize) -> Vec<u8> {
    let mut x = 0x1234_5678_9abc_def0_u64;
    (0..bytes).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect()
}

fn network_seconds(available: &[usize], inbound_mib_s: f64) -> Option<f64> {
    if available.len() < DATA_SHARDS { return None; }
    let mut helper_times: Vec<f64> = available.iter().map(|&i| {
        let (_, rtt_ms, mib_s) = NODES[i];
        rtt_ms / 1000.0 + (BLOCK_SIZE as f64 / MIB) / mib_s
    }).collect();
    helper_times.sort_by(f64::total_cmp);
    // Helpers send in parallel, but the replacement's inbound link is shared.
    Some(helper_times[DATA_SHARDS - 1].max((DATA_SHARDS * BLOCK_SIZE) as f64 / MIB / inbound_mib_s))
}

fn main() {
    let stripe_len = DATA_SHARDS * BLOCK_SIZE;
    let payload = sample_input(STRIPES * stripe_len);
    let raw_start = Instant::now();
    let raw_copy = payload.clone();
    let raw_ms = raw_start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(raw_copy, payload);

    let start = Instant::now();
    let stripes: Vec<_> = payload.chunks(stripe_len).map(|c| encode_stripe(c).unwrap()).collect();
    let encode_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut decoded = Vec::with_capacity(payload.len());
    let start = Instant::now();
    for stripe in &stripes {
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0] = None;
        decoded.extend(reconstruct(shares, &stripe.manifest).unwrap());
    }
    let repair_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(decoded, payload);

    let start = Instant::now();
    let poly_stripes: Vec<_> = payload.chunks(stripe_len).map(|c| polynomial_code::encode_stripe(c).unwrap()).collect();
    let poly_encode_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut poly_decoded = Vec::with_capacity(payload.len());
    let start = Instant::now();
    for stripe in &poly_stripes {
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0] = None;
        poly_decoded.extend(polynomial_code::reconstruct(shares, stripe.original_len, &stripe.shard_hashes).unwrap());
    }
    let poly_repair_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(poly_decoded, payload);
    let start = Instant::now();
    for stripe in &stripes {
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0] = None; shares[1] = None;
        assert_eq!(reconstruct(shares, &stripe.manifest).unwrap().len(), stripe_len);
    }
    let rs_two_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    for stripe in &poly_stripes {
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0] = None; shares[1] = None;
        assert_eq!(polynomial_code::reconstruct(shares, stripe.original_len, &stripe.shard_hashes).unwrap().len(), stripe_len);
    }
    let poly_two_ms = start.elapsed().as_secs_f64() * 1000.0;

    let logical = payload.len();
    let physical: usize = stripes.iter().flat_map(|s| &s.shards).map(Vec::len).sum();
    let helper_bytes = STRIPES * DATA_SHARDS * BLOCK_SIZE;
    let replacement_bytes = STRIPES * BLOCK_SIZE;
    let per_stripe_cpu_s = repair_ms / 1000.0 / STRIPES as f64;
    println!("# DarkRock storage benchmark\n");
    println!("Host: {} ({}), {} physical / {} logical CPUs, {} bytes RAM", sysctl("machdep.cpu.brand_string"), std::env::consts::ARCH, sysctl("hw.physicalcpu"), sysctl("hw.logicalcpu"), sysctl("hw.memsize"));
    println!("Build: {} | input: {} MiB pseudorandom | stripe: 4 × 256 KiB data + 2 × 256 KiB parity | iterations: one pass over {} stripes", if cfg!(debug_assertions) { "debug" } else { "release" }, logical / 1024 / 1024, STRIPES);
    println!("\n| Mode | Local operation | Measured local wall time | Encoded bytes | Calculated one-share repair traffic |");
    println!("|---|---|---:|---:|---|");
    println!("| Raw, no formula | in-memory copy | {:.2} ms | {} | impossible without an independent copy |", raw_ms, logical);
    println!("| RS 4+2 | encode | {:.2} ms | {} | any 4 of 6 shares |", encode_ms, physical);
    println!("| RS 4+2 | reconstruct data with share 0 missing | {:.2} ms | {} | {} helper bytes read; {} replacement bytes written |", repair_ms, physical, helper_bytes, replacement_bytes);
    println!("| GF256 polynomial parity 4+2 | encode | {:.2} ms | {} | any 4 of 6 shares |", poly_encode_ms, physical);
    println!("| GF256 polynomial parity 4+2 | reconstruct data with share 0 missing | {:.2} ms | {} | {} helper bytes read; {} replacement bytes written |", poly_repair_ms, physical, helper_bytes, replacement_bytes);
    println!("\nTwo missing data shares (0 and 1): RS {:.2} ms; polynomial parity {:.2} ms; calculated traffic for both: {} helper bytes read and {} replacement bytes written.\n", rs_two_ms, poly_two_ms, helper_bytes, 2 * replacement_bytes);
    println!("Storage overhead: {:.2}× for both; RS encode throughput: {:.1} MiB/s; polynomial parity encode throughput: {:.1} MiB/s.", physical as f64 / logical as f64, logical as f64 / MIB / (encode_ms / 1000.0), logical as f64 / MIB / (poly_encode_ms / 1000.0));
    println!("The custom row is a simple systematic GF(2^8) MDS construction in the Reed-Solomon family, separate from symbolic polynomial canonicalization. It does not claim a novel repair-bandwidth advantage. Timings include in-memory share setup and hashing; no disk or network bytes were actually transferred. This is one run, not a stable performance estimate.\n");
    println!("\n## Hypothetical node sprawl (modeled, not network-tested)\n");
    println!("Six shares are placed on six region-labelled nodes with assumed RTT and per-node bandwidth. A replacement has a 25 MiB/s inbound cap. Four fastest available helpers send full shares in parallel. Each stripe waits for its slowest selected helper; stripes rebuild sequentially. The measured local reconstruction time is then added.\n");
    println!("| Offline share nodes | Available | Recoverable | RS rebuild | Polynomial parity rebuild |");
    println!("|---|---:|---|---:|---:|");
    for offline in [vec![0], vec![0, 1], vec![0, 1, 2]] {
        let available: Vec<_> = (0..NODES.len()).filter(|i| !offline.contains(i)).collect();
        let duration = network_seconds(&available, 25.0)
            .map(|s| format!("{:.2} s", STRIPES as f64 * (s + per_stripe_cpu_s)))
            .unwrap_or_else(|| "unrecoverable".into());
        let poly_duration = network_seconds(&available, 25.0)
            .map(|s| format!("{:.2} s", STRIPES as f64 * s + poly_repair_ms / 1000.0))
            .unwrap_or_else(|| "unrecoverable".into());
        println!("| {} | {} | {} | {} | {} |", offline.iter().map(|i| NODES[*i].0).collect::<Vec<_>>().join(", "), available.len(), if available.len() >= DATA_SHARDS { "yes" } else { "no" }, duration, poly_duration);
    }
    println!("\nThe modeled times exclude discovery, authentication, queueing, disk I/O, retries, contention, encryption, compression, and actual node churn. They are scenario estimates, not Storj performance claims.");
    let _ = PARITY_SHARDS;
}
