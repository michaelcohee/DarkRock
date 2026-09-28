//! Read-only 1 GB mixed-file benchmark for a reversible zero-difference mock.
//! This is not polynomial canonicalization for arbitrary bytes.
use darkrock_storage::storage::{encode_stripe, reconstruct, STRIPE_SIZE};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

const SELECTION: &str = "target/canon-mock-1gb/selection.nul";
const CHUNK: usize = 256 * 1024;
const MANIFEST_BYTES_PER_STRIPE: u64 = 4 + 4 + 32 + 6 * 32;
const MAX_ANCHOR_CANDIDATES: usize = 8;

#[derive(Clone, Copy)]
enum Kind { Raw, Delta { base: usize } }
struct Object { file: usize, offset: u64, len: usize, kind: Kind }
#[derive(Default)]
struct Ledger {
    payload_bytes: u64,
    manifest_bytes: u64,
    index_bytes: u64,
    reference_bytes: u64,
    coded_bytes: u64,
    unique_objects: usize,
}
impl Ledger {
    fn protected_total(&self) -> u64 {
        self.coded_bytes + (self.manifest_bytes + self.index_bytes + self.reference_bytes).div_ceil(2) * 3
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }
fn selected() -> Result<Vec<PathBuf>, String> {
    let root = std::env::var_os("DARKROCK_SOURCE_ROOT")
        .map(PathBuf::from)
        .ok_or("set DARKROCK_SOURCE_ROOT to your local source corpus")?;
    let bytes = fs::read(SELECTION).map_err(|e| e.to_string())?;
    if !bytes.ends_with(&[0]) { return Err("selection missing terminator".into()); }
    let mut out = Vec::new();
    for part in bytes[..bytes.len()-1].split(|&b| b == 0) {
        let rel = std::str::from_utf8(part).map_err(|e| e.to_string())?;
        let path = Path::new(rel);
        if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err("invalid selection path".into());
        }
        out.push(root.join(path));
    }
    Ok(out)
}
fn at(path: &Path, offset: u64, len: usize) -> Result<Vec<u8>, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0; len];
    f.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
