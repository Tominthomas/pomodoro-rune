#!/usr/bin/env python3
"""Draws the menu bar's glyphs into assets/menubar/: the sun at eight heights from high to half
set, the held sun, the lantern, the lit lantern and the moon, each over the same waterline. They
are template images, black on clear, so macOS tints them for a light, dark or tinted menu bar.

Each is 18 x 22 points drawn at 2x (36 x 44 pixels). macOS shows a status item's image at most
22 points tall, so the 22-point canvas is shown pixel for pixel on a Retina screen, and the art
inside it is 18 points tall, the size of the system's own menu bar icons. The lantern's outline
follows the paper body `src/lantern.rs` draws on the water.

    python3 tools/glyphs.py                       # writes the glyphs
    python3 tools/glyphs.py --sheet sheet.png     # also writes a contact sheet, light and dark
"""

import argparse
import math
import struct
import sys
import zlib
from collections.abc import Callable
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "assets/menubar"
W, H = 18.0, 22.0
SCALE = 2
SAMPLES_PER_AXIS = 4
HEIGHTS = 8

WATER = 16.5
WATER_LINE = 1.5
SUN_CLEARANCE = 1.0
SUN_R = 4.6
SUN_HIGH = 3.0 + SUN_R

Shape = Callable[[float, float], bool]


def disc(cx: float, cy: float, r: float) -> Shape:
    return lambda x, y: (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def ring(cx: float, cy: float, r: float, width: float) -> Shape:
    return lambda x, y: abs(math.hypot(x - cx, y - cy) - r) <= width / 2


def segment(a: tuple[float, float], b: tuple[float, float], width: float) -> Shape:
    """A stroke from `a` to `b` with round caps."""

    def inside(x: float, y: float) -> bool:
        dx, dy = b[0] - a[0], b[1] - a[1]
        t = max(0.0, min(1.0, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy or 1.0)))
        return math.hypot(x - a[0] - t * dx, y - a[1] - t * dy) <= width / 2

    return inside


def polygon(points: list[tuple[float, float]]) -> Shape:
    def inside(x: float, y: float) -> bool:
        hit = False
        for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1], strict=True):
            if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
                hit = not hit
        return hit

    return inside


def outline(points: list[tuple[float, float]], width: float) -> Shape:
    edges = [segment(a, b, width) for a, b in zip(points, points[1:] + points[:1], strict=True)]
    return lambda x, y: any(e(x, y) for e in edges)


def union(*shapes: Shape) -> Shape:
    return lambda x, y: any(s(x, y) for s in shapes)


def minus(shape: Shape, cut: Shape) -> Shape:
    return lambda x, y: shape(x, y) and not cut(x, y)


def above(y_max: float, shape: Shape) -> Shape:
    return lambda x, y: y < y_max and shape(x, y)


def cubic(p0, p1, p2, p3, steps: int = 12) -> list[tuple[float, float]]:
    out = []
    for i in range(1, steps + 1):
        t = i / steps
        u = 1 - t
        out.append(
            (
                u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1],
            )
        )
    return out


WATERLINE = segment((1.5, WATER), (W - 1.5, WATER), WATER_LINE)
CLEAR = WATER - WATER_LINE / 2 - SUN_CLEARANCE


def sun_y(height: int) -> float:
    """Where the sun's middle is at `height`, 0 high in the sky to `HEIGHTS - 1` half set."""
    return SUN_HIGH + (WATER - SUN_HIGH) * height / (HEIGHTS - 1)


