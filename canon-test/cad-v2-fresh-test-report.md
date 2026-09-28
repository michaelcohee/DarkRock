# Fresh Tier A held-out Z-mode result

**Corpus:** `darkrock-cad-dxf-v2`, 132 test DXFs, 85,440,965 source bytes. A3 v2 was frozen in commit `b0f355e` before this corpus was generated. This is the one coordinated opening of its test half. All rows use RedTail-X variable-tail 4+2, the same whole-ledger, protected-manifest catalog, and charged-bootstrap accounting, and byte-and-SHA-256 exact restore under all 21 one/two-share loss patterns.

**Reproduction:** `DARKROCK_CORPUS_ID=darkrock-cad-dxf-v2 python3 canon-test/make_cad_corpus.py canon-test/cad-corpus-v2`; generated manifest SHA-256 `4a1d3ed4f27c95e441cdc1060a9be8e111413cbf9f44341a792c48bdcee31611`.

| Arm | Index view | Physical protected bytes | Versus C2 |
|---|---|---:|---:|
| C2 | Protected | 11,369,532 | 1.00× |
| C1 | Protected | 13,712,514 | 1.21× |
| A3 v2 | Protected | 44,757,804 | 3.94× |
| A3 v2 | Derived | 16,670,892 | 1.47× |
| A2 | Protected | 14,555,964 | 1.28× |
| A1 | Protected | 15,424,938 | 1.36× |
| A3 v1 | Protected | 86,231,298 | 7.58× |
| A3 v1 | Derived | 58,144,386 | 5.11× |

For A1, A2, C1 and C2, the serialized hash index is **Protected**. A Derived-index version of those four baselines was not built or measured. The A3 lines separately encode a Protected key index or omit it as a Derived cache; the latter must rebuild the same verified candidate set from recovered files. Neither view removes file payload or undo bytes.

**Interpretation:** C2 is smallest. Frozen A3 v2 is 3.94× C2 with its index protected and 1.47× C2 with that index derived. The held-out result does not show a storage win for A3 v2; it confirms the development warning. The source data are the frozen corpus manifest, `cad-v2-fresh-test-controls-z.json`, and the two A3 TSV result files.
