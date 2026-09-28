# Frozen C2 base-selection experiment

**Rule:** [pre-generation freeze](./c2-base-selection-freeze.md). Mode Z, complete protected ledger, RedTail-X variable-tail 4+2. All selected patches are decoded and checked byte-for-byte and by SHA-256; all ledger bytes pass 21 one/two-share-loss patterns. Candidate scoring is only a lookup hint.

Freeze commit: `c4525d0ddad35553256c0e760c8c508d36d8c7bb`. Generated v3 test manifest SHA-256: `5fa962fc3843e2f6e4746d9fa5954147190403af2b40fc9ccc15d100f42184b3`. Original local Tier B conversion manifest SHA-256: `5ce26ceb2be36dfecd43c929d0cd77a2f9331bd822fc7c426977ade66b139810`. The public copy redacts its local output path and has a different file hash.

## Tier A: new generated v3 test-only split

132 files; 85,521,732 source bytes.

| Arm | Protected bytes | Patch hits | Mean patch bytes | Fallbacks / eligible | Different base vs name | Harm / comparable | Verify failures |
|---|---:|---:|---:|---:|---:|---:|---:|
| C2-0 | 18,685,896 | 0 | — | 0/0 (—) | 90 | 0/0 (—) | 0 |
| C2-name | 11,354,940 | 90 | 31,971.4 | 18/108 (16.7%) | 0 | 0/90 (0.0%) | 0 |
| C2-sketch | 11,952,642 | 29 | 385.5 | 0/29 (0.0%) | 61 | 0/29 (0.0%) | 0 |
| C2-key | 11,266,362 | 61 | 37,918.5 | 13/74 (17.6%) | 37 | 0/61 (0.0%) | 0 |
| C2-name+key | 11,354,940 | 90 | 31,971.4 | 18/108 (16.7%) | 0 | 0/90 (0.0%) | 0 |

**C2-key pass condition:** met on this corpus. Exact restore passed; harm rate is shown above.

C2-key is 686,280 protected bytes below C2-sketch and 88,578 below C2-name on this generated split.

## Tier B: 47 real converted DXFs

47 files; 137,460,149 source bytes.

| Arm | Protected bytes | Patch hits | Mean patch bytes | Fallbacks / eligible | Different base vs name | Harm / comparable | Verify failures |
|---|---:|---:|---:|---:|---:|---:|---:|
| C2-0 | 23,139,486 | 0 | — | 0/0 (—) | 28 | 0/0 (—) | 0 |
| C2-name | 18,507,450 | 28 | 194,504.0 | 0/28 (0.0%) | 0 | 0/28 (0.0%) | 0 |
| C2-sketch | 18,254,496 | 25 | 188,330.4 | 0/25 (0.0%) | 17 | 0/21 (0.0%) | 0 |
| C2-key | 18,169,422 | 32 | 211,589.2 | 5/37 (13.5%) | 14 | 0/26 (0.0%) | 0 |
| C2-name+key | 18,299,088 | 34 | 208,583.4 | 5/39 (12.8%) | 6 | 0/28 (0.0%) | 0 |

**C2-key pass condition:** met on this corpus. Exact restore passed; harm rate is shown above.

C2-key is 85,074 protected bytes below C2-sketch and 338,028 below C2-name on these converted real DXFs.

The generated Tier A filename families are an oracle. Tier B uses declared filename prefixes. The first earlier unique file in a family is C2-name's only base; name+key uses keys for orphans. The different-base count includes one arm selecting a patch while the other falls back. Harm compares accepted patches only when both arms patch the same file. Feature-extraction CPU and memory are outside protected-byte counts. No v3 CAD transform/edit path was built.

The table compares the five arms **under this frozen serialized-ledger format**. Its physical-byte numbers should not be subtracted from earlier C2 reports that used different metadata layouts. The byte advantage alone does not establish that computing and maintaining file-level geometry keys is worthwhile in a deployed store; that cost was outside this experiment.
