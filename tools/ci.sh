#!/bin/sh
# Every check the repo has, in order, stopping at the first failure. CI runs this; run it before
# pushing too. The headless checks render the app, so they need a Metal GPU, ffmpeg and numpy;
# without them, or with SKIP_HEADLESS=1, they are skipped and the script says why.
set -eu
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

command -v ruff >/dev/null 2>&1 || ruff() { pipx run ruff "$@"; }

step "cargo fmt --check"
cargo fmt --check
step "cargo clippy"
cargo clippy --locked --all-targets --release -- -D warnings
step "cargo test"
cargo test --locked --release
step "ruff"
ruff format --check tools
ruff check tools
step "scripts match tools/scripts.py"
python3 tools/scripts.py --check

skip=""
[ "${SKIP_HEADLESS:-0}" = 1 ] && skip="$skip, SKIP_HEADLESS=1"
system_profiler SPDisplaysDataType 2>/dev/null | grep -q 'Metal Support' || skip="$skip, no Metal GPU"
command -v ffmpeg >/dev/null 2>&1 || skip="$skip, no ffmpeg"
python3 -c 'import numpy' 2>/dev/null || skip="$skip, no numpy"
if [ -n "$skip" ]; then
  step "headless checks SKIPPED (${skip#, })"
  exit 0
fi
step "headless checks"
cargo build --locked --release
python3 tools/check.py
python3 tools/contrast.py
