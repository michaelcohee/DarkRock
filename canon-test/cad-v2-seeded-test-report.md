# Seeded Tier A test-only comparison

**Corpus:** `darkrock-cad-dxf-v2`; 132 generated test DXFs; 85,449,809 source bytes. Generated after the joint adapter/generator freeze. The earlier v2 run used the pre-update generator and is superseded by this table.

All cells use mode Z and RedTail-X variable-tail 4+2. Each cell restores every file byte-for-byte with SHA-256 verification and survives all 21 one/two-share loss patterns. The physical ledger counts protected payload, references, edits, manifests, index when Protected, catalog and charged bootstrap bytes.

**Generated manifest SHA-256:** `d670bc67a937fc55a21ae69baa3fdb15079fbd0771d58710d4add0d99e8a325c`

| Arm | Index view | Physical protected bytes | Versus C2 |
|---|---|---:|---:|
| C2 | Protected | 11,370,510 | 1.00× |
| C2 | Derived | not built | — |
| C1 | Protected | 13,716,198 | 1.21× |
| C1 | Derived | not built | — |
| A3 v2 | Protected | 44,758,662 | 3.94× |
| A3 v2 | Derived | 16,671,756 | 1.47× |
| A2 | Protected | 14,558,166 | 1.28× |
| A2 | Derived | not built | — |
| A1 | Protected | 15,428,628 | 1.36× |
| A1 | Derived | not built | — |
| A3 v1 | Protected | 86,230,812 | 7.58× |
| A3 v1 | Derived | 58,143,900 | 5.11× |

Derived means the A3 candidate index is rebuilt from protected data and checked for the identical verified lookup set. Derived-index versions of A1, A2, C1 and C2 were not built.

**Result:** C2 remains the smallest measured arm. A3 v2 uses 3.94× C2 with its index protected and 1.47× with that index derived.

Source ledgers: `cad-v2-seeded-test-controls-z.json`, `cad-v2-seeded-test-a3-v2-z.tsv`, `cad-v2-seeded-test-a3-v1-z.tsv`.
