# Menus

The menu bar has a Timer menu, a View menu and a Help menu next to the standard ones. Each item does what the dock control or key of the same name does, and its title follows the state: Timer's first item reads `Start Focus`, `Pause`, `Resume` or `Start Break`. An item that would do nothing, such as Skip while a block waits, is dimmed. Today and the place shown are ticked.

## Sub-features

- `menu-timer` lists the primary action (Space), `Skip` (S) and `Undo Skip` (⌘Z).
- `menu-view` lists `Today` (H), one item per place, `Next Place` (T), `Larger Text` (⌘+) and `Always Show Time`, a ticked toggle with no key.
- `menu-help` lists `Keys` (/).
- `menu-state` dims Skip and Undo Skip when they don't apply, and ticks Today, the current place, Larger Text and Always Show Time when they are on.

## How to get to it (user POV)

- Open the Timer, View or Help menu in the menu bar and pick an item.
- Type an item's key. The window takes the key first, so a letter typed into a name field stays a letter.

## Driving it with the headless renderer

Preconditions:

- The baseline from `README.md`. A script fires an item by id or by exact title with `{"t":0.3,"menu":"timer-press"}`. The ids are in `src/menu.rs`.

- **Every item.** Run `python3 tools/check.py menus`. `scripts/menus.json` starts, skips, undoes, opens Today, picks Mesa, steps to the next place and opens the keys, all from the menus.
- **One step.** Run `look scripts/menus.json --at mark:skipped --fps 30 phase undo`. `phase` reads `Skipped before a minute. No lantern.` and `undo` is drawn.
- **Dimmed items.** The Runtime test `the_menus_name_what_each_key_does_now_and_dim_what_would_do_nothing` asserts every title, dimmed and ticked state. Run it with `cargo test --release the_menus`.

## Gotchas

- Firing a dimmed item fails the whole headless run with `script menu: menu item is disabled: timer-skip`. Fire `timer-skip` only while a block or a break runs or waits.
- The menu bar as macOS draws it, and the bare-key equivalents shown beside each title, are real-window checks (SKILL.md, "Real window").
