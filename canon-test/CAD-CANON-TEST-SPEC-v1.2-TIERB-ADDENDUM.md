# Tier B DXF corpus and control policy — v1.2 addendum

**Date:** 2026-09-27. This addendum updates only Tier B in `CAD-CANON-TEST-SPEC.md` v1.1. Tier A's held-out test rules and win criteria remain unchanged. This policy is recorded before the first 47-file DXF control run.

## Corpus and claim

- Tier B now has **47 loose DWGs and two opaque ZIPs** in `canon-test/DWG/`. The old v1.1 count of 20 files and its 9.85 MB raw-DWG C2 result describe an earlier subset, not this corpus.
- ODA File Converter 27.1.0.0 produced the published R2018 ASCII DXF set `converted-dxf/oda-27.1-r2018-full/`. Its manifest SHA-256 is `5ce26ceb2be36dfecd43c929d0cd77a2f9331bd822fc7c426977ade66b139810`.
- The Tier B storage restore target is **each published DXF byte string**, never the source DWG byte string. The two ZIPs remain outside the converted-DXF rows and are not unpacked. Source DWG rows, if later measured, are separate references.
- Step 5 found 11 DXFs that the independent `ezdxf` audit repairs in memory; source-versus-DXF CAD fidelity has not been proven. Storage byte results on the published set must carry that qualification. Do not claim these 47 are semantically identical to the DWGs.

## Frozen controls for the converted DXFs

All files run in ascending output filename order. All dedup hash hits use SHA-256 plus full byte comparison. Every control restores the exact published DXF bytes and checks SHA-256. Unique payloads are packed in first-seen order across files, crossing object boundaries, into 1 MiB RedTail-X stripes. Every stripe is tested under six one-share and 15 two-share losses. SIMD is off in the release binaries used for this baseline.

- **A1:** 256 KiB fixed chunks, reset at each file boundary, exact dedup. Mode N stores raw unique chunks. Mode Z calls production `representation::choose` on each unique chunk (8 KiB initial entropy sample, 6.4-bit threshold, periodicity check, zstd level 3 on candidates), verifies the selected representation decodes to the chunk, then packs it.
- **C1:** `fastcdc` Python package 1.7.0, `min_size=4096`, `avg_size=16384`, `max_size=65536`, reset at each file boundary, exact dedup. Modes N and Z use the same packing and production selector rules as A1.
- **C2:** For each exact-unique whole file, choose the shorter of standalone zstd level 3 and a `zstd --patch-from=<first sorted file in its named family>` delta. The first family file is stored as standalone zstd. No patch chains. Exact duplicates reference the first matching file. Family prefixes are explicitly fixed in `tierb-dxf-controls.py`; unmatched names are singleton families. Both standalone and patch forms must decode byte-exactly before a choice is accepted. C2 has a mode Z row only.
- **Metadata model:** 232 bytes per stripe manifest, 36 per unique-object index entry, 8 per object reference, 8 per file, 1 representation tag per unique object, and 4 per patch/base ID. Sum then charge at 1.5×, rounded up to an integer byte. This is a modeled metadata figure separate from actual encoded payload share bytes.

`tierb-dxf-controls.py` is the executable policy. `canon_pack_select.rs` calls the production Rust representation selector. `canon_control_measure.rs` calls the actual RedTail-X encoder and reconstruction functions. No A2 or A3 Tier B result exists at this freeze; the later adapter freeze commit is a different provenance point. Results on this corpus cannot decide Tier A's primary canonizer win.
