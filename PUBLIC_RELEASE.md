# Public snapshot notes

This repository's working history is a private research record. Some earlier commits contain local machine paths. Publish the **clean snapshot** from `public-export/` as a new repository, not this private Git history.

The public snapshot contains code, generated-corpus tooling, and reports. It excludes the manufacturer's DWGs, the ODA-converted DXFs, generated corpora, local benchmark files, and the original local conversion manifest. The checked-in Tier B manifest is a path-redacted metadata copy. It contains filenames, sizes, and hashes, but no drawing bytes. Tier B results cannot be rerun without independently supplied source drawings and the pinned converter.

The README's opening scope is: **single volume, one M1, no network**. This limits all node-sprawl and routing claims to their measured local or modeled conditions.

The public repository is `michaelcohee/DarkRock`. Its initial commit was made from the clean snapshot directory; it does not expose the private commit history. No license file has been selected, so publication does not grant reuse rights beyond GitHub's normal viewing and forking terms.

## Provenance

The following commits are in the **private** research history, not the public repository. Dates are local time (America/New_York) on 2026-09-27. A result commit records an observation **after** the named split was generated; it is not a pre-generation freeze. The original v1 test split had already been read by an exploratory control, so only the newly generated v2/v3/v4 test halves support the stated held-out sequence.

| Private commit | Time (EDT) | Frozen or recorded | Split generation relationship |
|---|---|---|---|
| `addac59` | 19:05:32 | A3 v1 entity adapter and development round-trip report | Before later fresh v2; v1's original test half was already exploratory |
| `4488337` | 19:21:41 | Candidate verification hardening and pre-split mapping review | Before fresh v2 |
| `43dd6c2` | 19:35:19 | Full A3 v1 development ledger result | Dev only; before fresh v2 |
| `aefbfe2` | 20:08:41 | A3 dual-index development results and stop condition | Dev only; before fresh v2 |
| `b0f355e` | 20:23:53 | A3 v2 adapter, closed edit list, and development variant costs | Before the early v2 run and the later seeded v2 run |
| `ce4240f` | 20:36:39 | Early v2 comparison and Tier B pose census | After early v2; **superseded** by seeded v2 |
| `b9aeff7` | 20:42:59 | Seeded generator, verifier, and one-shot A3 v2 evaluation runner | Before the reported seeded v2 test half |
| `8cdbbf1` | 20:46:02 | One-shot seeded v2 held-out result | After seeded v2 |
| `c4525d0` | 20:59:32 | C2 base-selection rule, runner, and geometry-key extractor | Before v3 test half |
| `78d2454` | 21:01:33 | Frozen C2 base-selection results | After v3 |
| `2ee6014` | 21:19:16 | C2 hybrid rule and diagnostic runner | Before v4 test half |
| `06cffc8` | 21:21:42 | C2 v4 result, concentration, proposal funnel, and costs | After v4 |

### SHA-256 of files changed by the six freeze commits in the initial public snapshot

These are hashes of the **published snapshot contents**, not reconstructed hashes of each earlier private commit. Some files evolved between freezes; use the private commit IDs above to identify the historical versions. Paths below are relative to the public repository root.

