#!/usr/bin/env python3
"""
DarkRock CAD canonizer test corpus generator  (corpus id: darkrock-cad-dxf-v1)

Writes AutoCAD R12 ASCII DXF files (AC1009; opens in AutoCAD, LibreCAD, QCAD,
DraftSight, ezdxf) plus a ground-truth manifest.  Fully deterministic: the
same script version always produces byte-identical files (check SHA256SUMS).

    python3 make_cad_corpus.py OUT_DIR

Layout:  OUT_DIR/{dev,test}/<family>/<family>__vNN_<variant>.dxf
         OUT_DIR/manifest.csv, OUT_DIR/manifest.json, OUT_DIR/SHA256SUMS

Every base drawing gets the same 11 variants, each with a known relationship
to its base (see VARIANTS below and the spec, section 3).
No third-party packages required.
"""
import argparse, csv, hashlib, json, math, os, random, sys

CORPUS_ID = os.environ.get("DARKROCK_CORPUS_ID", "darkrock-cad-dxf-v1")
Q = 6  # all stored geometry is quantised to 6 decimals before writing

# ----------------------------------------------------------------- entities
# Entity = dict(t=type, layer=str, ...). Points are (x, y) tuples.

def L(layer, p1, p2):            return dict(t="LINE", layer=layer, p1=p1, p2=p2)
def C(layer, c, r):              return dict(t="CIRCLE", layer=layer, c=c, r=r)
def A(layer, c, r, a0, a1):      return dict(t="ARC", layer=layer, c=c, r=r, a0=a0, a1=a1)
def T(layer, p, h, s, rot=0.0):  return dict(t="TEXT", layer=layer, p=p, h=h, s=s, rot=rot)
def P(layer, pts, closed=False): return dict(t="POLYLINE", layer=layer, pts=list(pts), closed=closed)
def PT(layer, p):                return dict(t="POINT", layer=layer, p=p)

def rect(layer, x, y, w, h):
    return P(layer, [(x, y), (x + w, y), (x + w, y + h), (x, y + h)], True)

def q(v):   # quantise
    r = round(v, Q)
    return 0.0 if r == 0 else r

def qp(p):  return (q(p[0]), q(p[1]))

def quantise(ents):
    out = []
    for e in ents:
        e = dict(e)
        for k in ("p1", "p2", "c", "p"):
            if k in e: e[k] = qp(e[k])
        for k in ("r", "h", "a0", "a1", "rot"):
            if k in e: e[k] = q(e[k])
        if "pts" in e: e["pts"] = [qp(p) for p in e["pts"]]
        out.append(e)
    return out

# ------------------------------------------------------------------ writer
def num_short(v):   # AutoCAD-like shortest form: 12.5, 0.0, 100.0
    s = repr(float(v))
    if "e" in s or "E" in s:
        s = f"{v:.{Q}f}".rstrip("0")
        if s.endswith("."): s += "0"
    return s

def num_fixed(v):   # same value, fixed 6-decimal form: 12.500000
    return f"{v:.{Q}f}"

def gc(out, code, val):
    out.append(f"{code:>3}")
    out.append(str(val))

