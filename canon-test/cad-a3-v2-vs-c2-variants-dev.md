# A3 v2 versus C2 by variant — 44 development DXFs

Four families contribute one file to each declared variant. Values are **incremental physical protected bytes** as files are added in the frozen path order; each variant includes its share of index growth, manifest/catalog changes and final-share rounding. This is an attribution of the measured corpus total, not a stand-alone encode of each variant.

| Variant | Files | A3 v2 Protected Z | A3 v2 Derived Z | C2 Z | Protected − C2 |
|---|---:|---:|---:|---:|---:|
| v00_base | 4 | 1,394,682 | 407,394 | 407,598 | +987,084 |
| v01_exact_copy | 4 | 264 | 270 | 288 | -24 |
| v02_resave_timestamp | 4 | 3,870 | 3,864 | 1,326 | +2,544 |
| v03_entity_reorder | 4 | 326,052 | 326,052 | 57,570 | +268,482 |
| v04_number_format | 4 | 432,486 | 432,474 | 312,912 | +119,574 |
| v05_layer_rename | 4 | 701,784 | 701,790 | 292,014 | +409,770 |
| v06_translate | 4 | 1,417,470 | 430,182 | 430,044 | +987,426 |
| v07_rotate90 | 4 | 1,387,182 | 403,884 | 333,294 | +1,053,888 |
| v08_units_mm | 4 | 1,433,436 | 446,874 | 446,010 | +987,426 |
| v09_tiny_float_diff | 4 | 130,890 | 95,778 | 3,726 | +127,164 |
| v10_one_entity_edit | 4 | 9,876 | 9,078 | 2,118 | +7,758 |

**Result:** Protected A3 v2 costs more than C2 on 10 of 11 variant types. The sole cheaper type is the exact copy (264 versus 288 bytes across four copies). Translation, rotation, units scaling, and base drawings each add about 0.99–1.05 MB more than C2. The Derived view also exceeds C2 overall (3,257,640 versus 2,286,900 bytes).

The exact-copy row uses a true whole-file ingest bypass: duplicate files add no entity objects or geometry-index entries. The per-copy record-only marginal in `cad-a3-v2-full-dev.md` is a different, narrower accounting view.
