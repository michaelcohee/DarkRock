# CAD adapter v1 — frozen development mapping

**Status:** Mapping frozen after Tier A development tests on 2026-09-27. This freezes the adapter rules, not an A3 benchmark result or production storage format. The 132-file held-out Tier A test split was not opened. The exact restore target is the converted DXF byte stream, never the source DWG.

## Invariant and scope

For every DXF byte string `x`, `restore(ingest(x)) == x` and the restored SHA-256 must match. The polynomial key is only a candidate lookup. `src/polynomial.rs::canonicalize` drops near-zero terms and rounds coefficients to 12 exponential digits; therefore key equality is **not** byte equality and cannot authorize deletion by itself.

Parse raw group-code/value-line pairs while preserving the original byte spans, line endings, order, numeric spellings, and unknown tags. Scan both `BLOCKS` and `ENTITIES`. In `BLOCKS`, `BLOCK` and `ENDBLK` remain raw; supported entities between them can be indexed. Treat `POLYLINE` followed by its `VERTEX` records and `SEQEND` as one composite entity. A missing `SEQEND` makes the candidate raw.

Eligible v1 types: `LINE`, `ARC`, `CIRCLE`, `LWPOLYLINE`, composite `POLYLINE`, `POINT`, `TEXT`, `MTEXT`, `INSERT`, and `ELLIPSE`. `HATCH`, `DIMENSION`, `SPLINE`, proxies, XDATA, reactors, extension dictionaries, malformed tags, nonfinite geometry, and unknown types pass through as raw bytes. The current candidate parser treats numeric DXF codes 10–18, 20–28, 30–38, 40–48, 50–58, 210–218, 220–228, and 230–238 as geometry; other allowed standard metadata is not part of the key. A future CAD semantic review may narrow this code list; changes require a new mapping version.

For each finite numeric geometry tag, emit `Term { coefficient: parsed_f64, exponents: [type_id, child_index, group_code, occurrence_index] }`. `child_index` changes at each `VERTEX`. The candidate key has the fixed `cad-v1` prefix, entity type ID, and SHA-256 of the canonical form. Handles, owner, layer, color, and original text/spelling remain in literal bytes or exact edit data, never in the key. Text content is likewise retained as bytes; its absence from the key may create extra candidates, not false restores.

## Literal runs and matches

Unmatched eligible entities and raw spans are coalesced in file order into **literal-run objects**. Each run is compressed once with zstd level 3 when that is smaller; otherwise it stays raw. Only a matched eligible entity receives its own reference and edit. A base locator names a byte range inside a previously stored literal run. For a match, store `(base, prefix_len, suffix_len, middle_bytes)` such that `target = base[:prefix_len] + middle + base[len(base)-suffix_len:]`. The current prototype uses the first eight candidate bases per key and chooses an edit only when its middle plus a 16-byte local decision allowance is smaller than the raw entity. It verifies every match's exact bytes and SHA-256. Restoration also verifies every full file's length and SHA-256.

The in-memory prototype retains raw object caches to speed candidate search and validate decoded blocks. It does **not** yet implement a persisted/authenticated object index, reference/undo serialization, deletion, or a complete protected-byte ledger. `encoded literal bytes + edit middle bytes` is a diagnostic and excludes reference/index/manifest costs. No space win against A1/C1/C2 is claimed from that number.

## Development evidence

The 44 extracted Tier A development DXFs total 21,601,154 bytes. All restored byte-for-byte. The scanner marked 21,562,692 bytes as eligible entity spans and 38,462 bytes raw; 13,439,583 eligible bytes used matched references. The per-file parsed/raw/matched coverage is in [cad-adapter-dev-results.md](./cad-adapter-dev-results.md). This is **syntactic byte coverage**, not evidence of CAD fidelity or savings.

The seven focused unit tests cover composite `POLYLINE` and `BLOCKS`, the full type whitelist, unsupported raw fallback, metadata exclusion, near-zero/rounding key collision with exact restore, damaged edit rejection, and CRLF/malformed input. The development runner verified all 44 full-file hashes. It also encoded 257 compressed literal-run payload stripes with RedTail-X 4+2 and reconstructed each under all 15 two-share-loss pairs. `storage.rs` separately tests the six one-share losses and variable-final-share boundaries. The full serialized A3 object/index/undo stream has **not** yet been exercised through share loss because that durable format is not implemented.

## Next measurement gate

To claim an A3 result, serialize the entire object/index/reference/undo graph, authenticate metadata, apply the same mode and 4+2 accounting as the baselines, then restore every DXF after all one- and two-share loss patterns. Compare protected bytes with C1 and C2 on the **same** converted DXFs. Tier B remains separate; C2's frozen value is 18,502,067 protected bytes, so a strict byte win requires at most 18,502,066. A 2% margin would require at most 18,132,025 but is a stretch target, not the frozen Tier B rule.
