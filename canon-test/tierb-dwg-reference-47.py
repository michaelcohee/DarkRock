#!/usr/bin/env python3
"""Raw-DWG A1/C1 reference rows for all 47 drawings; ZIPs stay separate."""

from __future__ import annotations

import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile

from fastcdc import fastcdc

ROOT = Path(__file__).resolve().parent
INPUT_MANIFEST = ROOT / "dwg-input-manifest-2026-09-27.csv"
OUTPUT = ROOT / "tierb-dwg-reference-47-results.json"
spec = importlib.util.spec_from_file_location("tierb_policy", ROOT / "tierb-dxf-controls.py")
assert spec and spec.loader
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


def main():
    sources = []
    zips = []
    with INPUT_MANIFEST.open(newline="") as stream:
        for row in csv.DictReader(stream):
            path = ROOT / row["relative_path"]
            data = path.read_bytes()
            assert len(data) == int(row["bytes"])
            assert hashlib.sha256(data).hexdigest() == row["sha256"]
            if row["kind"] == "dwg":
                sources.append((path.name, data))
            elif row["kind"] == "zip":
                zips.append({"file": path.name, "bytes": len(data), "sha256": row["sha256"]})
            else:
                raise ValueError(f"unexpected input kind: {row['kind']}")
    sources.sort(key=lambda item: item[0])
    assert len(sources) == 47 and len(zips) == 2
    with tempfile.TemporaryDirectory(prefix="darkrock-dwg-reference-") as directory:
        temp = Path(directory)
        a1 = policy.run_chunk_arm(
            "dwg-a1", sources,
            lambda data: (data[i:i + policy.BLOCK] for i in range(0, len(data), policy.BLOCK)), temp)
        print("raw DWG A1 passed", flush=True)
        c1 = policy.run_chunk_arm(
            "dwg-c1", sources,
            lambda data: (chunk.data for chunk in fastcdc(
                data, min_size=4096, avg_size=16384, max_size=65536, fat=True)), temp)
        print("raw DWG C1 passed", flush=True)
    report = {
        "scope": "Raw-DWG reference only; separate from frozen converted-DXF arms",
        "source_manifest_sha256": hashlib.sha256(INPUT_MANIFEST.read_bytes()).hexdigest(),
        "dwg_files": len(sources), "dwg_source_bytes": sum(len(data) for _, data in sources),
        "opaque_zips": zips,
        "zip_pair_byte_identical": zips[0]["sha256"] == zips[1]["sha256"],
        "a1": a1, "c1": c1,
    }
    OUTPUT.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({arm: {mode: report[arm][mode]["total_protected_bytes"]
                           for mode in ("mode_n", "mode_z")}
                      for arm in ("a1", "c1")}, indent=2))


if __name__ == "__main__":
    main()
