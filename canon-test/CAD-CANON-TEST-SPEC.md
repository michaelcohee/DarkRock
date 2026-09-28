# DarkRock CAD Canonizer Test — Rules and Win Criteria

**Spec version:** v1.1 (pre-registered 2026-09-27; v1.1 adds the Tier B DWG rules; fix every rule below *before* the first run on the test split)
**Scope line (put this first in every report):** *Single volume, one Apple M1 Mac, CPU only. No network, no node loss, no Storj, no chain.*
**Owner:** Michael Cohee  **Builder/runner:** ChatGPT (Codex) in the DarkRock repo  **Reviewer:** Grok

---

## 1. The question

On CAD-only files, does the **floating-point polynomial canonizer** (`src/polynomial.rs` plus a DXF adapter) cut protected storage bytes compared with plain exact-byte dedup, when:

- every file restores **bit-exact**, and
- every undo byte and metadata byte is counted, and
- the same RedTail-X 4+2 layer protects every arm?

This is a **restore-and-bytes** experiment. How the polynomial looks doesn't matter.

## 2. What is frozen (identical across all arms)

| Item | Value |
|---|---|
| Erasure layer | RedTail-X `src/storage.rs::encode_stripe` / `reconstruct`, `reed-solomon-erasure = "6"`, 4 data + 2 parity, GF(2⁸) |
| Share length | variable final share, `s = ceil(t / 4)`, full stripe = 1 MiB (4 × 256 KiB) |
| SIMD | either on or off for **all** arms (record which); it does not change byte counts |
| Object identity | SHA-256; every hash hit confirmed by full byte comparison |
| Packing | every arm appends its unique payload objects, in first-seen order, to one packed stream, cut into 1 MiB stripes (objects may span stripes). Same rule for all arms |
| Metadata model | same constants as `canon-mock-dark-1gb-m1.md`: 232 B per stripe manifest, 36 B per unique-object index entry, 8 B per object reference, 8 B per file, 1 B representation tag per unique object, 4 B per delta/base ID. Metadata charged at **1.5×** |
| Disk / host | same Mac, same volume, release build, rustup toolchain from README |
| Repo state | record `git rev-parse HEAD` at freeze; no code changes after freeze except bug fixes that are logged and re-run on **all** arms |

## 3. Corpus

### Tier A (required): `darkrock-cad-dxf-v1` (synthetic, ground truth known)

- **Format:** AutoCAD R12 ASCII DXF (`AC1009`) only. No PDFs, no DWG, no images.
- **Size:** 176 files, 107,435,636 bytes. 16 drawing families × 11 variants.
- **Generator:** `make_cad_corpus.py`, which is deterministic (same script → same bytes; check `SHA256SUMS`). `verify_corpus.py` re-checks hashes and every ground-truth relation. It must print `0 errors` before any run.
- **Families:** title blocks (ANSI D, ISO A1), flange plates, spur gears, office floor plan, site contours, PCB drill map, stair section, hex-nut array, two free-style sketches, door schedule, bracket parts, parking layout, electrical panels, truss elevations.

| Split | Families | Files | Bytes | Use |
|---|---|---:|---:|---|
| **dev** | f00, f03, f08, f10 | 44 | 21,601,154 | Build and tune the adapter here only |
| **test** | the other 12 | 132 | 85,834,482 | Untouched until freeze; **the win is decided here only** |

**Variants of each base drawing (ground truth in `manifest.csv`):**

| Variant | Relation | What changed | Expectation |
|---|---|---|---|
| v00_base | base | — | store once |
| v01_exact_copy | identical | nothing (new file name) | all arms must dedup |
| v02_resave_timestamp | content_equal | `$TDUPDATE` only (same length) | canonizer should collapse; A1 already catches most chunks |
| v03_entity_reorder | content_equal | entity order shuffled | canonizer should collapse |
| v04_number_format | content_equal | `12.5` → `12.500000` (same values, byte offsets shift) | canonizer should collapse |
| v05_layer_rename | renamed | layer names changed | may collapse **with counted undo** |
| v06_translate | congruent | whole drawing moved | may collapse **with counted undo** |
| v07_rotate90 | congruent | rotated 90° | may collapse **with counted undo** |
| v08_units_mm | scaled | all lengths × 25.4 | may collapse **with counted undo** |
| v09_tiny_float_diff | tiny_diff | a few coordinates differ in the 6th decimal | **trap:** must never merge free |
| v10_one_entity_edit | design_change | one entity moved 5 units | **trap:** must never merge free |

### Tier B (reported separately): real-world DWG in `canon-test/DWG/`

Michael's folder: 20 files, 19,959,938 bytes. It holds manufacturer door, sidelight and patio-door details in near-duplicate families, plus unrelated hospital furniture and a shrub block. `tierB-dwg-profile.md` has the measured profile. Rules:

