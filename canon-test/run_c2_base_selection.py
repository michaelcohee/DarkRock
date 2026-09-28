#!/usr/bin/env python3
"""Frozen C2 base-selection experiment; no adapter or transform edits."""
from __future__ import annotations

from collections import defaultdict
import csv
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

from fastcdc import fastcdc


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
TIER_A = HERE / "cad-corpus-v3-test-only"
TIER_B = HERE / "converted-dxf" / "oda-27.1-r2018-full"
FROZEN_B = HERE / "tierb-dxf-frozen-manifest.json"
OUTPUT = HERE / "c2-base-selection-v3-results.json"
REPORT = HERE / "c2-base-selection-v3-report.md"
ARMS = ("C2-0", "C2-name", "C2-sketch", "C2-key", "C2-name+key")
KEYS = ROOT / "target" / "release" / "cad_file_keys"
MEASURE = ROOT / "target" / "release" / "canon_full_ledger_measure"
CORPUS_ID = "darkrock-cad-dxf-v3"
MIN_GAIN = 1024
TOP_K = 3
PREFIXES = (
    "077233_slhd-", "081316_series-8150-", "081373_universal-system-",
    "081476_", "083200_", "083213_", "083921", "084243_1016",
    "084243_tx9200-", "085313.20_", "102313_-elite-",
    "102313_solare-", "105736_by0120-",
)


