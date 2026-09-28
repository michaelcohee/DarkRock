Single volume, one M1, no network.

# DarkRock / RedTail-X

Michael Cohee · systems engineer, New York

DarkRock is a local storage research prototype. Its measured path uses a compression selector, variable-length final shares with standard Reed-Solomon 4+2, and byte-exact reconstruction checks. The node-sprawl exercises use directories on one physical volume. They do not demonstrate multi-host durability, network repair, blockchain operation, or a production storage service.

## What the experiments found

| Question | Current result |
|---|---|
| Compression selection (Ξ gate) | Entropy and periodicity choose whether to try zstd in mode Z. Its separate contribution was not isolated in the CAD comparison. |
| RedTail-X 4+2 | In the [raw no-zstd PDF control](raw-redtail-no-zstd-m1.md), variable final shares saved 218,568,756 shard bytes (17.93%) versus fixed 4+2 across 213 files. Reported CAD arms restored exact bytes under all 6 one-share and 15 two-share loss patterns. |
| Polynomial CAD canonizer as a storage format | Rejected. On the seeded held-out split, A3 v2 with its index rebuilt from protected data used 8.1% more protected bytes than plain fixed-chunk dedup, after being about 9.8% smaller on development files ([A3 v2 ledger](canon-test/cad-a3-v2-full-dev.md); [A1 control](canon-test/cad-dev-full-ledger-comparison.md)). Family patching (C2) was smallest. See [the held-out comparison](canon-test/cad-v2-seeded-test-report.md). |
| Posed-sibling geometry matches | Rare in the real converted-DXF census; most of the largest family's candidates were short segments rather than shared profiles. See [the census](canon-test/cad-tierb-posed-sibling-census.md). |
| Geometry keys for patch-base selection | Replicated on two frozen generated splits (v3 and v4): about 6% fewer protected bytes than byte-sketch selection and 0.8–0.9% fewer than filename families. On 47 real converted DXFs (exploratory): 0.5% fewer than sketch and 1.8% fewer than filename families, with most of the margin concentrated in 3–5 drawings. Key building took 18–30% of encode time. See [the v4 follow-up](canon-test/c2-hybrid-v4-report.md). |

**Takeaway:** for this CAD workload, the practical design is family-based zstd patching with a byte-sketch fallback, protected by RedTail-X 4+2. The polynomial canonizer failed as a storage format. Its geometry key helps choose patch bases on generated variants, but on real drawings the gain is small and not yet worth its compute cost.

These are local measurements on stated corpora. The 47 manufacturer drawings were used only for private evaluation; the DWG files and converted DXFs are **not distributed** in this repository. Their reported figures cannot be independently reproduced from this repository alone.

## Build and run

```sh
cargo test
cargo run --release --bin benchmark
python3 canon-test/make_cad_corpus.py canon-test/cad-corpus-v4-test-only \
  --corpus-id darkrock-cad-dxf-v4 --split test
python3 canon-test/verify_corpus.py canon-test/cad-corpus-v4-test-only
```

The CAD corpus generator creates synthetic ASCII DXFs and a manifest. Generated corpora are ignored by Git because they can be rebuilt from the script. The v4 test set was already opened for the reported experiment; regenerating it reproduces that set rather than creating a new holdout.

The CAD base-selection scripts additionally need `fastcdc==1.7.0` and the `zstd` command-line program. ODA conversion scripts require a locally installed, pinned ODA File Converter and source drawings supplied by the user. Local PDF benchmark binaries take a corpus directory as an argument. No private corpus path is embedded in the public-facing code.

## Boundaries

- `src/storage.rs` uses standard Reed-Solomon parity, with RedTail-X final-share sizing and an in-memory manifest. The default 4+2 code can recover any two missing shares in the tested stripe model.
- `src/polynomial_code.rs` is an experimental 4+2 MDS implementation, not a novel alternative to Reed-Solomon. `src/polynomial.rs` is a separate symbolic canonicalization experiment, not a byte-exact file deduper.
- `src/dxf_adapter.rs` uses geometry keys only to propose candidates; raw bytes and verified edits remain authoritative. A3 as a storage format did not win the held-out comparison.
- The routing and sprawl results are modeled or single-host exercises. They do not measure real network transfer, independent failure domains, or blockchain behavior.
- Encryption, a persistent authenticated metadata service, multi-node coordination, and production operations remain outside the tested storage core.

The [freeze records](canon-test/c2-base-selection-freeze.md) and [v4 hybrid rule](canon-test/c2-hybrid-v4-freeze.md) state the tested policies before their generated splits. The freeze commit hashes cited in reports refer to the private working history, which is available on request. The [provenance notes](PUBLIC_RELEASE.md) list the frozen files; the generated splits can be regenerated deterministically with the commands there and checked against the manifest hashes in each report. Public results include the limits of each comparison.
