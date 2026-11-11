//! The controls, bottom right: start or pause, let go (or skip a break), Today, the place and
//! the keys. They come out while the pointer moves or after Tab, each with a tooltip naming its
//! key, and stay while the pointer rests on their corner. Hidden, only their look goes: they
//! take no clicks and no focus, their corner still keeps clicks from the sky, and `VoiceOver`
//! still finds and presses them. `?` opens every key at once.

use std::f32::consts::TAU;

use rune::shape::{LineCap, Shape};
use rune::tok::TextStyle;
use rune::{El, PopupMenu, Tx, canvas, col, el, row, text};

use crate::drawing::ring;
use crate::readout::{BRIGHT, QUIET, SMALL, button, line, pill};
use crate::theme::{Ink, PLACES, Place, rgb};
use crate::timer::{Clock, Event, Phase};
use crate::{CONTROLS_LINGER, State, carry, guarded, open_today, set_place};

/// The corner the controls live in, measured from the window's bottom right.
const DOCK: (f32, f32) = (236.0, 56.0);

/// Runs a control's action, unless it is the click that brought the window forward. A click
/// leaves no focus behind, so Space goes back to the timer.
fn act(f: impl Fn(&mut State, &mut Tx) + 'static) -> impl Fn(&mut State, &mut Tx) + 'static {
    guarded(move |s, tx| {
        f(s, tx);
        if !tx.focus_visible() {
            tx.blur();
        }
    })
}

/// A control in the dock: the button the keyboard and assistive technology reach, around `look`,
/// which draws it only while the dock is out. The button itself never fades, since a faded
/// element leaves the accessibility tree.
fn docked(k: Ink, key: &str, label: &str, keys: &str, radius: f32, shown: bool, look: El) -> El {
    el().key(key.to_string())
        .radius(radius)
        .role("button")
        .label(label.to_string())
        .help(format!("{label}  {keys}"))
        .shortcut_hint(keys.to_string())
        .focusable(shown)
        .focus_ring(1.5, 1.5, k.text.alpha(0.95))
        .child(look.key(format!("{key}-look")).opacity(if shown { 1.0 } else { 0.0 }))
}

fn icon(k: Ink, size: f32, glyph: El) -> El {
    el().size(size, size).radius(size / 2.0).center().fill(k.shade.alpha(0.45)).hover(k.text.alpha(0.14)).child(glyph)
}

pub fn view(s: &State, k: Ink) -> El {
    let shown = s.controls_shown();
    let t = &s.timer;
    let c = k.text.alpha(BRIGHT);
    let running = matches!(t.clock, Clock::Ticking { .. });
    // Play is the primary control: larger, and drawn in the ink itself.
    let on = k.shade;
    let play = if running {
        Shape::new().line([5.0, 3.0], [5.0, 13.0], 2.4, LineCap::Round, on).line(
            [11.0, 3.0],
            [11.0, 13.0],
            2.4,
            LineCap::Round,
            on,
        )
    } else {
        Shape::new().fill(vec![[4.5, 2.0], [14.0, 8.0], [4.5, 14.0]], on)
    };
    let play_label = match t.clock {
        Clock::Ticking { .. } => "Pause",
        Clock::Paused { .. } => "Resume",
        _ => "Start",
    };
    let look = el()
        .size(38.0, 38.0)
        .radius(19.0)
        .center()
        .fill(k.text.alpha(0.92))
        .hover(k.shade.alpha(0.12))
        .child(canvas(play).size(16.0, 16.0));
    let play = docked(k, "control-play", play_label, "Space", 19.0, shown, look)
        .on_press_tx(act(|s, tx| carry(s, Event::Press { now: tx.clock() }, tx)));
    let mut r = row().key("controls").gap(6.0).center_y();
    let started = matches!(t.clock, Clock::Ticking { .. } | Clock::Paused { .. });
    if t.phase == Phase::Focus && started {
        // Letting go is a word, not a glyph: it's the one action that ends a block.
        r = r.child(
            docked(k, "control-skip", "Let go of this block", "S", 12.0, shown, pill(k, "let go"))
                .on_press_tx(act(|s, tx| carry(s, Event::Skip { now: tx.clock() }, tx))),
        );
    } else if t.phase != Phase::Focus && !matches!(t.clock, Clock::Ringing { .. }) {
        let glyph = Shape::new().fill(vec![[2.5, 3.0], [8.5, 7.0], [2.5, 11.0]], c).line(
            [11.0, 3.0],
            [11.0, 11.0],
            1.6,
            LineCap::Round,
            c,
        );
        r = r.child(
            docked(k, "control-skip", "Skip break", "S", 15.0, shown, icon(k, 30.0, canvas(glyph).size(14.0, 14.0)))
                .on_press_tx(act(|s, tx| carry(s, Event::Skip { now: tx.clock() }, tx))),
        );
    }
    r = r.child(play);
    // Today: the day's lanterns as a small bar chart.
    let today = Shape::new()
        .line([1.5, 12.5], [12.5, 12.5], 1.2, LineCap::Round, c)
        .line([3.5, 10.0], [3.5, 7.0], 2.2, LineCap::Round, c)
        .line([7.0, 10.0], [7.0, 3.0], 2.2, LineCap::Round, c)
        .line([10.5, 10.0], [10.5, 5.5], 2.2, LineCap::Round, c);
    // A place: hills under a low sun.
    let place = Shape::new()
        .fill(vec![[0.5, 12.5], [5.0, 5.0], [8.0, 9.0], [10.0, 7.0], [13.5, 12.5]], c)
        .fill(ring([10.5, 3.2], [1.8, 1.8], 12, 0.0, TAU), c);
    let place_label = format!("Place: {}", Place::at(s.place).name);
    let keys = text("?").font(TextStyle::new(13.0, 620.0)).color(c);
    let r = r
        .child(
            docked(k, "control-today", "Today", "H", 15.0, shown, icon(k, 30.0, canvas(today).size(14.0, 14.0)))
                .on_press_tx(act(|s, _| open_today(s, true))),
        )
        .child(
            docked(k, "control-place", &place_label, "T", 15.0, shown, icon(k, 30.0, canvas(place).size(14.0, 14.0)))
                .on_press_tx(act(pick_place)),
        )
        .child(
            docked(k, "control-keys", "Keys", "?", 15.0, shown, icon(k, 30.0, keys))
                .on_press_tx(act(|s, _| s.keys_open = true)),
        );
    // The corner itself takes the pointer: resting there keeps the controls out, and a click
    // aimed at a fading control never reaches the sky behind it. While they are hidden a cover
    // over them takes the click instead.
    let dock = el()
        .key("dock")
        .role("none")
        .pin(None, None, Some(0.0), Some(0.0))
        .size(DOCK.0, DOCK.1)
        .on_drag(|_: &mut State, _, _| {})
        .on_hover_change(|s: &mut State, inside, tx: &mut Tx| {
            s.over_controls = inside;
            if !inside {
                s.controls_until = tx.clock() + CONTROLS_LINGER;
                tx.wake_at(s.controls_until);
            }
        })
        .child(r.pin(None, None, Some(14.0), Some(14.0)));
    if shown {
        dock
    } else {
        dock.child(
            el().key("dock-cover")
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .on_drag(|_: &mut State, _, _| {})
                .a11y_hidden(),
        )
    }
}

/// The places by name, in a menu from the place control.
fn pick_place(s: &mut State, tx: &mut Tx) {
    let names = PLACES.iter().map(|p| (p.name, p.name));
    tx.popup_menu(PopupMenu::picker(names, Place::at(s.place).name), "control-place", |s: &mut State, picked, tx| {
        if let Some(i) = picked.and_then(|name| PLACES.iter().position(|p| p.name == name)) {
            set_place(s, i, tx);
        }
    });
}

const KEYS: [(&str, &str); 12] = [
    ("Space", "start, pause or resume"),
    ("↑  ↓", "a minute longer or shorter, while it waits"),
    ("drag", "the sun up or down to set the length; space starts it"),
    ("S", "let go of a block, or skip a break"),
    ("U", "undo a let go, until you start again"),
    ("L", "name the next block"),
    ("1  2  3", "how it felt: deep, steady, drifting"),
    ("H", "Today and the last four weeks"),
    ("T", "the next place"),
    ("Tab", "show the controls"),
    ("?", "these keys"),
    ("Esc", "close"),
];

/// Every key on one card, over a dimmed scene.
pub fn keys() -> El {
    // Each row wraps its description and grows, so nothing runs past the card.
    let rows = KEYS.iter().enumerate().fold(col().gap(8.0), |c, (i, (k, what))| {
        c.child(
            row()
                .key(format!("key-row-{i}"))
                .gap(12.0)
                .items_start()
                .child(el().w(58.0).child(line(Ink::light(), *k, SMALL, 600.0, BRIGHT)))
                .child(
                    text(what.to_string())
                        .key(format!("key-what-{i}"))
                        .font(TextStyle::new(SMALL, 450.0))
                        .color(Ink::light().text.alpha(QUIET))
                        .wrap()
                        .w(214.0),
                ),
        )
    });
    let card = col()
        .key("keys")
        .w(324.0)
        .padding(20.0)
        .radius(16.0)
        .fill(rgb(0x0d1128).alpha(0.95))
        .gap(14.0)
        .role("dialog")
        .label("Keys")
        .child(row().justify_between().center_y().child(line(Ink::light(), "Keys", 15.0, 520.0, BRIGHT)).child(
            button(Ink::light(), "keys-close", "Close", "Close  Esc").on_press_tx(guarded(|s, _| s.keys_open = false)),
        ))
        .child(rows);
    el().key("keys-veil")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(rgb(0x05071a).alpha(0.5))
        .center()
        .modal(true, |s: &mut State| s.keys_open = false)
        .child(card)
}
