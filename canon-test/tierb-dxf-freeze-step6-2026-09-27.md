# Tier B converted-DXF freeze and controls — Step 6

**Scope:** Single volume, one Apple M1 Mac, CPU only. No network, no node loss, no Storj, no chain. The restore target is the **published converted DXF bytes**, not the original DWG bytes.

## Frozen corpus

- Corpus ID: `darkrock-tierb-oda27.1-r2018-dxf-47-v1`.
- Published data: `canon-test/converted-dxf/oda-27.1-r2018-full/`, 47 DXFs totaling **137,460,149 bytes**. The two original ZIPs are opaque and outside these converted-DXF rows.
- Original frozen `conversion-manifest.json` SHA-256: **`5ce26ceb2be36dfecd43c929d0cd77a2f9331bd822fc7c426977ade66b139810`**. The public `tierb-dxf-frozen-manifest.json` contains the same file records and converter identity, but its machine-specific `output_dir` was replaced with a repository-relative path for publication; its own file hash therefore differs. Local reruns verify the original manifest hash and compare all other fields with the public copy. Reuse the exact output bytes for Tier B arms; a reconversion needs a new corpus ID.
- Local provenance: initial policy/code commit `e8409420c0326bf8984e64d2ccdaafea7ce30491`. Two harness bug-fix commits followed: `1b625f7a6eb4064b75632f61c1523ed1dc879bd9` enabled FastCDC payload output after the first C1 restore check failed, and `8f158123cf59a93cc5558b8c33e9f65a86291a5e` corrected a summary-print filter after the complete rows had been saved. All controls were rerun after both fixes; the final run exited successfully and its row values exactly matched the preceding complete run. These commits are **not** the later DXF adapter freeze or the held-out Tier A freeze.
- Versioned Tier B policy: `CAD-CANON-TEST-SPEC-v1.2-TIERB-ADDENDUM.md`. The original v1.1 document's 20-file Tier B count and 9.85 MB raw-DWG control belong to the older subset and are not reused here.

## Measured controls on the same 47 DXFs

The protected total below equals **actual RedTail-X 4+2 share bytes + modeled metadata charged at 1.5×**. Mode N means no compression; mode Z uses compression. Rows use first-seen packed unique payloads across files and variable-length final shares. `fastcdc` 1.7.0 used 4/16/64 KiB min/average/max; zstd CLI was 1.5.7; the Rust selector used the production entropy/periodicity gate and level-3 zstd. RS SIMD was off. Full policy and commands are in `tierb-dxf-controls.py` and the v1.2 addendum.

| Control | Mode | Packed unique payload | Actual share bytes | Modeled protected metadata | Total protected |
|---|---|---:|---:|---:|---:|
| A1: exact 256 KiB chunks | N | 136,935,861 | 205,403,796 | 82,896 | **205,486,692** |
| C1: FastCDC chunks | N | 120,634,679 | 180,952,020 | 408,270 | **181,360,290** |
| A1: exact 256 KiB chunks | Z | 15,621,710 | 23,432,568 | 42,528 | **23,475,096** |
| C1: FastCDC chunks | Z | 14,801,591 | 22,202,388 | 373,122 | **22,575,510** |
| C2: whole-file zstd or base delta | Z | 12,329,322 | 18,493,986 | 8,081 | **18,502,067** |

Against A1 in the matching mode, C1 saved **24,126,402 bytes (11.74%)** in N and **899,586 bytes (3.83%)** in Z. C2 saved **4,973,029 bytes (21.18%)** against A1 Z and **4,073,443 bytes (18.04%)** against C1 Z. C2 selected 19 standalone files and 28 patches; no exact whole-file duplicate was found in this converted set. A1 found two confirmed chunk hits; C1 found 709. Both recorded zero hash-bucket mismatches.

## Verification and limits

- Every input DXF was checked against the frozen manifest before measurement. All A1/C1 file references rebuilt their original DXF bytes and SHA-256; every selected zstd representation was decoded and compared byte-for-byte; every C2 standalone or patch payload was likewise decoded and compared.
- The actual RedTail-X encoder/reconstructor checked **all six single-share and 15 two-share loss patterns on every packed stripe** in every row. The row-specific stripe counts and per-file C2 choices are in `tierb-dxf-controls-2026-09-27.json`.
- These are layered checks, not a persistent multi-node store or an integrated file read after disk failure. The metadata amount is modeled; payload share bytes were produced by the Rust encoder.
- Step 5 found 11 DXFs that `ezdxf` repaired in memory, and no independent original-DWG-versus-DXF geometry comparison was performed. This freeze supports a **converted-DXF byte-restoration benchmark** only. It does not certify DWG conversion fidelity.
- A2 and A3 have not been run on Tier B. These control rows set the bar for a later adapter; they do not establish that polynomial canonization beats dedup, compression, or delta coding. Tier B remains separate from the held-out Tier A decision.

## Reproduction

With the pinned `fastcdc` 1.7.0 package available and the two release binaries built from the recorded source commits, run:

```sh
python3 canon-test/tierb-dxf-controls.py  # requires fastcdc 1.7.0
```

The `/tmp` package path is the run environment, not a required permanent location. The script reads only the frozen corpus, verifies its hashes, uses temporary packed streams, and writes the machine-readable control report outside the corpus directory. It does not alter the DWGs or published DXFs.
