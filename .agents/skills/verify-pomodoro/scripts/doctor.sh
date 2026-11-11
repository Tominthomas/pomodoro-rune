#!/bin/sh
# Read-only: is this checkout worth driving headless? Prints one line per check and exits 1 if
# any fails. Run from anywhere: .agents/skills/verify-pomodoro/scripts/doctor.sh
cd "$(dirname "$0")/../../../.." || exit 1
APP=target/release/pomodoro
failed=0
ok() { printf 'ok    %s\n' "$*"; }
bad() { printf 'FAIL  %s\n' "$*"; failed=1; }
note() { printf 'note  %s\n' "$*"; }

[ "$(uname)" = Darwin ] && ok "macOS $(sw_vers -productVersion)" || bad "not macOS; headless rendering needs a Mac"
system_profiler SPDisplaysDataType 2>/dev/null | grep -q 'Metal Support' && ok "Metal GPU" || bad "no Metal GPU"
command -v ffmpeg >/dev/null 2>&1 && ok "ffmpeg" || bad "no ffmpeg (needed for --video and tools/check.py)"
python3 -c 'import numpy' 2>/dev/null && ok "numpy" || bad "no numpy (needed by tools/check.py)"

if [ ! -x "$APP" ]; then
  bad "$APP missing; run: source ~/.cargo/env && cargo build --release"
else
  stale=$(find src assets Cargo.toml Cargo.lock -newer "$APP" -type f 2>/dev/null | head -3)
  [ -z "$stale" ] && ok "$APP is newer than every source file" || bad "$APP is older than: $stale; rebuild"
  scenes=$("$APP" --list-scenes 2>/dev/null | tr '\n' ' ')
  [ -n "$scenes" ] && ok "scenes: $scenes" || bad "$APP --list-scenes printed nothing"
  tmp=$(mktemp -d)
  if "$APP" --scene focus-late --tree "$tmp/t.json" --at 0.6 --scale 1 >/dev/null 2>&1 \
    && python3 -c 'import json,sys; sys.exit(not any(e.get("key") == "time" for e in json.load(open(sys.argv[1]))["elements"]))' "$tmp/t.json"; then
    ok "headless render reaches the readout (scene focus-late has key 'time')"
  else
    bad "headless render failed: $APP --scene focus-late --tree $tmp/t.json --at 0.6"
  fi
  rm -rf "$tmp"
fi

python3 tools/scripts.py --check >/dev/null 2>&1 && ok "scripts/ matches tools/scripts.py" \
  || bad "scripts/ differs from tools/scripts.py; run python3 tools/scripts.py --check"

running=$(pgrep -x pomodoro | tr '\n' ' ')
[ -z "$running" ] && ok "no pomodoro process running" \
  || note "pomodoro pids running: $running(not yours unless you recorded the pid; leave them alone)"
prefs="$HOME/Library/Application Support/dev.tomin.pomodoro/prefs.json"
[ -f "$prefs" ] && note "real prefs exist at $prefs; headless runs never read or write it" \
  || note "no real prefs file yet; headless runs never create one"

exit $failed
