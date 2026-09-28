# Converter wrapper — Step 3 of 7

**Status:** Implemented and tested; the 47-file batch has not run.

The wrapper is [`convert_dwg_batch.py`](convert_dwg_batch.py). It pins ODA File Converter 27.1.0.0 by executable SHA-256, uses R2018 ASCII DXF with audit off, and requires an explicit output directory. It checks every source against the 49-file Step 1 manifest, including the two opaque ZIPs, while converting only the 47 DWGs.

## Behavior

1. Rejects a missing/changed converter, changed/added/removed source file, symlink/subdirectory, duplicate case-insensitive output name, overlapping source and output paths, existing output directory, or insufficient free space.
2. Copies the DWGs to a temporary input directory and runs ODA there. Original files are not passed as a writable conversion target.
3. Checks that exactly one nonempty R2018 ASCII DXF with entities is produced per DWG. Unexpected `.err` files or missing outputs fail the run.
4. Rechecks every original source hash after conversion. Writes converter stdout/stderr logs and a JSON manifest with source/output hashes, sizes, versions, options, and output mapping. Publishes the validated directory with one same-volume rename. On a converter/validation error, publishes no output directory and saves a sibling `*.failed-<id>.json` report.

## Checks run

- Full 47-DWG dry run: source manifest and converter pin validated.
- Five-version pilot copies: all five converted through the wrapper, produced a manifest and logs, and passed output validation.
- Existing output destination: rejected without overwrite.
- Same-size source mutation: rejected by SHA-256 before conversion.
- Fake damaged DWG with a valid-looking header: ODA produced a `.err` file, so the wrapper rejected the output and retained a failure report.
- One-file positive run after the final space-guard change: passed.

The first five-file run exposed a wrapper parser bug: Python's broad `splitlines()` treated a non-ASCII byte in a DXF text value as another line break. The parser now splits only at actual CR/LF delimiters. The independent ezdxf audit of that DXF had already passed; no source or converter output was at fault.

## Step 4 command, when ready

```sh
python3 canon-test/convert_dwg_batch.py \
  --output-dir canon-test/converted-dxf/oda-27.1-r2018-full \
  --dry-run

python3 canon-test/convert_dwg_batch.py \
  --output-dir canon-test/converted-dxf/oda-27.1-r2018-full
```

Free space briefly fell to about **608 MiB** during testing. After removing the temporary independent-parser installation, it was about **1.2 GiB**. The full-batch guard requires about **587 MiB** for this 47-DWG source set. Recheck free space immediately before Step 4; the five-file pilot showed DXF expansion of roughly 4× overall, and conversion may need additional temporary space. No full-batch output directory exists yet.
