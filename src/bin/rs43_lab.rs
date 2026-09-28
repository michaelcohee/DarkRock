//! Separate single-host 4+3 RS lab, using the existing 192 MiB script corpus.
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

const ROOT: &str = "target/sprawl-lab-43";
const SOURCE_ROOT: &str = "target/sprawl-lab-v2/source";
const FILES: usize = 384;
const FILE_BYTES: usize = 512 * 1024;
const DATA: usize = 4;
const PARITY: usize = 3;
const SHARES: usize = DATA + PARITY;
const NODES: usize = 18;
const MAGIC: &[u8; 8] = b"DR43LAB1";
const MARKER: &[u8] = b"darkrock-rs43-lab-v1\n";

#[derive(Clone)]
struct Record {
    id: u32,
    original_len: u32,
    share_len: u32,
    original_hash: [u8; 32],
    nodes: [u8; SHARES],
    share_hashes: [[u8; 32]; SHARES],
}

fn root() -> PathBuf { PathBuf::from(ROOT) }
fn source_path(id: u32) -> PathBuf { root().join("source").join(format!("script-{id:04}.sh")) }
fn source_origin(id: u32) -> PathBuf { PathBuf::from(SOURCE_ROOT).join(format!("script-{id:04}.sh")) }
fn node_path(node: u8) -> PathBuf {
    root().join(format!("zone-{}", node / 6)).join(format!("node-{node:02}"))
}
fn share_path(id: u32, share: usize, node: u8) -> PathBuf {
    node_path(node).join(format!("share-{id:04}-{share}.bin"))
}
fn hash(data: &[u8]) -> [u8; 32] { Sha256::digest(data).into() }

fn hmac_sha256(key: &[u8; 32], body: &[u8]) -> [u8; 32] {
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for i in 0..32 { inner_pad[i] ^= key[i]; outer_pad[i] ^= key[i]; }
    let mut inner = Sha256::new();
    inner.update(inner_pad); inner.update(body);
    let mut outer = Sha256::new();
    outer.update(outer_pad); outer.update(inner.finalize());
    outer.finalize().into()
}

fn ensure_lab() -> Result<(), String> {
    if fs::read(root().join(".lab-marker")).map_err(|e| e.to_string())? != MARKER {
        return Err("invalid 4+3 lab marker".into());
    }
    Ok(())
}

