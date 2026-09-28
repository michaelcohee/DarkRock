use darkrock_storage::left_hand::LeftHandStore;
use std::time::Instant;

fn main() {
    let mut matrix = vec![vec![2], vec![1, 1], vec![10, -10, 2]];
    for i in 0..4_997_i64 {
        let left = i - 2_498;
        matrix.push(vec![left, 2 - left]);
    }
    let mut store = LeftHandStore::new(2);
    let start = Instant::now();
    store.ingest(matrix);
    let ingest_us = start.elapsed().as_secs_f64() * 1_000_000.0;
    let count = store.verified_count();
    let full_bytes = store.estimated_payload_bytes();
    let full_alloc = store.allocated_bytes_estimate();
    let start = Instant::now();
    let receipt = store.isolate_primary().expect("verified primary");
    let isolate_us = start.elapsed().as_secs_f64() * 1_000_000.0;
    println!("# Left-hand pair isolation benchmark\n");
    println!("5,000 deterministic entries targeting 2; each generated pair [a, 2-a] is valid. Release build; single run.\n");
    println!("| Metric | Full matrix | Primary store after isolation |");
    println!("|---|---:|---:|");
    println!("| Verified entries | {} | {} |", count, store.verified_count());
    println!("| Estimated compact integer payload | {} bytes | {} byte |", full_bytes, store.estimated_payload_bytes());
    println!("| Approximate Rust allocation, excluding allocator metadata | {} bytes | {} bytes |", full_alloc, store.allocated_bytes_estimate());
    println!("\nPrimary: {:?}; rejected: {}; dropped: {} entries, {} estimated payload bytes.", receipt.kept_primary, receipt.rejected_count, receipt.dropped_count, receipt.dropped_payload_bytes);
    println!("Receipt still held: ~{} allocated bytes. Ingest: {:.1} µs; isolate: {:.1} µs.", receipt.allocated_bytes_estimate(), ingest_us, isolate_us);
    println!("\nThe 1-byte figure is an unsigned-magnitude encoding estimate for [2], not process memory. The full audit receipt retains the dropped entries, so total live memory remains larger until the receipt is archived or released. This rule does not erasure-code or reconstruct file bytes.");
}
