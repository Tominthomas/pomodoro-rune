# Pomodoro verification map

This directory is the maintained source for verifying what a user sees in Pomodoro. Read this index, then use the matching feature file as the recipe. `../SKILL.md` covers building, the doctor, evidence and cleanup.

## Baseline preconditions

- `source ~/.cargo/env && cargo build --release` has run, and `.agents/skills/verify-pomodoro/scripts/doctor.sh` exits 0.
- `RUN=shots/verify/<UTC stamp>-<feature>` exists for this run's evidence.
- A run with no `--scene` is a first launch at 14:00 UTC on Wednesday 11 November 2026. It has an empty log, 25-minute focus blocks, 5- and 15-minute breaks, the place Coast, and the length hint showing.
- A run with `--scene NAME` starts from that seeded moment. `today` has five blocks this morning, labelled `Reading & notes` and `Problem set`, and four weeks behind it.

## Driving conventions

- `look` means `.agents/skills/verify-pomodoro/scripts/look.py`, and `app` means `target/release/pomodoro`.
- Every recipe drives a committed script in `scripts/` at a mark it names, or a throwaway script the file gives in full. Save a throwaway script as `$RUN/<name>.json`, never in `scripts/`.
- Act through keys, keyed taps (`{"type":"tap","key":"control-play"}`), sky taps at `190,200` and drags on the sun. Never set state another way.
- Read text through `look`. Text drawn with `ink` under 0.5 counts as not shown.
- Expected strings are copied from the app. Compare them exactly, including `·` and the straight apostrophe in `you're`.

## Proof and skip reporting

- Keep `look --out` captures from before and after the action, the `tools/check.py <script>` output, and a `--video ... --keys --pointer` of the journey.
- For a saved change, show it again from a second view, such as Today's numbers, the lantern's label, or a `src/tests.rs` relaunch test.
- Record the feature file and sub-feature ID with each artifact in `$RUN/log.txt`.
- If a path can't be driven headless, list it as unverified with the reason from the feature's `Gotchas` or SKILL.md's "Real window". Never report it as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph on the behavior a user sees. Then come exactly four H2s in this order.

1. `Sub-features` lists short IDs, one line each.
2. `How to get to it (user POV)` lists every entry point a user has.
3. `Driving it with the headless renderer` starts with `Preconditions:`, then pairs each user action with an exact command and the result to see.
4. `Gotchas` lists traps that waste or invalidate a run.

## Features

- [Focus block](./focus-block.md) covers starting a block, naming it, the bell, the lantern and the reflection.
- [Pause and resume](./pause-resume.md) covers Space, the sky, the play control, the Resume button and the knot on the lantern.
- [Skip and undo](./skip-undo.md) covers ending a block early, the ring it leaves, undo, and skipping a break.
- [Set the length](./set-length.md) covers dragging the sun, the arrow keys, the hint and the long break's length.
- [Long break](./long-break.md) covers the fourth lantern, the night, the lanterns rising and dawn.
- [Today and history](./today-history.md) covers Today's numbers, block detail, feelings, renaming and past days.
- [Places](./places.md) covers Coast, Lake and Mesa by key and by menu.

Not mapped yet, though `tools/check.py` covers them: the controls in sight while a block waits and receding once it runs (`hover`, `dock`), the activating click (`activate`, `activate-control`), the keys card (`keys`) and long labels (`long-label`).
