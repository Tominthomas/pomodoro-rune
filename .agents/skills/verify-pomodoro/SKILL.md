---
name: verify-pomodoro
description: Drive the pomodoro-rune macOS app (the Rune sunset pomodoro timer) the way a user does and capture proof. The app is rendered headless with the release binary's own renderer, scripted keys and clicks, and an element and VoiceOver tree, all with no window. Use it to prove that a change to the timer, the sunset, the controls, Today or the places works as a user sees it, before opening or updating a PR.
---

# Verify pomodoro

The surface is one fixed 380 x 540 point macOS window. Headless mode runs the real release binary with the real view, key handlers and renderer. Only the window, the clock and storage are swapped. Headless runs start at 14:00 UTC on Wednesday 11 November 2026 and keep their prefs in memory, so a run never reads or writes `~/Library/Application Support/dev.tomin.pomodoro/prefs.json`. Any number of headless runs can go side by side.

Commands run from the repo root. `$SKILL` is `.agents/skills/verify-pomodoro`.

## Launch

```sh
source ~/.cargo/env && cargo build --release
```

The binary is `target/release/pomodoro`. Nothing stays running. Each headless command renders and exits, so there is no server to wait for. The build is ready once `$SKILL/scripts/doctor.sh` exits 0.

A bare `target/release/pomodoro` (or `cargo run --release`) opens the real window and writes the real prefs file. Don't do that to verify. See "Real window" below.

## Doctor

```sh
$SKILL/scripts/doctor.sh
```

It is read-only. It checks macOS, a Metal GPU, `ffmpeg` and `numpy`. It checks that the binary is newer than every file in `src/`, `assets/`, `Cargo.toml` and `Cargo.lock`, lists the scenes, renders one tree, checks that `scripts/` matches `tools/scripts.py`, and lists any running `pomodoro` pids without touching them. Run it first, and again whenever a render looks wrong. A stale binary is the usual cause.

## Drive

Pick the feature file in [`features/`](features/README.md). It names the scene, script, marks and keys for each journey. The pieces are listed here.

- **A script** is timestamped input in JSON. The 18 committed ones in `scripts/` come from `tools/scripts.py`. Edit that file and run `python3 tools/scripts.py` to add or change a committed one, because CI fails when they differ. A throwaway journey goes in the evidence directory as its own JSON file, never in `scripts/`. Events are `{"t":1.0,"type":"key","name":"s"}` (names include `" "`, `Up`, `Down`, `Enter`, `Escape`, `Tab`, and `"cmd":true` for ⌘), `{"type":"type","text":"Essay"}`, `{"type":"tap","key":"control-play"}` or `"x","y"` in points, `move`, `down`, `up`, `{"type":"wheel","x":190,"y":400,"dy":-2000}`, `{"focus":false}`, `{"type":"menu-pick","id":"Mesa"}`, and `{"type":"mark","name":"letgo"}`.
- **A scene** (`--scene NAME`) starts from a seeded moment instead of a first launch: `focus-early focus-late paused bell let-go short long evening last-minute night-minute today busy controls`. `today` has five blocks this morning and four weeks of history. A first launch has an empty log, 25-minute blocks and the length hint showing.
- **Read the screen** at a mark with `$SKILL/scripts/look.py` (see Helpers). Assert on `text` and `label`, and treat text with `ink` under 0.5 as unreadable. That is the floor `tools/check.py` and `src/tests.rs` use.
- **The named checks** are `python3 tools/check.py SCRIPT ...` for the checks on one script, plus the probes `pixels`, `bell-light` and `text-swap`. An unknown name fails. Run the checks for the scripts a change touches. `tools/ci.sh` runs everything and is CI's job.
- **The Rust tests** in `src/tests.rs` drive the same handlers through Rune's `Runtime` with no GPU, including relaunch from what was saved. Run one by name: `cargo test --release space_starts_pauses_and_resumes_the_sunset`.

Prefer element keys (`control-play`, `day-lantern-0`, `detail-rename`) to coordinates. The sky has no single key target that stays clear of the readout, so scripts tap it at `190,200`.

Use `--fps 60` for any script where a tap or a digit must land on time (`start`, `activate`, `activate-control`, `drag`, `today`, `bell`). Use `--fps 10` for the five-minute scripts (`cycle`, `reflect`, `undo`, `lantern-*`). The timer runs on deadlines, not frames, so 10 fps keeps those quick and gives the same answers. `tools/check.py` has the table in `FPS`.

## Evidence

```sh
RUN=shots/verify/$(date -u +%Y%m%dT%H%M%SZ)-<feature>; mkdir -p "$RUN"
```

`/shots/` is gitignored, and cleanup never removes `shots/verify/`. Keep these in `$RUN`:

