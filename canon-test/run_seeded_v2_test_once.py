#!/usr/bin/env python3
"""One coordinated, write-once generation, verification, and Z-mode comparison."""
from __future__ import annotations

import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CORPUS = HERE / "cad-corpus-v2-test-only"
STEM = "cad-v2-seeded-test"
CONTROLS = HERE / f"{STEM}-controls-z.json"
V1 = HERE / f"{STEM}-a3-v1-z.tsv"
V2 = HERE / f"{STEM}-a3-v2-z.tsv"
REPORT = HERE / f"{STEM}-report.md"
CORPUS_ID = "darkrock-cad-dxf-v2"


def run(args: list[str], env: dict[str, str] | None = None) -> None:
    print("Running:", " ".join(args), flush=True)
    subprocess.run(args, cwd=ROOT, env=env, check=True)


def main() -> None:
    if CORPUS.exists() or any(p.exists() for p in (CONTROLS, V1, V2, REPORT)):
        raise SystemExit("One-shot outputs already exist; refusing a second run")
    for name in ("cad_a3_v1_dual", "cad_a3_v2_dev", "canon_pack_select", "canon_full_ledger_measure"):
        if not (ROOT / "target" / "release" / name).is_file():
            raise SystemExit(f"Missing prebuilt release binary: {name}")
    run([sys.executable, str(HERE / "make_cad_corpus.py"), str(CORPUS),
         "--corpus-id", CORPUS_ID, "--split", "test"])
    run([sys.executable, str(HERE / "verify_corpus.py"), str(CORPUS)])

    env = os.environ.copy()
    env.update(DARKROCK_CORPUS_ROOT=str(CORPUS), DARKROCK_SPLIT="test",
               DARKROCK_ARMS="A1,A2,C1,C2", DARKROCK_MODES="Z",
               DARKROCK_NO_PREFIX="1", DARKROCK_OUTPUT=str(CONTROLS))
    run([sys.executable, str(HERE / "run_dev_full_controls.py")], env)
    run([str(ROOT / "target" / "release" / "cad_a3_v2_dev"), str(CORPUS / "test"), str(V2)])
    run([str(ROOT / "target" / "release" / "cad_a3_v1_dual"), str(CORPUS / "test"), str(V1)])

    manifest_bytes = (CORPUS / "manifest.json").read_bytes()
    corpus = json.loads(manifest_bytes)
    files = corpus["files"]
    assert corpus["corpus_id"] == CORPUS_ID
    assert len(files) == 132 and all(row["split"] == "test" for row in files)
    source_bytes = sum(int(row["bytes"]) for row in files)
    controls = json.loads(CONTROLS.read_text())
    assert len(controls["source_paths"]) == len(files)
    rows: dict[str, dict[str, int]] = {}
    for arm in ("C2", "C1", "A2", "A1"):
        cell = controls["arms"][arm]["Z"]
        assert cell["all_files_byte_and_sha256_restored"] is True
        assert cell["physical"]["all_21_loss_patterns_exact"] == "true"
        rows[arm] = {"Protected": int(cell["physical"]["total_physical_bytes"])}
    for path, arm in ((V2, "A3 v2"), (V1, "A3 v1")):
        rows[arm] = {}
        for item in csv.DictReader(path.open(), delimiter="\t"):
            assert item["mode"] == "Z" and int(item["files"]) == len(files)
            assert int(item["source_bytes"]) == source_bytes
            assert item["all_21_loss_patterns_exact"] == "true"
            rows[arm][item["view"]] = int(item["protected_bytes"])
        assert set(rows[arm]) == {"Protected", "Derived"}

    c2 = rows["C2"]["Protected"]
    out = ["# Seeded Tier A test-only comparison", "",
           f"**Corpus:** `{CORPUS_ID}`; {len(files)} generated test DXFs; "
           f"{source_bytes:,} source bytes. Generated after the joint adapter/generator freeze. "
           "The earlier v2 run used the pre-update generator and is superseded by this table.", "",
           "All cells use mode Z and RedTail-X variable-tail 4+2. Each cell restores every file "
           "byte-for-byte with SHA-256 verification and survives all 21 one/two-share loss patterns. "
           "The physical ledger counts protected payload, references, edits, manifests, index when Protected, "
           "catalog and charged bootstrap bytes.", "",
           f"**Generated manifest SHA-256:** `{hashlib.sha256(manifest_bytes).hexdigest()}`", "",
           "| Arm | Index view | Physical protected bytes | Versus C2 |", "|---|---|---:|---:|"]
    for arm in ("C2", "C1", "A3 v2", "A2", "A1", "A3 v1"):
        for view in ("Protected", "Derived"):
            if view in rows[arm]:
                n = rows[arm][view]
                out.append(f"| {arm} | {view} | {n:,} | {n / c2:.2f}× |")
            elif view == "Derived":
                out.append(f"| {arm} | Derived | not built | — |")
    out.extend(["", "Derived means the A3 candidate index is rebuilt from protected data "
                "and checked for the identical verified lookup set. Derived-index versions "
                "of A1, A2, C1 and C2 were not built.", "",
                "**Result:** " + ("C2 remains the smallest measured arm. " if c2 == min(v["Protected"] for v in rows.values()) else "A protected arm is smaller than C2. ")
                + f"A3 v2 uses {rows['A3 v2']['Protected'] / c2:.2f}× C2 with its index protected "
                + f"and {rows['A3 v2']['Derived'] / c2:.2f}× with that index derived.", "",
                f"Source ledgers: `{CONTROLS.name}`, `{V2.name}`, `{V1.name}`.", ""])
    REPORT.write_text("\n".join(out))
    print(f"One-shot table written: {REPORT}", flush=True)


if __name__ == "__main__":
    main()
