# A3 v1 dual-index development report

44 Tier A development DXFs only. The original candidate index was deleted for the Derived view; all files were restored from durable literal/edit/reference data and re-ingested. The complete per-file lookup and verify-pass vector matched. Every serialized ledger and its catalog survived all 21 one/two-share loss patterns under RedTail-X variable-tail 4+2.

| Mode | View | Ledger bytes | Index bytes | Data shares | Catalog shares | Bootstrap | Protected bytes |
|---|---|---:|---:|---:|---:|---:|---:|
| N | Protected | 17486356 | 2653088 | 26229534 | 6156 | 1632 | 26237322 |
| N | Derived | 14833268 | 0 | 22249902 | 5436 | 1632 | 22256970 |
| Z | Protected | 10506750 | 2653088 | 15760128 | 3996 | 1632 | 15765756 |
| Z | Derived | 7853662 | 0 | 11780496 | 2916 | 1632 | 11785044 |

Rebuild test: 19948 geometry keys; 34469 verified entity matches; per-file lookup, exact-match and edited-match counts identical after replay. Cache RAM and rebuild CPU are not included in physical bytes. A3 v1 Protected Z = 15765756 bytes, versus C2 Z = 2,286,900 bytes on the same dev set: A3 v1 still loses.
