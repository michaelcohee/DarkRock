# A2 exact normalization — Tier A development

Scope: 44 generated DXFs in the development half only. The fresh test split remains unopened. A2 applies shortest Python binary64 round-trip numeric spelling, LF/trailing-whitespace normalization, and removes `$TDCREATE`, `$TDUPDATE`, `$TDINDWG`, and `$TDUSRTIMER` header entries into a counted undo record. It then uses A1's exact 256 KiB chunk dedup. It does not reorder entities, merge geometry, or rewrite handles. The undo is a protected object in the serialized ledger. Without that undo, normalization is lossy with respect to the original bytes and is **not** an exact-storage result.

| Arm | Mode N protected bytes | Mode Z protected bytes |
|---|---:|---:|
| A1 | 25,962,840 | 3,612,642 |
| A2 | 23,722,482 | 3,354,036 |
| C2 | 29,550,450 | 2,286,900 |

For A2 Z, the 44 undo objects total 1,480,718 logical bytes before representation selection; 57,221 numeric spellings and 88 volatile header fields generated 57,309 undo operations. This corpus had no EOL/trailing-space changes. Every A2 file was reconstructed byte-for-byte with its original SHA-256, and the full serialized ledger plus catalog survived all 21 one/two-share-loss patterns under RedTail-X variable-tail 4+2. The five focused A2 unit tests cover shortest-form examples, exponent and signed-zero forms, CRLF/trailing spaces, a missing header variable, and binary DXF rejection.

The serialized measurement and per-file A2 counters are in `cad-dev-full-controls.json`. A2 Z saves 258,606 protected bytes (7.16%) versus A1 Z, but remains 1,067,136 bytes (46.7%) above C2 Z on this development corpus.
