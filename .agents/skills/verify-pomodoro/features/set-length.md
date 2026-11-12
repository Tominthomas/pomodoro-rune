# Set the length

While a block or a break waits, the user sets its length in one-minute steps by dragging the sun up or down, with the arrow keys, or with the − and + buttons beside the time. The sun waits at the top for every length. While held it glides under the pointer, the time snaps to whole minutes, and once let go it eases back to the top. Letting go only sets the length, and Space or a tap starts it. On a first launch a hint offers both ways until one is used or put away. A waiting break's length is set the same way. While a break runs, a drag sets the focus block after it.

## Sub-features

- `length-arrows` makes a block one minute longer with Up or Right and one minute shorter with Down or Left.
- `length-steppers` makes it a minute shorter or longer with the `shorter` and `longer` buttons, which sit beside the time while the clock waits. They are always in the accessibility tree and drawn once the pointer moves or Tab is pressed.
- `length-drag` raises or lowers the sun to set the length. The phase line reads `release to set the length` while dragging.
- `length-release` sets the length on release and starts nothing.
- `length-hint` shows `Drag the sun to set the length` with a `got it` button on a first launch, under the phase line `The sun sets as you focus.` then, on its own line, `A finished block lights a lantern.`.
- `length-break` sets a waiting break's length with the arrows or a drag.
- `length-next` sets the next focus block's length with a drag while a break runs or is paused. The time reads the next block's minutes with `next focus` beside it while held, and the break's own time again once let go.

## How to get to it (user POV)

- Press the arrow keys while the phase line ends in `space to begin` or `break ready`.
- Move the pointer, then click − or + beside the time.
- Press on the sun and drag up or down, then release.
- Click `got it` (key `hint-dismiss`) to put the hint away.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`, a first launch with a 25-minute block waiting. Use `--fps 60` for `drag.json`.

- **Hint.** Run `look scripts/drag.json --at 0.1 hint time`. `hint` is drawn and `time` reads `25:00`.
- **Drag.** Press on the sun at `186,54` and drag down to `186,85`. Run `look scripts/drag.json --at mark:held phase time sun --out $RUN/held`. `phase` reads `release to set the length`, `time` reads `16 min`, and `sun` is 31 points below the top (`rect` y 49). Then release, and run `look scripts/drag.json --at mark:released phase time hint sun --out $RUN/released`. `phase` reads `focus · space to begin`, `time` reads `16:00`, `hint` is absent, and `sun` is back at the top (`rect` y 18).
- **Start after drag.** Press Space. Run `look scripts/drag.json --at mark:started phase`. `phase` reads `focus`.
- **Arrows.** Press Down 24 times. Run `look scripts/cycle.json --at mark:wound --fps 10 time`. `time` reads `01:00`.
- **Steppers.** Save this as `$RUN/stepper.json` and run `look $RUN/stepper.json --at mark:stepped time shorter longer`. After −, − and +, `time` reads `24:00`, and `shorter` and `longer` are labelled `A minute shorter` and `A minute longer`.

  ```json
  {"events":[{"t":0.2,"type":"move","x":200,"y":300},{"t":0.4,"type":"tap","key":"shorter"},{"t":0.6,"type":"tap","key":"shorter"},{"t":0.8,"type":"tap","key":"longer"},{"t":1.2,"type":"mark","name":"stepped"}]}
  ```

- **Break length.** Press Down 14 times while the long break waits. Run `look scripts/cycle.json --at mark:long-set --fps 10 time`. `time` reads `01:00`.
- **Break by drag.** Save this as `$RUN/drag-waiting-break.json`, then run `look $RUN/drag-waiting-break.json --scene bell --at mark:break phase time` and the same at `mark:dragged`. `phase` ends in `5-minute break ready`, then in `1-minute break ready`, and `time` goes from `05:00` to `01:00`.

  ```json
  {"events":[{"t":8.0,"type":"mark","name":"break"},{"t":9.5,"type":"down","x":186,"y":140},{"t":9.6,"type":"move","x":186,"y":170},{"t":9.7,"type":"move","x":186,"y":250},{"t":9.9,"type":"up","x":186,"y":250},{"t":11.0,"type":"mark","name":"dragged"}]}
  ```

- **Next block during a break.** Run `look scripts/drag-break.json --scene short --fps 60 --at mark:held time time-unit` and the same at `mark:released` and `mark:next`. While held, `time` reads `48 min` beside `next focus`. Once let go, `phase` reads `rest` and the break's time runs on. After S skips the break, `time` reads `48:00`.
- **Arrows ignored while running.** Run `cargo test --release the_arrows_set_the_length_only_while_the_block_waits`. It passes.
- **Arrows ignored over Today and the keys card.** Run `cargo test --release the_arrows_leave_the_length_alone_while_today_or_the_keys_are_open`. It passes.
- **Proof.** Run `python3 tools/check.py drag drag-break cycle | tee $RUN/check.txt` and `app --script scripts/drag.json --video $RUN/drag.mp4 --fps 30 --pointer`.

## Gotchas

- A drag during a focus block that has started, running or paused, does nothing. During a waiting break it sets the break. During a running or paused break it sets the next focus block, and the sun stays below the sea: only the time changes.
- A press that moves less than 4 points is a tap, which starts the block instead of setting it.
- The sun waits at the top, `186,54` at the first size, whatever the length. The drag works anywhere on the sky; pressing on the sun is what a person does.
- Space or a tap while the sun is held does nothing until it is let go.
- The click that brings the window forward never starts a drag. A script with `{"focus":false}` first needs a second press.
- `shorter` and `longer` are absent until the pointer moves or Tab is pressed. A tap on them before that finds nothing.
- The hint shows only on a first launch with an empty log. The `today` scene and every scene with blocks never show it.
