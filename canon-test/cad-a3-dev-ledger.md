# A3 full incremental protected-byte ledger — Tier A dev

44 source DXFs restored byte-for-byte from each fully serialized ledger. Both complete ledgers and their manifest catalogs passed all 21 one/two-share loss patterns. Binary DXF is unsupported. No Tier A test file was read.

All actual ledger bytes are striped with RedTail-X 4+2. The stripe-manifest catalog is also 4+2 coded. The finite bootstrap is six physical copies of each catalog-stripe manifest (240 bytes) and a 32-byte catalog digest; this is a charged root assumption, not authenticated production metadata.

| Mode | Ledger bytes | Literal objects | Edit bytes | Ordered references | Key/hash index | File manifests | Framing | Data shares | Manifest shares | Bootstrap | Total physical |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| N | 17486356 | 8174164 | 3726003 | 2931150 | 2653088 | 1936 | 15 | 26229534 | 6156 | 1632 | 26237322 |
| Z | 10506750 | 1194558 | 3726003 | 2931150 | 2653088 | 1936 | 15 | 15760128 | 3996 | 1632 | 15765756 |

Key lookup hits: 34469; verified matched entities: 34469 (24575 exact-hash, 9894 edited); verified-hit rate among key lookups: 100.00%. Pass-through: 38462/21601154 bytes (0.1781%). The only v1 edit that fired was `(prefix_len, suffix_len, literal_middle)`, 9894 times. No canonical key alone caused a merge.

V2 proposal diagnostic only: per-file zstd level-3 compression of edit **middle bytes alone** would change 3726003 raw bytes to 522147 selected payload bytes across 8 compressed files, before tags, offsets, hashes, references, indexes, manifests, and 4+2. These bytes are **not** used in the v1 table.

