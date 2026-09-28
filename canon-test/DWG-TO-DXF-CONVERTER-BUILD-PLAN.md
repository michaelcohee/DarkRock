# DWG → DXF conversion integration: build plan

**Status:** Plan only, 2026-09-27. No converter installed, no conversion run, and no original drawing changed.

## Purpose and boundary

Convert Michael's 47 loose `.dwg` files in `canon-test/DWG/` into a frozen, auditable DXF corpus for the Tier B canonizer experiment. The two `.zip` files remain opaque bytes and are not converted or unpacked. This work builds a **repeatable wrapper and validation process around a pinned converter**; it does not implement a DWG parser from scratch.

The exact restore target for the Tier B A1/A2/A3/C1/C2 comparison is each **converted DXF byte string**. The original DWGs get a separate raw-byte reference table. A byte-perfect DXF restore does not imply a byte-perfect DWG restore. Converted DXF outputs may omit or alter CAD semantics; check that separately before treating them as useful drawings.

## Converter choice

1. **Selected primary converter: [ODA File Converter](https://www.opendesign.com/guestfiles/oda_file_converter?language=en).** Pilot it on this Apple Silicon Mac. ODA lists a macOS arm64 build and both GUI and command-line operation; its documented inputs include source/target directories, file filter, output version/type, recursion and audit flag. Record the exact installed binary, version, executable SHA-256, and invocation discovered from its own local help. Do not assume command arguments from an older release.
2. Keep [LibreDWG `dwg2dxf`](https://www.gnu.org/software/libredwg/manual/html_node/Programs.html) as a separately labelled fallback or cross-check only if ODA cannot process a file acceptably. Its documentation lists DWG-to-DXF conversion but describes aspects of DXF output as experimental; do not silently substitute its results for ODA results. Pin its version and options independently.
3. Pilot at least one R2000, R2004, R2007, R2010, and R2018 DWG before fixing the output DXF version. Prefer an ASCII DXF version that preserves the needed entities and objects across these files. If down-conversion changes content, either choose a newer output version and support it in the adapter, or mark that input unsupported. Do not force R12 solely because the synthetic Tier A files are R12.

ODA is chosen now; its installed version, output DXF version, audit setting, and conversion options become fixed **before** the Tier B benchmark. If the converter changes, produce a new corpus ID and rerun every Tier B arm.

## Inputs and outputs

- Inputs: read `canon-test/DWG/*.dwg` without modifying or deleting them. Record the 47 file names, lengths, DWG signature/version, and SHA-256 before conversion.
- Outputs: write to a separate `canon-test/converted-dxf/<corpus-id>/` directory. Never write DXFs into `canon-test/DWG/` or overwrite a prior frozen conversion. Exclude generated conversion output and the original `canon-test/DWG/` from routine Git staging; keep the manifest, wrapper code, and report available for review.
- Build a manifest with a stable source-to-output path mapping. For each file record input/output SHA-256 and byte lengths, converter identity, options, exit status, warnings, detected DXF version, and any unsupported entities. Preserve full stdout/stderr logs outside the source directory.
- Keep the two ZIPs opaque in every arm. Record their hashes and the known loose-DWG overlap as a separate observation; do not count unpacking as canonization.

## Implementation steps

1. **Preflight.** Confirm arm64 macOS compatibility, local free space, converter availability, and all expected source hashes. Refuse to run if source or output directories overlap or an output path already exists. Do not install a converter or alter the DWGs as part of preflight.
2. **Pilot.** Convert three version-diverse drawings into temporary output. Inspect the actual DXF encoding and header, entity/object warnings, layer/entity counts, and representative views. Decide and record one converter configuration for the full batch.
3. **Wrapper.** Add a small command-line wrapper under `canon-test/` that invokes the pinned converter, maps input names safely to unique output names, captures logs, writes the manifest atomically, and treats missing, empty, duplicate, or failed outputs as errors. No silent fallback converter.
4. **Batch.** Convert all 47 loose DWGs once into a new corpus directory. Hash every output. A second conversion to a separate temporary directory may check reproducibility; if hashes differ, inspect the cause and freeze one output set rather than merging versions.
5. **Validate.** Confirm 47 nonempty ASCII DXFs, parseable group-code pairs and `EOF`, expected output version, and source-to-output mapping. Compare CAD content where supported: entity counts/types, layers, blocks, extents, and a few visual samples. Log every difference and reject files whose conversion cannot be trusted. These checks assess conversion fidelity; they are distinct from later byte-exact storage restore.
6. **Freeze Tier B corpus.** Record a corpus ID and manifest hash. The benchmark must consume exactly these bytes. Recompute A1, C1, and C2 on the **converted DXFs** before comparing with A2/A3. Preserve the raw-DWG A1/C1 numbers as reference rows only.
7. **Run storage checks later.** For each Tier B arm, decode every stored object to the frozen DXF bytes, compare SHA-256 and bytes, then verify RedTail-X reconstruction under all 6 one-share and 15 two-share loss patterns. Count conversion-independent storage bytes, undo bytes, index/references, and metadata under the same model.

## Acceptance criteria

- Original 47 DWGs have unchanged hashes; both ZIPs remain untouched.
- Exactly 47 DXF outputs map one-to-one to the loose DWGs, or a clearly listed rejected subset stops the claim for those files.
- Every accepted output has recorded converter identity and a stable SHA-256; converter warnings and semantic differences are visible in the report.
- Tier B comparisons use the same frozen DXF bytes as their restore target. C2 is rerun on those DXFs under one explicitly frozen policy; the earlier 9.85 MB result on raw DWGs is not reused as the converted-DXF bar.
- The report states which claim was tested: DXF byte restoration and measured storage bytes, plus separately assessed CAD-content fidelity. It makes no claim to reconstruct the original DWG bytes from DXF.

## Current project facts

`canon-test/DWG/` currently contains 49 files: 47 DWGs and 2 ZIPs (44,514,995 bytes total). No `ODAFileConverter` or `dwg2dxf` executable was found in the current command path. The existing Tier B profile and raw-DWG controls covered the earlier 20-file set only; they must be rerun for this 49-file set and are not converted-DXF results. The checked-in v1.1 spec still states the earlier 20-file count, so its Tier B inventory needs a versioned update before a frozen run. See `CAD-CANON-TEST-SPEC.md` v1.1, §3 Tier B, for the experiment rules.
