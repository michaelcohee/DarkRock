# ODA batch conversion — Step 4

**Date:** 2026-09-27  
**Scope:** Convert the 47 loose DWGs in `canon-test/DWG/` once, preserving the two ZIPs as opaque inputs. This is a conversion record, not a CAD fidelity or storage benchmark result.

## Published output

- Corpus directory: `canon-test/converted-dxf/oda-27.1-r2018-full/`
- Converter: ODA File Converter 27.1.0.0, native arm64; executable SHA-256 `21a8f029e7d41af64088f65492404d97ffec0d2f7886703b3b725f5ac4a55783`.
- Options: `ACAD2018 DXF 0 0 *.dwg` (ASCII DXF, no recursive scan, converter audit off).
- Result: 47 nonempty DXFs, 137,460,149 output bytes total, all reporting DXF version `AC1032` (R2018). The wrapper reported 192,696 entity records in aggregate.
- Converter exit status was 0, and its captured stderr was empty. The wrapper found exactly one output for each DWG, validated basic DXF group-code structure, hashed each output, and published the completed directory atomically.
- Manifest: `conversion-manifest.json`, SHA-256 `5ce26ceb2be36dfecd43c929d0cd77a2f9331bd822fc7c426977ade66b139810`. All 47 published file hashes and sizes were checked against it. All 49 original input hashes and sizes still matched the Step 1 preflight inventory.
- Free disk space was 1,295,331,328 bytes at conversion start and approximately 1.1 GiB after the temporary repeat checks.

## Repeat conversion check

Two temporary repeat conversions were compared with the published output; neither replaced or merged into it. Full DXF bytes were **not reproducible**: 0 of 47 files were byte-identical in the first repeat. In that comparison, 36 differed only in header timestamps (`$TDUPDATE`/`$TDUUPDATE`); 11 also differed elsewhere.

A section-level comparison against the second repeat found differences in `HEADER` for 47 files, `OBJECTS` for 8, and `ACDSDATA` for 7 (the latter two groups overlap). No differences were found in `ENTITIES`, `BLOCKS`, `TABLES`, or `CLASSES` in that sampled repeat. This is evidence of stable drawing-section bytes across **these runs**, not proof of general converter determinism or source-to-DXF geometric equivalence. The published output set and its hashes must be used unchanged for any comparison that follows.

Repeat-check records: `converted-dxf/oda-27.1-r2018-repeat-check.json` and `converted-dxf/oda-27.1-r2018-section-repeat-check.json`.

## Remaining before the benchmark

Step 5 must independently parse and audit every published DXF and inspect CAD semantics and representative views against the originals. The initial wrapper checks establish file structure and an output inventory, not conversion fidelity. Step 6 then records the accepted corpus ID and manifest hash as the formal benchmark freeze; all Tier B arms must use the same DXF bytes. The earlier 9.85 MB patch result was on the older raw-DWG set and cannot serve as a converted-DXF benchmark bar.
