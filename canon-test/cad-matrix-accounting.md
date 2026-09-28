# CAD protected-byte matrix: accounting rules

**Status:** Superseded broad protocol. The 2026-09-27 scope cut restricts this experiment to RedTail-X variable-tail 4+2. R0 fixed 4+2, 4+3, custom P+Q, packing variants, and A3 v3 are held. The development stop gate compared A3 v2 **Protected** bytes with C2 on 44 DXFs. It fired; the owner then expressly authorized freezing v2 and running one fresh Tier A comparison anyway.

## Earlier rows and dimensions (held)

Rows are P, A1, A2, A3 v1, C1, C2, and A3 v2 when its durable format exists. Run N and Z modes; R0 fixed 4+2, R1 RedTail-X variable 4+2, R2 variable 4+3, and R3 custom P+Q 4+2; and packed-stream and per-file striping. A cell requires a self-contained serialized ledger, byte-and-SHA-256 exact restore of every source file, and reconstruction under every applicable one-, two-, or three-share loss pattern. Report physical share bytes plus protected metadata and bootstrap bytes. R3 encode/reconstruct timing is a separate column.

## A3 v2 index views

Report **every built A3 v2 cell twice** using the same corpus, chosen representation, file payloads, references, edit stream, redundancy code, and packing policy:

1. **Protected index:** serialize and protect the complete geometry-key/hash/locator index. Include its framing, hashes, and its share-padding effect in physical bytes. Recover the ledger after each share-loss pattern and validate every locator against its decoded raw object.
2. **Rebuildable cache:** omit the key index from the protected ledger. After each recovery, restore every file byte-for-byte from the durable object/reference/edit graph, rescan those restored files, rebuild the geometry-key candidate index, and check that candidate lookup still works. Count the durable graph and any metadata required to locate and rebuild it. Report rebuild CPU time and the resulting index size in RAM separately; RAM is not protected storage.

Compute the two physical byte counts from two independently serialized streams through the same coding path. Do **not** obtain the cache result by subtracting `index_bytes × 1.5` from the protected result: final-share rounding, stripe boundaries, and metadata framing can change the physical delta. The cache view is valid only if reads and recovery do not require that index before it can be rebuilt. A missing or incomplete index-rebuild check makes that cell `not built`.

The existing `run_cad_v2_dev.py` result is a file-level geometry-ranked zstd patch experiment with estimated metadata. It is **not** either of these fully serialized A3 v2 cells. Its development number must not be placed in this matrix as measured protected bytes.

## Current development measurements

The A2 implementation and P control currently have full serialized R1 packed 4+2 measurements on the 44-file development split. They are not extrapolated into the other redundancy or packing cells.

| Arm | N protected bytes | Z protected bytes |
|---|---:|---:|
| P | 32,420,034 | 32,420,034 |
| A1 | 25,962,840 | 3,612,642 |
| A2 | 23,722,482 | 3,354,036 |
| C1 | 21,523,590 | 3,150,030 |
| C2 | 29,550,450 | 2,286,900 |

These measurements include the serialized object graph, index, file manifests, 4+2 protection, and bootstrap catalog policy used by `run_dev_full_controls.py`. The A3 dual-index development measurements appear in the reports below; the held matrix dimensions remain `not built`.

## Scope-cut result

The dual-index A3 v1 and v2 development reports are in `cad-a3-v1-dual-dev.md` and `cad-a3-v2-full-dev.md`. The corrected v2 run, which bypasses ingestion for whole-file exact copies, measured **7,237,992 Protected Z** and **3,257,640 Derived Z** versus C2's **2,286,900** bytes on the same 44 files. That crossed the development stop threshold. The owner later explicitly authorized a frozen fresh Tier A run; its result is `cad-v2-fresh-test-report.md`. Tier B received only the read-only posed-sibling census in `cad-tierb-posed-sibling-census.md`.
