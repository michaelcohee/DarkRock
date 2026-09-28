//! Isolated parity/recovery timing on identical 16 MiB input, excluding SHA-256.
use darkrock_storage::{polynomial_code, storage::{BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS}};
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::{Duration, Instant}};

const STRIPES: usize = 16;
const TRIALS: usize = 5;

fn median(mut v: Vec<Duration>) -> f64 {
    v.sort();
    v[v.len() / 2].as_secs_f64() * 1000.0
}
fn input() -> Vec<Vec<Vec<u8>>> {
    let mut x = 7u64;
    (0..STRIPES).map(|_| {
        let mut shards = vec![vec![0u8; BLOCK_SIZE]; DATA_SHARDS + PARITY_SHARDS];
        for shard in &mut shards[..DATA_SHARDS] {
            for byte in shard {
                x ^= x << 13; x ^= x >> 7; x ^= x << 17;
                *byte = x as u8;
            }
        }
        shards
    }).collect()
}
fn bench_encode(name: &str, base: &[Vec<Vec<u8>>], rs: Option<&ReedSolomon>) -> Vec<Vec<Vec<u8>>> {
    let mut times = Vec::new();
    let mut last = Vec::new();
    for _ in 0..TRIALS {
        let mut stripes = base.to_vec(); // preparation excluded
        let start = Instant::now();
        for shards in &mut stripes {
            match rs {
                Some(codec) => codec.encode(&mut *shards).unwrap(),
                None => polynomial_code::encode_parity_in_place(shards).unwrap(),
            }
            black_box(&shards[5]);
        }
        times.push(start.elapsed());
        last = stripes;
    }
    println!("{name},encode,{:.3}", median(times));
    last
}
fn bench_repair(name: &str, encoded: &[Vec<Vec<u8>>], rs: Option<&ReedSolomon>, missing: &[usize]) {
    let mut times = Vec::new();
    for _ in 0..TRIALS {
        let mut stripes: Vec<Vec<Option<Vec<u8>>>> = encoded.iter().map(|set| set.iter().cloned().map(Some).collect()).collect();
        for set in &mut stripes { for &i in missing { set[i] = None; } }
        let start = Instant::now();
        for shares in &mut stripes {
            match rs {
                Some(codec) => codec.reconstruct_data(&mut *shares).unwrap(),
                None => {
                    let recovered = polynomial_code::reconstruct_data_unchecked(std::mem::take(shares)).unwrap();
                    black_box(recovered);
                }
            }
        }
        times.push(start.elapsed());
    }
    println!("{name},reconstruct_{},{:.3}", missing.len(), median(times));
}
fn bench_hash(encoded: &[Vec<Vec<u8>>]) {
    let mut times = Vec::new();
    for _ in 0..TRIALS {
        let start = Instant::now();
        for shards in encoded {
            for shard in shards { black_box(Sha256::digest(shard)); }
        }
        times.push(start.elapsed());
    }
    println!("sha256,all_6_shares_per_stripe,{:.3}",median(times));
    let mut times = Vec::new();
    for _ in 0..TRIALS {
        let start = Instant::now();
        for shards in encoded {
            for shard in &shards[..DATA_SHARDS] { black_box(Sha256::digest(shard)); }
        }
        times.push(start.elapsed());
    }
    println!("sha256,original_4_data_shares_per_stripe,{:.3}",median(times));
}
fn main() {
    println!("arch={},rs_simd_feature={},input_mib=16,stripes={},share_kib=256,trials={}",std::env::consts::ARCH,cfg!(feature="rs-simd"),STRIPES,TRIALS);
    println!("implementation,operation,median_ms");
    let base=input();
    let rs=ReedSolomon::new(DATA_SHARDS,PARITY_SHARDS).unwrap();
    let reference=bench_encode("crate_rs",&base,Some(&rs));
    let custom=bench_encode("custom_gf256",&base,None);
    assert!(reference.iter().zip(&base).all(|(a,b)| a[..4] == b[..4]));
    bench_hash(&reference);
    for missing in [&[0usize][..],&[0usize,1usize][..]] {
        bench_repair("crate_rs",&reference,Some(&rs),missing);
        bench_repair("custom_gf256",&custom,None,missing);
    }
}