fn write_sync(path: &Path, data: &[u8], private: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let mut file = OpenOptions::new().write(true).create_new(true)
        .mode(if private { 0o600 } else { 0o644 }).open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(data).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(())
}

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn placement(id: u32) -> [u8; SHARES] {
    let mut pairs = [[0_u8; 2]; 3];
    for (zone, pair) in pairs.iter_mut().enumerate() {
        let x = mix((id as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (zone as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
            ^ 0x1234_5678_9abc_def0);
        let first = (x % 6) as u8;
        let mut second = ((x >> 32) % 5) as u8;
        if second >= first { second += 1; }
        *pair = [zone as u8 * 6 + first, zone as u8 * 6 + second];
    }
    let extra_zone = id as usize % 3;
    let x = mix((id as u64).wrapping_mul(0xd6e8_feb8_6659_fd93) ^ 0xa076_1d64_78bd_642f);
    let mut extra_local = (x % 6) as u8;
    while pairs[extra_zone].iter().any(|&node| node % 6 == extra_local) {
        extra_local = (extra_local + 1) % 6;
    }
    let mut nodes = [0_u8; SHARES];
    for share in 0..6 {
        let zone = (share / 2 + extra_zone) % 3;
        nodes[share] = pairs[zone][share % 2];
    }
    nodes[6] = extra_zone as u8 * 6 + extra_local;
    nodes
}

fn codec() -> Result<ReedSolomon, String> {
    ReedSolomon::new(DATA, PARITY).map_err(|e| e.to_string())
}

fn encode(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if bytes.is_empty() || bytes.len() > DATA * 256 * 1024 { return Err("invalid stripe length".into()); }
    let share_len = bytes.len().div_ceil(DATA);
    let mut shares = vec![vec![0_u8; share_len]; SHARES];
    for (dst, src) in shares[..DATA].iter_mut().zip(bytes.chunks(share_len)) {
        dst[..src.len()].copy_from_slice(src);
    }
    codec()?.encode(&mut shares).map_err(|e| e.to_string())?;
    Ok(shares)
}

fn serialize(records: &[Record]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for r in records {
        body.extend_from_slice(&r.id.to_le_bytes());
        body.extend_from_slice(&r.original_len.to_le_bytes());
        body.extend_from_slice(&r.share_len.to_le_bytes());
        body.extend_from_slice(&r.original_hash);
        body.extend_from_slice(&r.nodes);
        for h in &r.share_hashes { body.extend_from_slice(h); }
    }
    body
}

fn take<const N: usize>(data: &[u8], pos: &mut usize) -> Result<[u8; N], String> {
    let end = pos.checked_add(N).ok_or("manifest offset overflow")?;
    let bytes = data.get(*pos..end).ok_or("truncated manifest")?;
    *pos = end;
    bytes.try_into().map_err(|_| "invalid manifest field".into())
}

fn load_manifest() -> Result<Vec<Record>, String> {
    ensure_lab()?;
    let key: [u8; 32] = fs::read(root().join("manifest.key")).map_err(|e| e.to_string())?
        .try_into().map_err(|_| "invalid key length")?;
    let bytes = fs::read(root().join("manifest.bin")).map_err(|e| e.to_string())?;
    let body_len = bytes.len().checked_sub(32).ok_or("truncated MAC")?;
    let (body, tag) = bytes.split_at(body_len);
    let expected = hmac_sha256(&key, body);
    let mismatch = expected.iter().zip(tag).fold(0_u8, |acc, (a, b)| acc | (a ^ b));
    if mismatch != 0 { return Err("manifest authentication failed".into()); }
    let mut pos = 0;
    if &take::<8>(body, &mut pos)? != MAGIC { return Err("manifest version mismatch".into()); }
    if u32::from_le_bytes(take::<4>(body, &mut pos)?) as usize != FILES {
        return Err("manifest count mismatch".into());
    }
    let mut records = Vec::with_capacity(FILES);
    for expected_id in 0..FILES {
        let id = u32::from_le_bytes(take::<4>(body, &mut pos)?);
        let original_len = u32::from_le_bytes(take::<4>(body, &mut pos)?);
        let share_len = u32::from_le_bytes(take::<4>(body, &mut pos)?);
        let original_hash = take::<32>(body, &mut pos)?;
        let nodes = take::<SHARES>(body, &mut pos)?;
        let mut share_hashes = [[0_u8; 32]; SHARES];
        for h in &mut share_hashes { *h = take::<32>(body, &mut pos)?; }
        if id as usize != expected_id || original_len as usize != FILE_BYTES
            || share_len != original_len.div_ceil(DATA as u32) || nodes != placement(id) {
            return Err("manifest geometry or placement mismatch".into());
        }
        records.push(Record { id, original_len, share_len, original_hash, nodes, share_hashes });
    }
    if pos != body.len() { return Err("manifest trailing bytes".into()); }
    Ok(records)
}

fn init() -> Result<(), String> {
    if root().exists() { return Err(format!("{ROOT} exists; preserving prior run")); }
    let started = Instant::now();
    fs::create_dir_all(root()).map_err(|e| e.to_string())?;
    write_sync(&root().join(".lab-marker"), MARKER, false)?;
    let mut records = Vec::with_capacity(FILES);
    let mut source_bytes = 0_u64;
    let mut share_bytes = 0_u64;
    for id in 0..FILES as u32 {
        let original = source_origin(id);
        let local = source_path(id);
        fs::create_dir_all(local.parent().ok_or("invalid source path")?).map_err(|e| e.to_string())?;
        fs::hard_link(&original, &local).or_else(|_| fs::copy(&original, &local).map(|_| ()))
            .map_err(|e| format!("{}: {e}", original.display()))?;
        let bytes = fs::read(&local).map_err(|e| e.to_string())?;
        if bytes.len() != FILE_BYTES { return Err(format!("wrong source size: {id}")); }
        source_bytes += bytes.len() as u64;
        let shares = encode(&bytes)?;
        let nodes = placement(id);
        for (share, data) in shares.iter().enumerate() {
            write_sync(&share_path(id, share, nodes[share]), data, false)?;
            share_bytes += data.len() as u64;
        }
        records.push(Record {
            id, original_len: bytes.len() as u32, share_len: shares[0].len() as u32,
            original_hash: hash(&bytes), nodes,
            share_hashes: std::array::from_fn(|i| hash(&shares[i])),
        });
    }
    let mut key = [0_u8; 32];
    File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut key))
        .map_err(|e| format!("random key: {e}"))?;
    write_sync(&root().join("manifest.key"), &key, true)?;
    let mut manifest = serialize(&records);
    manifest.extend_from_slice(&hmac_sha256(&key, &manifest));
    write_sync(&root().join("manifest.bin"), &manifest, false)?;
    println!("init43: files={FILES} nodes={NODES} source_bytes={source_bytes} share_bytes={share_bytes} manifest_bytes={} elapsed_ms={}", manifest.len(), started.elapsed().as_millis());
    Ok(())
}

