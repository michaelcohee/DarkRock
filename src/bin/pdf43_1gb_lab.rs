//! Disk-backed 4+3 RS test over copied real PDFs. The source directory is never written.
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const ROOT: &str = "target/pdf-43-1gb";
const FILES: usize = 220;
const DATA: usize = 4;
const PARITY: usize = 3;
const SHARDS: usize = DATA + PARITY;
const STRIPE_BYTES: usize = 4 * 256 * 1024;
const MARKER: &[u8] = b"darkrock-pdf-rs43-1gb-v1\n";
const MAGIC: &[u8; 8] = b"DRPDF431";

#[derive(Clone)]
struct Record {
    file: u32,
    stripe: u32,
    len: u32,
    hash: [u8; 32],
    nodes: [u8; SHARDS],
    shard_hashes: [[u8; 32]; SHARDS],
}

fn root() -> PathBuf { PathBuf::from(ROOT) }
fn source(file: u32) -> PathBuf { root().join("source").join(format!("file-{file:04}.pdf")) }
fn shard_path(r: &Record, shard: usize) -> PathBuf {
    root().join(format!("zone-{}", r.nodes[shard] / 6))
        .join(format!("node-{:02}", r.nodes[shard]))
        .join(format!("f{:04}-s{:05}-h{shard}.bin", r.file, r.stripe))
}
fn hash(data: &[u8]) -> [u8; 32] { Sha256::digest(data).into() }
fn codec() -> Result<ReedSolomon, String> { ReedSolomon::new(DATA, PARITY).map_err(|e| e.to_string()) }
fn check_marker() -> Result<(), String> {
    if fs::read(root().join(".lab-marker")).map_err(|e| e.to_string())? != MARKER {
        return Err("wrong PDF lab marker".into());
    }
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
fn placement(file: u32, stripe: u32) -> [u8; SHARDS] {
    let id = ((file as u64) << 32) | stripe as u64;
    let mut pairs = [[0_u8; 2]; 3];
    for (zone, pair) in pairs.iter_mut().enumerate() {
        let x = mix(id.wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (zone as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
            ^ 0x1234_5678_9abc_def0);
        let first = (x % 6) as u8;
        let mut second = ((x >> 32) % 5) as u8;
        if second >= first { second += 1; }
        *pair = [zone as u8 * 6 + first, zone as u8 * 6 + second];
    }
    let extra_zone = (file as usize + stripe as usize) % 3;
    let mut extra_local = (mix(id ^ 0xa076_1d64_78bd_642f) % 6) as u8;
    while pairs[extra_zone].iter().any(|&n| n % 6 == extra_local) { extra_local = (extra_local + 1) % 6; }
    let mut nodes = [0_u8; SHARDS];
    for shard in 0..6 {
        let zone = (shard / 2 + extra_zone) % 3;
        nodes[shard] = pairs[zone][shard % 2];
    }
    nodes[6] = extra_zone as u8 * 6 + extra_local;
    nodes
}
fn encode(bytes: &[u8], rs: &ReedSolomon) -> Result<Vec<Vec<u8>>, String> {
    if bytes.is_empty() || bytes.len() > STRIPE_BYTES { return Err("invalid stripe length".into()); }
    let share_len = bytes.len().div_ceil(DATA);
    let mut shards = vec![vec![0_u8; share_len]; SHARDS];
    for (dst, src) in shards[..DATA].iter_mut().zip(bytes.chunks(share_len)) {
        dst[..src.len()].copy_from_slice(src);
    }
    rs.encode(&mut shards).map_err(|e| e.to_string())?;
    Ok(shards)
}
fn manifest_bytes(records: &[Record]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(12 + records.len() * 275);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for r in records {
        bytes.extend_from_slice(&r.file.to_le_bytes());
        bytes.extend_from_slice(&r.stripe.to_le_bytes());
        bytes.extend_from_slice(&r.len.to_le_bytes());
        bytes.extend_from_slice(&r.hash);
        bytes.extend_from_slice(&r.nodes);
        for h in &r.shard_hashes { bytes.extend_from_slice(h); }
    }
    let checksum = hash(&bytes);
    bytes.extend_from_slice(&checksum);
    bytes
}
fn take<const N: usize>(bytes: &[u8], pos: &mut usize) -> Result<[u8; N], String> {
    let end = pos.checked_add(N).ok_or("manifest overflow")?;
    let out = bytes.get(*pos..end).ok_or("truncated manifest")?;
    *pos = end;
    out.try_into().map_err(|_| "bad manifest field".into())
}
fn load() -> Result<Vec<Record>, String> {
    check_marker()?;
    let bytes = fs::read(root().join("manifest.bin")).map_err(|e| e.to_string())?;
    let end = bytes.len().checked_sub(32).ok_or("truncated manifest checksum")?;
    if hash(&bytes[..end]) != bytes[end..] { return Err("manifest checksum mismatch".into()); }
    let mut pos = 0;
    if &take::<8>(&bytes[..end], &mut pos)? != MAGIC { return Err("manifest magic mismatch".into()); }
    let count = u32::from_le_bytes(take::<4>(&bytes[..end], &mut pos)?) as usize;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let file = u32::from_le_bytes(take::<4>(&bytes[..end], &mut pos)?);
        let stripe = u32::from_le_bytes(take::<4>(&bytes[..end], &mut pos)?);
        let len = u32::from_le_bytes(take::<4>(&bytes[..end], &mut pos)?);
        let original_hash = take::<32>(&bytes[..end], &mut pos)?;
        let nodes = take::<SHARDS>(&bytes[..end], &mut pos)?;
        let mut shard_hashes = [[0_u8; 32]; SHARDS];
        for h in &mut shard_hashes { *h = take::<32>(&bytes[..end], &mut pos)?; }
        if file as usize >= FILES || len == 0 || len as usize > STRIPE_BYTES || nodes != placement(file, stripe) {
            return Err("invalid manifest geometry".into());
        }
        records.push(Record { file, stripe, len, hash: original_hash, nodes, shard_hashes });
    }
    if pos != end { return Err("manifest trailing bytes".into()); }
    Ok(records)
}
fn read_stripe(r: &Record) -> Result<Vec<u8>, String> {
    let mut f = File::open(source(r.file)).map_err(|e| e.to_string())?;
    use std::io::{Seek, SeekFrom};
    f.seek(SeekFrom::Start(r.stripe as u64 * STRIPE_BYTES as u64)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0_u8; r.len as usize];
    f.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    if hash(&bytes) != r.hash { return Err(format!("source copy changed: file {} stripe {}", r.file, r.stripe)); }
    Ok(bytes)
}
fn read_shards(r: &Record) -> Result<(Vec<Option<Vec<u8>>>, u64), String> {
    let mut out = Vec::with_capacity(SHARDS);
    let mut read = 0;
    for shard in 0..SHARDS {
        match fs::read(shard_path(r, shard)) {
            Ok(bytes) => {
                read += bytes.len() as u64;
                if bytes.len() != (r.len as usize).div_ceil(DATA) || hash(&bytes) != r.shard_hashes[shard] {
                    return Err(format!("invalid shard: file {} stripe {} shard {shard}", r.file, r.stripe));
                }
                out.push(Some(bytes));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => out.push(None),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok((out, read))
}
fn decode(mut shards: Vec<Option<Vec<u8>>>, r: &Record, rs: &ReedSolomon) -> Result<Vec<u8>, String> {
    rs.reconstruct_data(&mut shards).map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity((r.len as usize).div_ceil(DATA) * DATA);
    for i in 0..DATA {
        let shard = shards[i].as_ref().ok_or("missing reconstructed data")?;
        if hash(shard) != r.shard_hashes[i] { return Err("reconstructed shard hash mismatch".into()); }
        bytes.extend_from_slice(shard);
    }
    bytes.truncate(r.len as usize);
    if hash(&bytes) != r.hash { return Err("reconstructed original hash mismatch".into()); }
    Ok(bytes)
}
fn init() -> Result<(), String> {
    check_marker()?;
    if root().join("manifest.bin").exists() { return Err("manifest exists; preserving prior run".into()); }
    let start = Instant::now();
    let rs = codec()?;
    let mut records = Vec::new();
    let (mut source_bytes, mut shard_bytes, mut fixed_bytes, mut tails) = (0_u64, 0_u64, 0_u64, 0_usize);
    let mut buffer = vec![0_u8; STRIPE_BYTES];
    for file in 0..FILES as u32 {
        let mut input = File::open(source(file)).map_err(|e| format!("source copy {file}: {e}"))?;
        let mut stripe = 0_u32;
        loop {
            let mut n = 0;
            while n < STRIPE_BYTES {
                let got = input.read(&mut buffer[n..]).map_err(|e| e.to_string())?;
                if got == 0 { break; }
                n += got;
            }
            if n == 0 { break; }
            if n < STRIPE_BYTES { tails += 1; }
            let bytes = &buffer[..n];
            let shards = encode(bytes, &rs)?;
            let nodes = placement(file, stripe);
            let record = Record { file, stripe, len: n as u32, hash: hash(bytes), nodes,
                shard_hashes: std::array::from_fn(|i| hash(&shards[i])) };
            for (i, shard) in shards.iter().enumerate() {
                write_new(&shard_path(&record, i), shard)?;
                shard_bytes += shard.len() as u64;
            }
            source_bytes += n as u64;
            fixed_bytes += (SHARDS * 256 * 1024) as u64;
            records.push(record);
            stripe += 1;
            if n < STRIPE_BYTES { break; }
        }
    }
    let manifest = manifest_bytes(&records);
    write_new(&root().join("manifest.bin"), &manifest)?;
    println!("init: files={FILES} stripes={} tails={tails} source_bytes={source_bytes} variable_43_shard_bytes={shard_bytes} fixed_43_shard_bytes={fixed_bytes} padding_saved={} manifest_bytes={} elapsed_ms={}", records.len(), fixed_bytes - shard_bytes, manifest.len(), start.elapsed().as_millis());
    Ok(())
}
fn verify() -> Result<(), String> {
    let start = Instant::now();
    let records = load()?;
    let rs = codec()?;
    let (mut source_bytes, mut read) = (0_u64, 0_u64);
    for r in &records {
        let original = read_stripe(r)?;
        let (mut shards, bytes) = read_shards(r)?;
        if shards.iter().any(Option::is_none) { return Err("missing share during verify".into()); }
        read += bytes;
        for i in [0, 1, 2] { shards[i] = None; }
        if decode(shards, r, &rs)? != original { return Err("three-loss decode differs".into()); }
        source_bytes += original.len() as u64;
    }
    println!("verify: files={FILES} stripes={} source_bytes={source_bytes} share_bytes_read={read} three_loss_exact=true elapsed_ms={}", records.len(), start.elapsed().as_millis());
    Ok(())
}
fn lose_nodes(nodes: &[u8]) -> Result<(), String> {
    let records = load()?;
    let mut bytes = 0_u64;
    let mut count = 0_usize;
    for r in &records {
        for i in 0..SHARDS {
            if nodes.contains(&r.nodes[i]) {
                let path = shard_path(r, i);
                bytes += fs::metadata(&path).map_err(|e| e.to_string())?.len();
                fs::remove_file(path).map_err(|e| e.to_string())?;
                count += 1;
            }
        }
    }
    println!("lose_nodes: nodes={nodes:?} deleted_shares={count} deleted_bytes={bytes}");
    Ok(())
}
fn repair() -> Result<(), String> {
    let start = Instant::now();
    let records = load()?;
    let rs = codec()?;
    let (mut affected, mut scan, mut used, mut written) = (0_usize, 0_u64, 0_u64, 0_u64);
    for r in &records {
        let (mut shards, read) = read_shards(r)?;
        scan += read;
        let missing: Vec<_> = (0..SHARDS).filter(|&i| shards[i].is_none()).collect();
        if missing.is_empty() { continue; }
        if missing.len() > PARITY { return Err(format!("stripe below four survivors: file {} stripe {}", r.file, r.stripe)); }
        used += (DATA * (r.len as usize).div_ceil(DATA)) as u64;
        for i in 0..SHARDS {
            if shards[i].is_some() && shards.iter().filter(|s| s.is_some()).count() > DATA { shards[i] = None; }
        }
        let restored = decode(shards, r, &rs)?;
        if restored != read_stripe(r)? { return Err("repaired source mismatch".into()); }
        let rebuilt = encode(&restored, &rs)?;
        for i in missing {
            if hash(&rebuilt[i]) != r.shard_hashes[i] { return Err("rebuilt shard hash mismatch".into()); }
            write_new(&shard_path(r, i), &rebuilt[i])?;
            written += rebuilt[i].len() as u64;
        }
        affected += 1;
    }
    let mut audit_read = 0_u64;
    let mut residual = 0_usize;
    for r in &records {
        let (shards, read) = read_shards(r)?;
        audit_read += read;
        if shards.iter().any(Option::is_none) { residual += 1; }
    }
    println!("repair: affected_stripes={affected} scan_bytes_read={scan} reconstruction_bytes_used={used} rebuilt_bytes_written={written} post_audit_bytes_read={audit_read} residual_stripes={residual} elapsed_ms={}", start.elapsed().as_millis());
    if residual > 0 { return Err("repair incomplete".into()); }
    Ok(())
}
fn spectrum() -> Result<(), String> {
    let records = load()?;
    for losses in [3_u32, 4] {
        let (mut sets, mut bad_sets, mut worst) = (0_u32, 0_u32, 0_usize);
        for mask in 1_u32..(1 << 18) {
            if mask.count_ones() != losses { continue; }
            sets += 1;
            let bad = records.iter().filter(|r| r.nodes.iter().filter(|&&n| mask & (1 << n) != 0).count() > PARITY).count();
            if bad > 0 { bad_sets += 1; }
            worst = worst.max(bad);
        }
        println!("spectrum: failed_nodes={losses} node_sets={sets} sets_with_unrecoverable_stripe={bad_sets} worst_unrecoverable_stripes={worst}");
    }
    Ok(())
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd] if cmd == "init" => init(),
        [_, cmd] if cmd == "verify" => verify(),
        [_, cmd] if cmd == "repair" => repair(),
        [_, cmd] if cmd == "spectrum" => spectrum(),
        [_, cmd, a, b, c] if cmd == "lose-nodes" => {
            let nodes = [a, b, c].map(|x| x.parse::<u8>().map_err(|_| "invalid node"));
            let nodes = nodes.into_iter().collect::<Result<Vec<_>, _>>()?;
            if nodes.iter().any(|&x| x >= 18) || nodes[0] == nodes[1] || nodes[0] == nodes[2] || nodes[1] == nodes[2] {
                return Err("need three distinct nodes in 0..17".into());
            }
            lose_nodes(&nodes)
        }
        _ => Err("usage: pdf43_1gb_lab init|verify|lose-nodes A B C|repair|spectrum".into()),
    }
}
fn main() { if let Err(e) = run() { eprintln!("error: {e}"); std::process::exit(1); } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_uses_seven_distinct_nodes_and_two_or_three_per_zone() {
        for file in 0..FILES as u32 {
            for stripe in 0..10 {
                let nodes = placement(file, stripe);
                let mut unique = nodes.to_vec(); unique.sort(); unique.dedup();
                assert_eq!(unique.len(), 7);
                let mut counts = [0; 3];
                for node in nodes { counts[(node / 6) as usize] += 1; }
                counts.sort(); assert_eq!(counts, [2, 2, 3]);
            }
        }
    }
}
