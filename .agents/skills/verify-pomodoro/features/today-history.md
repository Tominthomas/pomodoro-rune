# Today and history

`H` opens Today. It has a timeline of the day's lanterns and rings, the minutes focused, the lanterns lit, what the blocks were for, how they felt, and a grid of the last four weeks. A lantern or ring opens its block, where its feeling can be given or changed and its name changed. A day in the grid opens that day.

## Sub-features

- `today-open` opens Today with `H` or the `control-today` control (labelled Today) and closes it with `H`, `Esc` or `today-close` (Back to sunset). The `timer-bar` at the top shows the running timer and takes Pause, Resume, Start and Undo let go.
- `today-numbers` shows `stat-focused-value`, `stat-lanterns-value` and `stat-let-go-value`, the `day-lantern-N` and `day-ring-N` marks, `for-<label>` and `reflected`.
- `today-detail` opens a block from its mark into `detail`, with `detail-when` and `detail-about`.
- `today-felt` gives, changes or clears a block's feeling with `detail-felt-deep`, `detail-felt-steady` and `detail-felt-drifting`.
- `today-rename` renames a block with `detail-rename` and `detail-label-field`.
- `today-history` opens a past day from the grid (`day-<day number>`) and returns with `back-to-today`.
- `today-escape` closes the innermost thing first. That order is the rename field, then the block, then the past day, then Today.

## How to get to it (user POV)

- Press `H` or click the Today control.
- Click a lantern or ring on the timeline, then a word or `rename` / `name it`.
- Click a day in the four-week grid, then `Today` to come back.
- Press `Esc` to back out one step at a time.

## Driving it with the headless renderer

Preconditions:

- `today.json` starts from the `today` scene, with five blocks this morning and four weeks of history. Run it at `--fps 60`. Yesterday's grid key is `day-20767`.

- **Stray click.** Tap the header at `200,40`. Run `look scripts/today.json --scene today --at mark:clicked today-title`. `today-title` reads `Today`.
- **Open a block.** Tap `day-lantern-0`. Run `look scripts/today.json --scene today --at mark:detail detail-about detail-when --out $RUN/detail`. `detail-about` contains `Reading & notes` and `felt deep`.
- **Give a feeling.** Tap `day-lantern-1`, then `detail-felt-steady`. Run `look scripts/today.json --scene today --at mark:felt-given detail-about day-lantern-1 --out $RUN/felt`. `detail-about` and the label on `day-lantern-1` contain `felt steady`.
- **Clear it.** Tap `detail-felt-steady` again. At `mark:felt-cleared`, `detail-about` contains `feeling not given`.
- **Past day.** Tap `day-20767`. Run `look scripts/today.json --scene today --at mark:yesterday today-title back-to-today now --out $RUN/yesterday`. `today-title` reads `Tuesday`, `back-to-today` is drawn and `now` is absent. At `mark:back`, `today-title` reads `Today` and `now` is drawn.
- **Rename.** No committed script renames. Save this as `$RUN/rename.json`, then run `look $RUN/rename.json --scene today --at mark:renamed detail-about day-lantern-0 'for-*' --out $RUN/renamed`. `detail-about` starts with `Essay draft`, the label on `day-lantern-0` contains `Essay draft` and `for-Essay draft` exists.

  ```json
  {"events":[{"t":0.5,"type":"tap","key":"day-lantern-0"},{"t":1.2,"type":"tap","key":"detail-rename"},{"t":1.5,"type":"key","name":"a","cmd":true},{"t":1.6,"type":"type","text":"Essay draft"},{"t":2.4,"type":"key","name":"Enter"},{"t":3.0,"type":"mark","name":"renamed"}]}
  ```

- **A whole day's numbers.** Run `look scripts/cycle.json --at mark:today --fps 10 'stat-*' 'day-lantern-*' 'day-ring-*' reflected 'for-*'`. `stat-lanterns-value` reads `4`, `stat-let-go-value` reads `1`, `stat-focused-value` reads `5 min`, `reflected` reads `you said how 1 of 4 felt` and `for-Reading & notes` exists.
- **Escape order and relaunch.** Run `cargo test --release today_opens_a_block_and_a_past_day_and_escape_closes_the_innermost_first` and `cargo test --release l_names_the_block_and_today_knows_what_it_was_for`. Both pass.
- **Proof.** Run `python3 tools/check.py today cycle | tee $RUN/check.txt` and `app --scene today --script scripts/today.json --video $RUN/today.mp4 --fps 30 --pointer`.

## Gotchas

- `day-lantern-N` counts lit lanterns and `day-ring-N` counts blocks let go, each from 0 in time order. They aren't block serials.
- The day number in `day-<n>` is days since 1970 for a day that starts at 4 a.m. Headless runs are fixed at UTC, so today is `day-20768`.
- The rename field holds the old name when it opens. Select all with ⌘A before typing, or the new text is appended.
- Under the held top (`timer-bar` and the title), Today scrolls in `today-scroll`. History sits below the fold, so scroll first (a `{"type":"wheel","x":190,"y":400,"dy":-2000}` event) before tapping a day tile by key. The `busy` scene has 19 blocks in one day, and the strip (`day-strip-view`) scrolls sideways.
- Today's background takes no clicks. A tap off any control should leave Today open, and the `clicked` mark proves that.
