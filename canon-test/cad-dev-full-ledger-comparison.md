# CAD dev split: complete protected-byte ledger

**Scope:** 44 Tier A development DXFs, 21,601,154 original bytes, sorted by manifest path. No Tier A test file was read. The original 132-file test set had already been read by an older exploratory control, so a future unseen-data claim requires a fresh split.

## Method

Each arm serializes its actual selected payloads, ordered file references, object lengths and SHA-256 hashes, hash index, and file manifests into one byte stream. A3 also serializes every literal run, every prefix/suffix edit and middle byte, and the complete current polynomial-key-to-candidate/hash index. The entire stream is encoded with RedTail-X 4+2. Its per-stripe manifest catalog is separately encoded with 4+2. The finite bootstrap is six physical copies of each catalog-stripe manifest (240 bytes) plus a 32-byte catalog digest: **1,632 bytes** here. Every data and catalog stripe passed all 21 one/two-share loss patterns, and every file was restored byte-for-byte from the serialized stream. This is a local physical-byte model, not disk allocation or a network durability result. The root digest still needs an external trust mechanism in a deployed system.

Mode N stores selected objects raw. Mode Z uses the production entropy/periodicity zstd selector for A1, C1, and A3 literals. C2 Z chooses real zstd level-3 standalone or `--patch-from` against the first family file. **C2 N cannot use a zstd patch by definition**; its N row is exact whole-file dedup with raw unique files. C1 uses pinned `fastcdc` 1.7.0 at 4/16/64 KiB; A1 uses fixed 256 KiB chunks. All exact dedup hits confirm bytes after SHA-256. Metadata layouts for all four arms are serialized and counted, rather than the older estimated 1.5× constants.

| Arm | Mode N physical bytes | Mode Z physical bytes |
|---|---:|---:|
| A1: fixed-chunk exact dedup | **25,962,840** | **3,612,642** |
| C1: FastCDC exact dedup | **21,523,590** | **3,150,030** |
| C2: family patch control | **29,550,450** | **2,286,900** |
| A3 v1: entity polynomial-key lookup + literal/edits | **26,237,322** | **15,765,756** |

**Verdict:** A3 v1 loses to A1 and C1 in both modes and to C2 in mode Z. Its small N advantage over raw C2 is irrelevant to the stronger N controls. The 44-file dev result cannot support a storage-saving claim for A3.

## A3 ledger and lookup behavior

| Component of A3 serialized ledger | Mode N bytes | Mode Z bytes |
|---|---:|---:|
| Literal objects, including encoding tags, lengths and hashes | 8,174,164 | 1,194,558 |
| Literal edit middle bytes | 3,726,003 | 3,726,003 |
| Ordered references and edit descriptors | 2,931,150 | 2,931,150 |
| Polynomial key/hash index | 2,653,088 | 2,653,088 |
| File manifests + framing | 1,951 | 1,951 |
| **Serialized ledger** | **17,486,356** | **10,506,750** |

In mode Z, 4+2 data shards consume 15,760,128 bytes; the protected manifest catalog consumes 3,996 bytes; the charged bootstrap consumes 1,632 bytes. Total: **15,765,756**. [The A3 per-file ledger](./cad-a3-dev-ledger.md) reports each file's incremental physical bytes and parsed/raw coverage. [The control JSON](./cad-dev-full-controls.json) reports the same per-file incremental totals for A1, C1 and C2.

There were **34,469 key lookup hits** and **34,469 verified matches**: 24,575 exact-hash references and 9,894 uses of the only allowed v1 edit, `(prefix_len, suffix_len, literal_middle)`. Every accepted edit decoded to exact original bytes and SHA-256. This 100% selected-hit verification rate does **not** imply useful compression; references and index entries cost more than the bytes they avoided on this corpus. Raw pass-through was **38,462 / 21,601,154 bytes (0.1781%)**. No other edit type fired.

## Named v2 edit proposals — not applied to the table

1. **`EDIT_STREAM_ZSTD_PER_FILE`**: collect the literal middle bytes of matched-entity edits into one ordered, framed stream per file. Choose raw or zstd level 3 only after byte-exact decode verification; store the mode, uncompressed length, SHA-256, compressed length, byte offsets for every edit, and all references in the protected ledger. The dev diagnostic compressed the **middle bytes alone** from 3,726,003 to 522,147 bytes across eight files. That excludes the new framing/offset costs, so it is not an A3 result.
2. **`NUMBER_SPELLING_UNDO`**: for a whitelisted numeric DXF tag whose value is equal under an **exact decimal** comparison, reference one canonical numeric token and store the original token's complete byte spelling (including sign, decimal digits, exponent, whitespace and line ending) as a counted undo record. Preserve group-code and occurrence position. Do not classify an `f64`-rounded collision or a near-zero dropped term as a spelling change; those remain literal or use the existing verified edit. Verify the full entity and file bytes and SHA-256 before accepting this edit. Test `12.5`/`12.500000`, exponent variants, signed zero, near-zero values, and >12-digit differences.

These additions need a new versioned edit grammar, serialized decoder, corruption tests, full 4+2 ledger, and dev comparison before acceptance. **They cannot by themselves close the observed gap:** A3's references plus key/hash index already total **5,584,238 pre-code bytes**, which is greater than the entire C2 Z protected result. Even if edit middle bytes compressed to the diagnostic 522,147 without any new overhead, the remaining A3 stream would be 7,302,894 bytes and its data shares alone about 10,954,344 bytes. Reducing per-entity reference/index granularity or choosing a whole-file patch fallback is a separate required design question.

## Reproduction and limits

`cargo run --release --bin cad_a3_dev_ledger` produces the A3 report. With `fastcdc` 1.7.0 available to Python, `python3 canon-test/run_dev_full_controls.py` produces the control JSON. `canon_full_ledger_measure` verifies the shared protection policy for controls. All commands read only `canon-test/cad-corpus-v1/dev/`; outputs are the two reports.

The root descriptor and digest are charged but replicated, while the ledger and manifest catalog use 4+2. This finite bootstrap is not a deployed authentication scheme. No filesystem allocation, encryption, network repair, or multi-node metadata quorum was measured. This dev result is intentionally a rejection of v1's storage-efficiency claim, not a published general benchmark.