fn read_shares(r: &Record) -> Result<(Vec<Option<Vec<u8>>>, u64), String> {
    let mut bytes_read = 0_u64;
    let mut shares = Vec::with_capacity(SHARES);
    for share in 0..SHARES {
        match fs::read(share_path(r.id, share, r.nodes[share])) {
            Ok(bytes) => {
                bytes_read += bytes.len() as u64;
                if bytes.len() != r.share_len as usize || hash(&bytes) != r.share_hashes[share] {
                    return Err(format!("invalid share: file={} share={share}", r.id));
                }
                shares.push(Some(bytes));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => shares.push(None),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok((shares, bytes_read))
}

fn decode(mut shares: Vec<Option<Vec<u8>>>, r: &Record) -> Result<Vec<u8>, String> {
    codec()?.reconstruct_data(&mut shares).map_err(|e| e.to_string())?;
    let mut output = Vec::with_capacity(DATA * r.share_len as usize);
    for (share, expected) in shares.into_iter().zip(&r.share_hashes).take(DATA) {
        let data = share.ok_or("reconstructed data share missing")?;
        if data.len() != r.share_len as usize || hash(&data) != *expected {
            return Err("reconstructed share hash mismatch".into());
        }
        output.extend(data);
    }
    output.truncate(r.original_len as usize);
    if hash(&output) != r.original_hash { return Err("original hash mismatch".into()); }
    Ok(output)
}

fn audit_snapshot(records: &[Record]) -> Result<([usize; 8], u64), String> {
    let mut counts = [0_usize; 8];
    let mut bytes_read = 0_u64;
    for r in records {
        let (shares, read) = read_shares(r)?;
        bytes_read += read;
        counts[shares.iter().filter(|share| share.is_none()).count()] += 1;
    }
    Ok((counts, bytes_read))
}

fn audit() -> Result<(), String> {
    let records = load_manifest()?;
    let (counts, read) = audit_snapshot(&records)?;
    println!("audit43: files={} missing_0={} missing_1={} missing_2={} missing_3={} missing_4plus={} present_bytes_read={read}",
        records.len(), counts[0], counts[1], counts[2], counts[3], counts[4..].iter().sum::<usize>());
    Ok(())
}

fn verify() -> Result<(), String> {
    let started = Instant::now();
    let records = load_manifest()?;
    let mut share_bytes = 0_u64;
    for r in &records {
        let source = fs::read(source_path(r.id)).map_err(|e| e.to_string())?;
        if source.len() != r.original_len as usize || hash(&source) != r.original_hash {
            return Err(format!("source mismatch: {}", r.id));
        }
        let (mut shares, read) = read_shares(r)?;
        if shares.iter().any(Option::is_none) { return Err(format!("missing share: {}", r.id)); }
        share_bytes += read;
        for share in 0..PARITY { shares[share] = None; }
        if decode(shares, r)? != source { return Err(format!("restored file mismatch: {}", r.id)); }
    }
    println!("verify43: files={} source_bytes={} share_bytes={share_bytes} exact=true elapsed_ms={}",
        records.len(), FILES * FILE_BYTES, started.elapsed().as_millis());
    Ok(())
}

fn repair() -> Result<(), String> {
    let started = Instant::now();
    let records = load_manifest()?;
    let mut affected = 0;
    let mut scan_bytes = 0_u64;
    let mut recovery_bytes = 0_u64;
    let mut written = 0_u64;
    for r in &records {
        let (mut shares, read) = read_shares(r)?;
        scan_bytes += read;
        let missing: Vec<_> = (0..SHARES).filter(|&i| shares[i].is_none()).collect();
        if missing.is_empty() { continue; }
        if missing.len() > PARITY { return Err(format!("file {} lost >3 shares", r.id)); }
        for i in 0..SHARES {
            if shares[i].is_some() && shares.iter().filter(|s| s.is_some()).count() > DATA {
                shares[i] = None;
            }
        }
        recovery_bytes += (DATA * r.share_len as usize) as u64;
        let original = decode(shares, r)?;
        let rebuilt = encode(&original)?;
        for share in missing {
            if hash(&rebuilt[share]) != r.share_hashes[share] {
                return Err("rebuilt share hash mismatch".into());
            }
            write_sync(&share_path(r.id, share, r.nodes[share]), &rebuilt[share], false)?;
            written += rebuilt[share].len() as u64;
        }
        affected += 1;
    }
    let (counts, post_audit_bytes) = audit_snapshot(&records)?;
    let residual: usize = counts[1..].iter().sum();
    println!("repair43: affected_files={affected} disk_scan_bytes={scan_bytes} recovery_bytes_used={recovery_bytes} rebuilt_bytes={written} post_audit_bytes={post_audit_bytes} residual_files={residual} elapsed_ms={}", started.elapsed().as_millis());
    if residual > 0 { return Err(format!("repair incomplete: {residual} files")); }
    Ok(())
}

fn offline_node(node: u8) -> Result<(), String> {
    load_manifest()?;
    if node as usize >= NODES { return Err("node must be 0..17".into()); }
    let parked = root().join("offline").join(format!("node-{node:02}"));
    fs::create_dir_all(parked.parent().ok_or("invalid parked path")?).map_err(|e| e.to_string())?;
    if parked.exists() { return Err("node already parked".into()); }
    fs::rename(node_path(node), &parked).map_err(|e| e.to_string())?;
    println!("offline43: node={node} zone={}", node / 6);
    Ok(())
}

fn online_node(node: u8) -> Result<(), String> {
    load_manifest()?;
    if node as usize >= NODES { return Err("node must be 0..17".into()); }
    let parked = root().join("offline").join(format!("node-{node:02}"));
    let active = node_path(node);
    if active.exists() {
        for entry in fs::read_dir(&parked).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let destination = active.join(entry.file_name());
            if destination.exists() {
                if fs::read(entry.path()).map_err(|e| e.to_string())?
                    != fs::read(&destination).map_err(|e| e.to_string())? {
                    return Err("parked and active share differ".into());
                }
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
            } else {
                fs::rename(entry.path(), destination).map_err(|e| e.to_string())?;
            }
        }
        fs::remove_dir(&parked).map_err(|e| e.to_string())?;
    } else {
        fs::rename(&parked, &active).map_err(|e| e.to_string())?;
    }
    println!("online43: node={node} zone={}", node / 6);
    Ok(())
}

fn show_placement(id: u32) -> Result<(), String> {
    let records = load_manifest()?;
    let r = records.get(id as usize).ok_or("file id out of range")?;
    println!("placement43: file={} nodes={:?} zones={:?}", id, r.nodes,
        r.nodes.map(|node| node / 6));
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd] if cmd == "init" => init(),
        [_, cmd] if cmd == "verify" => verify(),
        [_, cmd] if cmd == "audit" => audit(),
        [_, cmd] if cmd == "repair" => repair(),
        [_, cmd, value] if cmd == "offline-node" => offline_node(value.parse().map_err(|_| "invalid node")?),
        [_, cmd, value] if cmd == "online-node" => online_node(value.parse().map_err(|_| "invalid node")?),
        [_, cmd, value] if cmd == "placement" => show_placement(value.parse().map_err(|_| "invalid id")?),
        _ => Err("usage: rs43_lab init|verify|audit|repair|placement FILE_ID|offline-node NODE|online-node NODE".into()),
    }
}

