#!/usr/bin/env python3
"""Measures the contrast of every piece of text the app draws, in every place, light and dark,
in each moment the review pack shoots and in Today. For each text element it crops the render to
the element's box and compares the glyphs' cores (the brightest 3% of pixels for light ink, the
darkest for dark ink) with the ground under them (the median), as WCAG relative luminance. Every
text in sight is measured as drawn, however faint. Text drawn invisible is left out: the controls
while they have receded, and the bell's words on the scene, which are there for VoiceOver. The time
and the phase line must be on every scene but Today. Prints the lowest per scene and overall.

    cargo build --release && python3 tools/contrast.py
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
    "let-go": "let-go",
    "short": "short",
    "long": "long",
    "evening": "evening",
    "controls": "controls",
    "unsaved": "unsaved",
    "today": "today",
}
TARGET = 4.5
# Text drawn less opaque than this is not in sight: its colour's alpha times the opacity it inherits.
UNSEEN = 0.05
# How far Today's scrolling edges fade, in points (src/today.rs).
FADE = 18.0
REQUIRED = ("time", "phase")


def luminance(rgb):
    c = rgb / 255.0
    c = numpy.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    return 0.2126 * c[..., 0] + 0.7152 * c[..., 1] + 0.0722 * c[..., 2]


def render(d, place, theme, scene):
    script = Path(d) / f"place{place}.json"
    script.write_text(
        json.dumps(
            {
                "events": [{"t": 0.05 + 0.02 * i, "type": "key", "name": "t"} for i in range(place)]
                + [{"t": 0.5, "type": "mark", "name": "end"}]
            }
        )
    )
    png, tree = Path(d) / "s.png", Path(d) / "t.json"
    common = [APP, "--script", script, "--theme", theme, "--at", "0.6", "--scale", "2"] + (
        ["--scene", scene] if scene else []
    )
    subprocess.run([*common, "--snap", png], check=True, capture_output=True)
    subprocess.run([*common, "--tree", tree], check=True, capture_output=True)
    raw = subprocess.run(
        ["ffmpeg", "-loglevel", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        check=True,
        capture_output=True,
    ).stdout
    img = numpy.frombuffer(raw, numpy.uint8).reshape(1080, 760, 3).astype(float)
    return img, json.loads(tree.read_text())["elements"]


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


lowest = []
with tempfile.TemporaryDirectory() as d:
    for place, name in enumerate(["coast", "lake", "mesa"]):
        for theme in ["light", "dark"]:
            for shot, scene in SCENES.items():
                img, elements = render(d, place, theme, scene)
                worst = (99.0, "")
                keys = {e.get("key") for e in texts(elements)}
                for key in REQUIRED if shot != "today" else ():
                    if key not in keys:
                        worst = min(worst, (0.0, f"no {key}"))
                for e in texts(elements):
                    # A text's box can be as wide as its column; keep to about where its glyphs are.
                    x, y, w, h = e["rect"]
                    w = min(w, len(e["text"]) * 0.5 * h + 6.0)
                    x, y, w, h = (round(v * 2) for v in (x, y, w, h))
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
                print(f"{name:5} {theme:5} {shot:12} lowest {worst[0]:5.2f}:1  {worst[1]!r}")

ratio, where, what = min(lowest)
print(f"\nminimum {ratio:.2f}:1 at {where} ({what!r}); target {TARGET}:1")
sys.exit(0 if ratio >= TARGET else 1)
