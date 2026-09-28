#!/usr/bin/env python3
"""Frozen v4 hybrid and existing C2 arms, with sidecar diagnostics."""
from __future__ import annotations

from collections import defaultdict
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
from time import perf_counter_ns

from fastcdc import fastcdc

import run_c2_base_selection as base


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CORPUS = HERE / "cad-corpus-v4-test-only"
ID = "darkrock-cad-dxf-v4"
OUTPUT = HERE / "c2-hybrid-v4-results.json"
REPORT = HERE / "c2-hybrid-v4-report.md"
OLD = HERE / "c2-base-selection-v3-results.json"
ARMS = (*base.ARMS, "C2-hybrid")


def load_a() -> list[dict]:
    manifest = json.loads((CORPUS / "manifest.json").read_text())
    assert manifest["corpus_id"] == ID
    rows = manifest["files"]
    assert len(rows) == 132 and all(r["split"] == "test" for r in rows)
    sources = []
    for row in sorted(rows, key=lambda r: r["path"]):
        path = CORPUS / row["path"]
        raw = path.read_bytes()
        assert len(raw) == row["bytes"] and base.sha(raw).hex() == row["sha256"]
        sources.append(dict(name=row["path"], path=path, raw=raw, family=row["family"]))
    return sources


def features_timed(sources: list[dict]) -> dict:
    key_ns = chunk_ns = key_count = key_bytes = 0
    for i, row in enumerate(sources, 1):
        start = perf_counter_ns()
        row["chunks"] = {base.sha(c.data) for c in fastcdc(
            row["raw"], min_size=4096, avg_size=16384, max_size=65536, fat=True)}
        chunk_ns += perf_counter_ns() - start
        start = perf_counter_ns()
        output = subprocess.check_output([str(base.KEYS), str(row["path"])])
        row["keys"] = set(output.decode("ascii").splitlines())
        key_ns += perf_counter_ns() - start
        key_count += len(output.splitlines())
        key_bytes += sys.getsizeof(row["keys"]) + sum(sys.getsizeof(s) for s in row["keys"])
        if i % 25 == 0:
            print(f"  features {i}/{len(sources)}", flush=True)
    return dict(keys_built=key_count, unique_keys_in_file_sets=sum(len(r["keys"]) for r in sources),
                key_set_bytes=key_bytes, key_build_ms=key_ns/1e6, chunk_build_ms=chunk_ns/1e6)


def hybrid_candidates(row: dict, unique: list[int], sources: list[dict]) -> list[int]:
    named = next((i for i in unique if sources[i]["family"] == row["family"]), None)
    keyed = base.rank(row, unique, sources, "keys")[:1]
    return list(dict.fromkeys(([named] if named is not None else []) + keyed))