def write_dxf(doc, fmt="short", eol="\n"):
    f = num_short if fmt == "short" else num_fixed
    o = []
    for c in doc.get("comments", []):
        gc(o, 999, c)
    # HEADER
    gc(o, 0, "SECTION"); gc(o, 2, "HEADER")
    gc(o, 9, "$ACADVER"); gc(o, 1, "AC1009")
    gc(o, 9, "$INSBASE"); gc(o, 10, f(0.0)); gc(o, 20, f(0.0)); gc(o, 30, f(0.0))
    (x0, y0), (x1, y1) = doc["ext"]
    gc(o, 9, "$EXTMIN"); gc(o, 10, f(x0)); gc(o, 20, f(y0)); gc(o, 30, f(0.0))
    gc(o, 9, "$EXTMAX"); gc(o, 10, f(x1)); gc(o, 20, f(y1)); gc(o, 30, f(0.0))
    gc(o, 9, "$LUNITS"); gc(o, 70, 2)
    gc(o, 9, "$LUPREC"); gc(o, 70, 4)
    gc(o, 9, "$TDCREATE"); gc(o, 40, f"{doc['tdcreate']:.8f}")
    gc(o, 9, "$TDUPDATE"); gc(o, 40, f"{doc['tdupdate']:.8f}")
    gc(o, 0, "ENDSEC")
    # TABLES
    gc(o, 0, "SECTION"); gc(o, 2, "TABLES")
    gc(o, 0, "TABLE"); gc(o, 2, "LTYPE"); gc(o, 70, 1)
    gc(o, 0, "LTYPE"); gc(o, 2, "CONTINUOUS"); gc(o, 70, 0); gc(o, 3, "Solid line")
    gc(o, 72, 65); gc(o, 73, 0); gc(o, 40, f(0.0))
    gc(o, 0, "ENDTAB")
    gc(o, 0, "TABLE"); gc(o, 2, "LAYER"); gc(o, 70, len(doc["layers"]))
    for name, color in doc["layers"]:
        gc(o, 0, "LAYER"); gc(o, 2, name); gc(o, 70, 0); gc(o, 62, color); gc(o, 6, "CONTINUOUS")
    gc(o, 0, "ENDTAB")
    gc(o, 0, "TABLE"); gc(o, 2, "STYLE"); gc(o, 70, 1)
    gc(o, 0, "STYLE"); gc(o, 2, "STANDARD"); gc(o, 70, 0); gc(o, 40, f(0.0)); gc(o, 41, f(1.0))
    gc(o, 50, f(0.0)); gc(o, 71, 0); gc(o, 42, f(2.5)); gc(o, 3, "txt"); gc(o, 4, "")
    gc(o, 0, "ENDTAB")
    gc(o, 0, "ENDSEC")
    gc(o, 0, "SECTION"); gc(o, 2, "BLOCKS"); gc(o, 0, "ENDSEC")
    # ENTITIES
    gc(o, 0, "SECTION"); gc(o, 2, "ENTITIES")
    for e in doc["ents"]:
        t = e["t"]
        gc(o, 0, t); gc(o, 8, e["layer"])
        if t == "LINE":
            gc(o, 10, f(e["p1"][0])); gc(o, 20, f(e["p1"][1])); gc(o, 30, f(0.0))
            gc(o, 11, f(e["p2"][0])); gc(o, 21, f(e["p2"][1])); gc(o, 31, f(0.0))
        elif t in ("CIRCLE", "ARC"):
            gc(o, 10, f(e["c"][0])); gc(o, 20, f(e["c"][1])); gc(o, 30, f(0.0)); gc(o, 40, f(e["r"]))
            if t == "ARC":
                gc(o, 50, f(e["a0"])); gc(o, 51, f(e["a1"]))
        elif t == "TEXT":
            gc(o, 10, f(e["p"][0])); gc(o, 20, f(e["p"][1])); gc(o, 30, f(0.0))
            gc(o, 40, f(e["h"])); gc(o, 1, e["s"])
            if e["rot"]: gc(o, 50, f(e["rot"]))
        elif t == "POINT":
            gc(o, 10, f(e["p"][0])); gc(o, 20, f(e["p"][1])); gc(o, 30, f(0.0))
        elif t == "POLYLINE":
            gc(o, 66, 1); gc(o, 10, f(0.0)); gc(o, 20, f(0.0)); gc(o, 30, f(0.0))
            gc(o, 70, 1 if e["closed"] else 0)
            for (x, y) in e["pts"]:
                gc(o, 0, "VERTEX"); gc(o, 8, e["layer"])
                gc(o, 10, f(x)); gc(o, 20, f(y)); gc(o, 30, f(0.0))
            gc(o, 0, "SEQEND"); gc(o, 8, e["layer"])
    gc(o, 0, "ENDSEC")
    gc(o, 0, "EOF")
    return (eol.join(o) + eol).encode("ascii")

# ------------------------------------------------------------- transforms
def map_ents(ents, fp, fl=lambda v: v, fa=lambda v: v):
    out = []
    for e in ents:
        e = dict(e)
        for k in ("p1", "p2", "c", "p"):
            if k in e: e[k] = fp(e[k])
        if "pts" in e: e["pts"] = [fp(p) for p in e["pts"]]
        for k in ("r", "h"):
            if k in e: e[k] = fl(e[k])
        for k in ("a0", "a1"):
            if k in e: e[k] = fa(e[k]) % 360.0
        if e["t"] == "TEXT": e["rot"] = fa(e["rot"]) % 360.0
        out.append(e)
    return quantise(out)

