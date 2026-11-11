# Places

The same sunset plays over three places, Coast, Lake and Mesa. `T` steps to the next one, and the place control opens a menu of all three. The choice is saved. Light and dark follow the system appearance.

## Sub-features

- `place-key` steps Coast, Lake, Mesa, then back to Coast with `T`.
- `place-menu` picks a place from the `control-place` menu.
- `place-theme` draws each place in light and dark.

## How to get to it (user POV)

- Press `T`.
- Show the controls, click the place control (labelled `Place: <name>`) and pick a name.
- Switch the system between light and dark appearance.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`, starting at Coast. Save this script as `$RUN/place.json`.

  ```json
  {"events":[{"t":0.2,"type":"key","name":"t"},{"t":0.3,"type":"move","x":300,"y":515},{"t":1.4,"type":"mark","name":"lake"},{"t":1.5,"type":"tap","key":"control-place"},{"t":1.8,"type":"menu-pick","id":"Mesa"},{"t":3.0,"type":"mark","name":"mesa"},{"t":3.5,"type":"mark","name":"end"}]}
  ```

- **Key.** Press `T`. Run `look $RUN/place.json --at mark:lake control-place --out $RUN/lake`. `control-place` is labelled `Place: Lake` and the PNG shows pines and mountains.
- **Menu.** Show the controls, click `control-place` and pick `Mesa`. Run `look $RUN/place.json --at mark:mesa control-place --out $RUN/mesa`. `control-place` is labelled `Place: Mesa` and the PNG shows red rock buttes.
- **Every place, light and dark.** Run `tools/shots.sh`. `shots/review/places-<scene>.png` shows the three places side by side, light over dark, for every scene and Today.
- **Proof.** Run `app --script $RUN/place.json --video $RUN/places.mp4 --fps 30 --keys --pointer`. The video shows the scene changing on `T` and on the menu pick.

## Gotchas

- `control-place` stays in the tree at ink 0 when the controls are hidden, so its label is readable before the pointer moves. A tap on it does nothing until the controls show.
- Headless `menu-pick` proves the choice lands. It doesn't show the menu macOS draws, which only the real window shows.
- The saved place reaches `prefs.json` only in a real launch. No test in `src/tests.rs` relaunches into a saved place, so a change to saving it needs a new test or the real-window checklist.
- The colour change on switching eases over `rune::tok::motion::CALM`. Snap a second or more after the switch to see the settled place.
- `--theme light|dark` fixes the appearance for the whole run. A live switch is real-window only.
