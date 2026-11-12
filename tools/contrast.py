#!/usr/bin/env python3
"""Measures the contrast of every piece of text the app draws, in every place, light and dark,
in each moment the review pack shoots and in Today, at the first window size; and a few of those
moments again in a tall and a wide window and with Larger Text, in a roomy and the smallest window.
For each text element it crops the render to the element's box and compares the glyphs' cores
(the brightest 3% of pixels for light ink, the darkest for dark ink) with the ground under them
(the median), as WCAG relative luminance. Every
text in sight is measured as drawn, however faint. Text drawn invisible is left out: the controls
while they have receded, and the bell's words on the scene, which are there for VoiceOver. The time
and the phase line must be on every scene but Today; a running focus block hides the time until the
pointer calls it out, so its moments are shot with the pointer just moved. Prints the lowest per
scene and overall.

    cargo build --release && python3 tools/contrast.py [NAME ...]

With names, only the moments whose name holds one of them run: `today`, `@wide`, `larger-small`.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "target/release/pomodoro"
SCENES = {
    "wound": None,
    "focus-early": "focus-early",
    "focus-late": "focus-late",
    "paused": "paused",
    "bell": "bell",
    "skipped": "skipped",
    "short": "short",
    "long": "long",
    "evening": "evening",
    "controls": "controls",
    "unsaved": "unsaved",
    "today": "today",
}
# The other window sizes, and the moments taken at each: a size, and whether Larger Text is on.
SIZES = {
    "tall": ("420x780", False),
    "wide": ("900x600", False),
    "larger": ("475x675", True),
    "larger-small": ("380x540", True),
}
SIZED = ("wound", "focus-late", "bell", "controls", "today")
TARGET = 4.5
# Text drawn less opaque than this is not in sight: its colour's alpha times the opacity it inherits.
UNSEEN = 0.05
# How far Today's scrolling edges fade, in points (src/today.rs).
FADE = 18.0
REQUIRED = ("time", "phase")
# The moments of a running focus block, whose digits show only once the pointer moves.
CALLED = ("focus-early", "focus-late", "evening")


def luminance(rgb):
    c = rgb / 255.0
    c = numpy.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    return 0.2126 * c[..., 0] + 0.7152 * c[..., 1] + 0.0722 * c[..., 2]


def render(d, place, theme, scene, size="380x540", larger=False):
    """The render at 2x and its elements. Larger Text grows a window that is too small for it, so
    in the smallest window it is turned on and the window then made small again."""
    events = [{"t": 0.05 + 0.02 * i, "type": "key", "name": "t"} for i in range(place)]
    if scene in CALLED:
        events.append({"t": 0.12, "type": "move", "x": 200.0, "y": 300.0})
    if larger:
        events.append({"t": 0.1, "type": "key", "name": "=", "cmd": True})
        if size == "380x540":
            # After the window has finished growing for it.
            events.append({"t": 0.6, "type": "resize", "dw": -95, "dh": -135, "dur": 0.05})
    script = Path(d) / f"place{place}.json"
    at = "1.0" if larger else "0.6"
    script.write_text(json.dumps({"events": [*events, {"t": float(at) - 0.1, "type": "mark", "name": "end"}]}))
    png, tree = Path(d) / "s.png", Path(d) / "t.json"
    common = [APP, "--script", script, "--theme", theme, "--at", at, "--scale", "2", "--size", size] + (
        ["--scene", scene] if scene else []
    )
    subprocess.run([*common, "--snap", png], check=True, capture_output=True)
    subprocess.run([*common, "--tree", tree], check=True, capture_output=True)
    raw = subprocess.run(
        ["ffmpeg", "-loglevel", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        check=True,
        capture_output=True,
    ).stdout
    w, h = (int(v) * 2 for v in size.split("x"))
    img = numpy.frombuffer(raw, numpy.uint8).reshape(h, w, 3).astype(float)
    dump = json.loads(tree.read_text())
    # The tree is in the view's points, which Larger Text zooms; the render in the window's.
    return img, dump["elements"], w / 2 / dump["viewport"][0]


def light_ink(color):
    if isinstance(color, str) and color.startswith("#"):
        return luminance(numpy.array([int(color[i : i + 2], 16) for i in (1, 3, 5)], float)) > 0.4
    return True


def ink(e):
    color, opacity = e.get("color"), e.get("opacity")
    alpha = int(color[7:9], 16) / 255 if isinstance(color, str) and len(color) == 9 else 1.0
    return alpha * (1.0 if opacity is None else opacity)


def in_sight(e, view):
    """Text in Today's scrolling part counts only where it is wholly in view, clear of the edges the
    viewport fades to say there is more."""
    if "/today-scroll/" not in e["path"]:
        return True
    _, y, _, h = e["rect"]
    return y >= view[1] + FADE and y + h <= view[1] + view[3] - FADE


def texts(elements):
    view = next((e["rect"] for e in elements if e.get("key") == "today-scroll"), None)
    for e in elements:
        if view and not in_sight(e, view):
            continue
        if e.get("text") and e.get("rect") and e["text"].strip() not in ("", "·") and ink(e) >= UNSEEN:
            yield e


MOMENTS = [(shot, scene, "380x540", False) for shot, scene in SCENES.items()] + [
    (f"{shot}@{size}", SCENES[shot], *SIZES[size]) for size in SIZES for shot in SIZED
]
if sys.argv[1:]:
    MOMENTS = [m for m in MOMENTS if any(name in m[0] for name in sys.argv[1:])]

lowest = []
with tempfile.TemporaryDirectory() as d:
    for place, name in enumerate(["coast", "lake", "mesa"]):
        for theme in ["light", "dark"]:
            for shot, scene, size, larger in MOMENTS:
                img, elements, zoom = render(d, place, theme, scene, size, larger)
                worst = (99.0, "")
                keys = {e.get("key") for e in texts(elements)}
                for key in REQUIRED if not shot.startswith("today") else ():
                    if key not in keys:
                        worst = min(worst, (0.0, f"no {key}"))
                for e in texts(elements):
                    # A text's box can be as wide as its column; keep to about where its glyphs are.
                    x, y, w, h = e["rect"]
                    w = min(w, len(e["text"]) * 0.5 * h + 6.0)
                    x, y, w, h = (round(v * 2 * zoom) for v in (x, y, w, h))
                    crop = luminance(img[max(y, 0) : y + h, max(x, 0) : x + w].reshape(-1, 3))
                    if crop.size < 20:
                        continue
                    ground = numpy.percentile(crop, 50)
                    if light_ink(e.get("color")):
                        ratio = (numpy.percentile(crop, 97) + 0.05) / (ground + 0.05)
                    else:
                        ratio = (ground + 0.05) / (numpy.percentile(crop, 3) + 0.05)
                    worst = min(worst, (ratio, e["text"]))
                lowest.append((worst[0], f"{name}-{theme}-{shot}", worst[1]))
                print(f"{name:5} {theme:5} {shot:24} lowest {worst[0]:5.2f}:1  {worst[1]!r}")

ratio, where, what = min(lowest)
print(f"\nminimum {ratio:.2f}:1 at {where} ({what!r}); target {TARGET}:1")
sys.exit(0 if ratio >= TARGET else 1)
