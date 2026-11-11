# Set the length

While a block or a break waits, the user sets its length in one-minute steps by dragging the sun up or down, with the arrow keys, or with the − and + buttons beside the time. Letting go of the sun only sets the length, and Space or a tap starts it. On a first launch a hint offers both ways until one is used or put away. A waiting break's length is set the same way.

## Sub-features

- `length-arrows` makes a block one minute longer with Up or Right and one minute shorter with Down or Left.
- `length-steppers` makes it a minute shorter or longer with the `shorter` and `longer` buttons, which show beside the time while the clock waits and the controls show.
- `length-drag` raises or lowers the sun to set the length. The phase line reads `release to set the length` while dragging.
- `length-release` sets the length on release and starts nothing.
- `length-hint` shows `drag the sun or press ↑ ↓ to set the length` with a `got it` button on a first launch.
- `length-break` sets a waiting break's length with the arrows or a drag.

## How to get to it (user POV)

- Press the arrow keys while the phase line ends in `space to begin` or `space when you're ready`.
- Move the pointer, then click − or + beside the time.
- Press on the sun and drag up or down, then let go.
- Click `got it` (key `hint-dismiss`) to put the hint away.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`, a first launch with a 25-minute block waiting. Use `--fps 60` for `drag.json`.

- **Hint.** Run `look scripts/drag.json --at 0.1 hint time`. `hint` is drawn and `time` reads `25:00`.
- **Drag.** Press on the sun at `186,140` and drag down to `186,205`, then let go. Run `look scripts/drag.json --at mark:released phase time hint --out $RUN/released`. `phase` reads `focus · space to begin`, `time` reads whole minutes under `25:00`, and `hint` is absent.
- **Start after drag.** Press Space. Run `look scripts/drag.json --at mark:started phase`. `phase` reads `focus`.
- **Arrows.** Press Down 24 times. Run `look scripts/cycle.json --at mark:wound --fps 10 time`. `time` reads `01:00`.
- **Steppers.** Save this as `$RUN/stepper.json` and run `look $RUN/stepper.json --at mark:stepped time shorter longer`. After −, − and +, `time` reads `24:00`, and `shorter` and `longer` are labelled `A minute shorter` and `A minute longer`.

  ```json
  {"events":[{"t":0.2,"type":"move","x":200,"y":300},{"t":0.4,"type":"tap","key":"shorter"},{"t":0.6,"type":"tap","key":"shorter"},{"t":0.8,"type":"tap","key":"longer"},{"t":1.2,"type":"mark","name":"stepped"}]}
  ```

- **Break length.** Press Down 14 times while the long break waits. Run `look scripts/cycle.json --at mark:long-set --fps 10 time`. `time` reads `01:00`.
- **Break by drag.** Save this as `$RUN/drag-break.json`, then run `look $RUN/drag-break.json --scene bell --at mark:break phase time` and the same at `mark:dragged`. `phase` reads `rest · space when you're ready` both times, and `time` goes from `05:00` to `01:00`.

  ```json
  {"events":[{"t":8.0,"type":"mark","name":"break"},{"t":9.5,"type":"down","x":186,"y":140},{"t":9.6,"type":"move","x":186,"y":170},{"t":9.7,"type":"move","x":186,"y":250},{"t":9.9,"type":"up","x":186,"y":250},{"t":11.0,"type":"mark","name":"dragged"}]}
  ```

- **Arrows ignored while running.** Run `cargo test --release the_arrows_set_the_length_only_while_the_block_waits`. It passes.
- **Proof.** Run `python3 tools/check.py drag cycle | tee $RUN/check.txt` and `app --script scripts/drag.json --video $RUN/drag.mp4 --fps 30 --pointer`.

## Gotchas

- The sun moves only while the clock waits. A drag during a running block or a pause does nothing. During a waiting break it sets the break, not the next block. Two queued PRs change this. One will start the sun at the top whatever the length, and one will make a drag during rest set the next block. Update this file in each.
- A press that moves less than 4 points is a tap, which starts the block instead of setting it.
- The sun sits lower for shorter blocks, so drag coordinates depend on the length the block starts at. `186,140` is the sun at 25 minutes.
- The click that brings the window forward never starts a drag. A script with `{"focus":false}` first needs a second press.
- `shorter` and `longer` are absent until the pointer moves or Tab is pressed. A tap on them before that finds nothing.
- The hint shows only on a first launch with an empty log. The `today` scene and every scene with blocks never show it.