def run_hybrid(sources: list[dict], standalone: list[bytes], temp: Path) -> dict:
    unique = []
    seen = defaultdict(list)
    choices = []
    summaries = []
    attempts = failures = hits = fallback = eligible = duplicates = 0
    hit_bytes = []
    for i, row in enumerate(sources):
        digest = base.sha(row["raw"])
        duplicate = next((j for j in seen[digest] if sources[j]["raw"] == row["raw"]), None)
        if duplicate is not None:
            duplicates += 1
            choices.append(dict(tag=2, base=duplicate, payload=b""))
            summaries.append(dict(file=row["name"], method="duplicate", base=sources[duplicate]["name"],
                                  selected_bytes=0, standalone_bytes=len(standalone[i]), attempts=0))
            continue
        seen[digest].append(i)
        candidates = hybrid_candidates(row, unique, sources)
        if candidates:
            eligible += 1
        best = None
        for candidate in candidates:
            attempts += 1
            try:
                patch = base.zstd([f"--patch-from={sources[candidate]['path']}", "-c", str(row["path"])])
                restored = base.zstd(["-d", f"--patch-from={sources[candidate]['path']}", "-c"], patch)
                if restored != row["raw"] or base.sha(restored) != digest:
                    failures += 1
                    continue
            except subprocess.CalledProcessError:
                failures += 1
                continue
            if best is None or (len(patch), candidate) < (len(best[1]), best[0]):
                best = (candidate, patch)
        if best is not None and len(standalone[i]) - len(best[1]) >= 1024:
            candidate, payload = best
            choices.append(dict(tag=1, base=candidate, payload=payload))
            summaries.append(dict(file=row["name"], method="patch", base=sources[candidate]["name"],
                                  selected_bytes=len(payload), standalone_bytes=len(standalone[i]),
                                  attempts=len(candidates)))
            hits += 1
            hit_bytes.append(len(payload))
        else:
            choices.append(dict(tag=0, base=None, payload=standalone[i]))
            summaries.append(dict(file=row["name"], method="standalone", base=None,
                                  selected_bytes=len(standalone[i]), standalone_bytes=len(standalone[i]),
                                  attempts=len(candidates)))
            if candidates:
                fallback += 1
        unique.append(i)
    blob = base.serialize(sources, choices)
    base.decode(blob, sources, temp)
    physical = base.measure(blob, temp)
    return dict(protected_bytes=physical["total_physical_bytes"], ledger_bytes=len(blob),
                files=len(sources), unique_files=len(unique), exact_duplicates=duplicates,
                patch_hits=hits, mean_patch_bytes=sum(hit_bytes)/len(hit_bytes) if hit_bytes else None,
                eligible_files=eligible, fallbacks=fallback, fallback_rate=fallback/eligible if eligible else None,
                patch_attempts=attempts, verify_failures=failures,
                exact_restore=True, all_21_loss_patterns_exact=True, choices=summaries)


def funnel(arm: str, sources: list[dict], standalone: list[bytes]) -> dict:
    """Diagnostic replay; unchanged frozen rank/threshold, disjoint proposal categories."""
    unique = []
    seen = defaultdict(list)
    counts = dict(proposals=0, rejected_verify=0, rejected_no_base=0,
                  rejected_below_1024=0, valid_not_selected=0, accepted=0)
    for i, row in enumerate(sources):
        digest = base.sha(row["raw"])
        duplicate = next((j for j in seen[digest] if sources[j]["raw"] == row["raw"]), None)
        if duplicate is not None:
            continue
        seen[digest].append(i)
        candidates, threshold = base.candidates(arm, row, unique, sources)
        valid = []
        for candidate in candidates:
            counts["proposals"] += 1
            try:
                patch = base.zstd([f"--patch-from={sources[candidate]['path']}", "-c", str(row["path"])])
                decoded = base.zstd(["-d", f"--patch-from={sources[candidate]['path']}", "-c"], patch)
                if decoded != row["raw"] or base.sha(decoded) != digest:
                    counts["rejected_verify"] += 1
                    continue
            except subprocess.CalledProcessError:
                counts["rejected_verify"] += 1
                continue
            gain = len(standalone[i]) - len(patch)
            if gain <= 0:
                counts["rejected_no_base"] += 1
            elif gain < threshold:
                counts["rejected_below_1024"] += 1
            else:
                valid.append((len(patch), candidate))
        if valid:
            counts["accepted"] += 1
            counts["valid_not_selected"] += len(valid)-1
        unique.append(i)
    assert counts["proposals"] == sum(v for k,v in counts.items() if k!="proposals")
    return counts


