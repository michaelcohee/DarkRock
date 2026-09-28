# DWG → DXF converter preflight — Step 1 of 7

**Date:** 2026-09-27. **Status:** source inventory complete; converter availability unresolved. No source drawing was modified, no converter was installed, and no DXF output directory was created.

| Check | Result |
|---|---|
| Host | macOS 26.3, arm64 (Apple Silicon) |
| Workspace free space | 1,493,280 KiB (~1.42 GiB) at preflight; recheck before conversion |
| Input directory | `canon-test/DWG/`, 49 regular files, 44,514,995 bytes |
| Types | 47 `.dwg`, 2 `.zip`; no other file types or subdirectories |
| Empty files | 0 |
| DWG signatures | AC1015: 38; AC1018: 2; AC1021: 1; AC1024: 4; AC1032: 2 |
| Case-insensitive output-stem collisions | 0 |
| Planned output parent | `canon-test/converted-dxf/` does not exist; source and output paths do not overlap |
| ODA File Converter / LibreDWG tools | Neither `ODAFileConverter`, `dwg2dxf`, nor `dwgread` found in the command path or a shallow search of `/Applications`, `/opt/homebrew`, and `/usr/local` |

The full per-file hash inventory is [dwg-input-manifest-2026-09-27.csv](dwg-input-manifest-2026-09-27.csv). Its SHA-256 is `82d3506d50afa057edf3d19ed10df929c91814b1ca49138df4b4a868326ca67f`. All files had nonzero length and the DWGs had expected `AC10..` signatures. This manifest **establishes today's baseline**; there was no earlier 49-file hash manifest against which to authenticate the newly added files.

The current free space is adequate for a small pilot by file size, but not a guaranteed full-batch budget: DXF may expand beyond the 44.5 MB source set. Set a minimum free-space guard after measuring pilot output sizes, before converting all 47 DWGs.

**Step 2 entry condition:** install or point the wrapper to one chosen converter binary, record its version and executable hash, and run the version-diverse pilot. The plan does not authorize installing one automatically during preflight. The original ZIPs remain opaque throughout conversion.
