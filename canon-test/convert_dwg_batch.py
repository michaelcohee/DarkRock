#!/usr/bin/env python3
"""Pinned ODA DWG→DXF wrapper. Never modifies source drawings.

Example (Step 4, after review):
  python3 canon-test/convert_dwg_batch.py \
    --output-dir canon-test/converted-dxf/oda-27.1-r2018-2026-09-27
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
import tempfile
from uuid import uuid4
from datetime import datetime, timezone

HERE = Path(__file__).resolve().parent
PINNED_EXE = Path('/Applications/ODAFileConverter.app/Contents/MacOS/ODAFileConverter')
PINNED_INFO = Path('/Applications/ODAFileConverter.app/Contents/Info.plist')
PINNED_VERSION = '27.1.0.0'
PINNED_SHA256 = '21a8f029e7d41af64088f65492404d97ffec0d2f7886703b3b725f5ac4a55783'
OUTPUT_VERSION = 'ACAD2018'
OUTPUT_DXF_VERSION = 'AC1032'
CONVERTER_OPTIONS = [OUTPUT_VERSION, 'DXF', '0', '0', '*.dwg']


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def fail(message: str) -> None:
    raise ValueError(message)


def check_converter() -> dict:
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        fail('the pinned converter is the macOS arm64 build')
    if not PINNED_EXE.is_file() or not PINNED_INFO.is_file():
        fail(f'pinned ODA converter is missing: {PINNED_EXE}')
    with PINNED_INFO.open('rb') as file:
        version = plistlib.load(file).get('CFBundleShortVersionString')
    digest = sha256_file(PINNED_EXE)
    if version != PINNED_VERSION or digest != PINNED_SHA256:
        fail(f'ODA identity changed: version={version}, sha256={digest}')
    return {'path': str(PINNED_EXE), 'version': version, 'sha256': digest}


def check_paths(source_dir: Path, output_dir: Path) -> None:
    source = source_dir.resolve(strict=True)
    destination = output_dir.resolve()
    if not source.is_dir() or source == destination or source in destination.parents or destination in source.parents:
        fail('source and destination must be separate directories with no overlap')
    if output_dir.exists() or output_dir.is_symlink():
        fail(f'output directory already exists; refusing overwrite: {output_dir}')
    if any(part == '..' for part in output_dir.parts):
        fail('output path must not contain parent traversal')


def check_sources(source_dir: Path, source_manifest: Path) -> tuple[list[dict], str]:
    if not source_manifest.is_file():
        fail(f'source manifest missing: {source_manifest}')
    with source_manifest.open(newline='') as file:
        rows = list(csv.DictReader(file))
    if not rows:
        fail('source manifest is empty')
    expected: dict[str, dict] = {}
    for row in rows:
        rel = Path(row['relative_path'])
        if len(rel.parts) != 2 or rel.parts[0] != 'DWG' or rel.name in ('.', '..'):
            fail(f'unsafe source manifest path: {rel}')
        name = rel.name
        if name in expected:
            fail(f'duplicate source manifest name: {name}')
        if row['kind'] not in ('dwg', 'zip') or Path(name).suffix.lower() != '.' + row['kind']:
            fail(f'invalid file type: {name}')
        expected[name] = row
    actual = list(source_dir.iterdir())
    if any(not p.is_file() or p.is_symlink() for p in actual):
        fail('source directory contains a subdirectory or symlink')
    if {p.name for p in actual} != set(expected):
        fail(f'source set differs from manifest: missing={sorted(set(expected)-{p.name for p in actual})}, extra={sorted({p.name for p in actual}-set(expected))}')
    for name, row in expected.items():
        p = source_dir / name
        if p.stat().st_size != int(row['bytes']) or sha256_file(p) != row['sha256']:
            fail(f'source length or SHA-256 changed: {name}')
        with p.open('rb') as file:
            prefix = file.read(6)
        if row['kind'] == 'dwg' and (prefix.decode('ascii', 'replace') != row['signature'] or not row['signature'].startswith('AC10')):
            fail(f'DWG signature changed or unsupported: {name}')
    dwgs = [r for r in rows if r['kind'] == 'dwg']
    output_names = [Path(r['relative_path']).stem.casefold() + '.dxf' for r in dwgs]
    if len(output_names) != len(set(output_names)):
        fail('two DWGs would produce the same case-insensitive DXF output name')
    return dwgs, sha256_file(source_manifest)


def check_space(output_dir: Path, source_bytes: int) -> tuple[int, int]:
    existing = output_dir.parent
    while not existing.exists():
        existing = existing.parent
    free = shutil.disk_usage(existing).free
    minimum = max(512 * 1024 * 1024, 8 * source_bytes + 256 * 1024 * 1024)
    if free < minimum:
        fail(f'insufficient free space: {free} bytes; require at least {minimum} bytes')
    return free, minimum


def inspect_dxf(path: Path) -> dict:
    data = path.read_bytes()
    if not data or data.startswith(b'AutoCAD Binary DXF'):
        fail(f'empty or binary DXF output: {path.name}')
    # DXF values can contain non-ASCII bytes such as 0x85. str.splitlines()
    # treats those as line breaks; split only on the file's CR/LF delimiters.
    lines = data.decode('latin1').replace('\r\n', '\n').replace('\r', '\n').split('\n')
    if lines[-1] == '':
        lines.pop()
    if len(lines) % 2 != 0:
        fail(f'odd group-code/value line count: {path.name}')
    pairs = []
    for i in range(0, len(lines), 2):
        code = lines[i].strip()
        if not code.isdecimal():
            fail(f'invalid DXF group code at pair {i//2}: {path.name}')
        pairs.append((int(code), lines[i+1]))
    if not pairs or pairs[-1] != (0, 'EOF'):
        fail(f'DXF does not end in EOF: {path.name}')
    section = None
    next_section = False
    version = None
    entity_count = 0
    for i, (code, value) in enumerate(pairs):
        if (code, value) == (0, 'SECTION'):
            next_section = True
        elif next_section and code == 2:
            section, next_section = value, False
        elif (code, value) == (0, 'ENDSEC'):
            section = None
        elif section == 'HEADER' and (code, value) == (9, '$ACADVER') and i+1 < len(pairs):
            version = pairs[i+1][1]
        elif section == 'ENTITIES' and code == 0:
            entity_count += 1
    if version != OUTPUT_DXF_VERSION or entity_count == 0:
        fail(f'unexpected DXF version or no entities: {path.name}, version={version}, entities={entity_count}')
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'dxf_version': version, 'entity_records': entity_count}


@contextmanager
def retain_converter_failure(output_dir: Path, command: list[str], run: subprocess.CompletedProcess):
    try:
        yield
    except (OSError, ValueError) as error:
        name = f'{output_dir.name}.failed-{uuid4().hex[:8]}.json'
        failure_path = output_dir.parent / name
        failure_path.write_text(json.dumps({
            'error': str(error), 'command': command, 'exit_code': run.returncode,
            'stdout': run.stdout, 'stderr': run.stderr,
        }, indent=2) + '\n')
        raise


def convert(source_dir: Path, source_manifest: Path, output_dir: Path, timeout: int, dry_run: bool) -> dict:
    check_paths(source_dir, output_dir)
    converter = check_converter()
    dwgs, source_manifest_sha256 = check_sources(source_dir, source_manifest)
    free_bytes, minimum_free_bytes = check_space(output_dir, sum(int(r['bytes']) for r in dwgs))
    summary = {'source_count': len(dwgs), 'opaque_zip_count': len(list(source_dir.glob('*.zip'))),
               'source_manifest_sha256': source_manifest_sha256, 'converter': converter,
               'output_dir': str(output_dir.resolve()), 'output_version': OUTPUT_VERSION,
               'audit': False, 'recursive': False, 'free_bytes_at_start': free_bytes,
               'minimum_free_bytes': minimum_free_bytes}
    if dry_run:
        summary['status'] = 'dry_run_validated'
        return summary
    output_dir.parent.mkdir(parents=True, exist_ok=True)
    # Staging and final output are siblings on one filesystem. Only a fully
    # validated release directory is renamed into the requested final path.
    with tempfile.TemporaryDirectory(prefix='.oda-stage-', dir=output_dir.parent) as stage_name:
        stage = Path(stage_name)
        input_copy = stage / 'input'
        converted = stage / 'converted'
        release = stage / 'release'
        input_copy.mkdir(); converted.mkdir(); release.mkdir()
        for row in dwgs:
            name = Path(row['relative_path']).name
            shutil.copy2(source_dir / name, input_copy / name)
        command = [str(PINNED_EXE), str(input_copy), str(converted), *CONVERTER_OPTIONS]
        try:
            run = subprocess.run(command, capture_output=True, text=True, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            failure_path = output_dir.parent / f'{output_dir.name}.failed-{uuid4().hex[:8]}.json'
            failure_path.write_text(json.dumps({
                'error': f'converter timed out after {timeout} seconds', 'command': command,
                'stdout': (error.stdout or b'').decode('utf-8', 'replace'),
                'stderr': (error.stderr or b'').decode('utf-8', 'replace'),
            }, indent=2) + '\n')
            raise
        (release / 'converter.stdout.log').write_text(run.stdout)
        (release / 'converter.stderr.log').write_text(run.stderr)
        with retain_converter_failure(output_dir, command, run):
            if run.returncode != 0:
                fail(f'ODA exited {run.returncode}; stderr={run.stderr[:500]}')
            expected_outputs = {Path(row['relative_path']).stem + '.dxf': row for row in dwgs}
            actual_outputs = list(converted.iterdir())
            if any(not p.is_file() or p.is_symlink() for p in actual_outputs):
                fail('converter made a subdirectory or symlink')
            if {p.name for p in actual_outputs} != set(expected_outputs):
                fail(f'converter output set differs: missing={sorted(set(expected_outputs)-{p.name for p in actual_outputs})}, extra={sorted({p.name for p in actual_outputs}-set(expected_outputs))}')
            out_rows = []
            for dxf_name, row in expected_outputs.items():
                source_name = Path(row['relative_path']).name
                dxf = converted / dxf_name
                metrics = inspect_dxf(dxf)
                out_rows.append({'source': source_name, 'source_bytes': int(row['bytes']),
                                 'source_sha256': row['sha256'], 'source_signature': row['signature'],
                                 'output': dxf_name, **metrics})
                dxf.rename(release / dxf_name)
            # Verify the entire source set again, including opaque ZIPs.
            check_sources(source_dir, source_manifest)
        summary.update({'status': 'converted', 'created_utc': datetime.now(timezone.utc).isoformat(),
                        'command_options': CONVERTER_OPTIONS, 'converter_exit_code': run.returncode,
                        'converter_stderr_nonempty': bool(run.stderr.strip()), 'files': out_rows,
                        'total_dxf_bytes': sum(r['bytes'] for r in out_rows)})
        manifest = release / 'conversion-manifest.json'
        tmp_manifest = release / '.conversion-manifest.json.tmp'
        tmp_manifest.write_text(json.dumps(summary, indent=2) + '\n')
        tmp_manifest.replace(manifest)
        if output_dir.exists():
            fail('output directory appeared during conversion; refusing overwrite')
        os.replace(release, output_dir)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-dir', type=Path, default=HERE/'DWG')
    parser.add_argument('--source-manifest', type=Path, default=HERE/'dwg-input-manifest-2026-09-27.csv')
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--timeout-seconds', type=int, default=600)
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    try:
        result = convert(args.input_dir, args.source_manifest, args.output_dir, args.timeout_seconds, args.dry_run)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f'conversion refused/failed: {error}', file=sys.stderr)
        return 1
    print(json.dumps({k: v for k, v in result.items() if k != 'files'}, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