def worker(label: str, path: Path) -> None:
    sources = load_a() if label == "tier_a" else base.load_tier_b()
    print(f"{label}: {len(sources)} files", flush=True)
    start = perf_counter_ns()
    sidecar = features_timed(sources)
    standalone = []
    for row in sources:
        payload = base.zstd(["-c", str(row["path"])])
        assert base.zstd(["-d", "-c"], payload) == row["raw"]
        standalone.append(payload)
    sidecar["standalone_ms"] = (perf_counter_ns()-start)/1e6 - sidecar["key_build_ms"] - sidecar["chunk_build_ms"]
    arms = {}
    with tempfile.TemporaryDirectory(prefix="darkrock-c2-v4-") as directory:
        temp = Path(directory)
        for arm in ARMS:
            arm_start = perf_counter_ns()
            arms[arm] = run_hybrid(sources, standalone, temp) if arm == "C2-hybrid" else base.run_arm(arm, sources, standalone, temp)
            arms[arm]["encode_ms"] = (perf_counter_ns()-arm_start)/1e6
            print(f"  {arm}: {arms[arm]['protected_bytes']:,}", flush=True)
    base.compare(arms)
    sidecar["total_key_encode_ms"] = sidecar["key_build_ms"]+sidecar["chunk_build_ms"]+sidecar["standalone_ms"]+arms["C2-key"]["encode_ms"]
    sidecar["peak_process_rss_bytes"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    sidecar["peak_rss_excludes_children"] = True
    # Replay proposals after timed encoding so the diagnostic does not inflate encode_ms.
    funnels = {arm: funnel(arm, sources, standalone) for arm in ("C2-key", "C2-sketch")}
    assert funnels["C2-key"]["proposals"] == arms["C2-key"]["patch_attempts"]
    assert funnels["C2-sketch"]["proposals"] == arms["C2-sketch"]["patch_attempts"]
    assert funnels["C2-key"]["accepted"] == arms["C2-key"]["patch_hits"]
    assert funnels["C2-sketch"]["accepted"] == arms["C2-sketch"]["patch_hits"]
    result = dict(files=len(sources), source_bytes=sum(len(r["raw"]) for r in sources),
                  sidecar=sidecar, funnels=funnels, arms=arms)
    path.write_text(json.dumps(result, indent=2)+"\n")


def concentration(previous: dict) -> dict:
    arms = previous["tier_b"]["arms"]
    result = {}
    for other in ("C2-name", "C2-sketch"):
        paired = []
        for key, control in zip(arms["C2-key"]["choices"], arms[other]["choices"]):
            if key["method"] == "duplicate" or key["base"] == control["base"]:
                continue
            paired.append(dict(file=key["file"], key_base=key["base"], control_base=control["base"],
                               payload_saved=control["selected_bytes"]-key["selected_bytes"]))
        paired.sort(key=lambda r:(-r["payload_saved"],r["file"]))
        net = sum(r["payload_saved"] for r in paired)
        result[other] = dict(changed_files=len(paired), net_payload_saved=net,
                             gross_gains=sum(max(r["payload_saved"],0) for r in paired),
                             offsets=sum(min(r["payload_saved"],0) for r in paired),
                             top_share={str(k):sum(r["payload_saved"] for r in paired[:k])/net if net else None for k in (1,3,5)},
                             files=paired)
    return result


def fmt(n) -> str:
    return "—" if n is None else f"{n:,.1f}"


def report(result: dict) -> None:
    out = ["# Frozen C2 hybrid v4 follow-up", "",
           f"Freeze commit: `{result['freeze_commit']}`. Generated v4 manifest SHA-256: `{result['tier_a_manifest_sha256']}`. "
           "The v4 test split was generated after the freeze and opened once. "
           "The 47 converted real DXFs are **exploratory** because they were already used to compare pickers.", "",
           "## Real-file concentration from the prior C2-key result", "",
           "Per-file figures below are selected **payload** bytes, not a per-file assignment of 4+2 physical bytes. "
           "Top shares divide by the net payload margin; offsetting losses can make a share exceed 100%.", ""]
    for other in ("C2-name", "C2-sketch"):
        c = result["prior_tier_b_concentration"][other]
        out += [f"### Key versus {other}", "",
                f"{c['changed_files']} files selected different bases; net payload margin {c['net_payload_saved']:,} bytes "
                f"(gross gains {c['gross_gains']:,}; offsets {c['offsets']:,}). "
                f"Top 1/3/5 account for {100*c['top_share']['1']:.1f}% / {100*c['top_share']['3']:.1f}% / {100*c['top_share']['5']:.1f}% of net margin.", "",
                "| File | Key base | Comparator base | Payload bytes saved by key |", "|---|---|---|---:|"]
        for row in c["files"]:
            out.append(f"| `{row['file']}` | `{row['key_base'] or 'standalone'}` | "
                       f"`{row['control_base'] or 'standalone'}` | {row['payload_saved']:+,} |")
        out.append("")
    for label,title in (("tier_a","New generated v4 test-only split"),("tier_b","47 converted real DXFs — exploratory")):
        block=result[label]
        out += [f"## {title}", "", f"{block['files']} files; {block['source_bytes']:,} source bytes. "
                "All arms restore exactly and pass all 21 one/two-share loss patterns.", "",
                "| Arm | Protected bytes | Patch hits | Mean patch bytes | Fallbacks / eligible | Verify failures |",
                "|---|---:|---:|---:|---:|---:|"]
        for arm in ARMS:
            r=block["arms"][arm]
            out.append(f"| {arm} | {r['protected_bytes']:,} | {r['patch_hits']} | {fmt(r['mean_patch_bytes'])} | "
                       f"{r['fallbacks']}/{r['eligible_files']} | {r['verify_failures']} |")
        out += ["", "### Proposal funnel", "",
                "| Arm | Proposals | ≥ no-base size | Verify failures | Below 1,024-byte gain | Valid, not selected | Accepted |",
                "|---|---:|---:|---:|---:|---:|---:|"]
        for arm in ("C2-key", "C2-sketch"):
            f=block["funnels"][arm]
            out.append(f"| {arm} | {f['proposals']} | {f['rejected_no_base']} | {f['rejected_verify']} | "
                       f"{f['rejected_below_1024']} | {f['valid_not_selected']} | {f['accepted']} |")
        s=block["sidecar"]
        out += ["", "### Sidecar cost (outside protected bytes)", "",
                f"Keys built: {s['keys_built']:,} occurrences; unique key entries across file sets: "
                f"{s['unique_keys_in_file_sets']:,}. Python key-set memory: {s['key_set_bytes']:,} bytes. "
                f"Worker peak RSS: {s['peak_process_rss_bytes']:,} bytes. "
                f"Key-build time: {s['key_build_ms']:,.1f} ms; total C2-key encode time: "
                f"{s['total_key_encode_ms']:,.1f} ms ({100*s['key_build_ms']/s['total_key_encode_ms']:.1f}%).", ""]
    out += ["The sidecar RSS excludes peak memory of short-lived child processes. Timing is one run, "
            "not a distribution. The real-file table is exploratory. No v3 CAD transform/edit arm was built.", ""]
    REPORT.write_text("\n".join(out))


def main() -> None:
    if CORPUS.exists() or OUTPUT.exists() or REPORT.exists():
        raise SystemExit("One-shot v4 outputs already exist; refusing another generation/run")
    previous=json.loads(OLD.read_text())
    subprocess.run([sys.executable,str(HERE/"make_cad_corpus.py"),str(CORPUS),
                    "--corpus-id",ID,"--split","test"],check=True)
    subprocess.run([sys.executable,str(HERE/"verify_corpus.py"),str(CORPUS)],check=True)
    result=dict(freeze_commit=os.environ.get("DARKROCK_C2_V4_FREEZE_COMMIT",""),
                tier_a_manifest_sha256=base.sha((CORPUS/"manifest.json").read_bytes()).hex(),
                prior_tier_b_concentration=concentration(previous))
    with tempfile.TemporaryDirectory(prefix="darkrock-c2-v4-workers-") as directory:
        for label in ("tier_a","tier_b"):
            path=Path(directory)/f"{label}.json"
            subprocess.run([sys.executable,__file__,"--worker",label,str(path)],check=True,env=os.environ.copy())
            result[label]=json.loads(path.read_text())
    report(result)
    OUTPUT.write_text(json.dumps(result,indent=2)+"\n")
    print(f"Report: {REPORT}",flush=True)


if __name__ == "__main__":
    if len(sys.argv)==4 and sys.argv[1]=="--worker":
        worker(sys.argv[2],Path(sys.argv[3]))
    else:
        main()
