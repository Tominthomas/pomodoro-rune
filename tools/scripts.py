#!/usr/bin/env python3
"""Writes the headless input scripts in scripts/. Blocks in them are set to one minute with the
arrow keys, as a person would, so a whole cycle runs in five minutes of virtual time.

    python3 tools/scripts.py           rewrites scripts/*.json
    python3 tools/scripts.py --check   changes nothing; fails if any differs from what this writes
"""

import json
import sys
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "scripts"
SKY = (190.0, 200.0)
# The headless run's study day (14:00 UTC, Wednesday 11 November 2026), and the day before.
TODAY = (1_794_405_600 - 4 * 3600) // 86_400
YESTERDAY = TODAY - 1


def key(t, name):
    return {"t": round(t, 3), "type": "key", "name": name}


def typed(t, text):
    return {"t": t, "type": "type", "text": text}


def mark(t, name):
    return {"t": t, "type": "mark", "name": name}


def one_minute(t):
    """Winds a focus block down from 25 minutes to one."""
    return [key(t + 0.02 * i, "Down") for i in range(24)]


def label(t, text):
    """Names the next block; typing takes a moment per character before Enter."""
    return [key(t, "l"), typed(t + 0.1, text), key(t + 0.6 + 0.05 * max(0, len(text) - 15), "Enter")]


def lantern(paused):
    """One block, rated steady, then Today. With `paused` it starts a second earlier and holds
    for a second at a third of the way, so both blocks end at the same moment."""
    start = 1.5 if paused else 2.5
    events = [*one_minute(0.1), key(start, " ")]
    if paused:
        events += [key(start + 20.0, " "), key(start + 21.0, " ")]
    return [*events, key(66.0, "2"), key(66.5, "h"), mark(67.5, "end")]


def cycle():
    """Each block starts on a fresh second and moves on two seconds after its bell is done,
    so frame-rate rounding in a fast headless run never lands a key mid-bell."""
    events = [*label(0.1, "Reading & notes"), *one_minute(1.0), mark(1.6, "wound")]
    t = 2.0

    def block(name, pause=None, let_go=None):
        nonlocal t
        events.append(key(t, " "))
        end = t + 60.0
        if pause:
            events.extend([key(t + pause, " "), mark(t + pause + 0.5, f"{name}-paused"), key(t + pause + 2.0, " ")])
            end += 2.0
        if let_go:
            events.extend([key(t + let_go, "s"), mark(t + let_go + 1.0, f"{name}-letgo")])
            end = t + let_go
        events.extend(
            [mark(end - 0.5, f"{name}-last"), mark(end + 1.0, f"{name}-bell"), mark(end + 4.5, f"{name}-break")]
        )
        t = end + 5.0

    def skip_break():
        nonlocal t
        events.extend(
            [
                key(t, "s"),
                mark(t + 0.3, f"wound-{len([e for e in events if e.get('name', '').startswith('wound-')]) + 2}"),
            ]
        )
        t += 1.0

    block("b1")
    events.extend([key(t, "1"), mark(t + 0.3, "b1-rated")])
    t += 1.0
    skip_break()
    block("b2", pause=14.0)
    skip_break()
    block("b3", let_go=45.0)
    skip_break()
    block("b4")
    skip_break()
    block("b5")
    events.extend([mark(t, "long"), *[key(t + 0.2 + 0.02 * i, "Down") for i in range(14)], mark(t + 0.6, "long-set")])
    t += 1.0
    events.extend(
        [
            key(t, " "),
            mark(t + 1.0, "night-early"),
            mark(t + 40.0, "rising"),
            key(t + 41.0, "h"),
            mark(t + 42.0, "today"),
            mark(t + 43.0, "end"),
        ]
    )
    return events