| File | Source bytes | Parsed bytes | Raw pass-through | Key lookups | Verified exact | Verified edited | Incremental N physical bytes | Incremental Z physical bytes |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v00_base.dxf` | 7779 | 6990 | 789 | 0 | 0 | 0 | 30834 | 21072 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v01_exact_copy.dxf` | 7779 | 6990 | 789 | 85 | 85 | 0 | 12246 | 11664 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v02_resave_timestamp.dxf` | 7779 | 6990 | 789 | 85 | 85 | 0 | 12252 | 11670 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v03_entity_reorder.dxf` | 7779 | 6990 | 789 | 85 | 85 | 0 | 12246 | 11664 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v04_number_format.dxf` | 9804 | 8945 | 859 | 85 | 0 | 85 | 19950 | 19272 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v05_layer_rename.dxf` | 7677 | 6890 | 787 | 85 | 0 | 85 | 13872 | 13296 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v06_translate.dxf` | 8609 | 7804 | 805 | 0 | 0 | 0 | 30018 | 19014 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v07_rotate90.dxf` | 8329 | 7539 | 790 | 0 | 0 | 0 | 29598 | 19056 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v08_units_mm.dxf` | 8265 | 7426 | 839 | 0 | 0 | 0 | 29502 | 19368 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v09_tiny_float_diff.dxf` | 7784 | 6995 | 789 | 84 | 84 | 0 | 12822 | 11970 |
| `f00_title_block_ansi_d/f00_title_block_ansi_d__v10_one_entity_edit.dxf` | 7779 | 6990 | 789 | 84 | 84 | 0 | 12522 | 11934 |
| `f03_spur_gears/f03_spur_gears__v00_base.dxf` | 472740 | 471892 | 848 | 0 | 0 | 0 | 711654 | 108588 |
| `f03_spur_gears/f03_spur_gears__v01_exact_copy.dxf` | 472740 | 471892 | 848 | 12 | 12 | 0 | 3030 | 2388 |
| `f03_spur_gears/f03_spur_gears__v02_resave_timestamp.dxf` | 472740 | 471892 | 848 | 12 | 12 | 0 | 3030 | 2394 |
| `f03_spur_gears/f03_spur_gears__v03_entity_reorder.dxf` | 472740 | 471892 | 848 | 12 | 12 | 0 | 3030 | 2388 |
| `f03_spur_gears/f03_spur_gears__v04_number_format.dxf` | 515400 | 514497 | 903 | 12 | 0 | 12 | 774414 | 773328 |
| `f03_spur_gears/f03_spur_gears__v05_layer_rename.dxf` | 488876 | 488029 | 847 | 12 | 0 | 12 | 734118 | 733848 |
| `f03_spur_gears/f03_spur_gears__v06_translate.dxf` | 499697 | 498844 | 853 | 0 | 0 | 0 | 752448 | 128838 |
| `f03_spur_gears/f03_spur_gears__v07_rotate90.dxf` | 472764 | 471916 | 848 | 2 | 2 | 0 | 711462 | 102066 |
| `f03_spur_gears/f03_spur_gears__v08_units_mm.dxf` | 492983 | 492085 | 898 | 0 | 0 | 0 | 742014 | 115512 |
| `f03_spur_gears/f03_spur_gears__v09_tiny_float_diff.dxf` | 472745 | 471897 | 848 | 11 | 11 | 0 | 238578 | 33726 |
| `f03_spur_gears/f03_spur_gears__v10_one_entity_edit.dxf` | 472740 | 471892 | 848 | 11 | 11 | 0 | 3270 | 2622 |
| `f08_hex_nut_array/f08_hex_nut_array__v00_base.dxf` | 678583 | 677789 | 794 | 0 | 0 | 0 | 1975986 | 1008252 |
| `f08_hex_nut_array/f08_hex_nut_array__v01_exact_copy.dxf` | 678583 | 677789 | 794 | 4800 | 4800 | 0 | 613416 | 612840 |
| `f08_hex_nut_array/f08_hex_nut_array__v02_resave_timestamp.dxf` | 678583 | 677789 | 794 | 4800 | 4800 | 0 | 613782 | 612846 |
| `f08_hex_nut_array/f08_hex_nut_array__v03_entity_reorder.dxf` | 678583 | 677789 | 794 | 4800 | 4800 | 0 | 613416 | 613200 |
| `f08_hex_nut_array/f08_hex_nut_array__v04_number_format.dxf` | 837621 | 836767 | 854 | 4800 | 0 | 4800 | 1601370 | 1600710 |
| `f08_hex_nut_array/f08_hex_nut_array__v05_layer_rename.dxf` | 703786 | 702989 | 797 | 4800 | 0 | 4800 | 1323078 | 1322490 |
| `f08_hex_nut_array/f08_hex_nut_array__v06_translate.dxf` | 719843 | 719044 | 799 | 0 | 0 | 0 | 2037870 | 1004358 |
| `f08_hex_nut_array/f08_hex_nut_array__v07_rotate90.dxf` | 699983 | 699189 | 794 | 18 | 18 | 0 | 2006052 | 1009284 |
| `f08_hex_nut_array/f08_hex_nut_array__v08_units_mm.dxf` | 714959 | 714113 | 846 | 0 | 0 | 0 | 2030904 | 1013058 |
| `f08_hex_nut_array/f08_hex_nut_array__v09_tiny_float_diff.dxf` | 678778 | 677984 | 794 | 4628 | 4628 | 0 | 718410 | 668376 |
| `f08_hex_nut_array/f08_hex_nut_array__v10_one_entity_edit.dxf` | 678580 | 677786 | 794 | 4799 | 4799 | 0 | 614508 | 613188 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v00_base.dxf` | 753117 | 752095 | 1022 | 0 | 0 | 0 | 1139796 | 256668 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v01_exact_copy.dxf` | 753117 | 752095 | 1022 | 50 | 50 | 0 | 8136 | 7266 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v02_resave_timestamp.dxf` | 753117 | 752095 | 1022 | 50 | 50 | 0 | 8136 | 7266 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v03_entity_reorder.dxf` | 753117 | 752095 | 1022 | 50 | 50 | 0 | 8136 | 7266 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v04_number_format.dxf` | 816630 | 815558 | 1072 | 50 | 0 | 50 | 1227030 | 1226100 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v05_layer_rename.dxf` | 777329 | 776295 | 1034 | 50 | 0 | 50 | 1171008 | 1170126 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v06_translate.dxf` | 794424 | 793399 | 1025 | 0 | 0 | 0 | 1202118 | 264780 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v07_rotate90.dxf` | 763759 | 762737 | 1022 | 0 | 0 | 0 | 1155762 | 258570 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v08_units_mm.dxf` | 786898 | 785826 | 1072 | 0 | 0 | 0 | 1190826 | 285768 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v09_tiny_float_diff.dxf` | 753117 | 752095 | 1022 | 48 | 48 | 0 | 54000 | 16026 |
| `f10_freestyle_sketch_b/f10_freestyle_sketch_b__v10_one_entity_edit.dxf` | 753119 | 752097 | 1022 | 49 | 49 | 0 | 30672 | 11640 |