1. **Never merge Tier B into the Tier A table.** It has no ground truth, so it can support a claim but **can't decide the win**.
2. **Keep the DWG files out of any public repo.** They are manufacturers' drawings; add `canon-test/DWG/` to `.gitignore`.
3. **DWG is binary, so the canonizer needs DXF.** Convert every DWG once with one pinned converter (ODA File Converter or LibreDWG `dwg2dxf`; record the tool, version and output DXF version). **The restore target for arms A2 and A3 is the converted DXF, not the original DWG**, because no tool can rebuild identical DWG bytes from drawing content. State this in the first line of the Tier B table.
4. **Also report a DWG-bytes reference row:** A1 and C1 on the original `.dwg` files, with no conversion.
5. **Treat `.zip` files as opaque bytes in every arm.** Report separately that `Hospital-equipment.zip` contains a byte-identical copy of `MOBILIARIO HOSPITAL.dwg`. Catching that would be container unpacking, not canonization.
6. **Add control C2 (delta) for Tier B:** within each family, store the first file raw and later files as `zstd --patch-from=<first file>` deltas. Count the delta bytes plus the base reference. On these files, compression across files finds 24–43% redundancy that chunk dedup misses. So delta compression, not dedup, is the baseline the canonizer really has to beat here.
7. **Tier B supports a claim only if** A3 beats both C1 and C2 on the converted DXF, with G1 (byte-exact restore of every DXF) passing.

## 4. The DXF adapter (must be written down before the freeze)

`polynomial.rs::canonicalize` takes `Term { coefficient: f64, exponents: Vec<i64> }` and does not read files. The adapter that maps DXF → terms **is part of the claim**. Commit `cad-adapter-v1.md` stating:

1. How each DXF entity and table entry becomes terms (what the exponent vector encodes, what the coefficient holds).
2. The dedup unit: whole file, section, or entity.
3. The undo record `u_x`: everything the decoder needs to rebuild the exact original bytes from the canonical objects. This includes entity order, number spelling, line endings, comments, header values, and any rounded digits.
4. The decoder: `canonical objects + u_x → original bytes`.

