# A3 v2 frozen rules for the fresh Tier A run

**Freeze point:** the commit containing this file, `src/bin/cad_a3_v2_dev.rs`, and the paired development reports. The held-out corpus ID is `darkrock-cad-dxf-v2`; it must be generated only after this commit. The old 132-file test split is not the fresh holdout.

**Joint generator freeze:** a later commit freezes this unchanged adapter together with the updated `make_cad_corpus.py`, `verify_corpus.py`, and `run_seeded_v2_test_once.py`. It adds seed-driven title-block/stair parameters for non-v1 corpus IDs and `--split test`. The revised generator regenerated all 176 v1 files and the 132-file v1 test subset byte-identically before the joint commit. The earlier v2 run, made before these generator rules, is superseded for this comparison; its artifacts remain as provenance.

## Closed reference and edit list

The raw ASCII DXF scanner and geometry candidate key are the frozen v1 implementation in `src/dxf_adapter.rs`. A key proposes candidates only; it never authorizes a merge. A3 v2 may emit only these operations, in this order of preference:

1. **Whole-file exact reference:** SHA-256 match **and** byte comparison to an earlier file. The duplicate is not ingested into the entity store; its file record points backward to the prior exact file.
2. **Exact contiguous-run reference:** at least two adjacent matched entities map to adjacent byte ranges in the same stored object and their reconstructed bytes and hashes match exactly.
3. **`NUMBER_SPELLING_UNDO`:** the entity's group-code lines and all nonnumeric value lines match the candidate. Changed numeric value lines parse to the same finite `f64` value. The edit stores each original target line and its position; replay must reproduce the target length, SHA-256, and bytes. This rule may preserve changes such as signed-zero spelling but may not discard the original token.
4. **Generic verified entity edit:** retain the candidate locator, common prefix and suffix lengths, and literal middle bytes. Replay must reproduce the target length, SHA-256, and bytes.
5. **Literal object:** store bytes when no verified reference or edit is selected. Unknown DXF content remains literal.

The per-file operation stream, including edit data, is compressed with zstd level 3 only when smaller. No pose normalization, entity reordering, fuzzy merge, geometry rewrite, or handle rewrite is permitted. Binary DXF fails closed.

## Index views and durability

- **Protected:** serialize the complete geometry-key/hash/locator index and protect it with the same RedTail-X variable-tail 4+2 code as the object graph. Validate every locator against decoded object bytes.
- **Derived:** omit that candidate index from durable bytes. Restore every original DXF using only protected objects, references, edits, and manifests; re-ingest the unique restored files to rebuild the index. The key count and each file's lookup, verified, exact, and edited-match counts must match the frozen store. An index failure invalidates the Derived cell.

Both views count the whole serialized ledger, 4+2 shares, the protected stripe-manifest catalog, and six charged bootstrap copies under the same accounting policy. They are separately encoded and checked under all 21 one/two-share-loss patterns. A3 v2 Protected and Derived are different physical-byte results, not an index subtraction estimate.

## Development pre-freeze result

On 44 development DXFs in Z mode: **Protected 7,237,992 bytes; Derived 3,257,640 bytes; C2 2,286,900 bytes**. Exact restore passed. The owner chose to proceed to one fresh held-out run despite this loss. The grouped variant attribution is `cad-a3-v2-vs-c2-variants-dev.md`.
