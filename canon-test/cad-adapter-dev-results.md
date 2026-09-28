# CAD adapter v1 — Tier A development run

All 44 development DXFs restored byte-for-byte. No Tier A test DXF was read in this run; an older exploratory control had already read that original test split. Encoded literal-run payloads passed every two-share loss pair on 257 RedTail-X stripes (15 pairs per stripe); one-share loss is separately covered by `storage.rs` tests. **References, edit descriptors and the index were not striped in this check.**

The only v1 candidate edit is `(prefix_len, suffix_len, literal middle bytes)`; it fired 9894 times. Exact raw-hash references fired 24575 times. There were 34469 entity-key lookup hits; 24575 exact and 9894 edited matches passed byte and SHA-256 verification. Other lookup hits remained literal. No key equality alone caused a merge.

Raw input bytes: 21601154. Parsed eligible bytes: 21562692. Matched entity bytes: 13439583. Encoded literal objects plus uncompressed edit middle bytes (excluding references/index): 4910113. Literal encoded bytes: 1184110. Edit middle bytes: 3726003 raw, 439119 as one zstd block. The current layout's literal-plus-compressed-edit partial tally is 1623229 bytes before references, index, metadata and 4+2. This is an adapter diagnostic, **not** a protected-byte A3 benchmark.

