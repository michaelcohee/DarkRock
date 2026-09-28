#!/usr/bin/env python3
"""Independent check of darkrock-cad-dxf-v1: re-hash every file, parse every DXF,
and confirm each variant's ground-truth relation to its base.

    python3 verify_corpus.py CORPUS_DIR
"""
import csv, hashlib, os, sys
from collections import Counter

def parse(data):
    lines = data.decode("ascii").splitlines()
    assert len(lines) % 2 == 0, "odd line count"
    pairs = [(int(lines[i]), lines[i + 1]) for i in range(0, len(lines), 2)]
    assert pairs[-1] == (0, "EOF")
    header, layers, ents, sec, cur = {}, [], [], None, None
    depth = 0; var = None
    for code, val in pairs:
        if code == 0 and val == "SECTION": depth += 1; continue
        if code == 0 and val == "ENDSEC": depth -= 1; sec = None; continue
        if code == 2 and sec is None and depth == 1 and val in ("HEADER", "TABLES", "BLOCKS", "ENTITIES"): sec = val; continue
        if sec == "HEADER":
            if code == 9: var = val; header[var] = []
            else: header[var].append((code, float(val) if code in (10, 20, 30, 40) else val))
        elif sec == "TABLES" and code == 0 and val == "LAYER": layers.append(None)
        elif sec == "TABLES" and code == 2 and layers and layers[-1] is None: layers[-1] = val
        elif sec == "ENTITIES":
            if code == 0:
                if val in ("VERTEX", "SEQEND"):
                    cur.append(("0", val))
                else:
                    cur = [("0", val)]; ents.append(cur)
            else:
                v = float(val) if 10 <= code <= 59 else val
                cur.append((code, v))
    assert depth == 0, "unbalanced sections"
    return header, layers, [tuple(e) for e in ents]

def main(root):
    rows = list(csv.DictReader(open(os.path.join(root, "manifest.csv"))))
    parsed, errors = {}, []
    for r in rows:
        data = open(os.path.join(root, r["path"]), "rb").read()
        if hashlib.sha256(data).hexdigest() != r["sha256"]: errors.append(f"hash mismatch {r['path']}")
        if len(data) != int(r["bytes"]): errors.append(f"size mismatch {r['path']}")
        parsed[r["path"]] = (data, parse(data))
    for r in rows:
        data, (h, lay, ents) = parsed[r["path"]]
        bdata, (bh, blay, bents) = parsed[r["base"]]
        rel, v = r["relation"], r["variant"]
        same_bytes = data == bdata
        same_ents_ordered = ents == bents
        same_ents_multiset = Counter(ents) == Counter(bents)
        hdr_diff = sorted(k for k in set(h) | set(bh) if h.get(k) != bh.get(k))
        ok = {
            "base": same_bytes,
            "identical": same_bytes,
            "content_equal": (not same_bytes) and same_ents_multiset and lay == blay and
                             (hdr_diff == ["$TDUPDATE"] if v.startswith("v02") else hdr_diff == []) and
                             (not same_ents_ordered if v.startswith("v03") else same_ents_ordered),
            "renamed": (not same_bytes) and lay != blay and len(ents) == len(bents),
            "congruent": (not same_bytes) and len(ents) == len(bents) and not same_ents_multiset,
            "scaled": (not same_bytes) and len(ents) == len(bents) and not same_ents_multiset,
            "tiny_diff": (not same_bytes) and len(ents) == len(bents) and not same_ents_multiset and lay == blay,
            "design_change": (not same_bytes) and len(ents) == len(bents) and
                             sum(a != b for a, b in zip(ents, bents)) == 1,
        }[rel]
        if not ok: errors.append(f"relation '{rel}' not satisfied: {r['path']}  hdr_diff={hdr_diff}")
    n = len(rows); tot = sum(int(r["bytes"]) for r in rows)
    print(f"checked {n} files, {tot:,} bytes; {len(errors)} errors")
    for e in errors[:50]: print("  ", e)
    sys.exit(1 if errors else 0)

if __name__ == "__main__":
    main(sys.argv[1])
