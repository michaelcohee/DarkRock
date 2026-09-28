# A3 v2 full development ledger

Scope: 44 Tier A development DXFs; RedTail-X variable-tail 4+2; full serialized payload graph. Whole-file exact references precede exact contiguous-run references, then entity references. Numeric spelling undo fires only when numeric DXF tag values parse to the same finite number; original bytes are retained in a verified edit. All per-file operation/edit streams are compressed with zstd level 3 only when smaller. Candidate key index has Protected and Derived views. Every decoded file is byte-and-SHA-256 identical and every data/catalog stripe survives 21 share-loss patterns.

| Mode | View | Ledger bytes | Key index bytes | Protected bytes | Whole-file refs | Run refs | Entity refs | Number spelling undo |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| N | Protected | 14751933 | 2653088 | 22134972 | 4 | 197 | 9892 | 4947 |
| N | Derived | 12098845 | 0 | 18154260 | 4 | 197 | 9892 | 4947 |
| Z | Protected | 4823016 | 2653088 | 7237992 | 4 | 197 | 9892 | 4947 |
| Z | Derived | 2169928 | 0 | 3257640 | 4 | 197 | 9892 | 4947 |

## Second exact copies

Each row isolates the logical manifest/reference record for an exact second copy. The protected-byte column is the **final-stripe marginal** `physical(full ledger) − physical(full ledger − record bytes)` with all shared objects and indexes held fixed; it is not an independent second-file benchmark.

| Exact copy | File bytes | A3 v2 record | A3 v2 protected marginal | A1 record | A1 protected marginal |
|---|---:|---:|---:|---:|---:|
| `f00_title_block_ansi_d__v01_exact_copy.dxf` | 7779 | 45 | 66 | 48 | 72 |
| `f03_spur_gears__v01_exact_copy.dxf` | 472740 | 45 | 66 | 52 | 78 |
| `f08_hex_nut_array__v01_exact_copy.dxf` | 678583 | 45 | 66 | 56 | 84 |
| `f10_freestyle_sketch_b__v01_exact_copy.dxf` | 753117 | 45 | 66 | 56 | 84 |

Index rebuild replay: 19948 keys, 29522 verified matches, identical per-file verify-pass vector. C2 Z = 2,286,900 protected bytes on the same development set. A3 v2 Protected Z = 7237992; ratio 3.16×. The development stop threshold was crossed; the owner later authorized a frozen held-out run despite that result.
