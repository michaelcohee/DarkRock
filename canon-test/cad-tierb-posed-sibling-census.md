# Tier B posed-sibling candidate census

**Scope:** read-only feasibility count on the 47 ODA-converted DXFs. No stored payload, reference, undo, parity, or v3 adapter was created. All input bytes were checked against the pinned conversion manifest.

For LINE, CIRCLE, ARC, POINT, LWPOLYLINE and composite POLYLINE records in ENTITIES or BLOCKS, the key uses geometry relative to each file’s centroid and RMS scale, tests all four 90° orientations, rounds normalized values to 0.001, and ignores handles/layers. A record is counted when its key appears in another file of the same declared filename family. These are **candidate bytes**, including possible false matches; no byte-exact restore or net storage saving is implied. Unsupported records are excluded.

| Family | Files | Source bytes | Eligible entity bytes | Cross-file candidate bytes | Candidate / eligible |
|---|---:|---:|---:|---:|---:|
| 077233 | 2 | 3,898,594 | 2,796,662 | 685 | 0.02% |
| 081316 | 4 | 15,825,051 | 11,573,517 | 39,998 | 0.35% |
| 081373 | 3 | 15,904,948 | 2,735,419 | 0 | 0.00% |
| 081476 | 4 | 745,259 | 129,212 | 63,779 | 49.36% |
| 081613 | 1 | 9,406,564 | 6,792,733 | 0 | 0.00% |
| 083200 | 2 | 21,043,490 | 12,138,332 | 0 | 0.00% |
| 083213 | 4 | 11,509,089 | 10,583,237 | 749,871 | 7.09% |
| 083921 series | 3 | 6,619,685 | 3,844,674 | 384 | 0.01% |
| 084243 TX9200 sidelights | 4 | 13,463,108 | 4,072,063 | 848 | 0.02% |
| 084243 VersaMax | 4 | 10,889,585 | 3,058,266 | 428 | 0.01% |
| 08495101 | 1 | 6,197,769 | 1,653,613 | 0 | 0.00% |
| 085313.20 | 4 | 3,678,648 | 1,431,079 | 0 | 0.00% |
| 085653 | 1 | 1,033,251 | 64,496 | 0 | 0.00% |
| 087100.14 | 1 | 3,605,501 | 345,186 | 0 | 0.00% |
| 102313 Elite track | 2 | 3,099,643 | 333,326 | 0 | 0.00% |
| 102313 Solare | 2 | 2,359,902 | 404,738 | 9,105 | 2.25% |
| 105736 | 3 | 3,832,719 | 1,075,528 | 0 | 0.00% |
| MOBILIARIO HOSPITAL | 1 | 3,999,479 | 3,053,407 | 0 | 0.00% |
| arbusto_PB_verde_grande | 1 | 347,864 | 36,668 | 0 | 0.00% |
| **Total** | **47** | **137,460,149** | **66,122,156** | **865,098** | **1.31%** |

**083213 follow-up — exploratory, negative.** Its 749,871 candidate bytes comprise 4,266 `LWPOLYLINE` records; 4,110 are two-vertex segments (716,505 bytes). Nearly all candidate bytes occur across the `ooxoo` and `oozoo` drawings. The invariant key matches short segments, **not profiles**. No storage run was performed, and no transform-edit path is justified for this family. Because 083213 supplies most of the table's candidates, the 1.31% overall candidate figure overstates evidence for real posed siblings.

**Limits:** filename grouping is a declared heuristic, and per-file centroid/RMS normalization can miss siblings when drawings differ in content or include large unrelated blocks. Rounding and shape-only keys can create false positives. The counts are a filter for whether pose-aware matching is worth later research, not an A3 v3 benchmark.
