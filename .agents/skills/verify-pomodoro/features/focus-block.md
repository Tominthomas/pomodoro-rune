# Focus block

A focus block is one sunset. The user can name it, then starts it, and the sun sinks as the time runs out. At 00:00 the sun touches the sea, ripples spread and a lantern lights on the water. The app then asks how the block felt and a short break waits.

## Sub-features

- `focus-start` starts the clock from a tap on the sky, Space, or the play control.
- `focus-label` names the next block with `L`. The name carries over to later blocks and heads the break.
- `focus-sink` keeps the sun above the sea until 00:00, when it touches the water.
- `focus-bell` lights a lantern at the bell, with the phase line "1 of 4 lanterns · 5-minute break ready": the lanterns so far in the cycle of four and the break they earned.
- `focus-reflect` asks how the block felt. `1`, `2` or `3` answers it and is acknowledged. Left alone, the question fades 30 seconds into the break.

## How to get to it (user POV)

- Tap the sky, press Space, or click `Start focus` (`control-play`) while the phase line reads `focus · space to begin`. On a first launch it reads `The sun sets as you focus. A finished block lights a lantern.`.
- Move the pointer or press Tab, then click the play control (`control-play`, labelled `Start`).
- Press `L`, type a name and press Enter to name the block before starting.
- After the bell, press `1` (deep), `2` (steady) or `3` (drifting), or click a word under the question.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. Each script below starts from a first launch, except `bell`, which starts from the `last-minute` scene.

- **Start by tap.** Tap the sky. Run `look scripts/start.json --at mark:running time phase --out $RUN/running`. `time` reads `24:58`, says `24 minutes 58 seconds`, and `phase` reads `focus`.
- **Name and wind.** Press `L`, type `Reading & notes`, press Enter, then press Down 24 times. Run `look scripts/cycle.json --at mark:wound --fps 10 top-text time phase --out $RUN/wound`. `top-text` reads `Reading & notes`, `time` reads `01:00` and `phase` reads `focus · space to begin`.
- **Last second.** Run `look scripts/cycle.json --at mark:b1-last --fps 10 time`. `time` reads `00:01`.
- **Bell.** Run `look scripts/cycle.json --at mark:b1-bell --fps 10 phase time 'lantern-*' reflect-question 'felt-*' --out $RUN/bell`. `phase` reads `1 of 4 lanterns · 5-minute break ready`, `time` reads `1 min`, only `lantern-0` exists, `reflect-question` reads `How did that block feel? Optional.`, and `felt-deep`, `felt-steady` and `felt-drifting` are drawn.
- **Answer.** Press `1`. Run `look scripts/cycle.json --at mark:b1-rated --fps 10 reflect-question felt-deep`. `reflect-question` reads `Felt deep. Noted.` and `felt-deep` is absent.
- **Break.** Run `look scripts/cycle.json --at mark:b1-break --fps 10 phase top-text`. `phase` reads `1 of 4 lanterns · 5-minute break ready` and `top-text` reads `after Reading & notes`.
- **Fade unanswered.** Run `look scripts/reflect.json --at mark:unanswered --fps 10 reflect`, then the same at `mark:unanswered-late`. `reflect` is drawn first and absent later. At `mark:today`, `reflected` reads `you said how 1 of 2 felt`.
- **Sun meets the sea.** Run `look scripts/bell.json --scene last-minute --at mark:half sun` and the same at `mark:zero`. The sun's bottom edge (`rect` y plus height) is under 329 at `half` and within 0.6 of 330 at `zero`.
- **Proof.** Run `python3 tools/check.py start cycle reflect bell | tee $RUN/check.txt` and `app --scene last-minute --script scripts/bell.json --video $RUN/bell.mp4 --fps 30`. Every line passes, and the video shows the sun meeting the water and the lantern lighting.

## Gotchas

- `cycle.json` is five minutes of virtual time. At `--fps 60` it takes far longer to render and gives the same answers, so use `--fps 10`.
- `top-text` is always in the tree. Before a block is named it reads `+ name this block`, drawn while the block waits and at ink 0 once it runs. Check `ink`, not presence.
- `1`, `2` and `3` only rate while the question is open. Pressed at other times they do nothing, and a test that presses them early still passes on presence alone.
- The bell is silent by design. There is no sound to verify.
- A notification at the bell is only posted when the window isn't in front and the app runs as a bundle. Headless can't show it. See SKILL.md's "Real window".
