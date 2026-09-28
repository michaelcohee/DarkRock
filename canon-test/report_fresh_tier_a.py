#!/usr/bin/env python3
"""Assemble the one-shot fresh Tier A comparison from completed Z-mode ledgers."""
import csv,json,hashlib
from pathlib import Path

root=Path(__file__).resolve().parent
manifest_bytes=(root/'cad-corpus-v2'/'manifest.json').read_bytes()
corpus=json.loads(manifest_bytes)
assert corpus['corpus_id']=='darkrock-cad-dxf-v2'
controls=json.loads((root/'cad-v2-fresh-test-controls-z.json').read_text())
assert len(controls['source_paths'])==132
source_bytes=sum(int(r['bytes']) for r in corpus['files'] if r['split']=='test')
assert source_bytes==85_440_965
assert all(controls['arms'][a]['Z']['all_files_byte_and_sha256_restored'] for a in ('A1','A2','C1','C2'))
rows=[]
for arm in ('C2','C1','A2','A1'):
    item=controls['arms'][arm]['Z']
    assert item['physical']['all_21_loss_patterns_exact']=='true'
    rows.append((arm,'Protected',item['physical']['total_physical_bytes']))
for version in ('v2','v1'):
    for item in csv.DictReader((root/f'cad-v2-fresh-test-a3-{version}-z.tsv').open(),delimiter='\t'):
        assert item['files']=='132' and int(item['source_bytes'])==source_bytes and item['all_21_loss_patterns_exact']=='true'
        rows.append((item['arm'].replace('_',' '),item['view'],int(item['protected_bytes'])))
order={'C2':0,'C1':1,'A3 v2':2,'A2':3,'A1':4,'A3 v1':5}
rows.sort(key=lambda x:(order[x[0]],0 if x[1]=='Protected' else 1))
c2=next(n for arm,view,n in rows if arm=='C2')
out=['# Fresh Tier A held-out Z-mode result','',
     '**Corpus:** `darkrock-cad-dxf-v2`, 132 test DXFs, 85,440,965 source bytes. A3 v2 was frozen in commit `b0f355e` before this corpus was generated. This is the one coordinated opening of its test half. All rows use RedTail-X variable-tail 4+2, the same whole-ledger, protected-manifest catalog, and charged-bootstrap accounting, and byte-and-SHA-256 exact restore under all 21 one/two-share loss patterns.','',
     f'**Reproduction:** `DARKROCK_CORPUS_ID=darkrock-cad-dxf-v2 python3 canon-test/make_cad_corpus.py canon-test/cad-corpus-v2`; generated manifest SHA-256 `{hashlib.sha256(manifest_bytes).hexdigest()}`.','',
     '| Arm | Index view | Physical protected bytes | Versus C2 |','|---|---|---:|---:|']
for arm,view,n in rows:out.append(f'| {arm} | {view} | {n:,} | {n/c2:.2f}× |')
out.extend(['',
    'For A1, A2, C1 and C2, the serialized hash index is **Protected**. A Derived-index version of those four baselines was not built or measured. The A3 lines separately encode a Protected key index or omit it as a Derived cache; the latter must rebuild the same verified candidate set from recovered files. Neither view removes file payload or undo bytes.', '',
    '**Interpretation:** C2 is smallest. Frozen A3 v2 is 3.94× C2 with its index protected and 1.47× C2 with that index derived. The held-out result does not show a storage win for A3 v2; it confirms the development warning. The source data are the frozen corpus manifest, `cad-v2-fresh-test-controls-z.json`, and the two A3 TSV result files.',''])
(root/'cad-v2-fresh-test-report.md').write_text('\n'.join(out))