def sha(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def zstd(args: list[str], data: bytes | None = None) -> bytes:
    return subprocess.run(["zstd", "-q", "-3", *args], input=data,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          check=True).stdout


def family_b(name: str) -> str:
    return next((prefix for prefix in PREFIXES if name.startswith(prefix)), name)


def load_tier_a() -> list[dict]:
    manifest = json.loads((TIER_A / "manifest.json").read_text())
    assert manifest["corpus_id"] == CORPUS_ID
    rows = manifest["files"]
    assert len(rows) == 132 and all(row["split"] == "test" for row in rows)
    out = []
    for row in sorted(rows, key=lambda r: r["path"]):
        path = TIER_A / row["path"]
        raw = path.read_bytes()
        assert len(raw) == row["bytes"] and sha(raw).hex() == row["sha256"]
        out.append(dict(name=row["path"], path=path, raw=raw, family=row["family"]))
    return out


def load_tier_b() -> list[dict]:
    frozen = json.loads(FROZEN_B.read_text())
    local = json.loads((TIER_B / "conversion-manifest.json").read_text())
    # The public manifest redacts only its machine-specific output directory.
    assert {k:v for k,v in local.items() if k != "output_dir"} == {
        k:v for k,v in frozen.items() if k != "output_dir"}
    assert len(frozen["files"]) == 47
    out = []
    for row in sorted(frozen["files"], key=lambda r: r["output"]):
        path = TIER_B / row["output"]
        raw = path.read_bytes()
        assert len(raw) == row["bytes"] and sha(raw).hex() == row["sha256"]
        out.append(dict(name=row["output"], path=path, raw=raw,
                        family=family_b(row["output"])))
    return out


def features(sources: list[dict]) -> None:
    for i, row in enumerate(sources, 1):
        row["chunks"] = {sha(c.data) for c in fastcdc(
            row["raw"], min_size=4096, avg_size=16384,
            max_size=65536, fat=True)}
        output = subprocess.check_output([str(KEYS), str(row["path"])])
        row["keys"] = set(output.decode("ascii").splitlines())
        if i % 25 == 0:
            print(f"  features {i}/{len(sources)}", flush=True)


def rank(row: dict, earlier: list[int], sources: list[dict], field: str) -> list[int]:
    candidates = [(len(row[field] & sources[i][field]), i) for i in earlier]
    candidates = [(score, i) for score, i in candidates if score > 0]
    candidates.sort(key=lambda pair: (-pair[0], pair[1]))
    return [i for _, i in candidates[:TOP_K]]


def candidates(arm: str, row: dict, earlier: list[int], sources: list[dict]) -> tuple[list[int], int]:
    named = next((i for i in earlier if sources[i]["family"] == row["family"]), None)
    if arm == "C2-0":
        return [], MIN_GAIN
    if arm == "C2-name":
        return ([named] if named is not None else []), 1
    if arm == "C2-sketch":
        return rank(row, earlier, sources, "chunks"), MIN_GAIN
    if arm == "C2-key":
        return rank(row, earlier, sources, "keys"), MIN_GAIN
    if named is not None:
        return [named], 1
    return rank(row, earlier, sources, "keys"), MIN_GAIN


def u32(n: int) -> bytes:
    return struct.pack("<I", n)


def u64(n: int) -> bytes:
    return struct.pack("<Q", n)


def serialize(sources: list[dict], choices: list[dict]) -> bytes:
    out = bytearray(b"DRC2B1") + u32(len(sources))
    for row, choice in zip(sources, choices):
        name = row["name"].encode("utf-8")
        assert len(name) <= 65535
        out += struct.pack("<H", len(name)) + name
        out += u64(len(row["raw"])) + sha(row["raw"])
        out += bytes([choice["tag"]]) + u32(choice["base"] if choice["base"] is not None else 0xffffffff)
        out += u64(len(choice["payload"])) + choice["payload"]
    return bytes(out)


def decode(blob: bytes, sources: list[dict], temp: Path) -> None:
    pos = 0
    def take(n: int) -> bytes:
        nonlocal pos
        value = blob[pos:pos+n]
        if len(value) != n:
            raise ValueError("truncated ledger")
        pos += n
        return value
    assert take(6) == b"DRC2B1"
    assert struct.unpack("<I", take(4))[0] == len(sources)
    recovered = []
    for i, row in enumerate(sources):
        name_len = struct.unpack("<H", take(2))[0]
        assert take(name_len).decode("utf-8") == row["name"]
        raw_len = struct.unpack("<Q", take(8))[0]
        digest = take(32)
        tag = take(1)[0]
        base = struct.unpack("<I", take(4))[0]
        payload = take(struct.unpack("<Q", take(8))[0])
        if tag == 0:
            assert base == 0xffffffff
            raw = zstd(["-d", "-c"], payload)
        else:
            assert base < i
            if tag == 1:
                basepath = temp / "recovered-base.dxf"
                basepath.write_bytes(recovered[base])
                raw = zstd(["-d", f"--patch-from={basepath}", "-c"], payload)
            elif tag == 2:
                assert not payload
                raw = recovered[base]
            else:
                raise ValueError("unknown representation tag")
        assert len(raw) == raw_len and sha(raw) == digest
        assert raw == row["raw"] and digest == sha(row["raw"])
        recovered.append(raw)
    assert pos == len(blob)


def measure(blob: bytes, temp: Path) -> dict:
    path = temp / "ledger.bin"
    path.write_bytes(blob)
    output = subprocess.check_output([str(MEASURE), str(path)], text=True).strip()
    path.unlink()
    result = dict(part.split("=", 1) for part in output.split(","))
    assert result["all_21_loss_patterns_exact"] == "true"
    return {key: int(value) if value.isdigit() else value for key, value in result.items()}


def run_arm(arm: str, sources: list[dict], standalone: list[bytes], temp: Path) -> dict:
    unique = []
    seen = defaultdict(list)
    choices = []
    summaries = []
    attempts = verify_failures = patch_hits = fallback = eligible = duplicates = 0
    patch_lengths = []
    for i, row in enumerate(sources):
        digest = sha(row["raw"])
        duplicate = next((j for j in seen[digest] if sources[j]["raw"] == row["raw"]), None)
        if duplicate is not None:
            duplicates += 1
            choices.append(dict(tag=2, base=duplicate, payload=b""))
            summaries.append(dict(file=row["name"], method="duplicate", base=sources[duplicate]["name"],
                                  selected_bytes=0, standalone_bytes=len(standalone[i]), attempts=0))
            continue
        seen[digest].append(i)
        candidates_, threshold = candidates(arm, row, unique, sources)
        if candidates_:
            eligible += 1
        best = None
        for base in candidates_:
            attempts += 1
            try:
                patch = zstd([f"--patch-from={sources[base]['path']}", "-c", str(row["path"])])
                restored = zstd(["-d", f"--patch-from={sources[base]['path']}", "-c"], patch)
                if restored != row["raw"] or sha(restored) != digest:
                    verify_failures += 1
                    continue
            except subprocess.CalledProcessError:
                verify_failures += 1
                continue
            if best is None or (len(patch), base) < (len(best[1]), best[0]):
                best = (base, patch)
        if best is not None and len(standalone[i]) - len(best[1]) >= threshold:
            base, payload = best
            choices.append(dict(tag=1, base=base, payload=payload))
            summaries.append(dict(file=row["name"], method="patch", base=sources[base]["name"],
                                  selected_bytes=len(payload), standalone_bytes=len(standalone[i]),
                                  attempts=len(candidates_)))
            patch_lengths.append(len(payload))
            patch_hits += 1
        else:
            choices.append(dict(tag=0, base=None, payload=standalone[i]))
            summaries.append(dict(file=row["name"], method="standalone", base=None,
                                  selected_bytes=len(standalone[i]), standalone_bytes=len(standalone[i]),
                                  attempts=len(candidates_)))
            if candidates_:
                fallback += 1
        unique.append(i)
    blob = serialize(sources, choices)
    decode(blob, sources, temp)
    physical = measure(blob, temp)
    return dict(protected_bytes=physical["total_physical_bytes"], ledger_bytes=len(blob),
                files=len(sources), unique_files=len(unique), exact_duplicates=duplicates,
                patch_hits=patch_hits, mean_patch_bytes=sum(patch_lengths)/len(patch_lengths) if patch_lengths else None,
                eligible_files=eligible, fallbacks=fallback,
                fallback_rate=fallback/eligible if eligible else None,
                patch_attempts=attempts, verify_failures=verify_failures,
                exact_restore=True, all_21_loss_patterns_exact=True,
                choices=summaries)


def compare(arms: dict[str, dict]) -> None:
    reference = arms["C2-name"]["choices"]
    for arm, result in arms.items():
        differences = harm = comparable = 0
        for choice, named in zip(result["choices"], reference):
            if choice["method"] == "duplicate":
                continue
            if choice["base"] != named["base"]:
                differences += 1
            if choice["method"] == named["method"] == "patch":
                comparable += 1
                if choice["selected_bytes"] > named["selected_bytes"]:
                    harm += 1
        result["different_base_count"] = differences
        result["harm_count"] = harm
        result["harm_comparable_count"] = comparable
        result["harm_rate"] = harm/comparable if comparable else None


def run_corpus(label: str, sources: list[dict]) -> dict:
    print(f"{label}: {len(sources)} files, {sum(len(r['raw']) for r in sources):,} source bytes", flush=True)
    features(sources)
    standalone = []
    for row in sources:
        compressed = zstd(["-c", str(row["path"])])
        assert zstd(["-d", "-c"], compressed) == row["raw"]
        standalone.append(compressed)
    arms = {}
    with tempfile.TemporaryDirectory(prefix="darkrock-c2-base-") as directory:
        temp = Path(directory)
        for arm in ARMS:
            arms[arm] = run_arm(arm, sources, standalone, temp)
            print(f"  {arm}: {arms[arm]['protected_bytes']:,} protected bytes", flush=True)
    compare(arms)
    return dict(files=len(sources), source_bytes=sum(len(r["raw"]) for r in sources), arms=arms)


def fmt_rate(value):
    return "—" if value is None else f"{100*value:.1f}%"


def report(results: dict) -> None:
    out = ["# Frozen C2 base-selection experiment", "",
           "**Rule:** [pre-generation freeze](./c2-base-selection-freeze.md). Mode Z, complete protected ledger, "
           "RedTail-X variable-tail 4+2. All selected patches are decoded and checked byte-for-byte and by SHA-256; "
           "all ledger bytes pass 21 one/two-share-loss patterns. Candidate scoring is only a lookup hint.", ""]
    for label, title in (("tier_a", "Tier A: new generated v3 test-only split"),
                         ("tier_b", "Tier B: 47 real converted DXFs")):
        block = results[label]
        out += [f"## {title}", "", f"{block['files']} files; {block['source_bytes']:,} source bytes.", "",
                "| Arm | Protected bytes | Patch hits | Mean patch bytes | Fallbacks / eligible | Different base vs name | Harm / comparable | Verify failures |",
                "|---|---:|---:|---:|---:|---:|---:|---:|"]
        for arm in ARMS:
            r = block["arms"][arm]
            mean = "—" if r["mean_patch_bytes"] is None else f"{r['mean_patch_bytes']:,.1f}"
            out.append(f"| {arm} | {r['protected_bytes']:,} | {r['patch_hits']} | {mean} | "
                       f"{r['fallbacks']}/{r['eligible_files']} ({fmt_rate(r['fallback_rate'])}) | "
                       f"{r['different_base_count']} | {r['harm_count']}/{r['harm_comparable_count']} "
                       f"({fmt_rate(r['harm_rate'])}) | {r['verify_failures']} |")
        a = block["arms"]
        key = a["C2-key"]
        qualifies = key["protected_bytes"] < min(a["C2-sketch"]["protected_bytes"], a["C2-0"]["protected_bytes"])
        out += ["", f"**C2-key pass condition:** {'met' if qualifies else 'not met'} on this corpus. "
                "Exact restore passed; harm rate is shown above.", ""]
    out += ["The generated Tier A filename families are an oracle. Tier B uses declared filename prefixes. "
            "The first earlier unique file in a family is C2-name's only base; name+key uses keys for orphans. "
            "The different-base count includes one arm selecting a patch while the other falls back. "
            "Harm compares accepted patches only when both arms patch the same file. "
            "Feature-extraction CPU and memory are outside protected-byte counts. No v3 CAD transform/edit path was built.", ""]
    REPORT.write_text("\n".join(out))


def main() -> None:
    if TIER_A.exists() or OUTPUT.exists() or REPORT.exists():
        raise SystemExit("One-shot v3 outputs already exist; refusing a second run")
    assert KEYS.is_file() and MEASURE.is_file()
    subprocess.run([sys.executable, str(HERE / "make_cad_corpus.py"), str(TIER_A),
                    "--corpus-id", CORPUS_ID, "--split", "test"], check=True)
    subprocess.run([sys.executable, str(HERE / "verify_corpus.py"), str(TIER_A)], check=True)
    results = dict(freeze_commit=os.environ.get("DARKROCK_C2_FREEZE_COMMIT", ""),
                   corpus_id=CORPUS_ID,
                   tier_a_manifest_sha256=sha((TIER_A / "manifest.json").read_bytes()).hex(),
                   tier_b_manifest_sha256=sha((TIER_B / "conversion-manifest.json").read_bytes()).hex())
    results["tier_a"] = run_corpus("Tier A", load_tier_a())
    results["tier_b"] = run_corpus("Tier B", load_tier_b())
    report(results)
    OUTPUT.write_text(json.dumps(results, indent=2) + "\n")
    print(f"Report: {REPORT}", flush=True)


if __name__ == "__main__":
    main()
