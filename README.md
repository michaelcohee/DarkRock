Single volume, one M1, no network.

# DarkRock / RedTail-X

DarkRock is a local storage research prototype. Its measured path uses a compression selector, variable-length final shares with standard Reed-Solomon 4+2, and byte-exact reconstruction checks. The node-sprawl exercises use directories on one physical volume. They do not demonstrate multi-host durability, network repair, blockchain operation, or a production storage service.

## What the experiments found

| Question | Current result |
|---|---|
| Compression selection (Ξ gate) | Entropy and periodicity choose whether to try zstd in mode Z. Its separate contribution was not isolated in the CAD comparison. |
| RedTail-X 4+2 | Variable-length final shares remove much of the short-tail padding. Reported CAD arms restored exact bytes under all 6 one-share and 15 two-share loss patterns. |
| Polynomial CAD canonizer as a storage format | A3 v2 lost to the control on the seeded held-out split; see [the comparison](canon-test/cad-v2-seeded-test-report.md). |
| Posed-sibling geometry matches | Rare in the real converted-DXF census; most of the largest family's candidates were short segments rather than shared profiles. See [the census](canon-test/cad-tierb-posed-sibling-census.md). |
| Geometry keys for patch-base selection | A small protected-byte gain in the frozen generated tests. On 47 real converted DXFs, the edge over byte sketch was small and exploratory; key computation and memory have a measured cost. See [the v4 follow-up](canon-test/c2-hybrid-v4-report.md). |

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

The [freeze records](canon-test/c2-base-selection-freeze.md) and [v4 hybrid rule](canon-test/c2-hybrid-v4-freeze.md) state the tested policies before their generated splits. Public results include the relevant corpus manifest hashes and the limits of each comparison.
