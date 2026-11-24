#!/usr/bin/env python3
"""Draws the Torrz app icon in ordered-dither pixels, on the macOS icon grid.

    scripts/make-icon.py            # every variant into design/icons/
    scripts/make-icon.py classic    # and that one as the app's icon: assets/icon.svg (light),
                                    # assets/icon-dark.svg and assets/Torrz.icon

The canvas is 1024 with an 824 body centred in it (continuous corners, soft drop shadow), so
the Dock draws it the size of its neighbours. The artwork is a 48 x 48 pixel grid; a pixel is
lit when its shade beats the Bayer threshold for its cell.

assets/Torrz.icon is an Icon Composer document: the same pixels full-bleed over a fill, with a
dark appearance, which macOS 26 and later switch between on their own. scripts/make-app.sh
compiles it into the bundle.
"""
import json
import math
import pathlib
import sys

N = 48  # pixels across the body
BODY = 824
ORIGIN = (1024 - BODY) / 2
CELL = BODY / N
CX = N / 2

BAYER4 = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
BAYER8 = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
]


def lit(shade, x, y, matrix=BAYER4):
    """Whether pixel (x, y) is on for a shade from 0 (never) to 1 (always)."""
    n = len(matrix)
    return shade > (matrix[y % n][x % n] + 0.5) / (n * n)


def squircle(c=512, r=BODY / 2, n=5, steps=240):
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        ct, st = math.cos(t), math.sin(t)
        pts.append("%.1f %.1f" % (c + r * math.copysign(abs(ct) ** (2 / n), ct), c + r * math.copysign(abs(st) ** (2 / n), st)))
    return "M" + "L".join(pts) + "Z"


def arrow(x, y, dx=0, dy=0, top=9, neck=25, tip=40, stem=5, spread=13):
    """Whether pixel (x, y) is inside the download arrow (optionally shifted)."""
    px, py = x + 0.5 - dx, y - dy
    if top <= py < neck + 2 and abs(px - CX) <= stem:
        return True
    return neck <= py < tip and abs(px - CX) <= (tip - py) * spread / (tip - neck)


def mix(a, b, t):
    t = max(0.0, min(1.0, t))
    return "#%02x%02x%02x" % tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def pixels(color_at, origin=ORIGIN, cell=CELL):
    """One path per colour holding every pixel `color_at(x, y)` gives that colour.

    Pixels of a colour share a path, not a rect each, so a renderer fills neighbours as one
    shape and leaves no hairline seams between them at any scale.
    """
    at = lambda i: "%.2f" % (origin + i * cell)
    paths = {}
    for y in range(N):
        x = 0
        while x < N:
            c = color_at(x, y)
            if c is None:
                x += 1
                continue
            run = 1
            while x + run < N and color_at(x + run, y) == c:
                run += 1
            paths.setdefault(c, []).append("M%s %sH%sV%sH%sZ" % (at(x), at(y), at(x + run), at(y + 1), at(x)))
            x += run
    return "\n    ".join('<path fill="%s" d="%s"/>' % (c, "".join(d)) for c, d in paths.items())


def rain(x, y):
    """Blue pixels fall down the column and pack into a solid arrow: pieces arriving."""
    light, deep = (150, 204, 255), (47, 125, 246)
    color = mix(light, deep, (y - 4) / 36)
    if arrow(x, y):
        # Sparse at the top of the stem, solid from the neck down.
        return color if lit(0.18 + (y - 9) / 15, x, y) else None
    # The drizzle above the arrow, and a faint spill either side of the stem.
    if 2 <= y < 9 and abs(x + 0.5 - CX) <= 5:
        return color if lit(0.05 + (y - 2) * 0.022, x, y) else None
    return None


def classic(x, y):
    """One bit, as on a 1984 desktop: black arrow, checkerboard shadow, dithered ground."""
    ink = "INK"  # the variant's ink: black on cream, or cream on graphite
    if arrow(x, y):
        return ink
    if arrow(x, y, dx=2, dy=2):
        return ink if lit(0.5, x, y) else None
    if y >= 30:
        return ink if lit((y - 30) / 18 * 0.42, x, y) else None
    return None