def lantern(lit: bool) -> Shape:
    """The paper body of `src/lantern.rs`, with its two rims, floating just over the water."""
    w, h = 7.4, 9.0
    cx, cy = W / 2, CLEAR - 0.6 - h * 0.52
    body = [(cx - w * 0.38, cy - h * 0.5), (cx + w * 0.38, cy - h * 0.5)]
    body += cubic(
        body[-1], (cx + w * 0.75, cy - h * 0.2), (cx + w * 0.75, cy + h * 0.25), (cx + w * 0.42, cy + h * 0.5)
    )
    body.append((cx - w * 0.42, cy + h * 0.5))
    body += cubic(body[-1], (cx - w * 0.75, cy + h * 0.25), (cx - w * 0.75, cy - h * 0.2), body[0])
    rims = union(
        segment((cx - w * 0.4, cy - h * 0.55), (cx + w * 0.4, cy - h * 0.55), 1.5),
        segment((cx - w * 0.44, cy + h * 0.52), (cx + w * 0.44, cy + h * 0.52), 1.4),
    )
    if lit:
        rib = segment((cx, cy - h * 0.3), (cx, cy + h * 0.3), 0.9)
        paper = minus(polygon(body), rib)
    else:
        paper = outline(body, 1.3)
    return union(paper, rims, WATERLINE)


def moon() -> Shape:
    crescent = minus(disc(W / 2 - 0.5, 8.2, 5.2), disc(W / 2 + 2.0, 6.6, 4.4))
    return union(crescent, WATERLINE)


GLYPHS: dict[str, Shape] = {
    **{f"sun-{k}": union(above(CLEAR, disc(W / 2, sun_y(k), SUN_R)), WATERLINE) for k in range(HEIGHTS)},
    "held": union(ring(W / 2, sun_y(0), SUN_R - 0.7, 1.4), WATERLINE),
    "lantern": lantern(lit=False),
    "lantern-lit": lantern(lit=True),
    "moon": moon(),
}


def coverage(shape: Shape) -> list[list[int]]:
    """How much of each 2x pixel `shape` covers, 0 to 255."""
    px_w, px_h = int(W * SCALE), int(H * SCALE)
    step = 1 / (SCALE * SAMPLES_PER_AXIS)
    rows = []
    for py in range(px_h):
        row = []
        for px in range(px_w):
            hits = sum(
                shape((px * SAMPLES_PER_AXIS + i + 0.5) * step, (py * SAMPLES_PER_AXIS + j + 0.5) * step)
                for i in range(SAMPLES_PER_AXIS)
                for j in range(SAMPLES_PER_AXIS)
            )
            row.append(round(255 * hits / SAMPLES_PER_AXIS**2))
        rows.append(row)
    return rows


def png(path: Path, rows: list[list[tuple[int, int, int, int]]]) -> None:
    raw = b"".join(b"\0" + bytes(c for px in row for c in px) for row in rows)

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    head = struct.pack(">IIBBBBB", len(rows[0]), len(rows), 8, 6, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    )


def sheet(path: Path, drawn: dict[str, list[list[int]]]) -> None:
    """Every glyph on a light and a dark menu bar, tinted as macOS tints a template, at 3x the
    Retina size so the edges can be judged."""
    zoom, pad = 3, 12
    tiles = list(drawn.values())
    tw, th = len(tiles[0][0]) * zoom, len(tiles[0]) * zoom
    bars = [((236, 236, 236), (0, 0, 0)), ((40, 40, 44), (255, 255, 255))]
    width = pad + len(tiles) * (tw + pad)
    rows = []
    for bg, ink in bars:
        band = [[(*bg, 255)] * width for _ in range(th + 2 * pad)]
        for n, tile in enumerate(tiles):
            x0 = pad + n * (tw + pad)
            for y in range(th):
                for x in range(tw):
                    a = tile[y // zoom][x // zoom] * 0.85 / 255
                    band[pad + y][x0 + x] = (*(round(b + (i - b) * a) for b, i in zip(bg, ink, strict=True)), 255)
        rows += band
    png(path, rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--sheet", type=Path, help="also write a contact sheet of every glyph here")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    drawn = {}
    for name, shape in GLYPHS.items():
        drawn[name] = coverage(shape)
        png(OUT / f"{name}.png", [[(0, 0, 0, a) for a in row] for row in drawn[name]])
        print(f"{name}.png  {len(drawn[name][0])}x{len(drawn[name])}")
    if args.sheet:
        sheet(args.sheet, drawn)
        print(f"sheet: {args.sheet}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
