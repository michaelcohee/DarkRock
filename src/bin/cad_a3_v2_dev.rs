//! Development-only A3 v2 ledger: whole-file, exact run, then entity references.
use darkrock_storage::dxf_adapter::{AdapterStore, Item, LedgerMode};
use darkrock_storage::representation::{choose, decode, Encoding};
use darkrock_storage::storage::{encode_stripe, reconstruct, STRIPE_SIZE};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

fn hash(b: &[u8]) -> [u8; 32] {
    Sha256::digest(b).into()
}
fn u32(o: &mut Vec<u8>, n: usize) {
    o.extend_from_slice(&(n as u32).to_le_bytes())
}
fn u64(o: &mut Vec<u8>, n: usize) {
    o.extend_from_slice(&(n as u64).to_le_bytes())
}
struct R<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let e = self.p.checked_add(n).ok_or("offset overflow")?;
        let v = self.b.get(self.p..e).ok_or("truncated ledger")?;
        self.p = e;
        Ok(v)
    }
    fn n8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn n32(&mut self) -> Result<usize, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize)
    }
    fn n64(&mut self) -> Result<usize, String> {
        usize::try_from(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
            .map_err(|_| "length overflow".into())
    }
    fn sha(&mut self) -> Result<[u8; 32], String> {
        Ok(self.take(32)?.try_into().unwrap())
    }
}
fn gather(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            gather(&p, files)?
        } else if p.extension().is_some_and(|x| x == "dxf") {
            files.push(p)
        }
    }
    Ok(())
}
fn numeric(code: i32) -> bool {
    matches!(code,10..=59|110..=149|210..=239)
}
fn lines(b: &[u8]) -> Vec<&[u8]> {
    b.split_inclusive(|&x| x == b'\n').collect()
}
fn spelling_undo(base: &[u8], target: &[u8]) -> Option<Vec<(usize, Vec<u8>)>> {
    let a = lines(base);
    let b = lines(target);
    if a.len() != b.len() || a.len() % 2 != 0 {
        return None;
    }
    let mut edits = Vec::new();
    for i in (0..a.len()).step_by(2) {
        if a[i] != b[i] {
            return None;
        }
        let code = std::str::from_utf8(a[i].trim_ascii())
            .ok()?
            .parse::<i32>()
            .ok()?;
        if a[i + 1] != b[i + 1] {
            if !numeric(code) {
                return None;
            }
            let left = std::str::from_utf8(a[i + 1].trim_ascii())
                .ok()?
                .parse::<f64>()
                .ok()?;
            let right = std::str::from_utf8(b[i + 1].trim_ascii())
                .ok()?
                .parse::<f64>()
                .ok()?;
            if !left.is_finite() || !right.is_finite() || left != right {
                return None;
            }
            edits.push((i + 1, b[i + 1].to_vec()));
        }
    }
    if edits.is_empty() {
        None
    } else {
        Some(edits)
    }
}
fn matched_base<'a>(item: &Item, objects: &'a [Vec<u8>]) -> Option<&'a [u8]> {
    if let Item::Matched {
        base, start, end, ..
    } = item
    {
        objects.get(*base)?.get(*start..*end)
    } else {
        None
    }
}
fn exact_match(item: &Item, objects: &[Vec<u8>]) -> bool {
    if let Item::Matched {
        middle, len, hash, ..
    } = item
    {
        let Some(base) = matched_base(item, objects) else {
            return false;
        };
        middle.is_empty() && *len == base.len() && *hash == self::hash(base)
    } else {
        false
    }
}
fn build_unique_store(input: &[Vec<u8>]) -> Result<(AdapterStore, Vec<usize>), String> {
    let mut store = AdapterStore::default();
    let mut mapping = Vec::with_capacity(input.len());
    let mut seen = HashMap::<[u8; 32], Vec<usize>>::new();
    for (i, raw) in input.iter().enumerate() {
        let sha = hash(raw);
        if let Some(&prior) = seen
            .get(&sha)
            .and_then(|ids| ids.iter().find(|&&j| input[j] == *raw))
        {
            mapping.push(mapping[prior]);
            continue;
        }
        let id = store.ingest(raw)?;
        mapping.push(id);
        seen.entry(sha).or_default().push(i);
    }
    Ok((store, mapping))
}

