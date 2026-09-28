# Frozen C2 hybrid v4 follow-up

Freeze commit: `2ee60144998cd85c14a09a62be485ea02dd92d46`. Generated v4 manifest SHA-256: `de5ffbdbcbfa38b02212c75b9a79175c3389734059ceb1de1c7ffad21c7ad1e6`. The v4 test split was generated after the freeze and opened once. The 47 converted real DXFs are **exploratory** because they were already used to compare pickers.

## Real-file concentration from the prior C2-key result

Per-file figures below are selected **payload** bytes, not a per-file assignment of 4+2 physical bytes. Top shares divide by the net payload margin; offsetting losses can make a share exceed 100%.

### Key versus C2-name

14 files selected different bases; net payload margin 225,354 bytes (gross gains 238,159; offsets -12,805). Top 1/3/5 account for 31.4% / 72.2% / 83.8% of net margin.

| File | Key base | Comparator base | Payload bytes saved by key |
|---|---|---|---:|
| `087100.14_magic-2-frame.dxf` | `081373_universal-system-wall-mount-door.dxf` | `standalone` | +70,764 |
| `084243_tx9200-storm-non-impact-ss-fixed-sidelight--us801171-h-.dxf` | `084243_tx9200-storm-non-impact-bp-fixed-sidelight---us801170-h-.dxf` | `084243_tx9200-storm-impact-bp-fixed-sidelight--us801168-h-.dxf` | +55,187 |
| `102313_solare-double---22-double-glazed-sliding-door---head.dxf` | `102313_-elite-freestanding---01-sliding-door-track---glass-door.dxf` | `standalone` | +36,736 |
| `081316_series-8150---standard-nail-on---oxxo-sliding-door-with-water-leg-sill-and-standard-hardware.dxf` | `081316_series-8150---standard-nail-on---oxxo-sliding-door-with-low-sill-and-standard-hardware.dxf` | `081316_series-8150---standard-nail-on---ox-sliding-door-with-low-sill-and-standard-hardware.dxf` | +13,779 |
| `105736_by0120-72x80.dxf` | `105736_by0120-60x80.dxf` | `105736_by0120-36x80.dxf` | +12,396 |
| `MOBILIARIO HOSPITAL.dxf` | `085653_riot-glass-framing-details---2-05---sliding-door-and-fixed-panel-elevation.dxf` | `standalone` | +11,766 |
| `084243_1016202---assa-abloy-versamax-ohc-fbo-bi-part-narrow-smoke-rated-rev-4-0.dxf` | `084243_1016198---assa-abloy-versamax-ohc-fbo-single-slide-narrow-smoke-rated-rev-4-0.dxf` | `084243_1016197---assa-abloy-versamax-ohc-fbo-single-slide-narrow-rev-4-0.dxf` | +8,586 |
| `084243_tx9200-storm-impact-bp-fixed-sidelight--us801168-h-.dxf` | `077233_slhd-72-x-72---1.dxf` | `standalone` | +8,582 |
| `081373_universal-system-wall-mount-door.dxf` | `081373_universal-system-celling-mount-door.dxf` | `081373_universal-system-bi-pass-doors.dxf` | +7,531 |
| `084243_1016197---assa-abloy-versamax-ohc-fbo-single-slide-narrow-rev-4-0.dxf` | `08392108.dxf` | `standalone` | +7,306 |
| `102313_-elite-freestanding---01-sliding-door-track---glass-door.dxf` | `081613_v2-sliding-door-det-head-and-sill.dxf` | `standalone` | +3,755 |
| `084243_1016201---assa-abloy-versamax-ohc-fbo-bi-part-narrow-rev-4-0.dxf` | `084243_1016198---assa-abloy-versamax-ohc-fbo-single-slide-narrow-smoke-rated-rev-4-0.dxf` | `084243_1016197---assa-abloy-versamax-ohc-fbo-single-slide-narrow-rev-4-0.dxf` | +1,771 |
| `083213_2900-v2-oxo-section-detail.dxf` | `standalone` | `083213_2900-v2-ooxoo-section-detail.dxf` | -4,443 |
| `081476_series-440---sliding-jamb-options.dxf` | `standalone` | `081476_series-240---sliding-jamb-options.dxf` | -8,362 |

### Key versus C2-sketch

9 files selected different bases; net payload margin 56,717 bytes (gross gains 72,005; offsets -15,288). Top 1/3/5 account for 26.1% / 74.1% / 102.4% of net margin.

