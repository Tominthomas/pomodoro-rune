#!/usr/bin/env python3
"""Drives the release build headless through the scripts in scripts/ and checks what a person
would see and what VoiceOver would read: the time, the phase line, the lanterns, Today's
numbers. The last check compares pixels: a paused block's lantern must look different from an
unpaused one's, and a lantern must render the same twice.

    cargo build --release && python3 tools/scripts.py && python3 tools/check.py

Name scripts (`python3 tools/check.py dock today`) to run only their checks; the pixel and
frame probes are named `pixels`, `bell-light` and `text-swap`.
"""

import json
import subprocess
import sys
import tempfile
from itertools import pairwise
from pathlib import Path

import numpy

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "target/release/pomodoro"
# The timer runs on deadlines, not frames, so a coarse step keeps a five-minute script quick.
# The short script that checks digits to the second keeps full rate, where a tap lands on time.
FPS = {
    "start": "60",
    "activate": "60",
    "activate-control": "60",
    "dock": "30",
    "double-start": "60",
    "drag": "60",
    "today": "60",
    "bell": "60",
    "four-labels": "60",
    "busy-day": "30",
}
# Scripts that start from one of the app's review scenes rather than an empty first launch.
SCENE = {
    "today": "today",
    "bell": "last-minute",
    "night": "night-minute",
    "four-labels": "today",
    "busy-day": "busy",
}
YESTERDAY = (1_794_405_600 - 4 * 3600) // 86_400 - 1
# Text a person must read is drawn at least this opaque: its colour's alpha times the opacity it
# inherits. src/tests.rs holds the same floor.
READABLE = 0.5


def alpha(color):
    """The alpha of a dumped `#rrggbbaa` colour; one without it is opaque."""
    return int(color[7:9], 16) / 255 if isinstance(color, str) and len(color) == 9 else 1.0


def ink(e):
    """How opaque the element's text is drawn, its own alpha and inherited opacity together."""
    opacity = e.get("opacity")
    return alpha(e.get("color")) * (1.0 if opacity is None else opacity)


def run(script, at, out_flag, out, extra=()):
    scene = ["--scene", SCENE[script]] if script in SCENE else []
    cmd = [APP, "--script", ROOT / "scripts" / f"{script}.json", *scene, out_flag, out, "--at", f"mark:{at}", *extra]
    subprocess.run(cmd, check=True, capture_output=True)


def tree(script, at):
    """The keyed elements at a mark; under "@all" every element, keyed or not; and under "@a11y"
    the accessibility nodes by key."""
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "tree.json"
        run(script, at, "--tree", out, ["--fps", FPS.get(script, "10"), "--scale", "1"])
        dump = json.loads(out.read_text())
        t = {e["key"]: e for e in dump["elements"] if e.get("key")}
        t["@all"] = dump["elements"]
        t["@a11y"] = {}
        stack = [dump["accessibility"]]
        while stack:
            node = stack.pop()
            if node.get("key"):
                t["@a11y"][node["key"]] = node
            stack.extend(node.get("children", []))
        return t


def spoken(t, key):
    return t["@a11y"].get(key, {}).get("value")


def sun_bottom(t):
    _, y, _, h = t["sun"]["rect"]
    return y + h


def opacity(t, key):
    return t.get(key, {}).get("opacity")


