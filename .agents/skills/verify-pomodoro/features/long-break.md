# Long break

The fourth lantern of a cycle earns a long break of fifteen minutes. Night falls, the four lanterns lift off the water and rise among the stars, and when the break ends dawn opens the next block. A skipped block doesn't count toward the four.

## Sub-features

- `long-earn` gives the long break after four lit lanterns, not counting a skipped block.
- `long-night` darkens the sky and lifts the lanterns off the water once the break starts.
- `long-rise` puts all four lanterns up in the sky later in the break.
- `long-dawn` brings the morning back for the next block when the break ends. The sky only darkens from sunset into the break, never below where it settles.
- `long-skip` skips the long break with `S`, which starts a new cycle with no lanterns on the water.

## How to get to it (user POV)

- Finish four focus blocks in one cycle, then press Space or tap the sky to start the long break.
- Press `S` or click `Skip break` to skip it.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. `cycle.json` runs five one-minute blocks, with the third skipped, then sets the long break to one minute. `night.json` starts from the `night-minute` scene, which has four lanterns and a one-minute long break waiting.

- **Earned.** Run `look scripts/cycle.json --at mark:long --fps 10 phase 'lantern-*' --out $RUN/long`. `phase` reads `long rest · space when you're ready` and four lanterns exist, `lantern-0` to `lantern-3`.
- **Night falls.** Press Space. Run `look scripts/cycle.json --at mark:night-early --fps 10 phase 'lantern-*' --out $RUN/night`. `phase` reads `long rest, four lanterns up` and every lantern's centre (`rect` y plus half its height) is below 330, still on the water.
- **Lanterns rise.** Run `look scripts/cycle.json --at mark:rising --fps 10 'lantern-*' --out $RUN/rising`. Every lantern's centre is above 330.
- **Sky only darkens.** Run `python3 tools/check.py bell-light`. It passes for light and dark.
- **Words never overlap.** Run `python3 tools/check.py text-swap`. It passes over 72 moments of the bell and the long break's end.
- **Skip and relaunch.** Run `cargo test --release skipping_the_long_break_in_the_second_it_starts_begins_a_new_cycle_once` and `cargo test --release four_lanterns_earn_the_long_break_and_quitting_during_its_bell_keeps_it`. Both pass.
- **Proof.** Run `python3 tools/check.py cycle bell-light text-swap | tee $RUN/check.txt` and `app --scene night-minute --script scripts/night.json --video $RUN/long-break.mp4 --fps 30`. The video shows the lanterns rising and dawn.

## Gotchas

- `cycle.json` takes about five minutes of virtual time. Render it at `--fps 10`.
- A real long break is fifteen minutes. Only the scripts shorten it, by pressing Down while it waits.
- The lantern counts above use the `lantern-` prefix with digits only. `day-lantern-` keys belong to Today.
- `bell-light` and `text-swap` render many frames and take longer than a named script check. Run them only when the sky or the readout's words change.
