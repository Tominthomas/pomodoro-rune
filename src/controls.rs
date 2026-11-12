//! The controls, bottom right: start or pause, skip, Today, the place and
//! the keys. While a block or a break waits they stay out, each saying what it does, with
//! "Start focus" first. Once it runs they recede: they come out while the pointer moves or after
//! Tab, and stay while the pointer rests on their corner. Hidden, only their look goes: they take
//! no clicks and no focus, their corner still keeps clicks from the sky, and `VoiceOver` still
//! finds and presses them. Each has a tooltip naming its key; `?` opens every key at once.

use rune::shape::{LineCap, Shape};
use rune::tok::TextStyle;
use rune::{El, PopupMenu, Tx, canvas, col, el, row, text};

use crate::readout::{BRIGHT, QUIET, SMALL, button, line};
use crate::theme::{Ink, PLACES, Place, rgb};
use crate::timer::{Clock, Event, Phase, Timer};
use crate::{CONTROLS_LINGER, State, carry, guarded, open_today, set_place};

/// How far the controls sit in from the window's bottom right; their corner reaches that far
/// around them.
const MARGIN: f32 = 14.0;
/// How tall the dock's quieter controls stand.
const TALL: f32 = 28.0;

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
        .child(look.key(format!("{key}-look")).a11y_hidden().animated_changes().opacity(if shown { 1.0 } else { 0.0 }))
}

/// A quiet control's look: its words on a soft pill.
fn tab(k: Ink, words: &str) -> El {
    row()
        .h(TALL)
        .px(11.0)
        .gap(5.0)
        .center_y()
        .radius(TALL / 2.0)
        .fill(k.shade.alpha(0.5))
        .border(1.0, k.text.alpha(0.42))
        .hover(k.text.alpha(0.12))
        .child(line(k, words, SMALL, 540.0, BRIGHT))
}

/// How wide the primary control stands in every state, so Start, Pause and Resume share one
/// target and no other control ever moves into the place a press just landed.
const PRIMARY_W: f32 = 116.0;

/// What the primary control does now, in its own words. The dock, Today and the menu all say it.
pub fn press_words(t: &Timer) -> &'static str {
    match (&t.clock, t.settled().0) {
        (Clock::Ticking { .. }, _) => "Pause",
        (Clock::Paused { .. }, _) => "Resume",
        (_, Phase::Focus) => "Start focus",
        _ => "Start break",
    }
}

/// What Skip does now, as its name and as the consequence its tooltip gives first, while there
/// is a block or a break to skip. The dock and the menu both offer it.
pub fn skip_words(t: &Timer) -> Option<(&'static str, &'static str)> {
    match (t.phase, &t.clock) {
        (_, Clock::Ringing { .. }) | (Phase::Focus, Clock::Wound | Clock::Scrubbing { .. }) => None,
        (Phase::Focus, _) => Some(("Skip this block", "End this block now: its minutes count, no lantern")),
        _ => Some(("Skip break", "Skip the rest of this break")),
    }
}

/// The primary control's look: what pressing it does, in words beside its glyph, drawn in the ink
/// itself.
fn primary(k: Ink, t: &Timer) -> El {
    let on = Ink { text: k.shade, shade: k.text };
    let glyph = if matches!(t.clock, Clock::Ticking { .. }) {
        Shape::new().line([2.5, 1.5], [2.5, 9.5], 2.2, LineCap::Round, on.text).line(
            [7.5, 1.5],
            [7.5, 9.5],
            2.2,
            LineCap::Round,
            on.text,
        )
    } else {
        Shape::new().fill(vec![[1.0, 0.5], [9.0, 5.5], [1.0, 10.5]], on.text)
    };
    row()
        .w(PRIMARY_W)
        .h(34.0)
        .gap(7.0)
        .center()
        .radius(17.0)
        .fill(k.text.alpha(0.92))
        .hover(k.shade.alpha(0.12))
        .child(canvas(glyph).size(10.0, 11.0))
        .child(line(on, press_words(t), 12.0, 600.0, 1.0))
}