fn protect(bytes: &[u8]) -> Result<(u64, u64), String> {
    let mut coded = 0_u64;
    let mut manifests = 0_u64;
    for part in bytes.chunks(STRIPE_SIZE) {
        let stripe = encode_stripe(part)?;
        coded += stripe.shards.iter().map(|s| s.len() as u64).sum::<u64>();
        manifests += MANIFEST_BYTES_PER_STRIPE;
        let mut shares: Vec<_> = stripe.shards.into_iter().map(Some).collect();
        shares[0] = None; shares[5] = None;
        if reconstruct(shares, &stripe.manifest)? != part { return Err("RS two-loss round trip differs".into()); }
    }
    Ok((coded, manifests))
}
fn varint(mut n: usize, out: &mut Vec<u8>) {
    while n >= 0x80 { out.push((n as u8) | 0x80); n >>= 7; }
    out.push(n as u8);
}
fn read_varint(bytes: &[u8], pos: &mut usize) -> Result<usize, String> {
    let mut value = 0_usize;
    for shift in (0..=28).step_by(7) {
        let byte = *bytes.get(*pos).ok_or("truncated varint")?; *pos += 1;
        value |= ((byte & 0x7f) as usize) << shift;
        if byte & 0x80 == 0 { return Ok(value); }
    }
    Err("varint too long".into())
}
fn delta(base_id: usize, base: &[u8], target: &[u8]) -> Option<Vec<u8>> {
    if base.len() != target.len() || base_id > u32::MAX as usize { return None; }
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(&(base_id as u32).to_le_bytes());
    out.extend_from_slice(&(target.len() as u32).to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    let (mut previous, mut count) = (0, 0_u32);
    for (index, (&a, &b)) in base.iter().zip(target).enumerate() {
        if a == b { continue; }
        varint(index - previous, &mut out);
        out.push(a ^ b);
        previous = index;
        count += 1;
        if out.len() >= target.len() { return None; }
    }
    out[8..12].copy_from_slice(&count.to_le_bytes());
    if out.len() + 4 < target.len() { Some(out) } else { None }
}
fn undo_delta(bytes: &[u8], base: &[u8], expected_base: usize) -> Result<Vec<u8>, String> {
    if bytes.len() < 12 { return Err("short delta".into()); }
    let id = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if id != expected_base || len != base.len() { return Err("delta base mismatch".into()); }
    let mut out = base.to_vec();
    let (mut pos, mut index) = (12, 0_usize);
    for _ in 0..count {
        index = index.checked_add(read_varint(bytes, &mut pos)?).ok_or("delta index overflow")?;
        let value = *bytes.get(pos).ok_or("truncated delta value")?; pos += 1;
        let dst = out.get_mut(index).ok_or("delta position out of range")?;
        *dst ^= value;
    }
    if pos != bytes.len() { return Err("delta trailing bytes".into()); }
    Ok(out)
}
fn anchors(bytes: &[u8]) -> [u64; 3] {
    let width = bytes.len().min(64);
    let parts = [&bytes[..width], &bytes[bytes.len()/2..bytes.len()/2+width.min(bytes.len()-bytes.len()/2)], &bytes[bytes.len()-width..]];
    std::array::from_fn(|i| u64::from_le_bytes(hash(parts[i])[..8].try_into().unwrap()))
}
fn run() -> Result<(), String> {
    let started = Instant::now();
    let paths = selected()?;
    let mut objects: Vec<Object> = Vec::new();
    let mut exact = HashMap::<[u8;32], Vec<usize>>::new();
    let mut anchor_map = HashMap::<u64, Vec<usize>>::new();
    let (mut raw, mut byte, mut model) = (Ledger::default(), Ledger::default(), Ledger::default());
    let (mut input_bytes, mut logical_chunks, mut duplicate_chunks, mut delta_chunks, mut delta_payload, mut files) = (0_u64, 0_usize, 0_usize, 0_usize, 0_u64, 0_usize);
    let mut buffer = vec![0_u8; STRIPE_SIZE];
    for (file_id, path) in paths.iter().enumerate() {
        if !path.is_file() || path.is_symlink() { return Err(format!("selected source changed: {}", path.display())); }
        let mut input = File::open(path).map_err(|e| e.to_string())?;
        files += 1;
        raw.reference_bytes += 8; byte.reference_bytes += 8; model.reference_bytes += 8;
        let mut offset = 0_u64;
        loop {
            let mut n = 0;
            while n < STRIPE_SIZE {
                let got = input.read(&mut buffer[n..]).map_err(|e| e.to_string())?;
                if got == 0 { break; }
                n += got;
            }
            if n == 0 { break; }
            let data = &buffer[..n];
            let (coded, manifest) = protect(data)?;
            raw.payload_bytes += n as u64; raw.coded_bytes += coded; raw.manifest_bytes += manifest;
            input_bytes += n as u64;
            for (within, chunk) in data.chunks(CHUNK).enumerate() {
                logical_chunks += 1;
                byte.reference_bytes += 8; model.reference_bytes += 8;
                let here = offset + (within * CHUNK) as u64;
                let digest = hash(chunk);
                let matching = if let Some(ids) = exact.get(&digest) {
                    let mut found = None;
                    for &id in ids {
                        let prior = &objects[id];
                        if prior.len == chunk.len() && at(&paths[prior.file], prior.offset, prior.len)? == chunk {
                            found = Some(id); break;
                        }
                    }
                    found
                } else { None };
                if matching.is_some() { duplicate_chunks += 1; continue; }
                let mut candidates = Vec::new();
                let mut seen = HashSet::new();
                for anchor in anchors(chunk) {
                    if let Some(ids) = anchor_map.get(&anchor) {
                        for &id in ids.iter().rev().take(MAX_ANCHOR_CANDIDATES) {
                            if seen.insert(id) { candidates.push(id); }
                        }
                    }
                }
                let mut best: Option<(usize, Vec<u8>)> = None;
                for id in candidates.into_iter().take(MAX_ANCHOR_CANDIDATES) {
                    let prior = &objects[id];
                    if !matches!(prior.kind, Kind::Raw) || prior.len != chunk.len() { continue; }
                    let base = at(&paths[prior.file], prior.offset, prior.len)?;
                    if let Some(d) = delta(id, &base, chunk) {
                        if best.as_ref().is_none_or(|(_, b)| d.len() < b.len()) { best = Some((id, d)); }
                    }
                }
                let (kind, payload) = match best {
                    Some((base, bytes)) => { delta_chunks += 1; delta_payload += bytes.len() as u64; (Kind::Delta { base }, bytes) }
                    None => (Kind::Raw, chunk.to_vec()),
                };
                let (byte_coded, byte_manifest) = protect(chunk)?;
                byte.payload_bytes += chunk.len() as u64;
                byte.coded_bytes += byte_coded;
                byte.manifest_bytes += byte_manifest;
                byte.index_bytes += 36;
                byte.unique_objects += 1;
                let (model_coded, model_manifest) = protect(&payload)?;
                let restored = match kind {
                    Kind::Raw => payload.clone(),
                    Kind::Delta { base } => {
                        let b = &objects[base];
                        undo_delta(&payload, &at(&paths[b.file], b.offset, b.len)?, base)?
                    }
                };
                if restored != chunk { return Err(format!("model byte mismatch: file {file_id} offset {here}")); }
                model.payload_bytes += payload.len() as u64;
                model.coded_bytes += model_coded;
                model.manifest_bytes += model_manifest;
                // One representation-kind byte per unique object; delta bases also get an ID.
                model.index_bytes += 37 + if matches!(kind, Kind::Delta { .. }) { 4 } else { 0 };
                model.unique_objects += 1;
                let id = objects.len();
                objects.push(Object { file: file_id, offset: here, len: chunk.len(), kind });
                exact.entry(digest).or_default().push(id);
                if matches!(kind, Kind::Raw) {
                    for anchor in anchors(chunk) { anchor_map.entry(anchor).or_default().push(id); }
                }
            }
            offset += n as u64;
            if n < STRIPE_SIZE { break; }
        }
        if offset != fs::metadata(path).map_err(|e| e.to_string())?.len() { return Err("source length changed during run".into()); }
    }
    println!("corpus: files={files} bytes={input_bytes} logical_chunks={logical_chunks} chunk_bytes={CHUNK}");
    println!("model: exact_duplicate_chunks={duplicate_chunks} sparse_delta_chunks={delta_chunks} sparse_delta_payload_bytes={delta_payload} unique_objects={}", objects.len());
    for (name, l) in [("plain_redtail_42", &raw), ("byte_dedup_redtail_42", &byte), ("zero_delta_mock_redtail_42", &model)] {
        println!("ledger: name={name} payload_bytes={} coded_bytes={} manifest_bytes={} index_bytes={} reference_bytes={} protected_total_bytes={} unique_objects={}", l.payload_bytes, l.coded_bytes, l.manifest_bytes, l.index_bytes, l.reference_bytes, l.protected_total(), l.unique_objects);
    }
    println!("result: mock_minus_plain_bytes={} mock_minus_byte_dedup_bytes={} elapsed_ms={} two_share_loss_roundtrip_exact=true", model.protected_total() as i128 - raw.protected_total() as i128, model.protected_total() as i128 - byte.protected_total() as i128, started.elapsed().as_millis());
    Ok(())
}
fn main() { if let Err(e) = run() { eprintln!("error: {e}"); std::process::exit(1); } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_delta_restores_exact_bytes() {
        let base = vec![7_u8; 1000];
        let mut target = base.clone(); target[5] = 9; target[700] = 0;
        let encoded = delta(12, &base, &target).unwrap();
        assert_eq!(undo_delta(&encoded, &base, 12).unwrap(), target);
    }
}
