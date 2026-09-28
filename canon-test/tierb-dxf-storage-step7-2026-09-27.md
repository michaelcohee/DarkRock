# Tier B recovered-read validation — Step 7

**Scope:** Single volume, one Apple M1 Mac, CPU only. No network, no node loss, no Storj, no chain. The restore target is each **frozen converted DXF byte string**, not the source DWG.

## What the read check did

The checker verified the frozen manifest SHA-256 `5ce26ceb2be36dfecd43c929d0cd77a2f9331bd822fc7c426977ade66b139810` and every one of its 47 file hashes. For each available control row, it rebuilt the first-seen packed unique-object stream, used the actual RedTail-X 4+2 encoder, and reconstructed **every stripe under all six one-share and 15 two-share loss patterns**. It assembled a recovered packed stream from a two-share-loss reconstruction, decoded every stored object, followed the file references, and compared the rebuilt file with the frozen DXF by both bytes and SHA-256. The stored-byte and share-byte counts had to equal the Step 6 baseline for that row.

For C2, the checker decoded standalone compressed bases from the recovered stream, then decoded 28 patches using **those recovered bases**, not the original DXF source paths. The in-memory file references and reconstruction recipes were checked, but there is no persisted metadata service or real node placement here.

| Row | Files restored exactly | Objects decoded | References checked | Stripes | One/two-share scenarios | Step 6 total protected bytes |
|---|---:|---:|---:|---:|---:|---:|
| A1 mode N | 47/47 | 544 | 546 | 131 | 2,751 | 205,486,692 |
| A1 mode Z | 47/47 | 544 | 546 | 15 | 315 | 23,475,096 |
| C1 mode N | 47/47 | 5,316 | 6,025 | 116 | 2,436 | 181,360,290 |
| C1 mode Z | 47/47 | 5,316 | 6,025 | 15 | 315 | 22,575,510 |
| C2 mode Z | 47/47 | 47 | 47 | 12 | 252 | 18,502,067 |

**Result:** All five available rows passed: 235 file-restoration checks and 6,069 simulated one- or two-share loss cases across 289 stripes. The control arms have **zero undo bytes**; their index, reference, representation-tag, patch-base and manifest charges are in the Step 6 result under the same metadata model. The machine-readable run is `tierb-dxf-step7-results.json`.

## Scope limits and provenance

- A2 (cheap DXF normalization) and A3 (polynomial DXF canonization) **do not have implementations yet**. No storage read, undo-byte count, or win result for those arms is claimed. Completing Step 7 for *every planned arm* requires building and freezing the adapter first, then running both arms against this same DXF corpus.
- The Step 5 CAD-fidelity caveat still applies: `ezdxf` required in-memory repairs for 11 converted DXFs (72 `TABLECONTENT` deletions, 6 `XRECORD` owner-handle repairs, and 204 `ACAD_PROXY_OBJECT` deletions across those files). The published DXF bytes were not altered. This finding does **not** prove the converter lost visible geometry; it means these files have not been proven semantically equivalent to their original DWGs. Byte-exact restoration of a converted DXF is a different claim.
- The first end-to-end read attempt exposed a local disk-space limit when both packed and recovered streams were written to disk. The checker now streams recovered bytes to the process instead. The A1 mode N row was rerun and passed after that change; all final rows used the streaming read path.
- The Rust selector emits per-object length, encoding, and hash records for this check. The checker uses them to decode from the **recovered packed stream**; it does not silently reuse the pre-encode object list. Temporary packed streams are removed after each row. None of these tests deploys or persists storage nodes.

Run command with `fastcdc` 1.7.0 and `zstandard` 0.25.0 available:

```sh
python3 canon-test/tierb-dxf-step7.py  # requires fastcdc 1.7.0 and zstandard 0.25.0
```

The package path is a temporary local installation, not a required fixed path. `tierb-dxf-step7.py` and `src/bin/canon_storage_roundtrip.rs` implement the checks; `src/bin/canon_pack_select.rs` now optionally writes an object index. The frozen corpus and Step 6 control policy are unchanged.