pub fn view(s: &State, k: Ink) -> El {
    let shown = s.controls_shown();
    let t = &s.timer;
    let press = act(|s, tx| carry(s, Event::Press { now: tx.clock() }, tx));
    let play = docked(k, "control-play", press_words(t), "Space", 17.0, shown, primary(k, t));
    // The controls change places at once as the set changes, so no word ever slides over another;
    // only their looks fade.
    let mut r = row().key("controls").gap(6.0).center_y().instant_changes();
    if let Some((label, consequence)) = skip_words(t) {
        // Skip is a word, not a glyph, and sits left of the primary control, outside its place.
        let skip = act(|s, tx| carry(s, Event::Skip { now: tx.clock() }, tx));
        r = r.child(
            docked(k, "control-skip", label, "S", 14.0, shown, tab(k, "Skip"))
                .help(format!("{consequence}  S"))
                .on_press_tx(skip),
        );
    }
    let place = Place::at(s.place).name;
    let chevron = Shape::new().line([1.0, 2.5], [4.0, 5.5], 1.4, LineCap::Round, k.text.alpha(QUIET)).line(
        [4.0, 5.5],
        [7.0, 2.5],
        1.4,
        LineCap::Round,
        k.text.alpha(QUIET),
    );
    let keys = el()
        .size(TALL, TALL)
        .radius(TALL / 2.0)
        .center()
        .fill(k.shade.alpha(0.5))
        .border(1.0, k.text.alpha(0.42))
        .hover(k.text.alpha(0.12))
        .child(text("?").font(TextStyle::new(13.0, 620.0)).color(k.text.alpha(BRIGHT)));
    let r = r
        .child(play.on_press_tx(press))
        .child(
            docked(k, "control-today", "Today", "H", 14.0, shown, tab(k, "Today"))
                .on_press_tx(act(|s, tx| open_today(s, true, tx))),
        )
        .child(
            docked(
                k,
                "control-place",
                &format!("Place: {place}"),
                "T",
                14.0,
                shown,
                tab(k, place).child(canvas(chevron).size(8.0, 8.0)),
            )
            .on_press_tx(act(pick_place)),
        )
        .child(docked(k, "control-keys", "Keys", "?", 14.0, shown, keys).on_press_tx(act(|s, _| s.keys_open = true)));
    // The corner itself takes the pointer: resting there keeps the controls out, and a click
    // aimed at a fading control never reaches the sky behind it. While they are hidden a cover
    // over them takes the click instead.
    let dock = el()
        .key("dock")
        .role("none")
        .pin(None, None, Some(0.0), Some(0.0))
        .padding(MARGIN)
        .on_drag(|_: &mut State, _, _| {})
        .on_hover_change(|s: &mut State, inside, tx: &mut Tx| {
            s.over_controls = inside;
            if !inside {
                s.controls_until = tx.clock() + CONTROLS_LINGER;
                tx.wake_at(s.controls_until);
            }
        })
        .child(r);
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

/// Every key, grouped by what it is for: the timer, the block, and finding your way around.
const KEYS: [(&str, &[(&str, &str)]); 3] = [
    (
        "Timer",
        &[
            ("Space", "start, pause or resume"),
            ("↑  ↓", "a minute longer or shorter, while it waits"),
            ("drag", "the sun up or down to set the length; space starts it"),
            ("S", "skip: end a block early, keeping its minutes but no lantern; or skip a break"),
        ],
    ),
    (
        "Block",
        &[
            ("U", "undo a skip, until you start again"),
            ("L", "name the next block"),
            ("1  2  3", "how it felt: deep, steady, drifting"),
        ],
    ),
    (
        "Explore",
        &[
            ("H", "Today and the last four weeks"),
            ("T", "the next place"),
            ("Tab", "show the controls"),
            ("?  Esc", "these keys, and close what's open"),
        ],
    ),
];

/// Every key on one card, over a dimmed scene, and under them the last bell's words.
pub fn keys(status: Option<El>) -> El {
    // Each row wraps its description and grows, so nothing runs past the card.
    let mut n = 0;
    let groups = KEYS.iter().fold(col().gap(14.0), |c, (group, rows)| {
        let rows = rows.iter().fold(col().gap(7.0), |c, (k, what)| {
            n += 1;
            c.child(
                row()
                    .key(format!("key-row-{n}"))
                    .gap(12.0)
                    .items_start()
                    .child(el().w(58.0).child(line(Ink::light(), *k, SMALL, 600.0, BRIGHT)))
                    .child(
                        text(what.to_string())
                            .key(format!("key-what-{n}"))
                            .font(TextStyle::new(SMALL, 450.0))
                            .color(Ink::light().text.alpha(QUIET))
                            .wrap()
                            .w(214.0),
                    ),
            )
        });
        c.child(
            col()
                .key(format!("keys-{}", group.to_lowercase()))
                .gap(7.0)
                .child(line(Ink::light(), *group, 10.5, 640.0, QUIET).role("heading"))
                .child(rows),
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
        .child(groups);
    let card = match status {
        Some(words) => card.child(words),
        None => card,
    };
    el().key("keys-veil")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(rgb(0x05071a).alpha(0.5))
        .center()
        .modal(true, |s: &mut State| s.keys_open = false)
        .child(card)
}