SCRIPTS = {
    "start": [
        {"t": 0.3, "type": "tap", "x": SKY[0], "y": SKY[1]},
        mark(2.4, "running"),
        key(2.5, " "),
        mark(3.2, "paused"),
        {"t": 3.6, "type": "tap", "x": SKY[0], "y": SKY[1]},
        mark(5.0, "resumed"),
    ],
    # The pointer passes over the waiting window, then rests: the controls come and go.
    "hover": [{"t": 0.3, "type": "move", "x": 200.0, "y": 300.0}, mark(1.0, "shown"), mark(4.0, "rested")],
    # A block let go and taken straight back runs on to its bell as one block.
    "undo": [
        *one_minute(0.1),
        key(1.0, " "),
        key(21.0, "s"),
        mark(22.0, "letgo"),
        key(23.0, "u"),
        mark(23.5, "undone"),
        mark(62.5, "bell"),
        key(64.0, "h"),
        mark(65.0, "today"),
    ],
    # The click that brings the window forward starts nothing; the next one does.
    "activate": [
        {"focus": False, "t": 0.2},
        {"focus": True, "t": 1.0},
        {"t": 1.05, "type": "tap", "x": SKY[0], "y": SKY[1]},
        mark(2.0, "activated"),
        {"t": 2.5, "type": "tap", "x": SKY[0], "y": SKY[1]},
        mark(4.0, "started"),
    ],
    # Today stays put under a stray click, and a lantern opens its block.
    # A feeling is given later from the block's detail, and changed back; then History opens
    # yesterday, and Today comes back.
    "today": [
        {"t": 1.0, "type": "tap", "x": 200.0, "y": 40.0},
        mark(2.0, "clicked"),
        {"t": 2.5, "type": "tap", "key": "day-lantern-0"},
        mark(3.5, "detail"),
        {"t": 4.0, "type": "tap", "key": "day-lantern-1"},
        mark(4.8, "unrated"),
        {"t": 5.0, "type": "tap", "key": "detail-felt-steady"},
        mark(5.8, "felt-given"),
        {"t": 6.0, "type": "tap", "key": "detail-felt-steady"},
        mark(6.8, "felt-cleared"),
        {"t": 7.0, "type": "tap", "key": f"day-{YESTERDAY}"},
        mark(8.0, "yesterday"),
        {"t": 8.5, "type": "tap", "key": "back-to-today"},
        mark(9.5, "back"),
        mark(10.0, "end"),
    ],
    # The whole day in one: a label, a block to the bell, a paused block, one let go, two more
    # to earn the long break, and Today. Breaks are skipped with S.
    "cycle": cycle(),
    "lantern-plain": lantern(paused=False),
    "lantern-paused": lantern(paused=True),
    # The review videos: the last minute of a 25-minute sunset, and a one-minute long break.
    "bell": [mark(30.0, "half"), mark(60.0, "zero"), mark(70.0, "end")],
    "night": [key(0.5, " "), mark(74.0, "end")],
    # The click that brings the window forward lands on a hover control and does nothing.
    "activate-control": [
        {"t": 0.1, "type": "move", "x": 300, "y": 510},
        {"focus": False, "t": 0.4},
        {"focus": True, "t": 1.0},
        {"t": 1.05, "type": "tap", "key": "control-play"},
        mark(2.4, "activated"),
        {"t": 3.0, "type": "tap", "key": "control-play"},
        mark(4.0, "started"),
    ],
    # The pointer rests on the controls' corner: they stay. A click in the corner off any
    # control, and one where a control has faded, never reach the sky.
    "dock": [
        {"t": 0.3, "type": "move", "x": 300, "y": 515},
        mark(5.0, "resting"),
        {"t": 5.5, "type": "tap", "x": 150, "y": 520},
        mark(6.5, "corner-click"),
        {"t": 7.0, "type": "move", "x": 190, "y": 200},
        {"t": 7.1, "type": "move", "x": 190, "y": 202},
        mark(9.5, "faded"),
        {"t": 9.6, "type": "tap", "key": "control-play"},
        mark(10.5, "faded-click"),
    ],
    # Dragging the sun down sets a shorter block, and letting go only sets it.
    "drag": [
        {"t": 0.2, "type": "down", "x": 186, "y": 140},
        {"t": 0.3, "type": "move", "x": 186, "y": 170},
        {"t": 0.4, "type": "move", "x": 186, "y": 205},
        {"t": 0.6, "type": "up", "x": 186, "y": 205},
        mark(3.0, "released"),
        key(3.5, " "),
        mark(5.0, "started"),
    ],
    # A word given is noted, then folds away; a question left alone fades into the break.
    "reflect": [
        *one_minute(0.1),
        key(1.0, " "),
        mark(62.0, "bell"),
        key(66.0, "2"),
        mark(66.5, "noted"),
        mark(69.0, "folded"),
        key(70.0, "s"),
        key(71.0, " "),
        mark(160.0, "unanswered"),
        mark(170.0, "unanswered-late"),
        key(171.0, "h"),
        mark(172.0, "today"),
    ],
    # Every key on one card.
    "keys": [key(0.2, "?"), mark(1.0, "open")],
    # A label longer than the window has room for.
    "long-label": [*label(0.1, "Thermodynamics problem set, chapter seven, the long derivations"), mark(4.5, "end")],
}

written = {f"{name}.json": json.dumps({"events": events}, indent=1) + "\n" for name, events in SCRIPTS.items()}
if "--check" in sys.argv[1:]:
    on_disk = {p.name for p in OUT.glob("*.json")}
    stale = sorted(n for n in written.keys() | on_disk if n not in on_disk or (OUT / n).read_text() != written.get(n))
    for name in stale:
        print(f"scripts/{name} differs from tools/scripts.py")
    sys.exit(1 if stale else 0)
for name, text in written.items():
    (OUT / name).write_text(text)
    print(f"scripts/{name}: {len(SCRIPTS[name.removesuffix('.json')])} events")
