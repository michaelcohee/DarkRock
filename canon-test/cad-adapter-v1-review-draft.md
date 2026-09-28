# CAD adapter v1 — review draft

**Status:** Proposed mapping for Michael's review, **not frozen and not implemented**. This draft does not change RedTail-X or the Step 6/7 results. The test split must remain unopened until an implemented adapter passes dev tests and this mapping is accepted and committed as `cad-adapter-v1.md`.

## Claim and restore target

The adapter's input and exact restore target are the **frozen converted DXF bytes**. It makes no claim to regenerate source DWG bytes. `src/polynomial.rs::canonicalize` is an experimental candidate-grouping function, not an erasure code and not an exact byte serializer. A candidate match may suggest reuse; it never authorizes dropping original bytes by itself.

The required invariant for every file `x` is:

`decode(stored_objects(x), references(x), undo(x)) == x` **byte-for-byte**, followed by matching SHA-256. If the required undo makes a candidate more expensive than storing its raw bytes, store the raw bytes. No approximate or fuzzy match may bypass this rule.

## 1. Parse raw tags and preserve byte spans

Read the DXF as bytes. Identify each ASCII group-code line and its following value line without normalizing whitespace, line endings, number spelling, encoding, or order. Record the exact byte offset/length of each tag pair and section boundary. Reject malformed pairs rather than repairing them. `ezdxf` may be used in a *separate check* but its audited document model is never the source for byte restoration.

Partition the stream into ordered spans: preamble/header and other sections, entity records in `ENTITIES`, and trailer. An entity starts at a group-code `0` tag and ends immediately before the next group-code `0` tag; the scanner must preserve any whitespace and line-ending bytes at the boundary. File order is carried by references to these spans. Reconstruct by concatenating decoded spans in the original order. A round trip through the scanner with **all spans stored raw** must first equal the input exactly.

An entity is eligible for polynomial indexing only if its type and *every tag in its span* match the v1 whitelist. Proposed v1 eligible types are `LINE`, `ARC`, `CIRCLE`, `LWPOLYLINE`, and `TEXT`; the implementation must write down its exact allowed group-code list for each type before testing. Ordinary handles and owner links (such as group codes `5` and `330`) may be whitelisted, but stay byte-exact in the stored base/undo and never become polynomial coefficients. Extension dictionaries, reactors, XDATA, proxy data, unusual subclass records, duplicate tags that violate the declared grammar, or any unknown tag make the **entire entity a raw pass-through span**. `TABLECONTENT`, `ACAD_PROXY_OBJECT`, `ACAD_PROXY_ENTITY`, and `XRECORD` remain raw. This keeps the 11 Step 5 audit-flagged files in scope without relying on an `ezdxf` repair.

Report for every file: total bytes, eligible-entity bytes, raw pass-through bytes, number of eligible entities, and number rejected by each reason. Raw pass-through bytes are stored, deduplicated exactly if possible, and **charged as payload**.

## 2. Proposed polynomial term mapping

Use one polynomial candidate key **per eligible entity**, not one polynomial for the whole drawing. For a selected finite numeric geometry tag, emit:

`Term { coefficient: parsed_f64_value, exponents: [entity_type_id, group_code, occurrence_index] }`

The `entity_type_id` is a fixed integer from the v1 whitelist. `group_code` is the numeric DXF group code. `occurrence_index` is the zero-based count of that group code within this entity. Thus every geometry value has a unique exponent vector within its entity; distinct values are never intentionally summed into one term. Candidate identity also includes a fixed schema/version tag and the entity type. The exact list of geometry group codes, and whether nongeometry tags participate in the candidate key, is an **owner design decision before freeze**. Parsing to `f64` or producing the canonical string is only for candidate search; it is never the only copy of a numeric value.

Entity order across the file does not appear in the per-entity key. The ordered file reference list preserves it. Repeated identical entities remain repeated references, not a single entity silently substituted for two. A polynomial-key collision merely offers a candidate base; exact restoration decides whether the candidate can be used safely.

