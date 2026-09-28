# Tier B raw-DWG reference rows

**Scope:** 47 original DWGs, **43,360,777 source bytes**. These are a separate raw-byte reference and are **not comparable as storage savings** to the 47 converted DXFs, whose source bytes total 137,460,149. The two 577,109-byte ZIPs were verified unchanged and are excluded from these `.dwg` rows; they are byte-identical to one another and remain opaque.

All inputs matched the Step 1 source SHA-256 inventory. A1 used exact 256 KiB chunk dedup; C1 used FastCDC 1.7.0 at 4/16/64 KiB min/average/max. Mode Z called the same production representation selector used for the converted-DXF controls. Unique payloads were packed across files and measured with the actual RedTail-X 4+2 encoder; modeled metadata used the same Step 6 constants and 1.5× charge. Each file rebuilt byte-for-byte from its chunk references, and every stripe passed all 21 one- or two-share losses.

| Raw-DWG reference | Mode | Packed payload | Actual share bytes | Modeled protected metadata | Total protected |
|---|---|---:|---:|---:|---:|
| A1 fixed chunks | N | 43,360,777 | 65,041,170 | 28,073 | **65,069,243** |
| C1 FastCDC | N | 38,528,894 | 57,793,344 | 161,895 | **57,955,239** |
| A1 fixed chunks | Z | 21,718,049 | 32,577,078 | 20,765 | **32,597,843** |
| C1 FastCDC | Z | 21,896,995 | 32,845,494 | 156,327 | **33,001,821** |

C1 saved 7,114,004 bytes (10.93%) against A1 without compression, but **used 403,978 bytes more (1.24%)** after compression and metadata. A1 found no exact fixed-chunk hit; C1 found 165 confirmed content-defined chunk hits. These rows answer how the *original DWG bytes* behave under A1/C1. The adapter's restore target and Tier B win comparison remain the **frozen DXF bytes**.

Executable policy: `tierb-dwg-reference-47.py`; machine-readable details: `tierb-dwg-reference-47-results.json`. No source DWG, ZIP, or frozen DXF was modified.
