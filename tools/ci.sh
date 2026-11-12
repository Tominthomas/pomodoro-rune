#!/bin/sh
# Every check the repo has, in order, stopping at the first failure. CI runs this; run it before
# pushing too. The headless checks render the app, so they need a Metal GPU, ffmpeg and numpy;
# without them, or with SKIP_HEADLESS=1, they are skipped and the script says why.
set -eu
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

command -v ruff >/dev/null 2>&1 || ruff() { pipx run ruff "$@"; }

# CI installs the pinned tools in .github/workflows/ci.yml. Locally, put them on PATH or in
# target/tools/bin. A missing one is skipped with a message, except in CI, where it fails.
PATH="$PWD/target/tools/bin:$PATH"
has() {
  command -v "$1" >/dev/null 2>&1 && return 0
  if [ "${CI:-}" = true ]; then
    echo "$1 is not installed" >&2
    exit 1
  fi
  step "$1 SKIPPED (not installed; see .github/workflows/ci.yml for the version)"
  return 1
}

step "cargo fmt --check"
cargo fmt --check
if has typos; then
  step "typos"
  typos --hidden --exclude .git
fi
if has taplo; then
  step "taplo"
  toml=$(git ls-files '*.toml')
  RUST_LOG=warn taplo fmt --check $toml
  RUST_LOG=warn taplo check $toml
fi
if has cargo-machete; then
  step "cargo machete"
  cargo machete .
fi
if has cargo-deny; then
  step "cargo deny"
  cargo deny --locked check advisories licenses sources
fi
step "file sizes and folder shape"
python3 tools/shape.py
step "cargo clippy"
cargo clippy --locked --all-targets --release -- -D warnings
step "cargo doc"
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --document-private-items --release
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