| File | Total bytes | Parsed bytes | Raw bytes | Matched bytes | Eligible entities | Key lookups | Exact refs | Edited refs | Literal runs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v00_base.dxf` | 7779 | 6990 | 789 | 0 | 85 | 0 | 0 | 0 | 1 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v01_exact_copy.dxf` | 7779 | 6990 | 789 | 6990 | 85 | 85 | 85 | 0 | 2 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v02_resave_timestamp.dxf` | 7779 | 6990 | 789 | 6990 | 85 | 85 | 85 | 0 | 2 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v03_entity_reorder.dxf` | 7779 | 6990 | 789 | 6990 | 85 | 85 | 85 | 0 | 2 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v04_number_format.dxf` | 9804 | 8945 | 859 | 8945 | 85 | 85 | 0 | 85 | 2 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v05_layer_rename.dxf` | 7677 | 6890 | 787 | 6890 | 85 | 85 | 0 | 85 | 2 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v06_translate.dxf` | 8609 | 7804 | 805 | 0 | 85 | 0 | 0 | 0 | 1 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v07_rotate90.dxf` | 8329 | 7539 | 790 | 0 | 85 | 0 | 0 | 0 | 1 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v08_units_mm.dxf` | 8265 | 7426 | 839 | 0 | 85 | 0 | 0 | 0 | 1 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v09_tiny_float_diff.dxf` | 7784 | 6995 | 789 | 6718 | 85 | 84 | 84 | 0 | 3 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v10_one_entity_edit.dxf` | 7779 | 6990 | 789 | 6907 | 85 | 84 | 84 | 0 | 3 |
| `f03_spur_gears/f03_spur_gears__v00_base.dxf` | 472740 | 471892 | 848 | 0 | 12 | 0 | 0 | 0 | 1 |
| `f03_spur_gears/f03_spur_gears__v01_exact_copy.dxf` | 472740 | 471892 | 848 | 471892 | 12 | 12 | 12 | 0 | 2 |
| `f03_spur_gears/f03_spur_gears__v02_resave_timestamp.dxf` | 472740 | 471892 | 848 | 471892 | 12 | 12 | 12 | 0 | 2 |
| `f03_spur_gears/f03_spur_gears__v03_entity_reorder.dxf` | 472740 | 471892 | 848 | 471892 | 12 | 12 | 12 | 0 | 2 |
| `f03_spur_gears/f03_spur_gears__v04_number_format.dxf` | 515400 | 514497 | 903 | 514497 | 12 | 12 | 0 | 12 | 2 |
| `f03_spur_gears/f03_spur_gears__v05_layer_rename.dxf` | 488876 | 488029 | 847 | 488029 | 12 | 12 | 0 | 12 | 2 |
| `f03_spur_gears/f03_spur_gears__v06_translate.dxf` | 499697 | 498844 | 853 | 0 | 12 | 0 | 0 | 0 | 1 |
| `f03_spur_gears/f03_spur_gears__v07_rotate90.dxf` | 472764 | 471916 | 848 | 109 | 12 | 2 | 2 | 0 | 2 |
| `f03_spur_gears/f03_spur_gears__v08_units_mm.dxf` | 492983 | 492085 | 898 | 0 | 12 | 0 | 0 | 0 | 1 |
| `f03_spur_gears/f03_spur_gears__v09_tiny_float_diff.dxf` | 472745 | 471897 | 848 | 315152 | 12 | 11 | 11 | 0 | 2 |
| `f03_spur_gears/f03_spur_gears__v10_one_entity_edit.dxf` | 472740 | 471892 | 848 | 471834 | 12 | 11 | 11 | 0 | 3 |
| `f08_hex_nut_array/f08_hex_nut_array__v00_base.dxf` | 678583 | 677789 | 794 | 0 | 4800 | 0 | 0 | 0 | 1 |
| `f08_hex_nut_array/f08_hex_nut_array__v01_exact_copy.dxf` | 678583 | 677789 | 794 | 677789 | 4800 | 4800 | 4800 | 0 | 2 |
| `f08_hex_nut_array/f08_hex_nut_array__v02_resave_timestamp.dxf` | 678583 | 677789 | 794 | 677789 | 4800 | 4800 | 4800 | 0 | 2 |
| `f08_hex_nut_array/f08_hex_nut_array__v03_entity_reorder.dxf` | 678583 | 677789 | 794 | 677789 | 4800 | 4800 | 4800 | 0 | 2 |
| `f08_hex_nut_array/f08_hex_nut_array__v04_number_format.dxf` | 837621 | 836767 | 854 | 836767 | 4800 | 4800 | 0 | 4800 | 2 |
| `f08_hex_nut_array/f08_hex_nut_array__v05_layer_rename.dxf` | 703786 | 702989 | 797 | 702989 | 4800 | 4800 | 0 | 4800 | 2 |
| `f08_hex_nut_array/f08_hex_nut_array__v06_translate.dxf` | 719843 | 719044 | 799 | 0 | 4800 | 0 | 0 | 0 | 1 |
| `f08_hex_nut_array/f08_hex_nut_array__v07_rotate90.dxf` | 699983 | 699189 | 794 | 974 | 4800 | 18 | 18 | 0 | 10 |
| `f08_hex_nut_array/f08_hex_nut_array__v08_units_mm.dxf` | 714959 | 714113 | 846 | 0 | 4800 | 0 | 0 | 0 | 1 |
| `f08_hex_nut_array/f08_hex_nut_array__v09_tiny_float_diff.dxf` | 678778 | 677984 | 794 | 625319 | 4800 | 4628 | 4628 | 0 | 170 |
| `f08_hex_nut_array/f08_hex_nut_array__v10_one_entity_edit.dxf` | 678580 | 677786 | 794 | 677404 | 4800 | 4799 | 4799 | 0 | 3 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v00_base.dxf` | 753117 | 752095 | 1022 | 0 | 50 | 0 | 0 | 0 | 1 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v01_exact_copy.dxf` | 753117 | 752095 | 1022 | 752095 | 50 | 50 | 50 | 0 | 2 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v02_resave_timestamp.dxf` | 753117 | 752095 | 1022 | 752095 | 50 | 50 | 50 | 0 | 2 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v03_entity_reorder.dxf` | 753117 | 752095 | 1022 | 752095 | 50 | 50 | 50 | 0 | 2 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v04_number_format.dxf` | 816630 | 815558 | 1072 | 815558 | 50 | 50 | 0 | 50 | 2 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v05_layer_rename.dxf` | 777329 | 776295 | 1034 | 776295 | 50 | 50 | 0 | 50 | 2 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v06_translate.dxf` | 794424 | 793399 | 1025 | 0 | 50 | 0 | 0 | 0 | 1 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v07_rotate90.dxf` | 763759 | 762737 | 1022 | 0 | 50 | 0 | 0 | 0 | 1 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v08_units_mm.dxf` | 786898 | 785826 | 1072 | 0 | 50 | 0 | 0 | 0 | 1 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v09_tiny_float_diff.dxf` | 753117 | 752095 | 1022 | 721724 | 50 | 48 | 48 | 0 | 4 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v10_one_entity_edit.dxf` | 753119 | 752097 | 1022 | 737174 | 50 | 49 | 49 | 0 | 3 |