def pixels(script, at, key):
    with tempfile.TemporaryDirectory() as d:
        png = Path(d) / "crop.png"
        run(script, at, "--snap", png, ["--fps", "10", "--crop", key])
        raw = subprocess.run(
            ["ffmpeg", "-loglevel", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgba", "-"],
            check=True,
            capture_output=True,
        ).stdout
        return numpy.frombuffer(raw, numpy.uint8).reshape(-1, 4)


def text(t, key):
    """What the element keyed `key` says, if a person can read it: drawn too faint counts as
    not there."""
    e = t.get(key, {})
    return e.get("text") if ink(e) >= READABLE else None


def label(t, key):
    return t.get(key, {}).get("label") or ""


def phase(t):
    return text(t, "phase")


def fits(t):
    """Every visible text lies inside the window on all four sides, as drawn at its real size. Today
    below its held top scrolls, so there a text need only fit the width; the day's strip scrolls
    sideways too, so its marks need not."""
    return all(
        x >= -0.5 and x + w <= 380.5 and ("/today-scroll/" in e["path"] or (y >= -0.5 and y + h <= 540.5))
        for e in t["@all"]
        if e.get("text") and ink(e) > 0.05 and e.get("rect") and "/day-strip-view/" not in e["path"]
        for x, y, w, h in [e["rect"]]
    )


def held(t):
    """Today's timer, title and way back are where they belong whatever scrolls under them."""
    return all(
        t[k]["rect"][1] >= 0 and t[k]["rect"][1] + t[k]["rect"][3] <= 130
        for k in ("timer-bar", "today-title", "today-close")
    )


def scrolls_to(t, key):
    """The element keyed `key` is in the part of Today that scrolls."""
    return "/today-scroll/" in t.get(key, {}).get("path", "")


def apart(rects):
    """No two of these boxes overlap side to side."""
    rects = sorted(rects)
    return all(a[0] + a[2] <= b[0] + 0.5 for a, b in pairwise(rects))


def right_edge(t, key):
    x, _, w, _ = t[key]["rect"]
    return x + w


def lanterns(t, prefix="lantern-"):
    return sorted(k for k in t if k.startswith(prefix) and k[len(prefix) :].isdigit())


def shown(t):
    faint = [f"{k} {t[k].get('text')!r} at {ink(t[k]):.2f}" for k in ("time", "phase") if k in t and not text(t, k)]
    said = f"time={text(t, 'time')!r} phase={phase(t)!r} lanterns={len(lanterns(t))}"
    return f"{said} too faint: {', '.join(faint)}" if faint else said


CHECKS = [
    (
        "a tap on the sky starts the sunset",
        "start",
        "running",
        lambda t: text(t, "time") == "24:58" and phase(t) == "focus",
    ),
    (
        "Space pauses it where it was",
        "start",
        "paused",
        lambda t: text(t, "time") == "24:58" and phase(t) == "the sky waits · space to resume",
    ),
    ("a tap goes on from there", "start", "resumed", lambda t: text(t, "time") == "24:57" and phase(t) == "focus"),
    (
        "VoiceOver reads the time left, not just its name",
        "start",
        "running",
        lambda t: spoken(t, "time") == "24 minutes 58 seconds" and t["@a11y"]["time"].get("name") == "Time remaining",
    ),
    (
        "while a block waits, the controls stay out and Start focus says what it does",
        "hover",
        "waiting",
        lambda t: (
            opacity(t, "control-play-look") == 1.0
            and t["@a11y"].get("control-play", {}).get("name") == "Start focus"
            and opacity(t, "control-today-look") == 1.0
        ),
    ),
    (
        "once it runs they come out while the pointer moves",
        "hover",
        "shown",
        lambda t: opacity(t, "control-play-look") == 1.0,
    ),
    (
        "and fade once it rests, staying where VoiceOver finds them",
        "hover",
        "rested",
        lambda t: opacity(t, "control-play-look") == 0.0 and t["@a11y"].get("control-play", {}).get("name") == "Pause",
    ),
    (
        "a label is typed without starting the clock, and arrows set one minute",
        "cycle",
        "wound",
        lambda t: (
            text(t, "top-text") == "Reading & notes"
            and text(t, "time") == "01:00"
            and phase(t) == "focus · space to begin"
        ),
    ),
    ("the last second counts down", "cycle", "b1-last", lambda t: text(t, "time") == "00:01"),
    (
        "the bell lights a lantern for the block",
        "cycle",
        "b1-bell",
        lambda t: (
            phase(t) == "a lantern for that one"
            and (text(t, "time"), text(t, "time-unit")) == ("1 min", "focused")
            and (t["@a11y"]["time"].get("name"), spoken(t, "time")) == ("Block complete", "1 minute focused")
            and lanterns(t) == ["lantern-0"]
        ),
    ),
    (
        "then a short break waits, offering three words",
        "cycle",
        "b1-break",
        lambda t: phase(t) == "rest · space when you're ready" and lanterns(t) == ["lantern-0"] and "felt-deep" in t,
    ),
    (
        "the label carries over to the next block",
        "cycle",
        "wound-2",
        lambda t: text(t, "top-text") == "Reading & notes" and phase(t) == "focus · space to begin",
    ),
    ("a second block pauses", "cycle", "b2-paused", lambda t: phase(t) == "the sky waits · space to resume"),
    (
        "and still lights a lantern, with a knot for the pause",
        "cycle",
        "b2-break",
        lambda t: label(t, "lantern-1") == "Lantern 2, paused once",
    ),
    (
        "a block let go rings out with no lantern",
        "cycle",
        "b3-letgo",
        lambda t: phase(t) == "let go; the minutes still count" and len(lanterns(t)) == 2,
    ),
    (
        "and doesn't count toward the cycle",
        "cycle",
        "b3-break",
        lambda t: phase(t) == "rest · space when you're ready" and len(lanterns(t)) == 2,
    ),
    (
        "four lanterns earn the long break",
        "cycle",
        "long",
        lambda t: phase(t) == "long rest · space when you're ready" and len(lanterns(t)) == 4,
    ),
    ("the long break can be set to a minute too", "cycle", "long-set", lambda t: text(t, "time") == "01:00"),
    (
        "night falls and the lanterns lift off the water",
        "cycle",
        "night-early",
        lambda t: (
            phase(t) == "long rest, four lanterns up"
            and all(t[k]["rect"][1] + t[k]["rect"][3] / 2 > 330 for k in lanterns(t))
        ),
    ),
    (
        "and rise into the sky",
        "cycle",
        "rising",
        lambda t: len(lanterns(t)) == 4 and all(t[k]["rect"][1] + t[k]["rect"][3] / 2 < 330 for k in lanterns(t)),
    ),
    (
        "Today counts four lanterns and one let go, and the numbers add up",
        "cycle",
        "today",
        lambda t: (
            text(t, "stat-lanterns-value") == "4"
            and text(t, "stat-let-go-value") == "1"
            and "stat-sessions" not in t
            and len(lanterns(t, "day-lantern-")) == 4
            and len(lanterns(t, "day-ring-")) == 1
        ),
    ),
    (
        "the block let go is a ring that keeps its minutes",
        "cycle",
        "today",
        lambda t: "let go, 1 min kept" in label(t, "day-ring-0") and text(t, "stat-focused-value") == "5 min",
    ),
    (
        "Today knows what the day was for and how it felt",
        "cycle",
        "today",
        lambda t: (
            "for-Reading & notes" in t
            and "felt deep" in label(t, "day-lantern-0")
            and "feeling not given, paused once" in label(t, "day-lantern-1")
        ),
    ),
    (
        "an unanswered reflection is reported as unanswered, not guessed",
        "cycle",
        "today",
        lambda t: (
            label(t, "felt-said") == "How the blocks felt: 1 deep, 3 not marked"
            and "felt-part-steady" not in t
            and "felt-part-drifting" not in t
        ),
    ),
    (
        "after the bell, the reflection asks its question",
        "cycle",
        "b1-bell",
        lambda t: text(t, "reflect-question") == "How did that block feel? Optional." and "felt-deep" in t,
    ),
    (
        "and acknowledges the word given",
        "cycle",
        "b1-rated",
        lambda t: text(t, "reflect-question") == "Felt deep. Noted." and "felt-deep" not in t,
    ),
    (
        "the break names the block it follows",
        "cycle",
        "b1-break",
        lambda t: text(t, "top-text") == "after Reading & notes",
    ),
    (
        "a word given is noted for a moment",
        "reflect",
        "noted",
        lambda t: text(t, "reflect-question") == "Felt steady. Noted.",
    ),
    ("then the question folds away", "reflect", "folded", lambda t: "reflect" not in t),
    ("a question left alone stays a while into the break", "reflect", "unanswered", lambda t: "reflect" in t),
    ("then fades, and the lantern stays unanswered", "reflect", "unanswered-late", lambda t: "reflect" not in t),
    (
        "Today says so",
        "reflect",
        "today",
        lambda t: (
            label(t, "felt-said") == "How the blocks felt: 1 steady, 1 not marked"
            and "feeling not given" in label(t, "day-lantern-1")
        ),
    ),
    (
        "a block let go can be undone for a moment",
        "undo",
        "letgo",
        lambda t: (
            "undo" in t
            and phase(t) == "let go; the minutes still count"
            and (text(t, "time"), text(t, "time-unit")) == ("under a minute", "kept")
        ),
    ),
    (
        "undo puts the block back, running",
        "undo",
        "undone",
        lambda t: phase(t) == "focus" and text(t, "time") == "00:38" and "undo" not in t,
    ),
    (
        "and it reaches its bell as one block",
        "undo",
        "today",
        lambda t: text(t, "stat-lanterns-value") == "1" and "stat-let-go" not in t and "day-ring-0" not in t,
    ),
    (
        "the click that brings the window forward starts nothing",
        "activate",
        "activated",
        lambda t: text(t, "time") == "25:00" and phase(t) == "focus · click the sky or press space to begin",
    ),
    (
        "the next click starts the sunset",
        "activate",
        "started",
        lambda t: phase(t) == "focus" and text(t, "time") != "25:00",
    ),
    (
        "the activating click on a control does nothing either",
        "activate-control",
        "activated",
        lambda t: text(t, "time") == "25:00" and phase(t) == "focus · click the sky or press space to begin",
    ),
    ("and the next click on it starts the sunset", "activate-control", "started", lambda t: phase(t) == "focus"),
    (
        "two quick clicks on Start start the block and pause it, and end nothing",
        "double-start",
        "inspect",
        lambda t: (
            phase(t) == "the sky waits · space to resume"
            and t["@a11y"]["control-play"].get("name") == "Resume"
            and "undo" not in t
        ),
    ),
    (
        "the controls stay while the pointer rests on them",
        "dock",
        "resting",
        lambda t: opacity(t, "control-play-look") == 1.0,
    ),
    (
        "a click in their corner, off any control, never reaches the sky",
        "dock",
        "corner-click",
        lambda t: phase(t) == "focus",
    ),
    (
        "nor does a click where a control has faded",
        "dock",
        "faded-click",
        lambda t: phase(t) == "focus",
    ),
    (
        "letting go of the sun sets a shorter block and starts nothing",
        "drag",
        "released",
        lambda t: (
            phase(t) == "focus · space to begin"
            and text(t, "time") not in (None, "25:00")
            and text(t, "time").endswith(":00")
        ),
    ),
    ("Space starts it", "drag", "started", lambda t: phase(t) == "focus"),
    (
        "a long label is cut short inside the window",
        "long-label",
        "end",
        lambda t: (
            text(t, "top-text").startswith("Thermodynamics")
            and right_edge(t, "top-text") <= 24 + 232
            and right_edge(t, "top") < 380
        ),
    ),
    (
        "Today holds its timer and title with a block open, and History's legend scrolls up under them",
        "today",
        "felt-given",
        lambda t: fits(t) and held(t) and scrolls_to(t, "history-scale"),
    ),
    (
        "four long names keep inside the width, and the page still scrolls to the legend",
        "four-labels",
        "view",
        lambda t: (
            fits(t)
            and held(t)
            and "for-name-Calculus practice problems" in t
            and scrolls_to(t, "history-scale")
            and t["today-scroll"]["rect"][1] + t["today-scroll"]["rect"][3] <= 540.5
        ),
    ),
    (
        "a day of nineteen blocks keeps every mark's target apart, opening on the newest",
        "busy-day",
        "open",
        lambda t: (
            len(lanterns(t, "day-lantern-")) + len(lanterns(t, "day-ring-")) == 19
            and apart([t[k]["rect"] for k in lanterns(t, "day-lantern-") + lanterns(t, "day-ring-")])
            and t["day-lantern-14"]["rect"][0] + 24 <= t["day-strip-view"]["rect"][0] + t["day-strip-view"]["rect"][2]
            and t["day-lantern-14"]["rect"][0] >= t["day-strip-view"]["rect"][0]
        ),
    ),
    ("and with the strip's key row under its marks", "cycle", "today", lambda t: fits(t) and "strip-key" in t),
    ("the keys card wraps its rows inside the window", "keys", "open", lambda t: fits(t) and "key-what-2" in t),
    ("half a minute out, the sun's lower limb is still above the sea", "bell", "half", lambda t: sun_bottom(t) < 329.0),
    ("at 00:00 it touches the water", "bell", "zero", lambda t: abs(sun_bottom(t) - 330.0) < 0.6),
    (
        "a click on Today's background leaves it open",
        "today",
        "clicked",
        lambda t: text(t, "today-title") == "Today" and ("strip-note" in t or "strip-key" in t),
    ),
    (
        "a lantern in Today opens its block on two lines, clear of its close button",
        "today",
        "detail",
        lambda t: (
            "felt deep" in text(t, "detail-about")
            and "Reading & notes" in text(t, "detail-about")
            and right_edge(t, "detail-text") <= t["detail-close"]["rect"][0]
        ),
    ),
    (
        "a feeling not given at the bell can be given later",
        "today",
        "felt-given",
        lambda t: "felt steady" in text(t, "detail-about") and "felt steady" in label(t, "day-lantern-1"),
    ),
    ("and taken back", "today", "felt-cleared", lambda t: "feeling not given" in text(t, "detail-about")),
    (
        "a day picked in History opens that day",
        "today",
        "yesterday",
        lambda t: (
            text(t, "today-title") == "Tuesday"
            and "back-to-today" in t
            and "now" not in t
            and text(t, "stat-lanterns-value") != "4"
        ),
    ),
    (
        "and Today comes back",
        "today",
        "back",
        lambda t: text(t, "today-title") == "Today" and "now" in t and "back-to-today" not in t,
    ),
]

ONLY = set(sys.argv[1:])
PROBES = {"pixels", "bell-light", "text-swap"}
unknown = ONLY - {script for _, script, _, _ in CHECKS} - PROBES
if unknown:
    sys.exit(f"no checks named {', '.join(sorted(unknown))}")


def wanted(name):
    return not ONLY or name in ONLY


failed = 0
for name, script, at, ok in CHECKS:
    if not wanted(script):
        continue
    t = tree(script, at)
    good = ok(t)
    failed += not good
    print(f"{'pass' if good else 'FAIL'}  {name}  ({script} @ {at}: {shown(t)})")

if wanted("pixels"):
    plain = pixels("lantern-plain", "end", "day-lantern-art-0")
    again = pixels("lantern-plain", "end", "day-lantern-art-0")
    paused = pixels("lantern-paused", "end", "day-lantern-art-0")
    same = int((plain != again).any(axis=1).sum())
    differ = int((plain != paused).any(axis=1).sum())
    # One knot is a dot of 1.3 points, about 21 pixels at 2x; Today's page under it is still.
    good = same == 0 and differ > 15
    failed += not good
    print(
        f"{'pass' if good else 'FAIL'}  a paused block's lantern looks different; the same lantern renders the same  "
        f"({differ} of {len(plain)} pixels differ with one pause, {same} between two plain renders)"
    )


def frames(scene, script, start, count, fps, theme="light"):
    """Renders `count` frames from `start` seconds and returns them as (n, h, w, 3) arrays."""
    with tempfile.TemporaryDirectory() as d:
        cmd = [
            APP,
            "--scene",
            scene,
            "--script",
            ROOT / "scripts" / f"{script}.json",
            "--theme",
            theme,
            "--out",
            d,
            "--start",
            str(start),
            "--frames",
            str(count),
            "--fps",
            str(fps),
            "--scale",
            "1",
        ]
        subprocess.run(cmd, check=True, capture_output=True)
        out = []
        for png in sorted(Path(d).glob("frame_*.png")):
            raw = subprocess.run(
                ["ffmpeg", "-loglevel", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
                check=True,
                capture_output=True,
            ).stdout
            out.append(numpy.frombuffer(raw, numpy.uint8).reshape(540, 380, 3).astype(float))
        return out


def sky_light(img):
    """The sky's mean relative luminance, above the horizon and clear of the readout."""
    c = img[:320, :, :] / 255.0
    c = numpy.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    return float((0.2126 * c[..., 0] + 0.7152 * c[..., 1] + 0.0722 * c[..., 2]).mean())


for theme in ["light", "dark"] if wanted("bell-light") else []:
    seq = [sky_light(f) for f in frames("last-minute", "bell", 59.5, 60, 6, theme)]
    settled = seq[-1]
    low = min(seq)
    falls = all(b <= a + 0.002 for a, b in pairwise(seq))
    good = low >= settled - 0.002 and falls
    failed += not good
    print(
        f"{'pass' if good else 'FAIL'}  {theme}: from sunset to the break the sky only darkens, "
        "never below where the break settles  "
        f"(lowest {low:.4f}, settled {settled:.4f}, {'only darkening' if falls else 'brightens somewhere'})"
    )


def overlapping(elements):
    """Pairs of visible texts drawn over one another, two copies of one key included."""
    texts = [(e.get("text"), e["rect"]) for e in elements if e.get("text") and ink(e) > 0.05 and e.get("rect")]
    hits = []
    for i, (a, ra) in enumerate(texts):
        for b, rb in texts[i + 1 :]:
            if ra[0] < rb[0] + rb[2] and rb[0] < ra[0] + ra[2] and ra[1] < rb[1] + rb[3] and rb[1] < ra[1] + ra[3]:
                hits.append((a, b))
    return hits


if wanted("text-swap"):
    bad = []
    moments = [("night-minute", "night", 60.0 + i * 0.3) for i in range(36)] + [
        ("last-minute", "bell", 59.6 + i * 0.15) for i in range(36)
    ]
    for scene, script, at in moments:
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "t.json"
            subprocess.run(
                [
                    APP,
                    "--scene",
                    scene,
                    "--script",
                    ROOT / f"scripts/{script}.json",
                    "--tree",
                    out,
                    "--at",
                    str(at),
                    "--fps",
                    "30",
                    "--scale",
                    "1",
                ],
                check=True,
                capture_output=True,
            )
            elements = json.loads(out.read_text())["elements"]
        hint = any(e.get("key") == "hint" for e in elements)
        if overlapping(elements) or hint:
            bad.append((script, at, overlapping(elements), hint))
    good = not bad
    failed += not good
    print(
        f"{'pass' if good else 'FAIL'}  the bell and the long break's end swap their words "
        "with none drawn over another, and the hint never returns  "
        f"(72 moments; {bad[:3]})"
    )

sys.exit(1 if failed else 0)
