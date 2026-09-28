"""Render representative published DXFs for visual inspection."""

from pathlib import Path
import json

import cairosvg
import ezdxf
from ezdxf.addons.drawing import Frontend, RenderContext, svg
from ezdxf.addons.drawing.layout import Margins, Page, Units

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "converted-dxf" / "oda-27.1-r2018-full"
DEST = ROOT / "step5-previews"
SAMPLES = [
    "081476_series-440---sliding-jamb-options.dxf",  # R2000
    "08392108.dxf",  # R2004
    "08495101.dxf",  # R2007
    "102313_solare-double---23-double-glazed-sliding-door---sill.dxf",  # R2010
    "arbusto_PB_verde_grande.dxf",  # R2018
    "085313.20_new-construction-oxo-3-lite-vinyl-patio-door-as-432.dxf",  # audit-fix case
]


def main():
    DEST.mkdir(exist_ok=True)
    results = []
    for filename in SAMPLES:
        doc = ezdxf.readfile(SOURCE / filename)
        backend = svg.SVGBackend()
        Frontend(RenderContext(doc), backend).draw_layout(doc.modelspace(), finalize=True)
        page = Page(1200, 900, Units.px, margins=Margins.all(20))
        drawing = backend.get_string(page)
        target = DEST / (Path(filename).stem + ".png")
        cairosvg.svg2png(bytestring=drawing.encode(), write_to=str(target))
        results.append({"source": filename, "preview": target.name, "png_bytes": target.stat().st_size})
        print(filename, target.stat().st_size)
    (DEST / "manifest.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
