#!/usr/bin/env python3
"""Group frozen dev-prefix physical increments by declared DXF variant."""
import csv
import json
from collections import defaultdict
from pathlib import Path

root = Path(__file__).resolve().parent
rows = list(csv.DictReader((root / "cad-a3-v2-dev-incremental.tsv").open(), delimiter="\t"))
controls = json.loads((root / "cad-dev-full-controls.json").read_text())
control_paths = controls["source_paths"]
c2 = controls["arms"]["C2"]["Z"]["incremental_physical_bytes"]
manifest = {r["path"]: r for r in csv.DictReader((root / "manifest.csv").open()) if r["split"] == "dev"}
assert len(rows) == len(c2) == 44
groups = defaultdict(lambda: [0, 0, 0, 0])
for row, path, c2_bytes in zip(rows, control_paths, c2):
    assert row["path"] == path
    totals = groups[manifest[path]["variant"]]
    totals[0] += 1
    totals[1] += int(row["a3_protected_z_incremental"])
    totals[2] += int(row["a3_derived_z_incremental"])
    totals[3] += c2_bytes
assert sum(x[1] for x in groups.values()) == 7_237_992
assert sum(x[2] for x in groups.values()) == 3_257_640
assert sum(x[3] for x in groups.values()) == 2_286_900

out = ["# A3 v2 versus C2 by variant — 44 development DXFs", "",
       "Four families contribute one file to each declared variant. Values are **incremental physical protected bytes** as files are added in the frozen path order; each variant includes its share of index growth, manifest/catalog changes and final-share rounding. This is an attribution of the measured corpus total, not a stand-alone encode of each variant.", "",
       "| Variant | Files | A3 v2 Protected Z | A3 v2 Derived Z | C2 Z | Protected − C2 |",
       "|---|---:|---:|---:|---:|---:|"]
for variant, (count, protected, derived, baseline) in sorted(groups.items()):
    out.append(f"| {variant} | {count} | {protected:,} | {derived:,} | {baseline:,} | {protected-baseline:+,} |")
out.extend(["", "**Result:** Protected A3 v2 costs more than C2 on 10 of 11 variant types. The sole cheaper type is the exact copy (264 versus 288 bytes across four copies). Translation, rotation, units scaling, and base drawings each add about 0.99–1.05 MB more than C2. The Derived view also exceeds C2 overall (3,257,640 versus 2,286,900 bytes).", "",
            "The exact-copy row uses a true whole-file ingest bypass: duplicate files add no entity objects or geometry-index entries. The per-copy record-only marginal in `cad-a3-v2-full-dev.md` is a different, narrower accounting view.", ""])
(root / "cad-a3-v2-vs-c2-variants-dev.md").write_text("\n".join(out))