**Known hazards in the current `canonicalize` (the adapter must handle each one, and the gates will catch it if it doesn't):**

- **It adds up like terms.** Two different values given the same exponent vector get summed, which destroys data. Exponent vectors must be unique per stored value, or the loss goes into `u_x`.
- **It drops `|c| < 1e-9`.** A real coordinate of 0.0000001 would vanish unless it's kept in `u_x`.
- **It keeps about 13 significant digits** (`{:.12e}`). Any digits beyond that must go into `u_x`.
- **f64 doesn't preserve spelling.** `12.5` and `12.500000` parse to the same value, so the original spelling must go into `u_x`.

## 5. Arms

All arms process the same files in the same order (sorted by manifest path).

| Arm | Pipeline |
|---|---|
| **A1** | Raw bytes → fixed 256 KiB chunk exact dedup → pack → RedTail-X 4+2 |
| **A2** | Cheap DXF normalize → exact dedup → pack → 4+2. *Cheap normalize* = rewrite numbers in shortest round-trip form, normalize line endings and whitespace, move header timestamps (`$TDCREATE`, `$TDUPDATE`, `$TDINDWG`, `$TDUSRTIMER`) into `u_x`. No entity reordering and no polynomial. `u_x` is counted |
| **A3** | DXF adapter → `polynomial.rs::canonicalize` → exact dedup on canonical objects → pack (objects + `u_x`) → 4+2 |
| **C1** (control) | Raw → content-defined chunking dedup (FastCDC; min 4 KiB, avg 16 KiB, max 64 KiB; fixed before the run) → pack → 4+2 |

**Compression modes:** run every arm twice.

- **mode N:** no compression.
- **mode Z:** the existing `representation.rs` zstd selector applied to each unique object before packing.

Comparisons are always within the same mode.

## 6. Hard gates (any failure = no result can be called a win)

| Gate | Rule |
|---|---|
| **G1 Byte identity** | For every file in every arm and mode, `decode(store(x)) == x`, checked by SHA-256 **and** byte compare. Must be 176/176 in each run. One broken rebuild zeros the claim |
| **G2 Erasure** | Every stripe in every arm and mode reconstructs exactly under all **15** two-share loss patterns and all **6** one-share losses |
| **G3 No free merge** | No two byte-different files may map to the same stored objects with **zero** undo bytes. For the v09 and v10 traps, report the undo bytes; they must be > 0 and must restore exactly |
| **G4 Hash confirmation** | Every hash hit (raw chunks and canonical objects) is confirmed by byte compare. Report the count of confirmed hits and of mismatches (must be 0) |
| **G5 Held-out discipline** | Adapter built on dev only. Freeze commit recorded before the first test-split run. Every test-split run is reported, including losing runs. Retuning after seeing test results requires a fresh test set: regenerate with `CORPUS_ID = "darkrock-cad-dxf-v2"` |
| **G6 Accounting completeness** | Anything the decoder reads is counted: payload, `u_x`, references, index entries, manifests. Only compiled code and fixed constants are free. Any per-corpus dictionary or side table is counted |

## 7. Win criteria (test split only, decided in advance)

**Primary win (internal).** A3 counts as a canonizer win only if **all** of these hold:

1. All gates G1–G6 pass.
2. **A3 total protected bytes ≤ 0.95 × A1**, a saving of at least 5% after 4+2, **in both mode N and mode Z**.
3. **A3 ≤ 0.98 × A2**, at least 2% beyond cheap normalization in both modes. Otherwise the gain comes from text cleanup, not the canonizer.
4. **Content-equal recall:** summed over the v03 and v04 variants, A3's marginal stored bytes (bytes stored for the variant beyond its base) are **≤ 10% of A1's**.
5. **Named examples:** at least 2 near-duplicate pairs that A1 stored twice and A3 stored once. Give file names, A1 bytes, A3 bytes and A3 undo bytes.

**Public claim (what goes to Grok or the blog).** Primary win **and** **A3 ≤ 0.98 × C1 in mode Z**, meaning it beats content-defined chunking with compression by at least 2%.

**Not a win, even if the numbers look good:**

- A3 faster but bigger, which is the entropy-gate story again.
- Lower entropy offered as proof that files were "the same".
- Savings that exist only on the `identical` class, since A1 already gets those.
- Any value dropped (such as `|c| < 1e-9`) without its restore bytes counted.
- Tier B or PDFs mixed into the Tier A table.
- Any network, Storj or chain sentence in the results table.

**Why these thresholds:** on the 1 GB mixed folder, exact dedup saved 4.65% and the delta mock added 0.09%. A canonizer needs a margin about as large as the whole dedup win (5%) to matter after 4+2. This corpus is variant-heavy by design (4 of 11 variants are byte-identical or content-equal), so a working canonizer should clear 5% easily. **Clearing it here doesn't show real-world savings; that needs Tier B.**

## 8. Report template (`cad-canon-test-m1.md`)

```
Single volume, one Apple M1 Mac, CPU only. No network, no node loss.
Spec: CAD-CANON-TEST-SPEC v1.1 | corpus: darkrock-cad-dxf-v1 (verify_corpus: 0 errors)
Freeze commit: <hash> | adapter: cad-adapter-v1.md | rs-simd: on/off | date:
```

**Table 1: Totals, test split (132 files, 85,834,482 bytes)**, one table per mode

| Arm | Unique payload before RS | of which u_x | Share bytes (actual) | Modeled metadata ×1.5 | **Total protected** | vs A1 | vs A2 | vs C1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| A1 | | 0 | | | | — | | |
| A2 | | | | | | | — | |
| A3 | | | | | | | | |
| C1 | | 0 | | | | | | — |

**Table 2: Marginal bytes per variant class** (stored bytes for the variant beyond its base, summed over test families)

| Relation | A1 | A2 | A3 (of which u_x) | C1 |
|---|---:|---:|---:|---:|
| identical / content_equal (v02, v03, v04) / renamed / congruent / scaled / tiny_diff / design_change | | | | |

**Table 3: Gates.** G1 x/176, G2 stripes × 21 patterns passed, G3 trap undo bytes, G4 confirmed hits / mismatches, G5 freeze hash and number of test runs, G6 checklist.

**Table 4: Named examples** (≥ 2 pairs).

**Table 5: Failures and rejections.** Files the adapter rejected, restore failures, geometry drift. Should be empty; list any.

**Also report:** dev-split results in their own table, labelled as dev; wall times (informational, not a criterion); the exact commands.

## 9. What ChatGPT should build

1. `src/dxf_adapter.rs`: DXF parse → terms → canonical objects + `u_x`, and the exact decoder.
2. `src/bin/cad_canon_benchmark.rs`: runs A1, A2, A3 and C1 in modes N and Z, the gates, and all tables. Read-only on the corpus.
3. `cad-adapter-v1.md`: the adapter definition from §4, committed **before** the freeze.
4. Unit tests: adapter round-trip on every dev file; hazard tests (like-term collision, `|c| < 1e-9`, 13-digit rounding, `12.5` vs `12.500000`) each proving exact restore.
5. Outputs: `cad-canon-test-m1.md` (the report), plus a per-file CSV (arm, mode, file, stored bytes, u_x bytes, restore ok).

**Run order:** verify corpus → build and test the adapter on dev → write the adapter doc → commit and record the freeze hash → run the test split once → write the report whatever the outcome.

## 10. Note for slide 1 (Ξ gate)

The code does **not** have the ε-clamp bug. `representation.rs::entropy_bits_per_byte` clamps only the **log's input** at 1e-12 and weights each term by the true `p`. Absent bytes contribute exactly 0, and the result stays in [0, 8]. (In practice the clamp never triggers, because any byte that is present has `p ≥ 1/8192`.) Only the slide was wrong. Make the slide match the code:

$$\mathbb H_8(x) = -\sum_{\sigma}\hat p(\sigma)\,\log_2 \max\!\big(\hat p(\sigma),\,10^{-12}\big)\in[0,8],\qquad \Xi(x)=\mathbf 1\big[\mathbb H_8(x)<\tau\big],\ \tau=6.4$$

Also state on the slide that the gate samples only the first 8 KiB of each 256 KiB chunk, and that a periodicity check can override it (see README).