#[derive(Default)]
struct Counts {
    whole: usize,
    run: usize,
    entity: usize,
    number_undo: usize,
    literal: usize,
    edit_raw: usize,
    edit_encoded: usize,
    index_bytes: usize,
}
fn export(
    store: &AdapterStore,
    input: &[Vec<u8>],
    source_to_store: &[usize],
    mode: LedgerMode,
    protect_index: bool,
) -> Result<(Vec<u8>, Counts), String> {
    if source_to_store.len() != input.len() {
        return Err("source/store mapping length".into());
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"DRA3V2");
    out.push(match mode {
        LedgerMode::N => 0,
        LedgerMode::Z => 1,
    });
    out.push(protect_index as u8);
    u32(&mut out, store.objects.len());
    let mut counts = Counts::default();
    for raw in &store.objects {
        let (tag, payload) = match mode {
            LedgerMode::N => (0, raw.clone()),
            LedgerMode::Z => {
                let c = choose(raw)?;
                if decode(c.encoding, &c.payload)? != *raw {
                    return Err("object selection mismatch".into());
                }
                (if c.encoding == Encoding::Zstd { 1 } else { 0 }, c.payload)
            }
        };
        u64(&mut out, raw.len());
        out.extend_from_slice(&hash(raw));
        out.push(tag);
        u64(&mut out, payload.len());
        out.extend_from_slice(&payload);
    }
    u32(&mut out, input.len());
    let mut seen = HashMap::<[u8; 32], Vec<usize>>::new();
    for (file_id, raw) in input.iter().enumerate() {
        u64(&mut out, raw.len());
        out.extend_from_slice(&hash(raw));
        if let Some(&prior) = seen
            .get(&hash(raw))
            .and_then(|ids| ids.iter().find(|&&j| input[j] == *raw))
        {
            out.push(1);
            u32(&mut out, prior);
            counts.whole += 1;
            continue;
        }
        seen.entry(hash(raw)).or_default().push(file_id);
        out.push(0);
        let items = &store.files[source_to_store[file_id]].items;
        let mut ops = Vec::<Vec<u8>>::new();
        let mut at = 0;
        while at < items.len() {
            if exact_match(&items[at], &store.objects) {
                let Item::Matched { base, start, .. } = &items[at] else {
                    unreachable!()
                };
                let (mut end, mut n) = (*start, 0);
                while at + n < items.len() {
                    if let Item::Matched {
                        base: b,
                        start: s,
                        end: e,
                        ..
                    } = &items[at + n]
                    {
                        if *b == *base && *s == end && exact_match(&items[at + n], &store.objects) {
                            end = *e;
                            n += 1;
                            continue;
                        }
                    }
                    break;
                }
                if n >= 2 {
                    let mut op = vec![2];
                    u32(&mut op, *base);
                    u64(&mut op, *start);
                    u64(&mut op, end);
                    let bytes = store.objects[*base].get(*start..end).ok_or("invalid run")?;
                    op.extend_from_slice(&hash(bytes));
                    ops.push(op);
                    counts.run += 1;
                    at += n;
                    continue;
                }
            }
            let mut op = Vec::new();
            match &items[at] {
                Item::Literal(id) => {
                    op.push(0);
                    u32(&mut op, *id);
                    counts.literal += 1
                }
                Item::Matched {
                    base,
                    start,
                    end,
                    prefix,
                    suffix,
                    middle,
                    len,
                    hash: target_hash,
                } => {
                    let base_bytes = store
                        .objects
                        .get(*base)
                        .and_then(|x| x.get(*start..*end))
                        .ok_or("invalid entity locator")?;
                    if prefix + suffix > base_bytes.len() {
                        return Err("invalid edit".into());
                    }
                    let mut target = base_bytes[..*prefix].to_vec();
                    target.extend_from_slice(middle);
                    target.extend_from_slice(&base_bytes[base_bytes.len() - suffix..]);
                    if target.len() != *len || self::hash(&target) != *target_hash {
                        return Err("invalid target".into());
                    }
                    if let Some(edits) = spelling_undo(base_bytes, &target) {
                        op.push(3);
                        u32(&mut op, *base);
                        u64(&mut op, *start);
                        u64(&mut op, *end);
                        u64(&mut op, *len);
                        op.extend_from_slice(target_hash);
                        u32(&mut op, edits.len());
                        for (line, value) in edits {
                            u32(&mut op, line);
                            u32(&mut op, value.len());
                            op.extend_from_slice(&value)
                        }
                        counts.number_undo += 1;
                    } else {
                        op.push(1);
                        u32(&mut op, *base);
                        for n in [*start, *end, *prefix, *suffix, *len] {
                            u64(&mut op, n)
                        }
                        op.extend_from_slice(target_hash);
                        u64(&mut op, middle.len());
                        op.extend_from_slice(middle);
                        counts.entity += 1;
                    }
                }
            }
            ops.push(op);
            at += 1;
        }
        let mut stream = Vec::new();
        u32(&mut stream, ops.len());
        for op in ops {
            stream.extend_from_slice(&op)
        }
        counts.edit_raw += stream.len();
        let (tag, payload) = match mode {
            LedgerMode::N => (0, stream.clone()),
            LedgerMode::Z => {
                let z = zstd::bulk::compress(&stream, 3).map_err(|e| e.to_string())?;
                if z.len() < stream.len() {
                    (1, z)
                } else {
                    (0, stream.clone())
                }
            }
        };
        counts.edit_encoded += payload.len();
        out.push(tag);
        u64(&mut out, stream.len());
        u64(&mut out, payload.len());
        out.extend_from_slice(&payload);
    }
    if protect_index {
        let (v1, parts) = store.export_ledger(mode)?;
        let idx = v1
            .get(v1.len() - parts.key_hash_index..)
            .ok_or("missing protected index")?;
        out.extend_from_slice(idx);
        counts.index_bytes = idx.len()
    }
    Ok((out, counts))
}
fn verify(
    blob: &[u8],
    expected: &[Vec<u8>],
    original_store: &AdapterStore,
) -> Result<Vec<Vec<u8>>, String> {
    let mut r = R { b: blob, p: 0 };
    if r.take(6)? != b"DRA3V2" {
        return Err("v2 magic".into());
    }
    let mode = r.n8()?;
    if mode > 1 {
        return Err("mode".into());
    }
    let protected = r.n8()?;
    if protected > 1 {
        return Err("index view".into());
    }
    let count = r.n32()?;
    if count > 1_000_000 {
        return Err("too many objects".into());
    }
    let mut objects = Vec::new();
    for _ in 0..count {
        let len = r.n64()?;
        let sha = r.sha()?;
        let tag = r.n8()?;
        let pay = r.n64()?;
        if len > 64 * 1024 * 1024 || pay > 64 * 1024 * 1024 {
            return Err("object too large".into());
        }
        let b = r.take(pay)?;
        let raw = if tag == 0 {
            b.to_vec()
        } else if tag == 1 {
            zstd::stream::decode_all(b).map_err(|e| e.to_string())?
        } else {
            return Err("object tag".into());
        };
        if raw.len() != len || hash(&raw) != sha {
            return Err("object hash".into());
        }
        objects.push(raw)
    }
    if r.n32()? != expected.len() {
        return Err("file count".into());
    }
    let mut restored = Vec::<Vec<u8>>::new();
    for original in expected {
        let len = r.n64()?;
        let sha = r.sha()?;
        let kind = r.n8()?;
        let raw = if kind == 1 {
            let prior = r.n32()?;
            restored.get(prior).ok_or("forward whole-file ref")?.clone()
        } else if kind == 0 {
            let tag = r.n8()?;
            let stream_len = r.n64()?;
            let pay_len = r.n64()?;
            if stream_len > 64 * 1024 * 1024 || pay_len > 64 * 1024 * 1024 {
                return Err("stream too large".into());
            }
            let pay = r.take(pay_len)?;
            let stream = if tag == 0 {
                pay.to_vec()
            } else if tag == 1 {
                zstd::stream::decode_all(pay).map_err(|e| e.to_string())?
            } else {
                return Err("stream tag".into());
            };
            if stream.len() != stream_len {
                return Err("stream length".into());
            }
            let mut s = R { b: &stream, p: 0 };
            let n = s.n32()?;
            if n > 10_000_000 {
                return Err("too many ops".into());
            }
            let mut output = Vec::with_capacity(len);
            for _ in 0..n {
                let tag = s.n8()?;
                match tag {
                    0 => {
                        let id = s.n32()?;
                        output.extend_from_slice(objects.get(id).ok_or("literal id")?)
                    }
                    1 => {
                        let id = s.n32()?;
                        let start = s.n64()?;
                        let end = s.n64()?;
                        let prefix = s.n64()?;
                        let suffix = s.n64()?;
                        let target_len = s.n64()?;
                        let target_hash = s.sha()?;
                        let mid_len = s.n64()?;
                        let mid = s.take(mid_len)?;
                        let base = objects
                            .get(id)
                            .and_then(|x| x.get(start..end))
                            .ok_or("entity range")?;
                        if prefix + suffix > base.len() {
                            return Err("entity bounds".into());
                        }
                        let mut target = base[..prefix].to_vec();
                        target.extend_from_slice(mid);
                        target.extend_from_slice(&base[base.len() - suffix..]);
                        if target.len() != target_len || hash(&target) != target_hash {
                            return Err("entity hash".into());
                        }
                        output.extend_from_slice(&target)
                    }
                    2 => {
                        let id = s.n32()?;
                        let start = s.n64()?;
                        let end = s.n64()?;
                        let sha = s.sha()?;
                        let run = objects
                            .get(id)
                            .and_then(|x| x.get(start..end))
                            .ok_or("run range")?;
                        if hash(run) != sha {
                            return Err("run hash".into());
                        }
                        output.extend_from_slice(run)
                    }
                    3 => {
                        let id = s.n32()?;
                        let start = s.n64()?;
                        let end = s.n64()?;
                        let target_len = s.n64()?;
                        let target_hash = s.sha()?;
                        let count = s.n32()?;
                        let base = objects
                            .get(id)
                            .and_then(|x| x.get(start..end))
                            .ok_or("number base")?;
                        let mut ls: Vec<Vec<u8>> = lines(base).iter().map(|x| x.to_vec()).collect();
                        for _ in 0..count {
                            let line = s.n32()?;
                            let l = s.n32()?;
                            let old = s.take(l)?;
                            *ls.get_mut(line).ok_or("number line")? = old.to_vec()
                        }
                        let target = ls.concat();
                        if target.len() != target_len || hash(&target) != target_hash {
                            return Err("number undo hash".into());
                        }
                        output.extend_from_slice(&target)
                    }
                    _ => return Err("op tag".into()),
                }
            }
            if s.p != stream.len() {
                return Err("stream trailing".into());
            }
            output
        } else {
            return Err("file kind".into());
        };
        if raw.len() != len || hash(&raw) != sha || &raw != original {
            return Err("file byte/hash mismatch".into());
        }
        restored.push(raw)
    }
    if protected == 1 {
        let key_count = r.n32()?;
        if key_count > 10_000_000 {
            return Err("index count".into());
        }
        for _ in 0..key_count {
            let key_len = r.n32()?;
            if key_len > 1024 {
                return Err("key len".into());
            }
            r.take(key_len)?;
            let c = r.n32()?;
            if c > 8 {
                return Err("candidate count".into());
            }
            for _ in 0..c {
                let id = r.n32()?;
                let start = r.n64()?;
                let end = r.n64()?;
                let sha = r.sha()?;
                let b = objects
                    .get(id)
                    .and_then(|x| x.get(start..end))
                    .ok_or("index locator")?;
                if hash(b) != sha {
                    return Err("index hash".into());
                }
            }
        }
    }
    if r.p != blob.len() {
        return Err("trailing bytes".into());
    }
    if protected == 0 {
        let (rebuilt, _) = build_unique_store(&restored)?;
        let (rebuilt_keys, rebuilt_verified) = rebuilt.verify_index_rebuild()?;
        let (original_keys, original_verified) = original_store.verify_index_rebuild()?;
        if (rebuilt_keys, rebuilt_verified) != (original_keys, original_verified) {
            return Err("rebuilt index totals differ".into());
        }
        for (a, b) in rebuilt.files.iter().zip(&original_store.files) {
            let x = &a.coverage;
            let y = &b.coverage;
            if (
                x.lookup_hits,
                x.verified_matches,
                x.exact_hash_matches,
                x.edited_matches,
            ) != (
                y.lookup_hits,
                y.verified_matches,
                y.exact_hash_matches,
                y.edited_matches,
            ) {
                return Err("rebuilt verify-pass set differs".into());
            }
        }
    }
    Ok(restored)
}
fn physical(blob: &[u8]) -> Result<usize, String> {
    let mut catalog = Vec::new();
    catalog.extend_from_slice(b"DRMC1");
    catalog.extend_from_slice(&(blob.len() as u64).to_le_bytes());
    catalog.extend_from_slice(&(blob.len().div_ceil(STRIPE_SIZE) as u64).to_le_bytes());
    let mut total = 0;
    for chunk in blob.chunks(STRIPE_SIZE) {
        let stripe = encode_stripe(chunk)?;
        total += stripe.shards.iter().map(Vec::len).sum::<usize>();
        let m = &stripe.manifest;
        catalog.extend_from_slice(&(m.original_len as u64).to_le_bytes());
        catalog.extend_from_slice(&(m.share_len as u64).to_le_bytes());
        catalog.extend_from_slice(&m.original_hash);
        for h in &m.shard_hashes {
            catalog.extend_from_slice(h)
        }
        for a in 0..6 {
            for b in a..6 {
                let mut ss: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                ss[a] = None;
                if a != b {
                    ss[b] = None
                }
                if reconstruct(ss, m)? != chunk {
                    return Err("data loss".into());
                }
            }
        }
    }
    let stripes = catalog.len().div_ceil(STRIPE_SIZE);
    for chunk in catalog.chunks(STRIPE_SIZE) {
        let stripe = encode_stripe(chunk)?;
        total += stripe.shards.iter().map(Vec::len).sum::<usize>();
        for a in 0..6 {
            for b in a..6 {
                let mut ss: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                ss[a] = None;
                if a != b {
                    ss[b] = None
                }
                if reconstruct(ss, &stripe.manifest)? != chunk {
                    return Err("catalog loss".into());
                }
            }
        }
    }
    Ok(total + 6 * (32 + 240 * stripes))
}
fn physical_len(n: usize) -> usize {
    let full = n / STRIPE_SIZE;
    let tail = n % STRIPE_SIZE;
    let data = full * 6 * (STRIPE_SIZE / 4) + if tail == 0 { 0 } else { 6 * tail.div_ceil(4) };
    let catalog = 21 + 240 * n.div_ceil(STRIPE_SIZE);
    let catalog_shares = 6 * catalog.div_ceil(4);
    let bootstrap = 6 * (32 + 240 * catalog.div_ceil(STRIPE_SIZE));
    data + catalog_shares + bootstrap
}
fn run_generic(dir: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    gather(dir, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err("no DXFs in requested directory".into());
    }
    let input: Vec<Vec<u8>> = paths.iter().map(fs::read).collect::<Result<_, _>>()?;
    let (store, mapping) = build_unique_store(&input)?;
    let (mut protected, mut derived) = (0usize, 0usize);
    for (protect, slot) in [(true, &mut protected), (false, &mut derived)] {
        let (blob, _) = export(&store, &input, &mapping, LedgerMode::Z, protect)?;
        verify(&blob, &input, &store)?;
        *slot = physical(&blob)?;
    }
    fs::write(output,format!("arm\tmode\tview\tfiles\tsource_bytes\tprotected_bytes\tall_21_loss_patterns_exact\nA3_v2\tZ\tProtected\t{}\t{}\t{}\ttrue\nA3_v2\tZ\tDerived\t{}\t{}\t{}\ttrue\n",input.len(),input.iter().map(Vec::len).sum::<usize>(),protected,input.len(),input.iter().map(Vec::len).sum::<usize>(),derived))?;
    println!(
        "A3 v2 {} files: Protected={} Derived={}",
        input.len(),
        protected,
        derived
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 {
        return run_generic(Path::new(&args[1]), Path::new(&args[2]));
    }
    if args.len() != 1 {
        return Err("usage: cad_a3_v2_dev [DXF_DIR OUTPUT_TSV]".into());
    }
    let mut paths = Vec::new();
    gather(Path::new("canon-test/cad-corpus-v1/dev"), &mut paths)?;
    paths.sort();
    if paths.len() != 44 {
        return Err("expected 44 dev DXFs".into());
    }
    let input: Vec<Vec<u8>> = paths.iter().map(fs::read).collect::<Result<_, _>>()?;
    let (store, source_to_store) = build_unique_store(&input)?;
    let (keys, verified) = store.verify_index_rebuild()?;
    let mut report=String::from("# A3 v2 full development ledger\n\nScope: 44 Tier A development DXFs; RedTail-X variable-tail 4+2; full serialized payload graph. Whole-file exact references precede exact contiguous-run references, then entity references. Numeric spelling undo fires only when numeric DXF tag values parse to the same finite number; original bytes are retained in a verified edit. All per-file operation/edit streams are compressed with zstd level 3 only when smaller. Candidate key index has Protected and Derived views. Every decoded file is byte-and-SHA-256 identical and every data/catalog stripe survives 21 share-loss patterns.\n\n| Mode | View | Ledger bytes | Key index bytes | Protected bytes | Whole-file refs | Run refs | Entity refs | Number spelling undo |\n|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
    let mut z_protected = 0;
    let mut z_protected_len = 0;
    let mut z_derived = 0;
    for (mode_label, mode) in [("N", LedgerMode::N), ("Z", LedgerMode::Z)] {
        for (view, protected) in [("Protected", true), ("Derived", false)] {
            let (blob, c) = export(&store, &input, &source_to_store, mode, protected)?;
            verify(&blob, &input, &store)?;
            let bytes = physical(&blob)?;
            if mode_label == "Z" && protected {
                z_protected = bytes;
                z_protected_len = blob.len()
            }
            if mode_label == "Z" && !protected {
                z_derived = bytes;
            }
            report.push_str(&format!(
                "| {mode_label} | {view} | {} | {} | {bytes} | {} | {} | {} | {} |\n",
                blob.len(),
                c.index_bytes,
                c.whole,
                c.run,
                c.entity,
                c.number_undo
            ));
            println!("A3 v2 {mode_label} {view}: {bytes}")
        }
    }
    let a1_len = 2_406_595usize;
    let mut seen = HashMap::<[u8; 32], Vec<usize>>::new();
    report.push_str("\n## Second exact copies\n\nEach row isolates the logical manifest/reference record for an exact second copy. The protected-byte column is the **final-stripe marginal** `physical(full ledger) − physical(full ledger − record bytes)` with all shared objects and indexes held fixed; it is not an independent second-file benchmark.\n\n| Exact copy | File bytes | A3 v2 record | A3 v2 protected marginal | A1 record | A1 protected marginal |\n|---|---:|---:|---:|---:|---:|\n");
    for (i, raw) in input.iter().enumerate() {
        let sha = hash(raw);
        if seen
            .get(&sha)
            .is_some_and(|ids| ids.iter().any(|&j| input[j] == *raw))
        {
            let a3_record = 45usize;
            let a1_record = 44 + 4 * raw.len().div_ceil(262_144);
            report.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                paths[i].file_name().unwrap().to_string_lossy(),
                raw.len(),
                a3_record,
                physical_len(z_protected_len) - physical_len(z_protected_len - a3_record),
                a1_record,
                physical_len(a1_len) - physical_len(a1_len - a1_record)
            ));
        }
        seen.entry(sha).or_default().push(i)
    }
    report.push_str(&format!("\nIndex rebuild replay: {keys} keys, {verified} verified matches, identical per-file verify-pass vector. C2 Z = 2,286,900 protected bytes on the same development set. A3 v2 Protected Z = {z_protected}; ratio {:.2}×. The development stop threshold was crossed; the owner later authorized a frozen held-out run despite that result.\n",z_protected as f64/2_286_900.0));
    fs::write("canon-test/cad-a3-v2-full-dev.md", report)?;
    let mut prefix_store = AdapterStore::default();
    let mut prefix_mapping = Vec::<usize>::new();
    let mut prefix_seen = HashMap::<[u8; 32], Vec<usize>>::new();
    let mut prev_protected = 0usize;
    let mut prev_derived = 0usize;
    let mut increments=String::from("path\ta3_protected_z_incremental\ta3_derived_z_incremental\ta3_protected_z_cumulative\ta3_derived_z_cumulative\n");
    for (i, raw) in input.iter().enumerate() {
        let sha = hash(raw);
        if let Some(&prior) = prefix_seen
            .get(&sha)
            .and_then(|ids| ids.iter().find(|&&j| input[j] == *raw))
        {
            prefix_mapping.push(prefix_mapping[prior]);
        } else {
            prefix_mapping.push(prefix_store.ingest(raw)?);
            prefix_seen.entry(sha).or_default().push(i);
        }
        let (p, _) = export(
            &prefix_store,
            &input[..=i],
            &prefix_mapping,
            LedgerMode::Z,
            true,
        )?;
        let (d, _) = export(
            &prefix_store,
            &input[..=i],
            &prefix_mapping,
            LedgerMode::Z,
            false,
        )?;
        let pn = physical_len(p.len());
        let dn = physical_len(d.len());
        increments.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            paths[i].strip_prefix("canon-test/cad-corpus-v1")?.display(),
            pn - prev_protected,
            dn - prev_derived,
            pn,
            dn
        ));
        prev_protected = pn;
        prev_derived = dn;
    }
    if prev_protected != z_protected || prev_derived != z_derived {
        return Err("incremental physical total mismatch".into());
    }
    fs::write("canon-test/cad-a3-v2-dev-incremental.tsv", increments)?;
    Ok(())
}
