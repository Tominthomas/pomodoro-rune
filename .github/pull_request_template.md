<!-- Title: what changes for the person using the app, in plain words. -->

## Why

<!-- The problem, idea or review finding this answers. Link it if it lives somewhere. -->

## What changes

-

## Before and after

<!-- Same place, theme, window size and moment in both. Render them headless with the
verify-pomodoro skill (`look.py --out`) or `tools/shots.sh`, then drag the images in here.
Add rows for other states or sizes. Delete this section if nothing on screen changes. -->

| Before | After |
| --- | --- |
| <img width="320" alt="Before: " src=""> | <img width="320" alt="After: " src=""> |

## How I checked

- [ ] `tools/ci.sh` passes <!-- tests, checks, contrast minimum -->
- [ ] Drove the change the way a user would with verify-pomodoro <!-- script and marks -->
- [ ] Light and dark, every place, the smallest window and Larger Text, if it touches the screen
- [ ] The VoiceOver tree reads right (`look.py` `says`), if it touches controls or text

**Not checked:** <!-- what headless can't show here (live resizing, the menu bar, notifications), or "nothing" -->
