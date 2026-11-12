# Skip and undo

During a focus block, `S` or the `Skip` control ends the block early. It lights no lantern and its minutes still count, shown as a ring in Today. The line under the time says so (`Skipped. 21 min still count, no lantern.`). Until the break or the next block starts, an `Undo skip` button (on the scene, and as `timer-undo` in Today) can bring the block back, still running, and it then counts as one block. During a break the same key and control skip to the next block.

## Sub-features

- `skip-key` ends a running block with `S`.
- `skip-control` ends it with the `Skip` control (`control-skip`, labelled `Skip this block`).
- `skip-ring` shows the block in Today as a ring labelled `skipped, N min kept`. It doesn't count toward the four lanterns.
- `skip-undo` brings the block back with `U`, ⌘Z or the `undo` button. It reaches its bell as one block.
- `skip-break` skips a break with `S` or the `Skip` control (`control-skip`, labelled `Skip break`).

## How to get to it (user POV)

- Press `S` while a focus block runs or is paused.
- Show the controls, then click `Skip`. It sits left of the primary control, outside its place, and appears only while a block runs or is paused, or a break waits or runs.
- Press `U` or ⌘Z, or click `Undo skip`, before you start the break or the next block.
- During a break, press `S` or click `Skip`.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. `undo.json` winds the block to one minute, starts it at 1.0 s and presses `S` at 21.0 s.

- **Skip.** Press `S` 20 seconds into a one-minute block. Run `look scripts/undo.json --at mark:skipped --fps 10 time time-unit phase undo --out $RUN/skipped`. `phase` reads `Skipped before a minute. No lantern.`, `time` reads `under a minute` with `time-unit` `focused`, and `undo` is drawn, labelled `Undo skip`.
- **Undo.** Press `U` at 23.0 s. Run `look scripts/undo.json --at mark:undone --fps 10 time phase undo --out $RUN/undone`. `phase` reads `focus`, `time` reads `00:38` and `undo` is absent.
- **One block.** Run `look scripts/undo.json --at mark:today --fps 10 stat-lanterns-value stat-skipped day-ring-0 --out $RUN/today`. `stat-lanterns-value` reads `1`, and `stat-skipped` and `day-ring-0` are absent.
- **Ring kept.** Run `look scripts/cycle.json --at mark:b3-skipped --fps 10 phase 'lantern-*'`. `phase` reads `Skipped. 1 min still counts, no lantern.` and only two lanterns exist. At `mark:today`, `stat-skipped-value` reads `1` and `day-ring-0` is labelled with `skipped, 1 min kept`.
- **Skip a break.** Run `look scripts/cycle.json --at mark:wound-2 --fps 10 phase top-text`. After `S` in the break, `phase` reads `focus · space to begin` and `top-text` reads `Reading & notes`.
- **Control entry.** The committed scripts press `S`. To prove the click, save this as `$RUN/skip-click.json` and run `look $RUN/skip-click.json --at mark:skipped phase undo`. `phase` reads `Skipped before a minute. No lantern.`.

  ```json
  {"events":[{"t":0.1,"type":"key","name":" "},{"t":1.0,"type":"move","x":300,"y":515},{"t":1.5,"type":"tap","key":"control-skip"},{"t":2.5,"type":"mark","name":"skipped"}]}
  ```

- **Proof.** Run `python3 tools/check.py undo cycle | tee $RUN/check.txt` and `app --script scripts/undo.json --video $RUN/undo.mp4 --fps 30 --start mark:skipped --duration 5 --keys`.

## Gotchas

- The phase line explains the skip for as long as Undo is offered, so a break waiting after a skip says `Skipped. …`, not `1 of 4 lanterns · 5-minute break ready`. A mark after Space starts the break sees the usual break line and no `undo`.
- Records saved before the rename say `let_go`. They load as skipped blocks (`#[serde(alias)]` on `Outcome::Skipped`).
- `control-skip` is always in the accessibility tree while it applies, but its look is hidden until the controls show. Move the pointer over the window or press Tab first, or the tap lands on `dock-cover` and does nothing.
- During the bell's ring after a finished block, and while a focus block waits, there is no `control-skip` at all.
