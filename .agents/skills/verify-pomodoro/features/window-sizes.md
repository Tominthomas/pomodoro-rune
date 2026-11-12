# Window sizes and Larger Text

The window resizes from 380 x 540 points up. The sky and the sea fill it, the horizon stays at the same share of the height, and nothing in the scene grows: a bigger window shows more evening. The readout keeps the bottom left and the dock the bottom right. Today centres one column up to 460 points wide, and sets the four weeks beside the day once two 340-point columns fit. Larger Text (View menu, `⌘+`, and `⌘−` to go back) zooms the words and controls 1.25 times and draws the scene back at the window's own scale. It grows a window too small for it to 475 x 675. In a narrow view (under 372 points, such as Larger Text in the smallest window) the dock takes two rows, the readout moves up, the key card scrolls and Today's rows wrap.

## Sub-features

- `size-scene` fills any size with sky and sea, the horizon at 11/18 of the height.
- `size-corners` keeps the readout's time 184 points above the bottom left, and the dock 14 points in from the bottom right.
- `size-today` sets Today in one column, or two when wide.
- `larger-text` zooms words and controls, keeps the scene, is saved, and is ticked in the View menu.
- `larger-grow` grows a window too small for Larger Text to 475 x 675.
- `narrow` puts the dock in two rows with Start in its corner, and scrolls the key card.

## How to get to it (user POV)

- Drag the window's corner. The size is remembered at the next launch.
- Press `⌘+` or pick View > Larger Text. Press `⌘−` to go back.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. `--size WxH` launches at that size; a script's `{"type":"resize","dw":-95,"dh":-135,"dur":0.05}` resizes the window as a person would. A `--snap` keeps the launch size, so make a window that grew small again before snapping it.

- **Every size.** Run `python3 tools/check.py size-tall size-wide size-larger size-larger-small`. Each script waits, starts a block with the controls out, opens the keys, opens Today and scrolls to the four weeks. The checks assert nothing is cut or drawn over another, Pause takes Start's place, and the four weeks are reachable.
- **One moment.** Run `app --script scripts/size-wide.json --size 900x600 --snap $RUN/wide-today.png --at mark:today` and look at it: the four weeks stand beside the day.
- **Larger Text grows the window.** `python3 tools/check.py size-larger-small` checks the `grown` mark: the view is 380 x 540 of its points, so the window is 475 x 675.
- **Contrast.** `python3 tools/contrast.py` measures five moments at each size in every place, light and dark, besides the first-size pack.
- **Runtime tests.** `cargo test --release window::` checks the corners at three sizes, Today's columns, Larger Text's zoom and unchanged sun, and the narrow dock.

## Gotchas

- A tree's rects and `viewport` are in the view's points. With Larger Text they are the window's points divided by 1.25; a render is in the window's.
- Larger Text's grow is animated on the virtual clock. A resize event before it has finished is overridden by it; resize at 0.6 s or later.
- Live resizing with the pointer, and the frame macOS remembers, are real-window checks (SKILL.md, "Real window").
