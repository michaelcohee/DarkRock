#!/usr/bin/env python3
"""Frozen Tier B DXF A1/C1/C2 byte controls; sources and corpus are read-only."""

from __future__ import annotations

from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

from fastcdc import fastcdc

ROOT = Path(__file__).resolve().parent
CORPUS = ROOT / "converted-dxf" / "oda-27.1-r2018-full"
MANIFEST = CORPUS / "conversion-manifest.json"
RESULTS = ROOT / "tierb-dxf-controls-2026-09-27.json"
MEASURE = ROOT.parent / "target" / "release" / "canon_control_measure"
SELECT = ROOT.parent / "target" / "release" / "canon_pack_select"
BLOCK = 256 * 1024

FAMILY_PREFIXES = (
    "077233_slhd-", "081316_series-8150-", "081373_universal-system-",
    "081476_", "083200_", "083213_", "083921", "084243_1016",
    "084243_tx9200-", "085313.20_", "102313_-elite-",
    "102313_solare-", "105736_by0120-",
)


def digest(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def family(name: str) -> str:
    return next((prefix for prefix in FAMILY_PREFIXES if name.startswith(prefix)), name)


def zstd(args: list[str], data: bytes | None = None) -> bytes:
    return subprocess.run(
        ["zstd", "-q", "-3", *args], input=data,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
    ).stdout


def measure(path: Path) -> dict:
    completed = subprocess.run([str(MEASURE), str(path)], text=True,
                               capture_output=True, check=True)
    row = dict(piece.split("=", 1) for piece in completed.stdout.strip().split(","))
    return {key: int(value) if value.isdigit() else value for key, value in row.items()}


def metadata(row: dict, unique_count: int, refs: int, files: int, patches: int = 0) -> None:
    raw = 232 * row["stripes"] + 36 * unique_count + 8 * refs + 8 * files + unique_count + 4 * patches
    row["modeled_metadata_raw_bytes"] = raw
    row["modeled_metadata_protected_bytes"] = (3 * raw + 1) // 2
    row["total_protected_bytes"] = row["actual_share_bytes"] + row["modeled_metadata_protected_bytes"]


def run_chunk_arm(name: str, sources: list[tuple[str, bytes]], chunker, temp: Path) -> dict:
    objects: list[bytes] = []
    seen: dict[bytes, list[int]] = defaultdict(list)
    references: dict[str, list[int]] = {}
    hits = 0
    hash_bucket_mismatches = 0
    for filename, data in sources:
        refs = []
        for part in chunker(data):
            h = digest(part)
            match = next((i for i in seen[h] if objects[i] == part), None)
            if match is None:
                if seen[h]: hash_bucket_mismatches += 1
                match = len(objects)
                objects.append(part)
                seen[h].append(match)
            else:
                hits += 1
            refs.append(match)
        restored = b"".join(objects[i] for i in refs)
        assert restored == data and digest(restored) == digest(data), filename
        references[filename] = refs

    raw_path = temp / f"{name}-raw.bin"
    with raw_path.open("wb") as output:
        for obj in objects: output.write(obj)
    mode_n = measure(raw_path)
    raw_path.unlink()
    metadata(mode_n, len(objects), sum(map(len, references.values())), len(sources))

    framed = temp / f"{name}-framed.bin"
    packed = temp / f"{name}-selected.bin"
    with framed.open("wb") as output:
        for obj in objects:
            output.write(struct.pack("<Q", len(obj)))
            output.write(obj)
    selected = subprocess.run([str(SELECT), str(framed), str(packed)],
                              text=True, capture_output=True, check=True)
    framed.unlink()
    select_stats = {key: int(value) for key, value in
                    (piece.split("=", 1) for piece in selected.stdout.strip().split(","))}
    assert select_stats["objects"] == len(objects)
    mode_z = measure(packed)
    packed.unlink()
    metadata(mode_z, len(objects), sum(map(len, references.values())), len(sources))
    return {
        "mode_n": mode_n, "mode_z": mode_z,
        "source_bytes": sum(len(data) for _, data in sources),
        "unique_objects": len(objects), "references": sum(map(len, references.values())),
        "confirmed_hash_hits": hits, "hash_bucket_mismatches": hash_bucket_mismatches,
        "all_files_byte_and_sha256_restored": True,
        "mode_z_selector": select_stats,
    }


def run_c2(sources: list[tuple[str, bytes]], temp: Path) -> dict:
    source_map = dict(sources)
    base: dict[str, str] = {}
    unique: dict[bytes, list[str]] = defaultdict(list)
    methods = Counter()
    choices = []
    payload_path = temp / "c2-selected.bin"
    with payload_path.open("wb") as output:
        for filename, data in sources:
            h = digest(data)
            duplicate = next((name for name in unique[h] if source_map[name] == data), None)
            if duplicate is not None:
                methods["exact_duplicate"] += 1
                choices.append({"file": filename, "method": "exact_duplicate", "base": duplicate,
                                "stored_bytes": 0})
                continue
            unique[h].append(filename)
            standalone = zstd(["-c", str(CORPUS / filename)])
            assert zstd(["-d", "-c"], standalone) == data
            chosen = standalone
            method = "standalone_zstd"
            reference = None
            group = family(filename)
            if group in base:
                reference = base[group]
                patch = zstd([f"--patch-from={CORPUS / reference}", "-c", str(CORPUS / filename)])
                assert zstd(["-d", f"--patch-from={CORPUS / reference}", "-c"], patch) == data
                if len(patch) < len(chosen):
                    chosen, method = patch, "patch"
            else:
                base[group] = filename
            output.write(chosen)
            methods[method] += 1
            choices.append({"file": filename, "method": method,
                            "base": reference if method == "patch" else None,
                            "standalone_bytes": len(standalone), "stored_bytes": len(chosen)})
    row = measure(payload_path)
    payload_path.unlink()
    metadata(row, len(sources) - methods["exact_duplicate"], len(sources),
             len(sources), methods["patch"])
    return {"mode_z": row, "methods": dict(methods), "choices": choices,
            "all_files_byte_and_sha256_restored": True,
            "family_prefixes": list(FAMILY_PREFIXES)}


def main():
    manifest = json.loads(MANIFEST.read_text())
    sources = []
    for entry in sorted(manifest["files"], key=lambda item: item["output"]):
        data = (CORPUS / entry["output"]).read_bytes()
        assert len(data) == entry["bytes"] and digest(data).hex() == entry["sha256"]
        sources.append((entry["output"], data))
    assert len(sources) == 47
    with tempfile.TemporaryDirectory(prefix="darkrock-tierb-dxf-") as directory:
        temp = Path(directory)
        a1 = run_chunk_arm("a1", sources,
                           lambda data: (data[i:i + BLOCK] for i in range(0, len(data), BLOCK)), temp)
        print("A1 done", flush=True)
        c1 = run_chunk_arm("c1", sources,
                           lambda data: (chunk.data for chunk in fastcdc(
                               data, min_size=4096, avg_size=16384, max_size=65536,
                               fat=True)), temp)
        print("C1 done", flush=True)
        c2 = run_c2(sources, temp)
        print("C2 done", flush=True)
    report = {
        "scope": "Single volume, one Apple M1 Mac, CPU only. No network, no node loss, no Storj, no chain.",
        "restore_target": "published converted DXF bytes, not source DWG bytes",
        "corpus_manifest_sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),
        "source_files": len(sources), "source_bytes": sum(len(data) for _, data in sources),
        "fastcdc_version": __import__("fastcdc").__version__,
        "zstd_version": subprocess.check_output(["zstd", "--version"], text=True).strip(),
        "a1": a1, "c1": c1, "c2": c2,
    }
    RESULTS.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({arm: {mode: value["total_protected_bytes"]
                           for mode, value in report[arm].items() if mode in ("mode_n", "mode_z")}
                      for arm in ("a1", "c1", "c2")}, indent=2))


if __name__ == "__main__":
    main()