fn main() {
    if let Err(e) = run() { eprintln!("rs43_lab: {e}"); std::process::exit(1); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_has_seven_distinct_nodes_and_three_two_two_zones() {
        for id in 0..FILES as u32 {
            let nodes = placement(id);
            let mut unique = nodes.to_vec();
            unique.sort_unstable(); unique.dedup();
            assert_eq!(unique.len(), SHARES);
            let mut zones = [0_usize; 3];
            for node in nodes { zones[(node / 6) as usize] += 1; }
            zones.sort_unstable();
            assert_eq!(zones, [2, 2, 3]);
        }
    }

    #[test]
    fn every_three_erasure_pattern_restores_short_tail() {
        let data: Vec<u8> = (0..10_003).map(|i| (i * 41) as u8).collect();
        let shards = encode(&data).unwrap();
        let record = Record {
            id: 0, original_len: data.len() as u32, share_len: shards[0].len() as u32,
            original_hash: hash(&data), nodes: placement(0),
            share_hashes: std::array::from_fn(|i| hash(&shards[i])),
        };
        for a in 0..SHARES {
            for b in a + 1..SHARES {
                for c in b + 1..SHARES {
                    let mut available: Vec<_> = shards.iter().cloned().map(Some).collect();
                    available[a] = None; available[b] = None; available[c] = None;
                    assert_eq!(decode(available, &record).unwrap(), data);
                }
            }
        }
    }
}
