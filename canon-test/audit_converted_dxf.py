"""Independently audit the published ODA DXF batch without modifying it."""

from __future__ import annotations

import collections
import hashlib
import json
from pathlib import Path

import ezdxf

ROOT = Path(__file__).resolve().parent
CORPUS = ROOT / "converted-dxf" / "oda-27.1-r2018-full"
MANIFEST = CORPUS / "conversion-manifest.json"
REPORT = ROOT / "oda-step5-independent-audit.json"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(part)
    return h.hexdigest()


def point(value):
    return [float(v) for v in value]


def main():
    manifest = json.loads(MANIFEST.read_text())
    results = []
    for entry in manifest["files"]:
        path = CORPUS / entry["output"]
        issues = []
        if not path.is_file() or path.stat().st_size != entry["bytes"]:
            issues.append("missing or size mismatch")
        if path.is_file() and sha256(path) != entry["sha256"]:
            issues.append("hash mismatch")
        if issues:
            results.append({"file": entry["output"], "issues": issues})
            continue
        try:
            doc = ezdxf.readfile(path)
            auditor = doc.audit()
            model = doc.modelspace()
            types = collections.Counter(entity.dxftype() for entity in model)
            layers = sorted(layer.dxf.name for layer in doc.layers)
            blocks = sorted(block.name for block in doc.blocks)
            # Header extents are recorded claims, not independently recomputed
            # geometry bounds. Full recursive bounds are too costly for some
            # large drawings and can expand unsupported proxy objects.
            extents = {
                "min": point(doc.header.get("$EXTMIN", (0, 0, 0))),
                "max": point(doc.header.get("$EXTMAX", (0, 0, 0))),
            }
            if auditor.errors:
                issues.append(f"ezdxf audit errors: {len(auditor.errors)}")
            if auditor.fixes:
                issues.append(f"ezdxf audit fixes: {len(auditor.fixes)}")
            if not types:
                issues.append("empty modelspace")
            if doc.dxfversion != "AC1032":
                issues.append(f"unexpected version: {doc.dxfversion}")
            results.append(
                {
                    "file": entry["output"],
                    "source": entry["source"],
                    "sha256": entry["sha256"],
                    "version": doc.dxfversion,
                    "audit_errors": len(auditor.errors),
                    "audit_fixes": len(auditor.fixes),
                    "audit_fix_messages": dict(
                        collections.Counter(fix.message.split("(#", 1)[0] for fix in auditor.fixes)
                    ),
                    "modelspace_entities": sum(types.values()),
                    "modelspace_types": dict(sorted(types.items())),
                    "layer_count": len(layers),
                    "layers": layers,
                    "block_count": len(blocks),
                    "blocks": blocks,
                    "modelspace_extents": extents,
                    "issues": issues,
                }
            )
        except Exception as error:
            results.append(
                {"file": entry["output"], "issues": [f"{type(error).__name__}: {error}"]}
            )
    totals = {
        "files": len(results),
        "files_with_issues": sum(bool(r["issues"]) for r in results),
        "audit_errors": sum(r.get("audit_errors", 0) for r in results),
        "audit_fixes": sum(r.get("audit_fixes", 0) for r in results),
        "modelspace_entities": sum(r.get("modelspace_entities", 0) for r in results),
        "total_layers": sum(r.get("layer_count", 0) for r in results),
        "total_blocks": sum(r.get("block_count", 0) for r in results),
    }
    output = {
        "reader": f"ezdxf {ezdxf.__version__}",
        "corpus_manifest_sha256": sha256(MANIFEST),
        "totals": totals,
        "files": results,
    }
    REPORT.write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps(totals))
    print(REPORT)
    if totals["files_with_issues"]:
        for result in results:
            if result["issues"]:
                print(result["file"], result["issues"])
        raise SystemExit(1)


if __name__ == "__main__":
    main()