| File | SHA-256 |
|---|---|
| `.gitignore` | `0ba578c85af500da111f09dd3f136d1791435f1b641f24f69acd657771bfc62e` |
| `canon-test/cad-adapter-dev-results.md` | `ffaa66279c7a66ce518d24efd844c801ebb50007da6c7116aeb50271bb1cda03` |
| `canon-test/cad-adapter-v1.md` | `4ac961860fe85d68568879503ff5c7c20fb81c703614ff279fd4a6dde0b8b875` |
| `src/lib.rs` | `984a3c00e903a90239539aa8fdb2b1c88e9b74d698d3ca89b6ce1752146d94f6` |
| `src/dxf_adapter.rs` | `fc39812ef1ef59c6af6f38d4fa2dc2df1265511f68c7b6b67e87b231b01c8cb0` |
| `src/bin/cad_adapter_dev.rs` | `f070a8d74f8c25db6dc47625c98d09f4efb10295d772fbc4b921e3f924041fe4` |
| `canon-test/cad-mapping-pre-split-review.md` | `adf15e0591926b0b46eb132c4fd971a2c52a1dd5026b94694c4dfbbd6f617f61` |
| `src/bin/cad_geometry_keys.rs` | `a5c0640ea6b5e1b44c0a9b3959689e1de4bddbb88432392bcb1355e36309fdd0` |
| `canon-test/cad-a3-v2-dev-incremental.tsv` | `727bbdacd72693ae46be3a58d87eb57ae1b1701b669936fdb74e17a53ed2ac68` |
| `canon-test/cad-a3-v2-full-dev.md` | `978ed8d7bae5fd7fc0750aef5773d46c198f91634516d6109f3203e52744ccff` |
| `canon-test/cad-a3-v2-vs-c2-variants-dev.md` | `0a922a77b099a515b6a70377d3b4854afde9644662ffdfc9ce2f094e62860a78` |
| `canon-test/cad-adapter-v2-freeze.md` | `47e288cb74599d12fb50439bbf712be60e16a82b359e2471d24c3688479959a6` |
| `src/bin/cad_a3_v2_dev.rs` | `940583b513407cfabde6915a0301341e0fc3047defaecba095ae5c3e5bd7737c` |
| `canon-test/make_cad_corpus.py` | `c87a77667f1aed3da0c99ae44a4c93ddf123f7f9b9cede7ab077b7aa6c1d5f4f` |
| `canon-test/report_a3_v2_variants.py` | `dcbd436702d56cba131ae98039994b770f3c0d232e91f3ef28b4a58076d0ba01` |
| `canon-test/run_seeded_v2_test_once.py` | `79201b4d065a3f24214eeb90753797f10c4903a5ed5a805aaa90d544e9f358f1` |
| `canon-test/verify_corpus.py` | `7f7d8c67494b240d1353da0d37182c36409a9fe9ee7ce339fc5b7c1153248b3e` |
| `canon-test/c2-base-selection-freeze.md` | `61d9c89df57557ac154cfdf24fe9b6e8daf972edfdd3bb4be178c60d94aa5653` |
| `canon-test/cad-tierb-posed-sibling-census.md` | `6c0832f50501ae9d162325a5e7fc92078070e5a414678f8049fec47ed8e0432a` |
| `canon-test/run_c2_base_selection.py` | `f89a88c560dd240b434f6bc3ff99314b5c123d9759f94a696a0bfb97c65552ee` |
| `src/bin/cad_file_keys.rs` | `12eb0a47f895aee1a19bffe1ee1fd0c3c57554b45ef088a6a15265c295347974` |
| `canon-test/c2-hybrid-v4-freeze.md` | `4923f4d3c884cbe92b5fa05d2a291b3e86161ea46e44a0a5468c8cdd237fe3c5` |
| `canon-test/run_c2_hybrid_v4.py` | `2e04b3116304bb4e40428cccd403a589b568f4759552981a2f5ebc319e8c691f` |

### Regenerate and check the generated test halves

Run each block from the public repository root. Each command writes to a new temporary directory and leaves the published reports untouched. The comparison hashes the raw `manifest.json` bytes; reformatting that file changes its hash. `verify_corpus.py` separately checks all listed file bytes, sizes, hashes, and declared variant relations.

```sh
out=$(mktemp -d)
python3 canon-test/make_cad_corpus.py "$out" --corpus-id darkrock-cad-dxf-v2 --split test
python3 canon-test/verify_corpus.py "$out"
printf '%s  %s\n' d670bc67a937fc55a21ae69baa3fdb15079fbd0771d58710d4add0d99e8a325c "$out/manifest.json" | shasum -a 256 -c -
```

The v2 value is from `canon-test/cad-v2-seeded-test-report.md` (commit `8cdbbf1`). The earlier `canon-test/cad-v2-fresh-test-report.md` records `4a1d3ed4f27c95e441cdc1060a9be8e111413cbf9f44341a792c48bdcee31611` from the pre-update generator; that comparison is superseded, not silently discarded.

```sh
out=$(mktemp -d)
python3 canon-test/make_cad_corpus.py "$out" --corpus-id darkrock-cad-dxf-v3 --split test
python3 canon-test/verify_corpus.py "$out"
printf '%s  %s\n' 5fa962fc3843e2f6e4746d9fa5954147190403af2b40fc9ccc15d100f42184b3 "$out/manifest.json" | shasum -a 256 -c -
```

The v3 value is from `canon-test/c2-base-selection-v3-report.md` (commit `78d2454`).

```sh
out=$(mktemp -d)
python3 canon-test/make_cad_corpus.py "$out" --corpus-id darkrock-cad-dxf-v4 --split test
python3 canon-test/verify_corpus.py "$out"
printf '%s  %s\n' de5ffbdbcbfa38b02212c75b9a79175c3389734059ceb1de1c7ffad21c7ad1e6 "$out/manifest.json" | shasum -a 256 -c -
```

The v4 value is from `canon-test/c2-hybrid-v4-report.md` (commit `06cffc8`). These regenerations reproduce the generated corpus manifests, not the full storage benchmark or the third-party Tier B drawings.

The private Git history is available on request, subject to review for third-party drawing material and personal paths before sharing.
