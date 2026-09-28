#!/usr/bin/env python3
"""End-to-end read checks for existing Tier B controls on frozen DXF bytes."""

from __future__ import annotations

from collections import defaultdict
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

from fastcdc import fastcdc
import zstandard

ROOT = Path(__file__).resolve().parent
CORPUS = ROOT / "converted-dxf" / "oda-27.1-r2018-full"
FROZEN = ROOT / "tierb-dxf-frozen-manifest.json"
BASELINE = ROOT / "tierb-dxf-controls-2026-09-27.json"
RESULT = ROOT / "tierb-dxf-step7-results.json"
SELECT = ROOT.parent / "target" / "release" / "canon_pack_select"
ROUNDTRIP = ROOT.parent / "target" / "release" / "canon_storage_roundtrip"

spec = importlib.util.spec_from_file_location("tierb_policy", ROOT / "tierb-dxf-controls.py")
assert spec and spec.loader
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


def digest(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def verify_source(name: str, restored: bytes, original: bytes) -> None:
    assert restored == original, f"byte mismatch: {name}"
    assert digest(restored) == digest(original), f"hash mismatch: {name}"


def rs_recover(packed: Path, temp: Path, expected: dict) -> tuple[bytes, dict]:
    completed = subprocess.run([str(ROUNDTRIP), str(packed), "-"],
                               capture_output=True)
    if completed.returncode:
        raise RuntimeError(f"RS roundtrip failed ({completed.returncode}): {completed.stderr.decode(errors='replace').strip()}")
    stats_line = completed.stderr.decode().strip().splitlines()[-1]
    stats = dict(piece.split("=", 1) for piece in stats_line.split(","))
    recovered = completed.stdout
    assert packed.read_bytes() == recovered
    assert int(stats["stripes"]) == expected["stripes"]
    assert int(stats["share_bytes"]) == expected["actual_share_bytes"]
    assert int(stats["packed_bytes"]) == expected["packed_bytes"]
    assert stats["patterns_per_stripe"] == "21" and stats["recovered_exact"] == "true"
    return recovered, {key: int(value) if value.isdigit() else value for key, value in stats.items()}


def chunks_for(name: str, data: bytes):
    if name == "a1":
        return (data[i:i + policy.BLOCK] for i in range(0, len(data), policy.BLOCK))
    return (chunk.data for chunk in fastcdc(data, min_size=4096,
                                            avg_size=16384, max_size=65536, fat=True))


def build_chunk_objects(name: str, sources: list[tuple[str, bytes]]):
    objects: list[bytes] = []
    seen: dict[bytes, list[int]] = defaultdict(list)
    references = {}
    for filename, data in sources:
        refs = []
        for part in chunks_for(name, data):
            h = digest(part)
            match = next((i for i in seen[h] if objects[i] == part), None)
            if match is None:
                match = len(objects)
                objects.append(part)
                seen[h].append(match)
            refs.append(match)
        references[filename] = refs
    return objects, references


def check_chunk_arm(name: str, sources: list[tuple[str, bytes]], mode: str,
                    baseline: dict, temp: Path) -> dict:
    objects, references = build_chunk_objects(name, sources)
    packed = temp / "packed.bin"
    if mode == "mode_n":
        with packed.open("wb") as output:
            for obj in objects: output.write(obj)
        lengths = [(len(obj), len(obj), "raw", digest(obj).hex()) for obj in objects]
    else:
        framed = temp / "objects.framed"
        index = temp / "objects.tsv"
        with framed.open("wb") as output:
            for obj in objects:
                output.write(struct.pack("<Q", len(obj)))
                output.write(obj)
        subprocess.run([str(SELECT), str(framed), str(packed), str(index)],
                       capture_output=True, text=True, check=True)
        lengths = []
        for line in index.read_text().splitlines():
            raw, stored, kind, sha = line.split("\t")
            lengths.append((int(raw), int(stored), kind, sha))
        framed.unlink()
        index.unlink()
    assert len(lengths) == len(objects)
    recovered, rs = rs_recover(packed, temp, baseline)
    packed.unlink()
    decoder = zstandard.ZstdDecompressor()
    decoded = []
    offset = 0
    for raw_length, stored_length, kind, sha in lengths:
        segment = recovered[offset:offset + stored_length]
        assert len(segment) == stored_length
        offset += stored_length
        obj = decoder.decompress(segment, max_output_size=raw_length) if kind == "zstd" else segment
        assert len(obj) == raw_length and digest(obj).hex() == sha
        decoded.append(obj)
    assert offset == len(recovered)
    for filename, original in sources:
        verify_source(filename, b"".join(decoded[i] for i in references[filename]), original)
    return {"files_verified": len(sources), "objects_decoded": len(decoded),
            "references_verified": sum(map(len, references.values())), "undo_bytes": 0,
            "rs": rs, "baseline_total_protected_bytes": baseline["total_protected_bytes"]}


def check_c2(sources: list[tuple[str, bytes]], baseline: dict,
             baseline_choices: list[dict], temp: Path) -> dict:
    source_map = dict(sources)
    bases = {}
    seen: dict[bytes, list[str]] = defaultdict(list)
    choices = []
    packed = temp / "packed.bin"
    with packed.open("wb") as output:
        for filename, data in sources:
            h = digest(data)
            duplicate = next((other for other in seen[h] if source_map[other] == data), None)
            if duplicate is not None:
                choices.append((filename, "exact_duplicate", duplicate, 0))
                continue
            seen[h].append(filename)
            standalone = policy.zstd(["-c", str(CORPUS / filename)])
            group = policy.family(filename)
            chosen, method, reference = standalone, "standalone_zstd", None
            if group in bases:
                base = bases[group]
                patch = policy.zstd([f"--patch-from={CORPUS / base}", "-c", str(CORPUS / filename)])
                if len(patch) < len(chosen):
                    chosen, method, reference = patch, "patch", base
            else:
                bases[group] = filename
            choices.append((filename, method, reference, len(chosen)))
            output.write(chosen)
    assert [(n, m, b, size) for n, m, b, size in choices] == [
        (c["file"], c["method"], c.get("base"), c["stored_bytes"])
        for c in baseline_choices]
    recovered, rs = rs_recover(packed, temp, baseline)
    packed.unlink()
    decoded = {}
    offset = 0
    base_files = {}
    for filename, method, reference, length in choices:
        if method == "exact_duplicate":
            value = decoded[reference]
        else:
            payload = recovered[offset:offset + length]
            assert len(payload) == length
            offset += length
            if method == "standalone_zstd":
                value = policy.zstd(["-d", "-c"], payload)
            else:
                if reference not in base_files:
                    path = temp / (f"base-{len(base_files)}.dxf")
                    path.write_bytes(decoded[reference])
                    base_files[reference] = path
                value = policy.zstd(["-d", f"--patch-from={base_files[reference]}", "-c"], payload)
        verify_source(filename, value, source_map[filename])
        decoded[filename] = value
    assert offset == len(recovered)
    return {"files_verified": len(decoded), "objects_decoded": len(decoded),
            "references_verified": len(sources), "undo_bytes": 0,
            "patches_restored_from_recovered_bases": sum(m == "patch" for _, m, _, _ in choices),
            "rs": rs, "baseline_total_protected_bytes": baseline["total_protected_bytes"]}


def main():
    only = set(sys.argv[1:])
    frozen = json.loads(FROZEN.read_text())
    baseline = json.loads(BASELINE.read_text())
    local_manifest = (CORPUS / "conversion-manifest.json").read_bytes()
    assert digest(local_manifest).hex() == baseline["corpus_manifest_sha256"]
    # The public frozen copy differs only in its redacted local output path.
    local = json.loads(local_manifest)
    public = json.loads(FROZEN.read_text())
    assert {k:v for k,v in local.items() if k != "output_dir"} == {
        k:v for k,v in public.items() if k != "output_dir"}
    sources = []
    for entry in sorted(frozen["files"], key=lambda item: item["output"]):
        data = (CORPUS / entry["output"]).read_bytes()
        assert len(data) == entry["bytes"] and digest(data).hex() == entry["sha256"]
        sources.append((entry["output"], data))
    assert len(sources) == 47
    rows = {}
    if only and RESULT.exists():
        previous = json.loads(RESULT.read_text())
        assert previous["corpus_manifest_sha256"] == baseline["corpus_manifest_sha256"]
        rows.update(previous["rows"])
    with tempfile.TemporaryDirectory(prefix="darkrock-step7-") as directory:
        temp = Path(directory)
        for arm in ("a1", "c1"):
            for mode in ("mode_n", "mode_z"):
                label = f"{arm}_{mode}"
                if only and label not in only: continue
                rows[label] = check_chunk_arm(arm, sources, mode, baseline[arm][mode], temp)
                print(label, "passed", flush=True)
        if not only or "c2_mode_z" in only:
            rows["c2_mode_z"] = check_c2(sources, baseline["c2"]["mode_z"],
                                          baseline["c2"]["choices"], temp)
            print("c2_mode_z passed", flush=True)
    result = {"corpus_manifest_sha256": baseline["corpus_manifest_sha256"],
              "restore_target": "frozen converted DXF bytes",
              "files": len(sources), "rows": rows,
              "not_run": ["A2 cheap normalization adapter", "A3 polynomial DXF adapter"],
              "cad_fidelity_claim": False}
    RESULT.write_text(json.dumps(result, indent=2) + "\n")
    print("all_existing_control_rows_passed", len(rows))


if __name__ == "__main__":
    main()
