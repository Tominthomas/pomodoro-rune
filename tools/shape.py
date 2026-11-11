#!/usr/bin/env python3
"""Fails when a Rust file or folder has grown too big. A port of Rune's scripts/file-size.sh and
scripts/folder-shape.sh at the revision Cargo.toml pins, over the files Git tracks in src/ and tests/.

File size. Every line of a test file (tests.rs, *_tests.rs, *_test.rs, or anything in a tests/
folder) and every line of a #[cfg(test)] item is a test line; the rest are non-test lines. Test
items are found by counting braces, so a brace in a comment or a raw string can skew a count.
    more than 800 non-test lines                      warning
    more than 1500 non-test lines                     error
    more than 1000 lines in one test file or module   error

Folder shape. x.rs or x/ beside two or more x_*.rs files, or three x_*.rs files alone, is a
prefix family and an error: move them into x/. A folder with more than 40 non-test .rs files is
a warning, and with more than 60 an error.

    python3 tools/shape.py
"""

import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import NamedTuple

SOFT, HARD, TEST_CAP = 800, 1500, 1000
WIDE, TOO_WIDE, FAMILY = 40, 60, 3

TEST_STEM = re.compile(r"(^|[_.-])tests?$")
CFG_TEST = re.compile(r"^\s*#\[cfg\(test\)\]")
ATTRIBUTE = re.compile(r"^\s*#\[")


class Finding(NamedTuple):
    error: bool
    count: int
    path: str
    why: str


def is_test(path: Path) -> bool:
    return bool(TEST_STEM.search(path.stem)) or "tests" in path.parts[:-1]


def braces(line: str) -> int:
    """How many more braces `line` opens than it closes, outside strings, chars and comments."""
    line = re.sub(r"\\.", "", line)
    line = re.sub(r'"[^"]*"', "", line)
    line = re.sub(r"'.'", "", line)
    line = line.split("//", 1)[0]
    return line.count("{") - line.count("}")


def measure(path: Path) -> tuple[int, int]:
    """The non-test lines in `path`, and the lines in its longest test file or test item."""
    lines = path.read_text().splitlines()
    if is_test(path):
        return 0, len(lines)
    prod = longest = run = depth = 0
    # None outside test items, "head" between #[cfg(test)] and the item's opening brace, "body" inside it.
    state = None
    for line in lines:
        if state is None:
            if CFG_TEST.match(line):
                state, run = "head", 1
            else:
                prod += 1
            continue
        run += 1
        if state == "head":
            if ATTRIBUTE.match(line):
                continue
            depth = braces(line)
            if depth > 0:
                state = "body"
            elif "{" in line or line.rstrip().endswith(";"):
                state = None
        else:
            depth += braces(line)
            if depth <= 0:
                state = None
        if state is None:
            longest, run = max(longest, run), 0
    return prod, max(longest, run)


def sizes(files: list[Path]) -> list[Finding]:
    found = []
    for path in files:
        prod, tests = measure(path)
        if tests > TEST_CAP:
            found.append(Finding(True, tests, str(path), f"test lines in one file or module, limit {TEST_CAP}"))
        if prod > HARD:
            found.append(Finding(True, prod, str(path), f"non-test lines, limit {HARD}: split a module out first"))
        elif prod > SOFT:
            found.append(Finding(False, prod, str(path), f"non-test lines, soft limit {SOFT}"))
    return found


def folders(files: list[Path]) -> list[Finding]:
    stems: dict[Path, list[str]] = defaultdict(list)
    subdirs: dict[Path, set[str]] = defaultdict(set)
    for path in files:
        parts = path.parent.parts
        for i in range(1, len(parts)):
            subdirs[Path(*parts[:i])].add(parts[i])
        if not is_test(path):
            stems[path.parent].append(path.stem)
    found = []
    for folder in sorted(stems.keys() | subdirs.keys()):
        names = stems[folder]
        families = defaultdict(list)
        for stem in names:
            if m := re.match(r"([^_-]+)[_-]", stem):
                families[m.group(1)].append(stem)
        for prefix, family in sorted(families.items()):
            if len(family) >= (2 if prefix in names or prefix in subdirs[folder] else FAMILY):
                found.append(Finding(True, len(family), f"{folder}/{prefix}_*", f"files: move into {prefix}/"))
        if len(names) > WIDE:
            limit = f"limit {TOO_WIDE}" if len(names) > TOO_WIDE else f"soft limit {WIDE}"
            found.append(
                Finding(len(names) > TOO_WIDE, len(names), f"{folder}/", f"files: group into subfolders, {limit}")
            )
    return found


def main() -> int:
    os.chdir(Path(__file__).resolve().parent.parent)
    tracked = subprocess.check_output(["git", "ls-files", "src", "tests"], text=True).splitlines()
    files = [Path(f) for f in tracked if f.endswith(".rs")]
    found = sorted(sizes(files) + folders(files), key=lambda f: (not f.error, -f.count))
    annotate = os.environ.get("GITHUB_ACTIONS") == "true"
    for f in found:
        print(f"{f.count:6d}  {'ERROR' if f.error else 'warn':5}  {f.path} ({f.why})")
        if annotate:
            print(f"::{'error' if f.error else 'warning'} file={f.path}::{f.count} {f.why}")
    errors = sum(f.error for f in found)
    print(f"{len(files)} Rust files: {errors} errors, {len(found) - errors} warnings")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
