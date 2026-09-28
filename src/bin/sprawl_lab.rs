//! Disk-backed, single-host 18-node recovery exercise. No network or blockchain.
use darkrock_storage::storage::{DATA_SHARDS, PARITY_SHARDS, encode_stripe, reconstruct};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;

const ROOT: &str = "target/sprawl-lab-v2";
const FILES: usize = 384;
const FILE_BYTES: usize = 512 * 1024;
const NODES: usize = 18;
const OWNERS: usize = 64;
const MAGIC: &[u8; 8] = b"DRSPRL02";
const MARKER: &[u8] = b"darkrock-sprawl-lab-v2\n";

#[derive(Clone)]
struct Record {
    id: u32,
    original_len: u32,
    share_len: u32,
    original_hash: [u8; 32],
    nodes: [u8; 6],
    share_hashes: [[u8; 32]; 6],
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

fn root() -> PathBuf { PathBuf::from(ROOT) }
fn source_path(id: u32) -> PathBuf { root().join("source").join(format!("script-{id:04}.sh")) }
fn node_path(node: u8) -> PathBuf {
    root().join(format!("zone-{}", node / 6)).join(format!("node-{node:02}"))
}
fn share_path(id: u32, shard: usize, node: u8) -> PathBuf {
    node_path(node).join(format!("share-{id:04}-{shard}.bin"))
}

fn ensure_lab() -> Result<(), String> {
    let marker = fs::read(root().join(".lab-marker")).map_err(|e| e.to_string())?;
    if marker != MARKER { return Err("not a DarkRock sprawl lab; refusing mutation".into()); }
    Ok(())
}

fn write_sync(path: &Path, data: &[u8], private: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let mode = if private { 0o600 } else { 0o644 };
    let mut file = OpenOptions::new().write(true).create_new(true).mode(mode)
        .open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(data).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(())
}

fn placement(id: u32) -> [u8; 6] {
    let mut out = [0_u8; 6];
    let mut zone_pairs = [[0_u8; 2]; 3];
    for (zone, pair) in zone_pairs.iter_mut().enumerate() {
        let mut x = (id as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (zone as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
            ^ 0x1234_5678_9abc_def0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        let first = (x % 6) as u8;
        let mut second = ((x >> 32) % 5) as u8;
        if second >= first { second += 1; }
        *pair = [zone as u8 * 6 + first, zone as u8 * 6 + second];
    }
    for shard in 0..6 {
        let zone = (shard / 2 + id as usize % 3) % 3;
        out[shard] = zone_pairs[zone][shard % 2];
    }
    out
}

fn scripted_bytes(id: u32) -> Vec<u8> {
    // Each line is a shell comment. The generated scripts are data and never executed.
    let mut out = b"#!/bin/sh\n".to_vec();
    let mut state = (id as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let hex = b"0123456789abcdef";
    while out.len() < FILE_BYTES {
        let left = FILE_BYTES - out.len();
        if left == 1 { out.push(b'\n'); break; }
        out.push(b'#');
        for _ in 0..left.saturating_sub(2).min(62) {
            state ^= state << 13; state ^= state >> 7; state ^= state << 17;
            out.push(hex[(state & 15) as usize]);
        }
        out.push(b'\n');
    }
    out
}

fn serialize(records: &[Record]) -> Vec<u8> {
    let mut body = Vec::with_capacity(12 + records.len() * 242);
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
    let slice = data.get(*pos..end).ok_or("truncated manifest")?;
    *pos = end;
    slice.try_into().map_err(|_| "invalid manifest field".into())
}

fn load_manifest() -> Result<Vec<Record>, String> {
    ensure_lab()?;
    let key: [u8; 32] = fs::read(root().join("manifest.key")).map_err(|e| e.to_string())?
        .try_into().map_err(|_| "invalid manifest key length")?;
    let bytes = fs::read(root().join("manifest.bin")).map_err(|e| e.to_string())?;
    let body_len = bytes.len().checked_sub(32).ok_or("truncated manifest MAC")?;
    let (body, tag) = bytes.split_at(body_len);
    let expected = hmac_sha256(&key, body);
    let mismatch = expected.iter().zip(tag).fold(0_u8, |acc, (a, b)| acc | (a ^ b));
    if mismatch != 0 { return Err("manifest authentication failed".into()); }
    let mut pos = 0;
    if &take::<8>(body, &mut pos)? != MAGIC { return Err("manifest version mismatch".into()); }
    let count = u32::from_le_bytes(take::<4>(body, &mut pos)?) as usize;
    if count != FILES { return Err("unexpected manifest file count".into()); }
    let mut records = Vec::with_capacity(count);
    for expected_id in 0..count {
        let id = u32::from_le_bytes(take::<4>(body, &mut pos)?);
        let original_len = u32::from_le_bytes(take::<4>(body, &mut pos)?);
        let share_len = u32::from_le_bytes(take::<4>(body, &mut pos)?);
        let original_hash = take::<32>(body, &mut pos)?;
        let nodes = take::<6>(body, &mut pos)?;
        let mut share_hashes = [[0_u8; 32]; 6];
        for h in &mut share_hashes { *h = take::<32>(body, &mut pos)?; }
        if id as usize != expected_id || original_len as usize != FILE_BYTES
            || share_len != original_len.div_ceil(DATA_SHARDS as u32)
            || nodes != placement(id) {
            return Err("invalid manifest geometry or placement".into());
        }
        records.push(Record { id, original_len, share_len, original_hash, nodes, share_hashes });
    }
    if pos != body.len() { return Err("trailing manifest bytes".into()); }
    Ok(records)
}

fn init() -> Result<(), String> {
    if root().exists() { return Err(format!("{} already exists; preserving prior run", ROOT)); }
    let started = Instant::now();
    fs::create_dir_all(root()).map_err(|e| e.to_string())?;
    write_sync(&root().join(".lab-marker"), MARKER, false)?;
    let mut records = Vec::with_capacity(FILES);
    let mut source_bytes = 0_u64;
    let mut share_bytes = 0_u64;
    for id in 0..FILES as u32 {
        let source = scripted_bytes(id);
        write_sync(&source_path(id), &source, false)?;
        source_bytes += source.len() as u64;
        let stripe = encode_stripe(&source)?;
        let nodes = placement(id);
        for (shard, data) in stripe.shards.iter().enumerate() {
            write_sync(&share_path(id, shard, nodes[shard]), data, false)?;
            share_bytes += data.len() as u64;
        }
        records.push(Record {
            id, original_len: stripe.manifest.original_len as u32,
            share_len: stripe.manifest.share_len as u32,
            original_hash: stripe.manifest.original_hash,
            nodes, share_hashes: stripe.manifest.shard_hashes.try_into()
                .map_err(|_| "invalid encoder share count")?,
        });
    }
    let mut key = [0_u8; 32];
    File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut key))
        .map_err(|e| format!("random key: {e}"))?;
    write_sync(&root().join("manifest.key"), &key, true)?;
    let mut manifest = serialize(&records);
    manifest.extend_from_slice(&hmac_sha256(&key, &manifest));
    write_sync(&root().join("manifest.bin"), &manifest, false)?;
    println!("init: files={FILES} nodes={NODES} zones=3 source_bytes={source_bytes} share_bytes={share_bytes} manifest_bytes={} elapsed_ms={}", manifest.len(), started.elapsed().as_millis());
    Ok(())
}

fn lose_node(node: u8) -> Result<(), String> {
    load_manifest()?;
    if node as usize >= NODES { return Err("node must be 0..17".into()); }
    let path = node_path(node);
    let bytes: u64 = fs::read_dir(&path).map_err(|e| e.to_string())?
        .map(|e| e.and_then(|x| x.metadata()).map(|m| m.len())).collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?.iter().sum();
    fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
    println!("lost node={node} zone={} deleted_bytes={bytes}", node / 6);
    Ok(())
}

fn lose_zone(zone: u8) -> Result<(), String> {
    if zone >= 3 { return Err("zone must be 0..2".into()); }
    for node in zone * 6..zone * 6 + 6 { lose_node(node)?; }
    Ok(())
}

fn offline_node(node: u8) -> Result<(), String> {
    load_manifest()?;
    if node as usize >= NODES { return Err("node must be 0..17".into()); }
    let parked = root().join("offline").join(format!("node-{node:02}"));
    fs::create_dir_all(parked.parent().ok_or("invalid offline path")?).map_err(|e| e.to_string())?;
    if parked.exists() { return Err("node already parked".into()); }
    fs::rename(node_path(node), &parked).map_err(|e| e.to_string())?;
    println!("offline: node={node} zone={} parked={}", node / 6, parked.display());
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
            let name = entry.file_name();
            let destination = active.join(name);
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
    println!("online: node={node} zone={} restored_from={}", node / 6, parked.display());
    Ok(())
}

fn manifest_stripe(r: &Record) -> darkrock_storage::storage::StripeManifest {
    darkrock_storage::storage::StripeManifest {
        original_len: r.original_len as usize,
        share_len: r.share_len as usize,
        original_hash: r.original_hash,
        shard_hashes: r.share_hashes.to_vec(),
    }
}

fn read_shares(r: &Record) -> Result<(Vec<Option<Vec<u8>>>, u64), String> {
    let mut reads = 0_u64;
    let mut shares = Vec::with_capacity(6);
    for shard in 0..6 {
        match fs::read(share_path(r.id, shard, r.nodes[shard])) {
            Ok(bytes) => {
                reads += bytes.len() as u64;
                if bytes.len() != r.share_len as usize || hash(&bytes) != r.share_hashes[shard] {
                    return Err(format!("invalid share: file={} shard={shard}", r.id));
                }
                shares.push(Some(bytes));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => shares.push(None),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok((shares, reads))
}

fn repair() -> Result<(), String> { repair_with_pause(false) }

fn repair_with_pause(pause_after_snapshot: bool) -> Result<(), String> {
    let started = Instant::now();
    let records = load_manifest()?;
    let mut affected = 0;
    let mut paused = false;
    let mut bytes_scanned = 0_u64;
    let mut recovery_bytes_used = 0_u64;
    let mut bytes_written = 0_u64;
    for r in &records {
        let (mut shares, read) = read_shares(r)?;
        bytes_scanned += read;
        let missing: Vec<_> = (0..6).filter(|&i| shares[i].is_none()).collect();
        if missing.is_empty() { continue; }
        if missing.len() > PARITY_SHARDS { return Err(format!("file {} lost >2 shares", r.id)); }
        if pause_after_snapshot && !paused {
            let ready = root().join("repair-pause.ready");
            let resume = root().join("repair-pause.resume");
            if ready.exists() || resume.exists() { return Err("stale repair pause marker".into()); }
            let nodes = r.nodes.iter().map(u8::to_string).collect::<Vec<_>>().join(",");
            write_sync(&ready, format!("file_id={} nodes={}\n", r.id, nodes).as_bytes(), false)?;
            println!("repair_pause_ready: file_id={} nodes={} snapshot_shares_read={} waiting_for={}", r.id, nodes, 6 - missing.len(), resume.display());
            let wait_started = Instant::now();
            while !resume.exists() {
                if wait_started.elapsed().as_secs() > 120 { return Err("repair pause timed out".into()); }
                thread::sleep(std::time::Duration::from_millis(50));
            }
            fs::remove_file(&resume).map_err(|e| e.to_string())?;
            fs::remove_file(&ready).map_err(|e| e.to_string())?;
            paused = true;
        }
        // Read exactly four survivor shares for the actual recovery calculation.
        for i in 0..6 {
            if shares[i].is_some() && shares.iter().filter(|s| s.is_some()).count() > DATA_SHARDS {
                shares[i] = None;
            }
        }
        recovery_bytes_used += (DATA_SHARDS * r.share_len as usize) as u64;
        let original = reconstruct(shares, &manifest_stripe(r))?;
        let rebuilt = encode_stripe(&original)?;
        for &shard in &missing {
            let data = &rebuilt.shards[shard];
            if hash(data) != r.share_hashes[shard] { return Err("rebuilt share differs".into()); }
            write_sync(&share_path(r.id, shard, r.nodes[shard]), data, false)?;
            bytes_written += data.len() as u64;
        }
        affected += 1;
    }
    let (missing_counts, post_audit_bytes, _) = audit_snapshot(&records)?;
    let residual: usize = missing_counts[1..].iter().sum();
    println!("repair: affected_files={affected} disk_scan_bytes_read={bytes_scanned} recovery_bytes_used={recovery_bytes_used} rebuilt_bytes_written={bytes_written} post_audit_bytes_read={post_audit_bytes} residual_files={residual} elapsed_ms={}", started.elapsed().as_millis());
    if residual > 0 { return Err(format!("repair incomplete: {residual} files still have missing shares")); }
    Ok(())
}

fn audit_snapshot(records: &[Record]) -> Result<([usize; 7], u64, usize), String> {
    let mut missing_counts = [0_usize; 7];
    let mut present_bytes_read = 0_u64;
    let mut affected = [false; OWNERS];
    for r in records {
        let (shares, bytes) = read_shares(r)?;
        present_bytes_read += bytes;
        let missing = shares.iter().filter(|s| s.is_none()).count();
        missing_counts[missing] += 1;
        if missing > 0 { affected[r.id as usize % OWNERS] = true; }
    }
    Ok((missing_counts, present_bytes_read, affected.into_iter().filter(|x| *x).count()))
}

fn audit() -> Result<(), String> {
    let records = load_manifest()?;
    let (missing_counts, present_bytes_read, owners) = audit_snapshot(&records)?;
    println!("audit: files={} missing_0={} missing_1={} missing_2={} missing_3plus={} affected_owners={} present_bytes_read={present_bytes_read}",
        records.len(), missing_counts[0], missing_counts[1], missing_counts[2],
        missing_counts[3..].iter().sum::<usize>(), owners);
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
        let (shares, bytes) = read_shares(r)?;
        if shares.iter().any(Option::is_none) { return Err(format!("missing share: {}", r.id)); }
        share_bytes += bytes;
        let mut four = shares;
        four[4] = None; four[5] = None;
        if reconstruct(four, &manifest_stripe(r))? != source {
            return Err(format!("restored file mismatch: {}", r.id));
        }
    }
    println!("verify: files={} nodes={NODES} share_bytes={share_bytes} original_bytes={} exact=true elapsed_ms={}", records.len(), FILES * FILE_BYTES, started.elapsed().as_millis());
    Ok(())
}

fn health(records: &[Record], down_mask: u32) -> (usize, usize) {
    let mut degraded = 0;
    let mut unrecoverable = 0;
    for r in records {
        let missing = r.nodes.iter().filter(|&&node| down_mask & (1_u32 << node) != 0).count();
        if missing > 0 { degraded += 1; }
        if missing > PARITY_SHARDS { unrecoverable += 1; }
    }
    (degraded, unrecoverable)
}

fn affected_owners(records: &[Record], down_mask: u32) -> usize {
    let mut affected = [false; OWNERS];
    for r in records {
        if r.nodes.iter().any(|&node| down_mask & (1_u32 << node) != 0) {
            affected[r.id as usize % OWNERS] = true;
        }
    }
    affected.into_iter().filter(|present| *present).count()
}

fn owner_fanout() -> Result<(), String> {
    let records = load_manifest()?;
    println!("owner_fanout: simulated_owners={OWNERS} files_per_owner={}", FILES / OWNERS);
    for node in 0..NODES {
        let mask = 1_u32 << node;
        let (files, _) = health(&records, mask);
        let owners = affected_owners(&records, mask);
        println!("owner_fanout: node={node} affected_files={files} affected_owners={owners}");
    }
    Ok(())
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn churn(rounds: usize) -> Result<(), String> {
    if rounds == 0 || rounds > 32 { return Err("churn rounds must be 1..32".into()); }
    verify()?;
    let records = load_manifest()?;
    let seed = 0x6d1d_2e3f_4051_6273_u64;
    let mut state = seed;
    let mut tick = 0_u64;
    println!("churn: seed={seed:#x} rounds={rounds} simulated_tick_gaps=1..5 max_overlapping_down_nodes=2");
    for round in 1..=rounds {
        let failures = 1 + (next_random(&mut state) % 2) as usize;
        let mut down_mask = 0_u32;
        for increment in 1..=failures {
            tick += 1 + next_random(&mut state) % 5;
            let node = loop {
                let candidate = (next_random(&mut state) % NODES as u64) as u8;
                if down_mask & (1_u32 << candidate) == 0 { break candidate; }
            };
            lose_node(node)?;
            down_mask |= 1_u32 << node;
            let (degraded, unrecoverable) = health(&records, down_mask);
            let owners = affected_owners(&records, down_mask);
            println!("churn_increment: round={round} increment={increment} tick={tick} down_mask={down_mask:#07x} degraded_files={degraded} affected_owners={owners} unrecoverable_files={unrecoverable}");
            if unrecoverable > 0 { return Err("unexpected file below four live shares".into()); }
        }
        repair()?;
        verify()?;
        println!("churn_round_complete: round={round} repaired_down_nodes={failures}");
    }
    Ok(())
}

fn failure_spectrum() -> Result<(), String> {
    let records = load_manifest()?;
    for failed_nodes in 1..=4_u32 {
        let mut combinations = 0_u32;
        let mut sets_with_loss = 0_u32;
        let mut worst_files = 0_usize;
        for mask in 1_u32..1_u32 << NODES {
            if mask.count_ones() != failed_nodes { continue; }
            combinations += 1;
            let (_, unrecoverable) = health(&records, mask);
            if unrecoverable > 0 { sets_with_loss += 1; }
            worst_files = worst_files.max(unrecoverable);
        }
        println!("failure_spectrum: failed_nodes={failed_nodes} node_sets={combinations} sets_with_unrecoverable_file={sets_with_loss} worst_unrecoverable_files={worst_files}");
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd] if cmd == "init" => init(),
        [_, cmd] if cmd == "repair" => repair(),
        [_, cmd] if cmd == "repair-paused" => repair_with_pause(true),
        [_, cmd] if cmd == "verify" => verify(),
        [_, cmd] if cmd == "audit" => audit(),
        [_, cmd] if cmd == "owner-fanout" => owner_fanout(),
        [_, cmd] if cmd == "failure-spectrum" => failure_spectrum(),
        [_, cmd, value] if cmd == "lose-node" => lose_node(value.parse().map_err(|_| "invalid node")?),
        [_, cmd, value] if cmd == "lose-zone" => lose_zone(value.parse().map_err(|_| "invalid zone")?),
        [_, cmd, value] if cmd == "offline-node" => offline_node(value.parse().map_err(|_| "invalid node")?),
        [_, cmd, value] if cmd == "online-node" => online_node(value.parse().map_err(|_| "invalid node")?),
        [_, cmd, value] if cmd == "churn" => churn(value.parse().map_err(|_| "invalid round count")?),
        _ => Err("usage: sprawl_lab init|verify|repair|repair-paused|audit|lose-node 0..17|lose-zone 0..2|offline-node 0..17|online-node 0..17|owner-fanout|failure-spectrum|churn 1..32".into()),
    }
}

fn main() {
    if let Err(e) = run() { eprintln!("sprawl_lab: {e}"); std::process::exit(1); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stripe_has_two_distinct_nodes_per_zone() {
        let mut placements = std::collections::HashSet::new();
        for id in 0..FILES as u32 {
            let nodes = placement(id);
            let mut sorted = nodes.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), 6);
            placements.insert(sorted);
            for zone in 0..3 {
                assert_eq!(nodes.iter().filter(|&&node| node / 6 == zone).count(), 2);
            }
        }
        assert!(placements.len() >= 300, "placement diversity collapsed");
    }

    #[test]
    fn hmac_matches_known_sha256_vector() {
        let mut key = [0_u8; 32];
        key[..20].fill(0x0b);
        let actual = hmac_sha256(&key, b"Hi There");
        let expected = [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53,
            0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b, 0xf1, 0x2b,
            0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7,
            0x26, 0xe9, 0x37, 0x6c, 0x2e, 0x32, 0xcf, 0xf7,
        ];
        assert_eq!(actual, expected);
    }
}