def extents(ents):
    xs, ys = [], []
    for e in ents:
        pts = e.get("pts") or [e[k] for k in ("p1", "p2", "c", "p") if k in e]
        for x, y in pts:
            xs.append(x); ys.append(y)
        if "r" in e:
            xs += [e["c"][0] - e["r"], e["c"][0] + e["r"]]; ys += [e["c"][1] - e["r"], e["c"][1] + e["r"]]
    return ((q(min(xs)), q(min(ys))), (q(max(xs)), q(max(ys))))

# ------------------------------------------------------- drawing families
def fam_title_block(rng, w, h, sheet):
    E = [rect("BORDER", 0, 0, w, h), rect("BORDER", 10, 10, w - 20, h - 20)]
    tbw, tbh = min(180, w * 0.45), 60
    if CORPUS_ID != "darkrock-cad-dxf-v1":
        tbw *= rng.choice([0.82, 0.87, 0.93, 1.07])
        tbh = rng.randint(52, 68)
    x, y = w - 10 - tbw, 10
    E.append(rect("TITLE", x, y, tbw, tbh))
    for i in range(1, 4): E.append(L("TITLE", (x, y + i * tbh / 4), (x + tbw, y + i * tbh / 4)))
    E.append(L("TITLE", (x + tbw * 0.6, y), (x + tbw * 0.6, y + tbh * 0.75)))
    labels = ["TITLE", "DWG NO", "REV", "SCALE", "DATE", "DRAWN BY", "CHECKED", "SHEET"]
    for i, lab in enumerate(labels):
        E.append(T("TITLE", (x + 2 + (i % 2) * tbw * 0.6, y + 2 + (i // 2) * tbh / 4), 2.5, lab))
    E.append(T("TITLE", (x + 4, y + tbh * 0.8), 5.0, f"{sheet} TEMPLATE"))
    nz = int(w // 50)
    for i in range(nz):
        E.append(L("BORDER", (10 + i * (w - 20) / nz, 0), (10 + i * (w - 20) / nz, 10)))
        E.append(T("BORDER", (10 + (i + 0.4) * (w - 20) / nz, 3), 3.5, str(i + 1)))
    for j in range(int(h // 50)):
        E.append(T("BORDER", (3, 10 + (j + 0.4) * (h - 20) / (h // 50)), 3.5, chr(65 + j)))
    # revision table rows
    for r in range(12):
        E.append(L("TITLE", (x, h - 30 - r * 6), (x + tbw, h - 30 - r * 6)))
        E.append(T("TITLE", (x + 2, h - 28 - r * 6), 2.0, f"REV {chr(65 + r)}  -  ISSUED FOR REVIEW  -  2026-{(r % 12) + 1:02d}-15"))
    return E, [("BORDER", 7), ("TITLE", 3)]

def fam_flange(rng, n_parts):
    E = []
    for k in range(n_parts):
        cx, cy = (k % 6) * 260.0, (k // 6) * 260.0
        R = rng.uniform(80, 110); bore = rng.uniform(20, 40); n = rng.choice([4, 6, 8, 12]); bc = R * 0.72
        E += [C("PART", (cx, cy), R), C("PART", (cx, cy), bore), C("CENTER", (cx, cy), bc)]
        for i in range(n):
            a = 2 * math.pi * i / n
            E.append(C("HOLES", (cx + bc * math.cos(a), cy + bc * math.sin(a)), rng.uniform(5, 9)))
        E += [L("CENTER", (cx - R - 10, cy), (cx + R + 10, cy)), L("CENTER", (cx, cy - R - 10), (cx, cy + R + 10))]
        E.append(T("TEXT", (cx - R, cy - R - 18), 6, f"FLANGE F-{k + 1:03d}  {n}x HOLES ON BC {bc:.1f}"))
    return E, [("PART", 7), ("HOLES", 1), ("CENTER", 4), ("TEXT", 2)]

def fam_gear(rng, teeth, per_tooth, n_gears):
    E = []
    for g in range(n_gears):
        cx = g * 700.0
        rr, ra = 300.0 - g * 20, 320.0 - g * 20
        pts = []
        for i in range(teeth * per_tooth):
            a = 2 * math.pi * i / (teeth * per_tooth)
            phase = (i % per_tooth) / per_tooth
            r = ra if 0.25 < phase < 0.65 else rr + (ra - rr) * max(0.0, 1 - abs(phase - 0.45) * 4)
            pts.append((cx + r * math.cos(a), r * math.sin(a)))
        E.append(P("GEAR", pts, True))
        E += [C("GEAR", (cx, 0), 40), C("CENTER", (cx, 0), (rr + ra) / 2)]
        E.append(T("TEXT", (cx - 100, -380), 12, f"SPUR GEAR  Z={teeth}  M={2 * ra / (teeth + 2):.3f}"))
    return E, [("GEAR", 7), ("CENTER", 4), ("TEXT", 2)]

def fam_floor_plan(rng, rows, cols):
    E = []
    W, H, t = 6000.0, 4500.0, 150.0
    for r in range(rows):
        for c in range(cols):
            x, y = c * W, r * H
            for (a, b) in [((x, y), (x + W, y)), ((x + W, y), (x + W, y + H)), ((x + W, y + H), (x, y + H)), ((x, y + H), (x, y))]:
                E.append(L("A-WALL", a, b))
            E.append(rect("A-WALL", x + t, y + t, W - 2 * t, H - 2 * t))
            E.append(A("A-DOOR", (x + 600, y + t), 900, 0, 90))
            E.append(L("A-DOOR", (x + 600, y + t), (x + 600, y + t + 900)))
            E.append(T("A-ANNO", (x + W / 2 - 800, y + H / 2), 250, f"OFFICE {r + 1:02d}{c + 1:02d}"))
            E.append(T("A-ANNO", (x + W / 2 - 800, y + H / 2 - 400), 180, f"{(W * H) / 1e6:.1f} m2"))
            for k in range(rng.randint(2, 5)):   # furniture
                fx, fy = x + rng.uniform(500, W - 1800), y + rng.uniform(500, H - 1200)
                E.append(rect("A-FURN", fx, fy, rng.choice([1200, 1600, 1800]), rng.choice([600, 800])))
                E.append(C("A-FURN", (fx + 300, fy - 300), 250))
    return E, [("A-WALL", 7), ("A-DOOR", 3), ("A-ANNO", 2), ("A-FURN", 8)]

def fam_contours(rng, n_rings, pts_per):
    E = []
    hills = [(rng.uniform(-3000, 3000), rng.uniform(-3000, 3000), rng.uniform(800, 2500)) for _ in range(4)]
    for (hx, hy, hr) in hills:
        phases = [rng.uniform(0, 6.28) for _ in range(4)]
        for k in range(n_rings):
            base = hr * (1 - k / n_rings)
            pts = []
            for i in range(pts_per):
                a = 2 * math.pi * i / pts_per
                r = base * (1 + 0.08 * math.sin(3 * a + phases[0]) + 0.05 * math.sin(5 * a + phases[1])
                            + 0.03 * math.sin(11 * a + phases[2] + k * 0.1))
                pts.append((hx + r * math.cos(a), hy + r * math.sin(a)))
            E.append(P("C-TOPO-MAJR" if k % 5 == 0 else "C-TOPO-MINR", pts, True))
            if k % 5 == 0:
                E.append(T("C-TOPO-TEXT", (hx + base, hy), 60, f"EL {100 + k * 0.5:.1f}"))
    return E, [("C-TOPO-MAJR", 1), ("C-TOPO-MINR", 8), ("C-TOPO-TEXT", 2)]

def fam_pcb(rng, n_holes):
    E = [rect("BOARD", 0, 0, 160, 100)]
    for i in range(n_holes):
        x, y = rng.uniform(3, 157), rng.uniform(3, 97)
        d = rng.choice([0.3, 0.4, 0.6, 0.8, 1.0, 3.2])
        E.append(C("DRILL", (x, y), d / 2))
        if d < 1.0: E.append(C("PAD", (x, y), d / 2 + 0.25))
    for i in range(n_holes // 20):
        E.append(T("SILK", (rng.uniform(3, 150), rng.uniform(3, 95)), 1.0, f"U{i + 1}"))
    return E, [("BOARD", 7), ("DRILL", 1), ("PAD", 3), ("SILK", 2)]

def fam_stair(rng, steps):
    E = []
    rise, run = 175.0, 280.0
    if CORPUS_ID != "darkrock-cad-dxf-v1":
        rise, run = rng.uniform(165.0, 190.0), rng.uniform(260.0, 305.0)
    x = y = 0.0
    for i in range(steps):
        E.append(L("S-STAIR", (x, y), (x, y + rise))); E.append(L("S-STAIR", (x, y + rise), (x + run, y + rise)))
        E.append(T("S-ANNO", (x + 20, y + rise + 20), 40, f"T{i + 1}"))
        x += run; y += rise
    E.append(L("S-STAIR", (0, -200), (x, y - 200)))
    E.append(T("S-ANNO", (0, -500), 80, f"STAIR SECTION  {steps}R @ {rise:.0f} / {steps - 1}T @ {run:.0f}"))
    return E, [("S-STAIR", 7), ("S-ANNO", 2)]

def fam_hex(rng, n):
    E = []
    for k in range(n):
        cx, cy = (k % 20) * 40.0, (k // 20) * 40.0
        s = rng.choice([8.0, 10.0, 13.0, 17.0]) / 2
        R = s / math.cos(math.pi / 6)
        E.append(P("NUT", [(cx + R * math.cos(math.pi / 3 * i), cy + R * math.sin(math.pi / 3 * i)) for i in range(6)], True))
        E.append(C("NUT", (cx, cy), s * 0.55)); E.append(C("NUT", (cx, cy), s * 0.95))
        E.append(T("TEXT", (cx - 6, cy - s - 5), 2.0, f"M{int(s * 2 * 0.6)}"))
    return E, [("NUT", 7), ("TEXT", 2)]

def fam_freestyle(rng, strokes, pts_per):
    E = []
    for s in range(strokes):
        x, y = rng.uniform(0, 2000), rng.uniform(0, 1400)
        a, da = rng.uniform(0, 6.28), 0.0
        pts = []
        for i in range(pts_per):
            da = 0.85 * da + rng.gauss(0, 0.05)
            a += da
            step = rng.uniform(1.5, 4.0)
            x += step * math.cos(a); y += step * math.sin(a)
            pts.append((x, y))
        E.append(P(f"SKETCH-{s % 6}", pts, False))
    return E, [(f"SKETCH-{i}", i + 1) for i in range(6)]

def fam_table(rng, rows):
    cols = ["DOOR NO", "WIDTH", "HEIGHT", "TYPE", "FRAME", "FIRE RATING", "HARDWARE SET", "REMARKS"]
    widths = [60, 40, 40, 40, 50, 50, 70, 160]
    E = []
    X = [sum(widths[:i]) for i in range(len(widths) + 1)]
    for r in range(rows + 2):
        E.append(L("G-TABL", (0, -r * 10.0), (X[-1], -r * 10.0)))
    for x in X:
        E.append(L("G-TABL", (x, 0), (x, -(rows + 1) * 10.0)))
    for i, c in enumerate(cols):
        E.append(T("G-TEXT", (X[i] + 2, -7), 3.0, c))
    for r in range(rows):
        vals = [f"D{r + 101:04d}", str(rng.choice([810, 910, 1010, 1210])), str(rng.choice([2100, 2400])),
                rng.choice(["A", "B", "C", "F1"]), rng.choice(["HM", "AL", "WD"]), rng.choice(["-", "20 MIN", "60 MIN", "90 MIN"]),
                f"HW-{rng.randint(1, 24):02d}", rng.choice(["", "CLOSER", "PANIC BAR", "CARD READER", "VISION PANEL 300x600"])]
        for i, v in enumerate(vals):
            if v: E.append(T("G-TEXT", (X[i] + 2, -(r + 1) * 10.0 - 7), 2.5, v))
    return E, [("G-TABL", 7), ("G-TEXT", 2)]

def fam_bracket(rng, n):
    E = []
    for k in range(n):
        ox = k * 300.0
        w, h, t = rng.uniform(120, 200), rng.uniform(100, 160), rng.uniform(8, 14)
        E += [L("PART", (ox, 0), (ox + w, 0)), L("PART", (ox + w, 0), (ox + w, t)), L("PART", (ox + w, t), (ox + t + 10, t)),
              A("PART", (ox + t + 10, t + 10), 10, 180, 270), L("PART", (ox + t, t + 10), (ox + t, h)),
              L("PART", (ox + t, h), (ox, h)), L("PART", (ox, h), (ox, 0))]
        for i in range(3):
            E.append(C("HOLES", (ox + t + 30 + i * (w - t - 50) / 2, t / 2), 3.3))
        E.append(T("TEXT", (ox, -20), 6, f"BRACKET B-{k + 1:02d} t={t:.1f}"))
    return E, [("PART", 7), ("HOLES", 1), ("TEXT", 2)]

def fam_parking(rng, bays):
    E = []
    for b in range(bays):
        x = (b % 60) * 2500.0; y = (b // 60) * 13000.0
        E.append(L("C-PKNG-STRP", (x, y), (x, y + 5000))); E.append(L("C-PKNG-STRP", (x, y + 8000), (x, y + 13000)))
        E.append(T("C-PKNG-TEXT", (x + 900, y + 2500), 300, str(b + 1)))
        if rng.random() < 0.05:
            E.append(rect("C-PKNG-ADA", x + 200, y + 200, 2100, 4600))
            E.append(T("C-PKNG-ADA", (x + 700, y + 1200), 250, "ADA"))
    return E, [("C-PKNG-STRP", 7), ("C-PKNG-TEXT", 2), ("C-PKNG-ADA", 5)]

def fam_panel(rng, panels):
    E = []
    for p in range(panels):
        ox = p * 400.0
        E.append(rect("E-PANEL", ox, 0, 300, 900))
        E.append(T("E-TEXT", (ox + 10, 910), 12, f"PANEL LP-{p + 1}  208Y/120V 3PH 4W"))
        for i in range(42):
            side = i % 2; row = i // 2
            x = ox + (20 if side == 0 else 160); y = 860 - row * 40
            E.append(rect("E-BRKR", x, y - 30, 120, 30))
            E.append(T("E-TEXT", (x + 4, y - 20), 6, f"{i + 1:02d} {rng.choice(['LTG', 'REC', 'HVAC', 'SPARE', 'EF', 'WH'])} {rng.choice([15, 20, 30])}A"))
            E.append(L("E-WIRE", (x + (120 if side == 0 else 0), y - 15), (x + (140 if side == 0 else -20), y - 15)))
    return E, [("E-PANEL", 7), ("E-BRKR", 3), ("E-TEXT", 2), ("E-WIRE", 1)]

def fam_truss(rng, spans):
    E = []
    for s in range(spans):
        oy = s * 3000.0
        n = rng.randint(8, 14); span = rng.uniform(12000, 24000); rise = span * rng.uniform(0.15, 0.25)
        top = [(i * span / n, oy + rise * (1 - abs(2 * i / n - 1))) for i in range(n + 1)]
        bot = [(i * span / n, oy) for i in range(n + 1)]
        for i in range(n):
            E += [L("S-TRUS-CHRD", top[i], top[i + 1]), L("S-TRUS-CHRD", bot[i], bot[i + 1]),
                  L("S-TRUS-WEB", bot[i], top[i + 1] if i < n // 2 else top[i])]
        for i in range(1, n): E.append(L("S-TRUS-WEB", bot[i], top[i]))
        for pnt in top + bot: E.append(C("S-TRUS-NODE", pnt, 40))
        E.append(T("S-ANNO", (0, oy - 400), 150, f"TRUSS T{s + 1}  SPAN {span / 1000:.2f} m  {n} PANELS"))
    return E, [("S-TRUS-CHRD", 7), ("S-TRUS-WEB", 3), ("S-TRUS-NODE", 1), ("S-ANNO", 2)]

# family id -> (split, generator, kwargs). "dev" families may be used to build/tune
# the adapter; "test" families must not be looked at until the frozen run.
FAMILIES = [
    ("f00_title_block_ansi_d",  "dev",  lambda r: fam_title_block(r, 864, 559, "ANSI D")),
    ("f01_title_block_iso_a1",  "test", lambda r: fam_title_block(r, 841, 594, "ISO A1")),
    ("f02_flange_plates",       "test", lambda r: fam_flange(r, 60)),
    ("f03_spur_gears",          "dev",  lambda r: fam_gear(r, 96, 28, 3)),
    ("f04_floor_plan_office",   "test", lambda r: fam_floor_plan(r, 14, 16)),
    ("f05_site_contours",       "test", lambda r: fam_contours(r, 60, 180)),
    ("f06_pcb_drill_map",       "test", lambda r: fam_pcb(r, 6000)),
    ("f07_stair_section",       "test", lambda r: fam_stair(r, 18)),
    ("f08_hex_nut_array",       "dev",  lambda r: fam_hex(r, 1200)),
    ("f09_freestyle_sketch_a",  "test", lambda r: fam_freestyle(r, 90, 260)),
    ("f10_freestyle_sketch_b",  "dev",  lambda r: fam_freestyle(r, 50, 240)),
    ("f11_door_schedule",       "test", lambda r: fam_table(r, 900)),
    ("f12_bracket_parts",       "test", lambda r: fam_bracket(r, 24)),
    ("f13_parking_layout",      "test", lambda r: fam_parking(r, 1500)),
    ("f14_electrical_panels",   "test", lambda r: fam_panel(r, 40)),
    ("f15_truss_elevations",    "test", lambda r: fam_truss(r, 30)),
]

# ---------------------------------------------------------------- variants
# relation classes (ground truth, see spec section 3):
#   identical        byte-identical copy
#   content_equal    same drawing content; bytes differ only in timestamp / order / number spelling
#   renamed          same geometry, layer names changed
#   congruent        same shape moved or rotated (every coordinate changes)
#   scaled           same part in other units (x25.4)
#   tiny_diff        a few coordinates differ in the 6th decimal (TRAP: must not be merged losslessly-free)
#   design_change    one real entity edit (TRAP: must not be merged losslessly-free)
VARIANTS = [
    ("v00_base",            "base"),
    ("v01_exact_copy",      "identical"),
    ("v02_resave_timestamp","content_equal"),
    ("v03_entity_reorder",  "content_equal"),
    ("v04_number_format",   "content_equal"),
    ("v05_layer_rename",    "renamed"),
    ("v06_translate",       "congruent"),
    ("v07_rotate90",        "congruent"),
    ("v08_units_mm",        "scaled"),
    ("v09_tiny_float_diff", "tiny_diff"),
    ("v10_one_entity_edit", "design_change"),
]

RENAME = {"A-WALL": "WALLS", "A-DOOR": "DOORS", "A-ANNO": "ROOM-TAGS", "A-FURN": "FURNITURE", "PART": "0-PART",
          "HOLES": "0-HOLES", "TEXT": "NOTES", "CENTER": "CL", "BORDER": "FRAME", "TITLE": "TBLK"}

def make_variant(base, vname, rng):
    d = dict(base); d["ents"] = list(base["ents"]); d["comments"] = list(base["comments"])
    fmt = "short"; changed = 0
    if vname == "v02_resave_timestamp":
        d["tdupdate"] = base["tdupdate"] + 3.41666667
    elif vname == "v03_entity_reorder":
        rng.shuffle(d["ents"])
    elif vname == "v04_number_format":
        fmt = "fixed"
    elif vname == "v05_layer_rename":
        ren = {n: RENAME.get(n, "X-" + n) for n, _ in base["layers"]}
        d["layers"] = [(ren[n], c) for n, c in base["layers"]]
        d["ents"] = [dict(e, layer=ren[e["layer"]]) for e in base["ents"]]
    elif vname == "v06_translate":
        dx, dy = 12345.678, -2345.5
        d["ents"] = map_ents(base["ents"], lambda p: (p[0] + dx, p[1] + dy))
    elif vname == "v07_rotate90":
        d["ents"] = map_ents(base["ents"], lambda p: (-p[1], p[0]), fa=lambda a: a + 90.0)
    elif vname == "v08_units_mm":
        s = 25.4
        d["ents"] = map_ents(base["ents"], lambda p: (p[0] * s, p[1] * s), fl=lambda v: v * s)
        d["comments"] = d["comments"] + ["units: millimetres (source inches x 25.4)"]
    elif vname == "v09_tiny_float_diff":
        ents = []
        for e in base["ents"]:
            e = dict(e)
            if e["t"] == "CIRCLE" and rng.random() < 0.02:
                e["c"] = (q(e["c"][0] + 1e-6), e["c"][1]); changed += 1
            elif e["t"] == "LINE" and rng.random() < 0.02:
                e["p2"] = (e["p2"][0], q(e["p2"][1] - 1e-6)); changed += 1
            elif e["t"] == "POLYLINE" and rng.random() < 0.10:
                pts = list(e["pts"]); i = rng.randrange(len(pts)); pts[i] = (q(pts[i][0] + 1e-6), pts[i][1])
                e["pts"] = pts; changed += 1
            ents.append(e)
        if changed == 0:  # guarantee at least one
            e = dict(ents[0]); k = next(k for k in ("p1", "c", "p") if k in e) if "pts" not in e else None
            if k: e[k] = (q(e[k][0] + 1e-6), e[k][1])
            else: e["pts"] = [(q(e["pts"][0][0] + 1e-6), e["pts"][0][1])] + e["pts"][1:]
            ents[0] = e; changed = 1
        d["ents"] = ents
    elif vname == "v10_one_entity_edit":
        ents = list(base["ents"]); i = len(ents) // 2
        e = dict(ents[i])
        if "pts" in e: e["pts"] = [(q(x + 5.0), y) for x, y in e["pts"]]
        else:
            for k in ("p1", "p2", "c", "p"):
                if k in e: e[k] = (q(e[k][0] + 5.0), e[k][1])
        ents[i] = e; d["ents"] = ents; changed = 1
    if vname in ("v06_translate", "v07_rotate90", "v08_units_mm"):
        d["ext"] = extents(d["ents"])
    return d, fmt, changed

def main(out,split_filter="all"):
    rows = []
    sums = []
    for fi, (fam, split, gen) in enumerate(FAMILIES):
        if split_filter!="all" and split!=split_filter:continue
        rng = random.Random(f"{CORPUS_ID}/{fam}")
        ents, layers = gen(rng)
        ents = quantise(ents)
        base = dict(ents=ents, layers=layers, ext=extents(ents), comments=[f"{CORPUS_ID} {fam}"],
                    tdcreate=2461300.25 + fi, tdupdate=2461310.5 + fi * 1.5)
        famdir = os.path.join(out, split, fam); os.makedirs(famdir, exist_ok=True)
        base_sha = None
        for vname, rel in VARIANTS:
            vrng = random.Random(f"{CORPUS_ID}/{fam}/{vname}")
            doc, fmt, changed = (base, "short", 0) if vname in ("v00_base", "v01_exact_copy") else make_variant(base, vname, vrng)
            data = write_dxf(doc, fmt)
            name = f"{fam}__{vname}.dxf"
            path = os.path.join(famdir, name)
            with open(path, "wb") as fh: fh.write(data)
            sha = hashlib.sha256(data).hexdigest()
            if vname == "v00_base": base_sha = sha
            rel_path = os.path.relpath(path, out)
            sums.append(f"{sha}  {rel_path}")
            rows.append(dict(path=rel_path, family=fam, split=split, variant=vname, relation=rel,
                             bytes=len(data), sha256=sha, base=f"{split}/{fam}/{fam}__v00_base.dxf",
                             byte_equal_to_base=(sha == base_sha), entities=len(doc["ents"]),
                             edited_values=changed,
                             expect=EXPECT[rel]))
    with open(os.path.join(out, "manifest.csv"), "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
    with open(os.path.join(out, "manifest.json"), "w") as fh:
        json.dump(dict(corpus_id=CORPUS_ID, format="AutoCAD R12 ASCII DXF (AC1009)", generator="make_cad_corpus.py",
                       families=len(FAMILIES), variants=[v for v, _ in VARIANTS], files=rows), fh, indent=1)
    with open(os.path.join(out, "SHA256SUMS"), "w") as fh: fh.write("\n".join(sums) + "\n")
    tot = sum(r["bytes"] for r in rows)
    print(f"{len(rows)} files, {tot:,} bytes -> {out}")

EXPECT = {
    "base": "store",
    "identical": "must_dedup_all_arms",
    "content_equal": "canonizer_should_collapse",
    "renamed": "may_collapse_with_counted_undo",
    "congruent": "may_collapse_with_counted_undo",
    "scaled": "may_collapse_with_counted_undo",
    "tiny_diff": "trap_never_free",
    "design_change": "trap_never_free",
}

if __name__ == "__main__":
    parser=argparse.ArgumentParser(description="Generate the deterministic DarkRock CAD corpus")
    parser.add_argument("out",nargs="?",default="cad-corpus-v1")
    parser.add_argument("--corpus-id",default=CORPUS_ID)
    parser.add_argument("--split",choices=("all","dev","test"),default="all")
    args=parser.parse_args()
    CORPUS_ID=args.corpus_id
    main(args.out,args.split)