## 3. Stored object and undo rule

The first occurrence of an eligible entity in a candidate bucket is stored as **the exact raw entity span**, with its key and hash. An exact raw-byte repeat references that span with zero undo bytes. For a later candidate with different raw bytes, compare two representations:

1. Standalone raw entity span (then the common mode-N or mode-Z representation selector).
2. Reference to an already stored raw base plus a deterministic byte edit record. A minimal v1 edit record can be `(prefix_len, suffix_len, middle_bytes)`, where `target = base[:prefix_len] + middle_bytes + base[len(base)-suffix_len:]`. Choose the longest common prefix, then longest nonoverlapping suffix. Include lengths, base ID, representation tag, and all encoded edit bytes in the ledger.

Choose the lower **total protected** cost under the common packing and metadata model; verify the chosen decoder produces the exact target span before accepting it. This draft deliberately uses the polynomial to *find candidate bases*, while raw base plus counted undo provides exactness. To attribute a win to the polynomial rather than ordinary entity-level dedup/delta, add an **entity-dedup control without polynomial candidate selection**. If A3 only matches that control, report that the polynomial supplied no measured advantage.

## 4. Hazards in the current `canonicalize`

| Hazard | Required handling |
|---|---|
| Like terms are summed | Unique `[type, code, occurrence]` exponents per selected numeric tag; reject a duplicate exponent if the scanner did not deliberately assign distinct occurrences. |
| Terms with `abs(coefficient) < 1e-9` disappear | Never derive stored bytes from the filtered terms; preserve the original raw span or exact undo. Test `0`, `1e-10`, `-1e-10`, and signed zero spellings. |
| Coefficients are formatted to 12 exponential digits | Preserve original numeric spelling and all higher precision in the raw base/undo. Test two values that format to one canonical coefficient. |
| `f64` changes spelling and may round values | Never compare semantic equality as byte equality; verify decoded span bytes and SHA-256. Test `12.5`, `12.500000`, scientific notation, very large/small values, and invalid/nonfinite input. |

The candidate key's SHA-256 is an index lookup only. Every hit must be confirmed against the stored candidate and exact decoded bytes. Zero-coefficient canonicalization alone is **not** an exact storage codec.

## 5. Accounting and tests before freezing

Count raw pass-through payload, eligible base payload, every undo/edit byte, ordered references, key/index entries, tags, manifests, and any per-corpus dictionary. Apply the same RedTail-X 4+2 layer and mode-N/mode-Z rules as A1/C1. Validate the full file after object decoding, after each one-share loss, and after each two-share loss. Record entity coverage and marginal bytes per file/family, including the 11 audit-flagged DXFs.

Build and test on Tier A **dev** only. Required tests include raw-scanner identity; all supported entity types; unsupported/proxy raw fallback; entity reorder; duplicate entities; numeric spelling; near-zero and rounded coefficients; deliberate polynomial-key collisions; corrupted/missing undo; and a byte-exact restore under all 21 share-loss patterns. Write down any changes to this mapping, commit the final `cad-adapter-v1.md` and code, and only then run the held-out Tier A test split once under its v1.1 rules. Tier B is reported separately on the frozen 47 DXFs.

For Tier B, the frozen criterion is **A3 strictly below both C1 and C2** in comparable mode Z. C2 is 18,502,067 protected bytes, so a strict byte win is **at most 18,502,066**. A 2% margin against C2 would require **at most 18,132,025** bytes, but that is a *new optional stretch target*, not the existing Tier B criterion. The v1.1 2% condition applies to the Tier A public-claim comparison with C1.

## Decision to settle before implementation

Michael should confirm or replace the proposed per-entity numeric-term mapping and the eligible group-code whitelist. The exact restore and raw pass-through rules above can stay even if the polynomial mapping changes. This draft is a review artifact, not evidence that A3 saves storage or that the 11 flagged DXFs preserve all source-DWG CAD semantics.
