# Let go and undo

During a focus block, `S` or the `let go` control ends the block early. It lights no lantern and its minutes still count, shown as a ring in Today. For nine seconds an `Undo let go` button can bring the block back, still running, and it then counts as one block. During a break the same key and control skip to the next block.

## Sub-features

- `letgo-key` ends a running block with `S`.
- `letgo-control` ends it with the `let go` control (`control-skip`).
- `letgo-ring` shows the block in Today as a ring labelled `let go, N min kept`. It doesn't count toward the four lanterns.
- `letgo-undo` brings the block back with `U`, ⌘Z or the `undo` button. It reaches its bell as one block.
- `skip-break` skips a break with `S` or the `Skip break` control (`control-skip`).

## How to get to it (user POV)

- Press `S` while a focus block runs or is paused.
- Show the controls, then click `let go`. It appears only while a block runs or is paused.
- Press `U` or ⌘Z, or click `Undo let go`, within nine seconds.
- During a break, press `S` or click `Skip break`.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. `undo.json` winds the block to one minute, starts it at 1.0 s and presses `S` at 21.0 s.

- **Let go.** Press `S` 20 seconds into a one-minute block. Run `look scripts/undo.json --at mark:letgo --fps 10 time phase undo --out $RUN/letgo`. `phase` reads `let go; the minutes still count`, `time` reads `under a minute`, and `undo` is drawn, labelled `Undo let go`.
- **Undo.** Press `U` at 23.0 s. Run `look scripts/undo.json --at mark:undone --fps 10 time phase undo --out $RUN/undone`. `phase` reads `focus`, `time` reads `00:38` and `undo` is absent.
- **One block.** Run `look scripts/undo.json --at mark:today --fps 10 stat-lanterns-value stat-let-go day-ring-0 --out $RUN/today`. `stat-lanterns-value` reads `1`, and `stat-let-go` and `day-ring-0` are absent.
- **Ring kept.** Run `look scripts/cycle.json --at mark:b3-letgo --fps 10 phase 'lantern-*'`. `phase` reads `let go; the minutes still count` and only two lanterns exist. At `mark:today`, `stat-let-go-value` reads `1` and `day-ring-0` is labelled with `let go, 1 min kept`.
- **Skip a break.** Run `look scripts/cycle.json --at mark:wound-2 --fps 10 phase top-text`. After `S` in the break, `phase` reads `focus · space to begin` and `top-text` reads `Reading & notes`.
- **Control entry.** The committed scripts press `S`. To prove the click, save this as `$RUN/letgo-click.json` and run `look $RUN/letgo-click.json --at mark:letgo phase undo`. `phase` reads `let go; the minutes still count`.

  ```json
  {"events":[{"t":0.1,"type":"key","name":" "},{"t":1.0,"type":"move","x":300,"y":515},{"t":1.5,"type":"tap","key":"control-skip"},{"t":2.5,"type":"mark","name":"letgo"}]}
  ```

- **Proof.** Run `python3 tools/check.py undo cycle | tee $RUN/check.txt` and `app --script scripts/undo.json --video $RUN/undo.mp4 --fps 30 --start mark:letgo-2 --duration 5 --keys`.

## Gotchas

- The next PR renames this to Skip. Afterwards the copy, the ring label, the phase line and the control's text change. Update this file and `tools/check.py` in that PR.
- `time` reads `under a minute` while VoiceOver says `0 seconds` during the let-go ring (seen at `undo.json` mark `letgo`). Assert each one separately.
- The undo button disappears after nine seconds of window clock. A mark more than nine seconds after `S` sees no `undo`.
- `control-skip` exists only while the controls show. Move the pointer over the window or press Tab first, or the tap lands on an inert control and does nothing.
- During the bell's ring after a finished block there is no `control-skip` at all.