| File | Key base | Comparator base | Payload bytes saved by key |
|---|---|---|---:|
| `08392117.dxf` | `08392108.dxf` | `standalone` | +14,805 |
| `081476_sjo-640.dxf` | `081476_series-240---sliding-jamb-options.dxf` | `standalone` | +14,486 |
| `081476_series-540---sliding-jamb-options.dxf` | `081476_series-240---sliding-jamb-options.dxf` | `standalone` | +12,734 |
| `084243_tx9200-storm-impact-bp-fixed-sidelight--us801168-h-.dxf` | `077233_slhd-72-x-72---1.dxf` | `standalone` | +8,582 |
| `083213_2900-v2-oozoo-section-detail.dxf` | `083213_2900-v2-ooxoo-section-detail.dxf` | `standalone` | +7,453 |
| `084243_1016197---assa-abloy-versamax-ohc-fbo-single-slide-narrow-rev-4-0.dxf` | `08392108.dxf` | `standalone` | +7,306 |
| `083213_2900-v2-oozo-section-detail.dxf` | `083213_2900-v2-ooxoo-section-detail.dxf` | `standalone` | +6,639 |
| `MOBILIARIO HOSPITAL.dxf` | `085653_riot-glass-framing-details---2-05---sliding-door-and-fixed-panel-elevation.dxf` | `102313_-elite-freestanding---01-sliding-door-track---glass-door.dxf` | -2,723 |
| `102313_-elite-freestanding---01-sliding-door-track---glass-door.dxf` | `081613_v2-sliding-door-det-head-and-sill.dxf` | `083200_g3-hurricane-hook-rail-lns-door.dxf` | -12,565 |

## New generated v4 test-only split

132 files; 85,803,842 source bytes. All arms restore exactly and pass all 21 one/two-share loss patterns.

| Arm | Protected bytes | Patch hits | Mean patch bytes | Fallbacks / eligible | Verify failures |
|---|---:|---:|---:|---:|---:|
| C2-0 | 18,749,916 | 0 | — | 0/0 | 0 |
| C2-name | 11,403,672 | 90 | 32,106.7 | 18/108 | 0 |
| C2-sketch | 12,014,166 | 26 | 434.6 | 0/26 | 0 |
| C2-key | 11,304,276 | 62 | 38,094.0 | 12/74 | 0 |
| C2-name+key | 11,403,672 | 90 | 32,106.7 | 18/108 | 0 |
| C2-hybrid | 11,422,878 | 66 | 39,756.5 | 42/108 | 0 |

### Proposal funnel

| Arm | Proposals | ≥ no-base size | Verify failures | Below 1,024-byte gain | Valid, not selected | Accepted |
|---|---:|---:|---:|---:|---:|---:|
| C2-key | 186 | 19 | 0 | 25 | 80 | 62 |
| C2-sketch | 51 | 0 | 0 | 0 | 25 | 26 |

### Sidecar cost (outside protected bytes)

Keys built: 388,784 occurrences; sum of per-file distinct-key counts: 385,850. Python key-set memory: 66,385,716 bytes. Worker peak RSS: 302,825,472 bytes. Key-build time: 3,251.5 ms; total C2-key encode time: 10,909.7 ms (29.8%).

## 47 converted real DXFs — exploratory

47 files; 137,460,149 source bytes. All arms restore exactly and pass all 21 one/two-share loss patterns.

| Arm | Protected bytes | Patch hits | Mean patch bytes | Fallbacks / eligible | Verify failures |
|---|---:|---:|---:|---:|---:|
| C2-0 | 23,139,486 | 0 | — | 0/0 | 0 |
| C2-name | 18,507,450 | 28 | 194,504.0 | 0/28 | 0 |
| C2-sketch | 18,254,496 | 25 | 188,330.4 | 0/25 | 0 |
| C2-key | 18,169,422 | 32 | 211,589.2 | 5/37 | 0 |
| C2-name+key | 18,299,088 | 34 | 208,583.4 | 5/39 | 0 |
| C2-hybrid | 18,183,738 | 34 | 206,321.6 | 5/39 | 0 |

### Proposal funnel

| Arm | Proposals | ≥ no-base size | Verify failures | Below 1,024-byte gain | Valid, not selected | Accepted |
|---|---:|---:|---:|---:|---:|---:|
| C2-key | 85 | 9 | 0 | 4 | 40 | 32 |
| C2-sketch | 50 | 0 | 0 | 0 | 25 | 25 |

### Sidecar cost (outside protected bytes)

Keys built: 261,849 occurrences; sum of per-file distinct-key counts: 239,104. Python key-set memory: 40,450,801 bytes. Worker peak RSS: 375,291,904 bytes. Key-build time: 1,710.8 ms; total C2-key encode time: 9,260.1 ms (18.5%).

**Hybrid result:** C2-key remains smallest on both sets. The hybrid's 1,024-byte acceptance rule also applies to its name candidate, whereas C2-name accepts any strict saving; it therefore cannot be assumed to dominate C2-name even when it tests an additional base.

The sidecar RSS excludes peak memory of short-lived child processes. Timing is one run, not a distribution. The real-file table is exploratory. No v3 CAD transform/edit arm was built.
