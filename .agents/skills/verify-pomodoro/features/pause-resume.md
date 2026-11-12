# Pause and resume

A running block can be paused and resumed from the keyboard, the sky or the controls. A paused block still lights a lantern at its bell, with a small knot on the cord for each pause.

## Sub-features

- `pause-key` pauses with Space and resumes with Space.
- `pause-tap` pauses and resumes with a tap on the sky.
- `pause-controls` uses the play control (labelled `Pause`, then `Resume`) and the `Resume` button under the time.
- `pause-knot` gives the paused block's lantern the label `Lantern N, paused once` and draws it differently from a plain one.

## How to get to it (user POV)

- Press Space while a block runs. Press it again to resume.
- Tap the sky while a block runs. Tap it again to resume.
- Click `control-play`, or click `Resume` (key `resume`) while paused.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. Use `--fps 60` for `start.json`, because its digits are checked to the second.

- **Pause.** Tap the sky, then press Space at 2.5 s. Run `look scripts/start.json --at mark:paused time phase resume control-play --out $RUN/paused`. `time` reads `24:58`, `phase` reads `the sky waits · space to resume`, `resume` is drawn, and `control-play` is labelled `Resume`.
- **Resume.** Tap the sky at 3.6 s. Run `look scripts/start.json --at mark:resumed time phase --out $RUN/resumed`. `time` reads `24:57` and `phase` reads `focus`.
- **Pause inside a cycle.** Run `look scripts/cycle.json --at mark:b2-paused --fps 10 phase`. `phase` reads `the sky waits · space to resume`.
- **Knot on the lantern.** Run `look scripts/cycle.json --at mark:b2-break --fps 10 lantern-1`. `lantern-1` is labelled `Lantern 2, paused once`.
- **Knot as drawn.** Run `python3 tools/check.py pixels`. It passes when more than 40 pixels of `day-lantern-art-0` differ between `lantern-plain` and `lantern-paused`, and none differ between two plain renders.
- **Proof.** Run `python3 tools/check.py start cycle pixels | tee $RUN/check.txt` and `app --script scripts/start.json --video $RUN/pause.mp4 --fps 30 --keys --pointer`.

## Gotchas

- The `resume` button and the play control are separate keys. A recipe that only presses Space proves neither click.
- Pausing doesn't extend the block's planned length. Where the cycle needs both blocks to end at once, `lantern-paused` starts a second earlier and holds for a second.
- `cargo test --release space_starts_pauses_and_resumes_the_sunset` and `a_tap_on_the_sky_starts_pauses_and_resumes_it_too` cover the same path with no GPU.
