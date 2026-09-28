# RedTail-X alone: raw PDF input, no zstd or deduplication

**Date:** 2026-09-27. **Machine:** Apple M1, arm64. **Scope:** one local volume, no network. This is a storage-layout measurement, not a deployment or durability measurement across nodes.

## Method

The [raw benchmark](src/bin/raw_redtail_benchmark.rs) reads the original PDFs and encodes each file independently. It applies **no compression, entropy gate, deduplication, or cross-file packing**. Both arms use the same `reed-solomon-erasure` 4-data + 2-parity codec. The fixed arm writes six 256 KiB shares for every stripe, including a short final stripe. RedTail-X writes six shares of `ceil(stripe_source_bytes / 4)` for a short final stripe; full stripes still use 256 KiB shares.

For both arms, each stripe was encoded and then reconstructed after removing data share 0 and parity share 4. Surviving and reconstructed shares were checked with SHA-256; the reassembled source file's byte count and SHA-256 matched its input. Both arms performed hash work in their timed paths. This is one release-build pass, with no shard files written to disk. Times are descriptive only: they are not a repeated, controlled throughput study. The in-memory stripe manifests, filesystem metadata, and storage-node overhead are **excluded** from the shard-byte totals.

The single-file case used the median-sized PDF in the same 213-file local corpus (505,079 bytes; SHA-256 `6fb594f916f8f7aa42ac561fa71359c0b1896ed8f730201b67edd499c5ac6b30`). The group case used all 213 PDFs, totaling 666,933,563 source bytes. Neither the PDFs nor their personal filesystem paths are distributed in this repository.

## Actual encoded share bytes

| Input | Files | Source bytes | Stripes / short tails | Fixed 4+2 shard bytes | RedTail-X 4+2 shard bytes | Shard bytes saved | Saved vs fixed |
|---|---:|---:|---:|---:|---:|---:|---:|
| Median PDF | 1 | 505,079 | 1 / 1 | 1,572,864 | 757,620 | 815,244 | 51.83% |
| PDF group | 213 | 666,933,563 | 775 / 213 | 1,218,969,600 | 1,000,400,844 | 218,568,756 | 17.93% |

The coded-byte/source-byte ratio is **3.1141× fixed versus 1.5000× RedTail-X** for the single PDF and **1.8277× fixed versus 1.5000× RedTail-X** for the group. RedTail-X approaches the normal 4+2 ratio by removing most final-stripe padding; it does not change Reed-Solomon's parity ratio.

## One-pass time readout

| Input | Fixed encode, ms | RedTail-X encode, ms | Fixed two-share reconstruct, ms | RedTail-X two-share reconstruct, ms |
|---|---:|---:|---:|---:|
| Median PDF | 7.241 | 4.391 | 8.339 | 4.850 |
| PDF group | 6,616.191 | 5,912.597 | 7,601.786 | 6,439.678 |

Both timing paths include buffer allocation, Reed-Solomon work, and SHA-256 checks. The full RedTail-X path also constructs its stripe manifest. These single-run times should not be used as a general speed claim; the measured storage difference is the main result. The benchmark checks one data-plus-parity loss pattern per stripe. Existing `storage.rs` boundary tests separately passed all six one-share and 15 two-share patterns.

## Reproduction

```sh
cargo build --release --bin raw_redtail_benchmark
target/release/raw_redtail_benchmark --file /path/to/one.pdf
target/release/raw_redtail_benchmark --pdf-dir /path/to/pdf-directory
```

The corpus is private, so a different PDF group can produce a different fixed-padding ratio. The same benchmark code can measure that distribution without enabling zstd.
