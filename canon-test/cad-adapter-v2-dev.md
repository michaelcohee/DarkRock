# Early CAD file-level patch exploration (superseded for A3 v2 accounting)

**Status:** Implemented and measured on the 44 Tier A development DXFs only (2026-09-27). This is a development candidate, not a held-out result or a claim that polynomial canonization beats standard compression. The existing `cad-adapter-v1.md` freeze remains the record of v1.

**Later result:** The structural A3 v2 full-ledger run is [cad-a3-v2-full-dev.md](./cad-a3-v2-full-dev.md). Its corrected Protected Z result is 7,237,992 bytes, so the earlier 2,265,222-byte *modeled* file-level patch result below must not be cited as A3 v2's protected storage cost. The owner subsequently authorized one frozen fresh Tier A run, reported in [cad-v2-fresh-test-report.md](./cad-v2-fresh-test-report.md). Tier B has a separate read-only census.

## Why v2 was built

The v1 per-entity path restored all 44 development files exactly, but its current physical layout is too costly against a whole-file patch control. Independently compressed literal runs total 1,184,110 bytes; all edit middles are 3,726,003 raw bytes or 439,119 bytes compressed together with zstd. Those two streams total **1,623,229 bytes before references/index/metadata**. Under the current layout, 4+2 alone makes at least 2,434,848 share bytes, already above C2's measured **2,280,018 total modeled protected bytes**. This is a rejection of v1's current layout against C2 on dev, not a proof that every possible entity packer loses.

## V2 mapping

For each incoming DXF, compute a multiset of the same `cad-v1` geometry-only entity keys from the raw-tag scanner. For every previously stored file, rank candidate patch bases by `shared_key_occurrences / min(key_occurrences_current, key_occurrences_base)`. Try the top three previous files and the first file in the same development family (C2's base), when distinct. For a byte-identical input, store an exact reference. Otherwise compare real zstd level-3 standalone bytes with real `zstd --patch-from` bytes for those candidate bases and keep the smallest. Every selected patch is decoded against its stored, recursively restored base and checked against the original DXF bytes. The geometry score proposes bases only; zstd and byte-exact verification make the final choice.

The current dev implementation is [run_cad_v2_dev.py](./run_cad_v2_dev.py) with geometry-key extraction from `src/dxf_adapter.rs` via `src/bin/cad_geometry_keys.rs`. It is **file-level adaptive delta selection**. Any gain over C2 is attributable to choosing a better patch base; this experiment does not demonstrate that polynomial coefficients themselves compress the drawing.

## Same dev split, zstd mode

| Arm | Packed bytes | Actual 4+2 share bytes | Modeled protected metadata | Total modeled protected bytes |
|---|---:|---:|---:|---:|
| A1 exact 256 KiB chunk dedup + zstd | 2,396,943 | 3,595,416 | 7,470 | **3,602,886** |
| C2 first-family-base patch or standalone zstd | 1,517,256 | 2,275,884 | 4,134 | **2,280,018** |
| V2 geometry-ranked base patch or standalone zstd | 1,507,390 | 2,261,088 | 4,134 | **2,265,222** |

V2 saved **14,796 modeled protected bytes (0.65%)** against C2 and **1,337,664 bytes (37.1%)** against A1 on this development corpus. It chose 27 patches, 13 standalone frames, and four exact references. Five files changed size relative to C2; the largest two gains were 5,377 and 4,238 patch bytes in the `f10_freestyle_sketch_b` family. The per-file method, base, selected bytes, and parsed/raw coverage are in [cad-v2-dev-results.json](./cad-v2-dev-results.json). A1/C2 source and method details are in [cad-dev-controls-results.json](./cad-dev-controls-results.json).

Both controls and V2 used the installed zstd CLI v1.5.7. Their packed streams were encoded with the same RedTail-X 4+2 routine and rebuilt under all 21 one- and two-share loss patterns. V2 also restored every file from the selected stored payload graph, including bases that were themselves patches or exact references. Source DXFs were read only.

## Limits and test-split discipline

The 0.65% dev lead is small. The metadata model uses the same estimated constants as C2; it is not a persisted/authenticated index, and candidate-search CPU time was not measured. V2 may lose or tie on another corpus. It should be compared against a nearest-base patch selector **without** geometry keys before attributing even base-choice quality to the polynomial key.

The full protected-byte matrix now requires two separate A3 v2 measurements per built cell: one with the geometry-key index protected, and one with it omitted as a cache and rebuilt from recovered byte-exact files. See [the accounting protocol](./cad-matrix-accounting.md). Neither number in that paired comparison has been measured here; the 2,265,222-byte development figure above remains an earlier modeled result.

No Tier A test DXF was read in this v2 work. However, [cad-controls-2026-09-27.md](./cad-controls-2026-09-27.md) records that an earlier exploratory A1 run already read the original 132-file test split. That split is therefore **not pristine** for a final unseen-data claim. A new split/corpus must be generated and frozen before any independent held-out evaluation.