- `log.txt` holds every command and its output. Append with `2>&1 | tee -a "$RUN/log.txt"`.
- `<mark>.tree.json` and `<mark>.png` come from `look.py --out "$RUN/<mark>"`, at the moment before the action and the moment after it.
- `<journey>.mp4` is the journey as a user watches it, with the keys and clicks drawn on. Run `target/release/pomodoro --script S [--scene N] --video "$RUN/<journey>.mp4" --fps 30 --keys --pointer`. Add `--start mark:A --duration 6` to keep a long script short.
- `check.txt` is the output of `python3 tools/check.py <script>` for the scripts the change touches. Piping through `tee` hides the exit code, so the proof fails if `grep -c FAIL "$RUN/check.txt"` prints anything but 0.

A proof follows the user's path. It presses the keys, taps the keyed controls or drags the sun, and never sets state from a test hook. Capture the state before the action and after it, not only the end. For anything saved, also show the second view. That is Today's numbers or labels after the action, or a `src/tests.rs` relaunch test. Headless never touches the real prefs file, so a saved file on disk is a real-window check (below).

## Cleanup

Headless commands exit on their own, so there are no processes to stop. Delete throwaway script JSON and temp renders outside `$RUN`, and keep `$RUN`. If you started a real window yourself, kill only the pid you recorded with `$!`. Never use `pkill pomodoro` or `killall`. Tomin may have the app open, and a kill skips the save that runs on quit. Then confirm the proof survived with `ls "$RUN"`.

## Helpers

- `$SKILL/scripts/doctor.sh` is described under Doctor. It exits 1 if any check fails.
- `$SKILL/scripts/look.py SCRIPT --at mark:NAME [--scene NAME] [--fps N] [--theme light|dark] [--out PREFIX] [KEY ...]` prints each key's `text`, `label`, tooltip `help`, VoiceOver value `says`, `ink` and `rect` at that moment. A key ending in `*` is a prefix, and a key that isn't drawn prints `absent`. With `--out` it keeps `PREFIX.tree.json` and a 2x `PREFIX.png` of the same moment. Example: `$SKILL/scripts/look.py scripts/undo.json --at mark:letgo --fps 10 time phase undo`.
- `tools/shots.sh` writes the review pack into `shots/review/`. That is every scene in every place in light and dark, two videos, and redraw counts in `frames.txt`. Use it for a visual change that touches every scene.

## Real window (not yet proven)

Headless can't show these. Verify them in the real window or list them as unverified in the PR.

- Tooltip timing in the real window. Headless draws a control's tooltip in the PNG once the pointer rests on it, and the tree has its text as `help`.
- The place menu as macOS draws it. Headless `menu-pick` proves the choice lands.
- The click that brings an inactive window forward. Headless fakes it with `{"focus":false}` then `{"focus":true}`, but the real activation timing isn't covered.
- The last save being flushed to `prefs.json` on quit, and the saved file read back on the next launch.
- Notifications posted when the window isn't in front. They need an app bundle.
- Window chrome, dragging the window, resizing (once it exists), the Dock icon and the app icon.
- Following the system appearance live. Headless takes `--theme` once.

The agent shell here runs in Tomin's Aqua login session (`launchctl managername` prints `Aqua`). It has no Screen Recording permission (`screencapture -x` fails with "could not create image from display"). System Events scripting fails too (`osascript` gives syntax error -2741 on `application process`). So no agent has driven or captured the real window yet. Hand Tomin this checklist with the build and ask for a screenshot of each step.

1. Quit any running Pomodoro. Run `cargo run --release` (or open the installed bundle).
2. Hover a control. The tooltip names the key, for example "Start focus  Space".
3. Click the place control and pick Lake from the menu. The scene changes.
4. Click another app, then click the sky once. Nothing starts. Click again and the sunset starts.
5. Press S, then quit with ⌘Q before starting the break. Relaunch. Today shows a ring for the block.
6. With another app in front, let a one-minute block reach its bell. A notification arrives (bundle only).

The permission-gated path is untested, and every step of it is unproven. Rune's installer is pinned to the app's Rune revision at `~/.cargo/git/checkouts/rune-c9b805e9ed8a3730/bb310fc/scripts/mac-app`. Build a bundle with `mac-app --name Pomodoro --id dev.tomin.pomodoro --exe pomodoro --bin target/release/pomodoro --no-rune-ready`, which installs to `~/Applications/Pomodoro.app` and launches it. That launch uses the real prefs. Find the window's CGWindowID (no tool for that is installed here yet), capture it with `screencapture -l <windowid> "$RUN/window.png"`, and send input with System Events. All of that needs Tomin to grant Screen Recording and Accessibility to the T3 service. Before relying on any of it, run it once end to end and update this section.
