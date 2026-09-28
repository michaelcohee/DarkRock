# ODA DXF validation — Step 5

**Date:** 2026-09-27  
**Input:** The published 47-file batch in `converted-dxf/oda-27.1-r2018-full/`, manifest SHA-256 `5ce26ceb2be36dfecd43c929d0cd77a2f9331bd822fc7c426977ade66b139810`. The original 47 DWGs and two ZIPs were not changed.

## Structural and independent-reader results

- All 49 source files still match the Step 1 hashes; all 47 source-to-output mappings are present. All 47 outputs match their recorded sizes and SHA-256 hashes.
- All 47 outputs are nonempty ASCII DXFs with parseable group-code pairs, a final `EOF`, and `AC1032`/R2018 version. This structural check passed on the published bytes.
- Independent reader: `ezdxf` 1.4.4. All 47 files opened. It found 176,653 modelspace entities across the batch, 636 layer entries and 1,829 block definitions summed across files. Every file has nonempty modelspace. Per-file entity types, layers, blocks, and DXF header extents are recorded in `oda-step5-independent-audit.json`. Header extents are metadata claims; full geometry bounds were not recomputed.
- **36/47 files audited with zero errors and zero fixes. Eleven files had zero reported errors but 282 in-memory audit fixes.** The published DXFs were not modified by the audit. The independent reader's fixes mean the 11 cannot be called clean, lossless CAD conversions on this evidence.

| Affected source family | Files | Reader fixes per file | Fixes | Reader action |
|---|---:|---:|---:|---|
| `081316_series-8150` sliding doors | 4 | 18 | 72 | Deleted `TABLECONTENT` objects with invalid owner handles |
| `081476` sliding jamb options (`series-240`, `series-540`, `sjo-640`) | 3 | 2 | 6 | Repaired invalid owner handles of `XRECORD` objects |
| `085313.20` vinyl patio doors | 4 | 51 | 204 | Deleted `ACAD_PROXY_OBJECT` objects with invalid owner handles |

The four vinyl files also each contain 23 `ACAD_PROXY_ENTITY` modelspace entities. The rendered appearance alone does not establish that their custom-object semantics survived conversion. The 11 affected files are **quarantined from a CAD-fidelity acceptance claim** pending investigation. The full set can still be used as a fixed *DXF byte-restoration* test corpus if labeled as such, because the storage test's restore target is the published DXF byte string.

## Representative visual inspection

An independent DXF renderer produced six PNG previews from the published files: one each from source DWG versions R2000, R2004, R2007, R2010, and R2018, plus a flagged vinyl patio-door file. The sliding-jamb detail, sectional/elevation sheet, installation sheet, sill section, shrub block, and patio-door drawing all show recognizable geometry and text. See `step5-previews/manifest.json` and the PNGs in that directory. The flagged patio-door preview looks plausible despite its 51 audit fixes, so visual appearance is not enough to clear it.

These previews show that a second program can render the converted DXFs. There was **no independent DWG reader or source-DWG rendering comparison** in this step, so entity counts, layers, blocks, and extents could not be compared to original DWG semantics. No claim of byte-perfect DWG restoration or proven DWG-to-DXF geometric equivalence is made.

## Decision before Step 6

**Step 5 validation is complete, with a qualified result.** The 47-file batch passes byte inventory, syntax, parse, and render checks. It does **not** pass an unqualified CAD-fidelity acceptance: 11 files need reader repairs, and source-versus-DXF content has not been independently compared. Preserve the published bytes and this result. Before a CAD-fidelity claim or a formal Tier B freeze, investigate the 11 flagged files with a trusted DWG/DXF viewer or second DWG reader, and decide whether to accept them with explicit limitations, regenerate a versioned corpus, or exclude them. Do not silently replace any file in this batch.

The Step 6 storage comparison, if run on these DXFs, must use one identical output set for every arm and label its restore target as **converted DXF bytes**. Earlier raw-DWG compression results are not a benchmark bar for this 47-file DXF set.
