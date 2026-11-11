#!/usr/bin/env python3
"""Renders the release build headless at one moment of a script and prints what a person sees
and what VoiceOver reads for the keys asked for. With --out it also keeps the tree and a PNG of
that same moment as evidence.

    look.py SCRIPT --at mark:NAME [--scene NAME] [--fps N] [--theme light|dark] [--out PREFIX] [KEY ...]

KEY is an element key (`time`, `phase`, `undo`) or a prefix ending in `*` (`day-lantern-*`).
With no keys it prints every keyed element that has text or a label. Exits 1 if a render fails.
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
APP = ROOT / "target/release/pomodoro"


def ink(e):
    """How opaque the element's text is drawn; tools/check.py reads text the same way, and treats
    under 0.5 as not readable."""
    c = e.get("color")
    a = int(c[7:9], 16) / 255 if isinstance(c, str) and len(c) == 9 else 1.0
    return a * (1.0 if e.get("opacity") is None else e["opacity"])


def render(args, flag, out, scale):
    cmd = [APP, "--script", args.script, flag, out, "--at", args.at, "--fps", args.fps, "--scale", scale]
    cmd += ["--scene", args.scene] if args.scene else []
    cmd += ["--theme", args.theme] if args.theme else []
    run = subprocess.run([str(c) for c in cmd], capture_output=True, text=True)
    if run.returncode:
        sys.exit(f"render failed: {' '.join(map(str, cmd))}\n{run.stderr}")


def spoken(node, out):
    if node.get("key"):
        out[node["key"]] = node
    for child in node.get("children", []):
        spoken(child, out)
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("script")
    p.add_argument("--at", required=True)
    p.add_argument("--scene")
    p.add_argument("--fps", default="60")
    p.add_argument("--theme", choices=["light", "dark"])
    p.add_argument("--out", help="keep PREFIX.tree.json and PREFIX.png")
    p.add_argument("keys", nargs="*")
    args = p.parse_args()

    with tempfile.TemporaryDirectory() as d:
        tree = Path(args.out + ".tree.json") if args.out else Path(d) / "tree.json"
        tree.parent.mkdir(parents=True, exist_ok=True)
        render(args, "--tree", tree, "1")
        dump = json.loads(tree.read_text())
    if args.out:
        render(args, "--snap", args.out + ".png", "2")

    a11y = spoken(dump["accessibility"], {})
    keyed = {e["key"]: e for e in dump["elements"] if e.get("key")}

    def wanted(key):
        if not args.keys:
            return bool(keyed[key].get("text") or keyed[key].get("label"))
        return any(key == k or (k.endswith("*") and key.startswith(k[:-1])) for k in args.keys)

    picks = [k for k in keyed if wanted(k)]
    print(f"{args.script} @ {args.at} (t={dump['t']:.2f}s, scene {args.scene or 'first launch'})")
    for k in picks:
        e = keyed[k]
        parts = [f"ink={ink(e):.2f}"]
        for field in ("text", "label", "help"):
            if e.get(field):
                parts.append(f"{field}={e[field]!r}")
        if a11y.get(k, {}).get("value"):
            parts.append(f"says={a11y[k]['value']!r}")
        parts.append(f"rect={e.get('rect')}")
        print(f"  {k}: {' '.join(parts)}")
    for k in args.keys:
        if not k.endswith("*") and k not in keyed:
            print(f"  {k}: absent")
    if args.out:
        print(f"kept {args.out}.tree.json and {args.out}.png")


main()