def dusk(x, y):
    """A dithered blue dusk rising behind a solid white arrow."""
    if arrow(x, y):
        return "#f4f6fb"
    if arrow(x, y, dx=2, dy=2):
        return "#101013"
    shade = ((y - 4) / 40) ** 1.25 if y > 4 else 0
    if lit(shade, x, y, BAYER8):
        return mix((96, 168, 255), (38, 108, 236), (y - 10) / 34)
    return None


# name: (painter, body top, body bottom, ink)
VARIANTS = {
    "rain": (rain, "#3a3a41", "#161618", None),
    "classic": (classic, "#f6f4ec", "#dedbd0", "#19191b"),
    "classic-dark": (classic, "#2e2e33", "#161618", "#efece2"),
    "dusk": (dusk, "#26262b", "#141416", None),
}


def svg(name):
    paint, top, bottom, ink = VARIANTS[name]
    body = squircle()
    art = pixels(paint).replace("INK", ink or "")
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient>
    <linearGradient id="rim" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".3"/><stop offset=".35" stop-color="#fff" stop-opacity=".04"/><stop offset="1" stop-color="#000" stop-opacity=".25"/></linearGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity=".35"/></filter>
    <clipPath id="clip"><path d="{body}"/></clipPath>
  </defs>
  <path d="{body}" fill="url(#body)" filter="url(#shadow)"/>
  <g clip-path="url(#clip)" shape-rendering="crispEdges">
    {art}
  </g>
  <path d="{body}" fill="none" stroke="url(#rim)" stroke-width="6" clip-path="url(#clip)"/>
</svg>
"""


def srgb(hex_color):
    r, g, b = (int(hex_color[i : i + 2], 16) / 255 for i in (1, 3, 5))
    return "srgb:%.5f,%.5f,%.5f,1.00000" % (r, g, b)


def icon_document(root, name):
    """Writes assets/Torrz.icon from `name` and `name`-dark: fill and ink for each appearance."""
    doc = root / "assets" / "Torrz.icon"
    (doc / "Assets").mkdir(parents=True, exist_ok=True)
    layers = []
    for appearance, variant in (("light", name), ("dark", name + "-dark")):
        paint, _, _, ink = VARIANTS[variant]
        # Full-bleed: an .icon's canvas is the body itself, and the system cuts the shape.
        art = pixels(paint, 0, 1024 / N).replace("INK", ink or "")
        (doc / "Assets" / f"{appearance}.svg").write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" shape-rendering="crispEdges">\n    %s\n</svg>\n' % art
        )
        layers.append(
            {
                "glass": False,
                "hidden-specializations": [{"value": appearance == "dark"}, {"appearance": "dark", "value": appearance == "light"}],
                "image-name": f"{appearance}.svg",
                "name": appearance,
            }
        )
    fill = lambda v: {"linear-gradient": [srgb(VARIANTS[v][1]), srgb(VARIANTS[v][2])]}
    icon = {
        "fill-specializations": [{"value": fill(name)}, {"appearance": "dark", "value": fill(name + "-dark")}],
        "groups": [{"layers": layers, "shadow": {"kind": "none", "opacity": 0}, "translucency": {"enabled": False, "value": 0.5}}],
        "supported-platforms": {"squares": ["macOS"]},
    }
    (doc / "icon.json").write_text(json.dumps(icon, indent=2) + "\n")
    return doc


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    out = root / "design" / "icons"
    out.mkdir(parents=True, exist_ok=True)
    for name in VARIANTS:
        (out / f"{name}.svg").write_text(svg(name))
    if len(sys.argv) > 1:
        name = sys.argv[1]
        if name not in VARIANTS:
            sys.exit("no variant %r: %s" % (name, ", ".join(VARIANTS)))
        (root / "assets" / "icon.svg").write_text(svg(name))
        print(root / "assets" / "icon.svg")
        if name + "-dark" in VARIANTS:
            (root / "assets" / "icon-dark.svg").write_text(svg(name + "-dark"))
            print(icon_document(root, name))


if __name__ == "__main__":
    main()
