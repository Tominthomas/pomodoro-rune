# Quiet digits

While a focus block runs, the sunset is the timer, so the time's digits step back. They come out while the pointer moves over the window or rests on the controls, and after Tab, then fade once it rests. They always show while a block waits, while the sun is dragged, while paused, at the bell and through a break. VoiceOver still reads the time. View > Always Show Time keeps them in sight, and is remembered.

## Sub-features

- `digits-hidden` draws the time at `ink` 0 while a focus block runs, with its `says` value intact.
- `digits-called` draws it again while the pointer moves or after Tab, and fades it 2 s after the pointer rests.
- `digits-paused` shows it while the block is paused.
- `digits-always` keeps it in sight through a running block once View > Always Show Time is ticked, saved as the `always-show-time` pref.

## How to get to it (user POV)

- Start a block and leave the pointer still.
- Move the pointer over the window, rest it on the controls, or press Tab.
- Press Space to pause.
- Pick View > Always Show Time in the menu bar.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. Use `--fps 60` for `digits.json`, because its digits are checked to the second.

- **Hidden.** Run `look scripts/digits.json --at mark:hidden time phase --out $RUN/hidden`. `time` has `ink` 0 and says `24 minutes 58 seconds`, and `phase` reads `focus`.
- **Called.** The pointer moves at 3.2 s. Run `look scripts/digits.json --at mark:called time --out $RUN/called`. `time` reads `24:57`. At `mark:rested` it is back at `ink` 0.
- **Paused.** Space at 6.6 s. Run `look scripts/digits.json --at mark:paused time phase --out $RUN/paused`. `time` reads `24:54` and `phase` reads `the sky waits · space to resume`.
- **Always.** View > Always Show Time at 7.4 s, then Space. Run `look scripts/digits.json --at mark:always time --out $RUN/always`. `time` reads `24:51` with the pointer still.
- **Proof.** Run `python3 tools/check.py digits | tee $RUN/check.txt` and `app --script scripts/digits.json --video $RUN/digits.mp4 --fps 30 --keys --pointer`.

## Gotchas

- The fade is about 0.2 s. A mark within that of a pointer move reads a half-drawn time.
- A tap is a pointer move too, so a script that starts a block with a sky tap shows the digits for 2 s after it.
- `cargo test --release digits` covers the rule as a table, the pointer, Tab, pause and the setting across a relaunch, with no GPU.
