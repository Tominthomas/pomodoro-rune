//! The readout, bottom left, small and still: what the block is for, the time, one quiet line
//! about what the scene is doing, and whatever is on offer right now: setting the length,
//! saying how a block felt, taking back a skip, resuming. It sits on a soft local shade, so
//! its small text reads in every place, light or dark.

use rune::tok::TextStyle;
use rune::{El, Key, KeyEvent, LiveRegion, Transition, Tx, col, el, row, text, text_input};

use crate::log::{self, Felt};
use crate::theme::{H, Ink, W};
use crate::timer::{Clock, Event, Phase, Timer};
use crate::{Asking, State, carry, edit_label, finish_label, guarded, rate};

/// Text on the scene: the main line, and the quieter ones under it. Both keep 4.5:1 on the
/// shade behind the readout.
pub const BRIGHT: f32 = 0.97;
pub const QUIET: f32 = 0.86;
/// The type scale: the time, what the block is for, and everything quieter.
pub const DISPLAY: f32 = 30.0;
pub const LEAD: f32 = 13.0;
pub const SMALL: f32 = 11.5;
/// Where the readout's first line sits.
const TOP: f32 = H - 184.0;
/// How wide a label may run before it's cut with an ellipsis.
const LABEL_W: f32 = 230.0;
/// The smallest a target is under the pointer, whatever it draws.
pub const HIT: f32 = 24.0;

pub fn line(k: Ink, s: impl Into<String>, size: f32, weight: f32, alpha: f32) -> El {
    text(s.into()).font(TextStyle::new(size, weight)).color(k.text.alpha(alpha))
}

/// A small text button on the scene, with a keyboard ring, at least `HIT` tall.
pub fn button(k: Ink, key: &str, label: &str, help: &str) -> El {
    pill(k, label)
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .focusable(true)
        .help(help.to_string())
        .focus_ring(1.5, 1.5, k.text.alpha(0.9))
}

/// How a small text button looks.
pub fn pill(k: Ink, label: &str) -> El {
    row()
        .min_h(HIT)
        .center_y()
        .px(10.0)
        .radius(12.0)
        .fill(k.shade.alpha(0.5))
        .border(1.0, k.text.alpha(0.42))
        .hover(k.text.alpha(0.12))
        .child(line(k, label, SMALL, 540.0, BRIGHT))
}

/// The soft shade the readout sits on, darkest at the window's corner. Dark ink already reads
/// on pale water, so its light shade stays faint: enough to quiet the texture, never a fog.
pub fn scrim(k: Ink) -> El {
    let shade = k.shade;
    let a = if k == Ink::dark() { 0.4 } else { 1.0 };
    el().key("scrim").absolute(0.0, H - 280.0).size(350.0, 280.0).a11y_hidden().radial_gradient(
        [0.05, 0.85],
        [340.0, 225.0],
        [
            (0.0, shade.alpha(0.62 * a)),
            (0.5, shade.alpha(0.5 * a)),
            (0.78, shade.alpha(0.2 * a)),
            (1.0, shade.alpha(0.0)),
        ],
    )
}

pub fn mmss(secs: u32) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// The time left as `VoiceOver` reads it: "24 minutes 58 seconds".
pub fn spoken(secs: u32) -> String {
    let unit = |n: u32, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    match (secs / 60, secs % 60) {
        (0, s) => unit(s, "second"),
        (m, 0) => unit(m, "minute"),
        (m, s) => format!("{} {}", unit(m, "minute"), unit(s, "second")),
    }
}

/// A length of time as `VoiceOver` reads it, rounded as `log::duration` shows it.
fn spoken_minutes(secs: u32) -> String {
    match (secs + 30) / 60 {
        0 => "under a minute".into(),
        m => spoken(m * 60),
    }
}

/// The line under the time: what the scene is doing, and what to do next.
pub fn phase_line(s: &State) -> String {
    let t = &s.timer;
    // A skip is explained for as long as it can be taken back: through its quiet ring and the
    // break that waits after it.
    if t.can_undo() {
        return skipped_line(s.study.log.blocks.last().map_or(0, |b| b.focused));
    }
    match (t.phase, &t.clock) {
        (Phase::Focus, Clock::Ringing { .. }) => "a lantern for that one",
        (_, Clock::Paused { .. }) => "the sky waits · space to resume",
        (_, Clock::Scrubbing { .. }) => "release to set the length",
        (Phase::Focus, Clock::Wound) if s.teaching() => "focus · click the sky or press space to begin",
        (Phase::Focus, Clock::Wound) => "focus · space to begin",
        (Phase::Focus, _) => "focus",
        (_, Clock::Ringing { .. }) => "the break is over",
        (Phase::Short, Clock::Wound) => "rest · space when you're ready",
        (Phase::Short, _) => "rest",
        (Phase::Long, Clock::Wound) => "long rest · space when you're ready",
        (Phase::Long, _) => "long rest, four lanterns up",
    }
    .into()
}

/// What a skip did with the block's `focused` seconds: they count, and light nothing.
fn skipped_line(focused: u32) -> String {
    match (focused + 30) / 60 {
        0 => "Skipped before a minute. No lantern.".into(),
        1 => "Skipped. 1 min still counts, no lantern.".into(),
        _ => format!("Skipped. {} still count, no lantern.", log::duration(focused)),
    }
}

/// What a bell says once it has rung: the phase it ended, and what waits now.
pub fn rang(ended: Phase, t: &Timer) -> String {
    match (ended, t.settled().0) {
        (Phase::Focus, next @ (Phase::Short | Phase::Long)) => {
            let kind = if next == Phase::Long { "long break" } else { "break" };
            format!("Focus complete. {}-minute {kind} ready.", t.lengths.of(next) / 60)
        }
        _ => "Break complete. Focus ready.".into(),
    }
}

/// The last bell's words, as a status assistive technology announces politely. `news` is true
/// only for the first view to draw a bell, so it is announced once, whichever page is open, and
/// a page or the key card drawing it later stays quiet.
pub fn status(s: &State, k: Ink, news: bool) -> Option<El> {
    let words = rang(s.rang?, &s.timer);
    Some(
        text(words.clone())
            .key("status")
            .role("status")
            .label(words)
            .live_region(if news { LiveRegion::Polite } else { LiveRegion::Off })
            .font(TextStyle::new(SMALL, 540.0))
            .color(k.text.alpha(BRIGHT)),
    )
}

pub fn view(s: &State, k: Ink) -> El {
    let t = &s.timer;
    let resting = t.phase != Phase::Focus;
    // Anchored at its top, so the time holds still whatever is offered under it. The top line
    // and the phase line keep one key each, so a change replaces the words in place.
    let mut out = col().key("readout").absolute(24.0, TOP).w(W - 48.0).gap(3.0).items_start();
    if resting {
        // During a break the top line is about the block just done, not the next one. It keeps
        // the label's shape, so the words change in place and never cross-fade over each other.
        let after =
            s.study.log.blocks.last().and_then(|b| b.label.clone()).map_or(String::new(), |l| format!("after {l}"));
        out = out.child(
            row()
                .key("top")
                .h(17.0)
                .max_w(LABEL_W)
                .child(line(k, after, LEAD, 480.0, QUIET).key("top-text").max_w(LABEL_W).truncate()),
        );
    } else {
        out = out.child(label(s, k));
    }
    out = out.child(time_row(s, k)).child(line(k, phase_line(s), SMALL, 460.0, QUIET).key("phase"));
    let mut offers = col().key("offers").gap(6.0).items_start();
    if s.teaching() {
        offers = offers.child(
            row()
                .key("hint")
                .gap(8.0)
                .center_y()
                .child(line(k, "or drag the sun first to change the length", 11.0, 480.0, BRIGHT))
                .child(
                    button(k, "hint-dismiss", "got it", "Hide this hint")
                        .on_press_tx(guarded(|s, _| s.put_hint_away())),
                ),
        );
    }
    if t.can_undo() {
        let undo = row()
            .key("undo")
            .role("button")
            .label("Undo skip")
            .focusable(true)
            .help("Bring the block back  U")
            .min_h(28.0)
            .center_y()
            .gap(8.0)
            .px(12.0)
            .radius(14.0)
            .fill(k.text.alpha(0.92))
            .hover(k.shade.alpha(0.1))
            .focus_ring(1.5, 1.5, k.text.alpha(0.9))
            .child(line(Ink { text: k.shade, shade: k.text }, "Undo skip", 12.0, 600.0, 1.0))
            .child(line(Ink { text: k.shade, shade: k.text }, "U", 11.0, 500.0, 0.8))
            .on_press_tx(guarded(|s, tx| carry(s, Event::Undo { now: tx.clock() }, tx)));
        offers = offers.child(undo);
    }
    if matches!(t.clock, Clock::Paused { .. }) {
        offers = offers.child(
            button(k, "resume", "Resume", "Resume  Space")
                .on_press_tx(guarded(|s, tx| carry(s, Event::Press { now: tx.clock() }, tx))),
        );
    }
    if let Some(r) = reflection(s, k) {
        offers = offers.child(r);
    }
    if let Some(e) = save_status(s, k) {
        offers = offers.child(e);
    }
    out.child(el().h(3.0)).child(offers)
}

/// Why the record isn't saved, wherever it can be changed: on the sunset and in Today. It is said
/// politely as it appears, offers to save again, and stays until a save works.
pub fn save_status(s: &State, k: Ink) -> Option<El> {
    let words = format!("Not saved: {}", s.save_error.as_ref()?);
    Some(
        row()
            .key("save-error")
            .gap(8.0)
            .center_y()
            .child(
                text(words.clone())
                    .key("save-error-text")
                    .role("status")
                    .label(words)
                    .live_region(LiveRegion::Polite)
                    .font(TextStyle::new(SMALL, 540.0))
                    .color(k.text.alpha(BRIGHT))
                    .shrink()
                    .wrap(),
            )
            .child(button(k, "save-retry", "Try again", "Save again").on_press_tx(guarded(|s, _| s.save()))),
    )
}

/// What the next block is for: a field while it's being named.
fn label(s: &State, k: Ink) -> El {
    if s.editing {
        return text_input(&s.label, TextStyle::new(LEAD, 480.0))
            .key("label-field")
            .label("Name for the next block")
            .single_line()
            .w(LABEL_W)
            .color(k.text.alpha(BRIGHT))
            .placeholder("what's this block for?", k.text.alpha(0.7))
            .on_edit(|s: &mut State| &mut s.label)
            .on_input_key(|s: &mut State, e: &KeyEvent, tx: &mut Tx| {
                if e.key != Key::Escape {
                    return false;
                }
                finish_label(s, false, tx);
                true
            })
            .on_submit(|s: &mut State, tx: &mut Tx| finish_label(s, true, tx));
    }
    let named = s.label();
    // Unnamed, the offer to name the block shows only while the block waits.
    let shy = named.is_none() && s.timer.clock != Clock::Wound;
    let alpha = if shy { 0.0 } else { BRIGHT };
    let full = named.unwrap_or_else(|| "+ name this block".into());
    let el = row()
        .key("top")
        .h(17.0)
        .max_w(LABEL_W)
        .role("button")
        .label(format!("What this block is for: {full}"))
        .help(format!("{full}  L"))
        .focus_ring(1.5, 2.0, k.text.alpha(0.9))
        .on_press_tx(guarded(edit_label))
        .child(line(k, full, LEAD, 480.0, alpha).key("top-text").max_w(LABEL_W).truncate());
    if shy { el.inert() } else { el.focusable(true) }
}

/// The time, with what it is when that isn't plain: a block's minutes once it ends, or "paused"
/// while it is held. While it waits, a minute either way sits beside it: drawn once
/// the pointer or Tab calls the controls out, and always there for `VoiceOver`.
fn time_row(s: &State, k: Ink) -> El {
    let t = &s.timer;
    // A block that just ended shows what it came to, not the time left, and says which.
    let (shown, unit, name, said) = match (&t.clock, s.study.log.blocks.last()) {
        (Clock::Ringing { kept, .. }, Some(b)) if t.phase == Phase::Focus => {
            let name = if *kept { "Block complete" } else { "Block skipped" };
            (log::duration(b.focused), Some("focused"), name, format!("{} focused", spoken_minutes(b.focused)))
        }
        (Clock::Scrubbing { .. }, _) => (format!("{} min", t.length() / 60), None, "Length", spoken(t.length())),
        (Clock::Paused { .. }, _) => {
            (mmss(t.shown(s.now)), Some("paused"), "Time remaining, paused", spoken(t.shown(s.now)))
        }
        _ => (mmss(t.shown(s.now)), None, "Time remaining", spoken(t.shown(s.now))),
    };
    let display =
        TextStyle { features: "'tnum' 1", variations: "'opsz' 32", tracking: -0.3, ..TextStyle::new(DISPLAY, 300.0) };
    let time =
        text(shown).key("time").font(display).color(k.text.alpha(BRIGHT)).role("timer").label(name).a11y_value(said);
    // One row in every state, so the digits keep their place and never fade in anew.
    let mut time = row().gap(6.0).items_end().child(time);
    if let Some(unit) = unit {
        time = time.child(line(k, unit, LEAD, 480.0, QUIET).key("time-unit").pb(6.0).a11y_hidden());
    }
    let mut r = row().gap(8.0).center_y();
    r = r.child(time);
    if t.clock == Clock::Wound {
        let shown = s.beckoned();
        r = r.child(el().w(6.0)).child(stepper(k, "shorter", "−", "A minute shorter  ↓", -1, shown)).child(stepper(
            k,
            "longer",
            "+",
            "A minute longer  ↑",
            1,
            shown,
        ));
    }
    r
}

/// A minute either way. Dragging the sun is the way the hint teaches; these are the same thing for
/// the keyboard and assistive technology, so they are always in the tree and drawn when beckoned.
fn stepper(k: Ink, key: &'static str, glyph: &str, help: &str, minutes: i32, shown: bool) -> El {
    el().key(key)
        .size(HIT, HIT)
        .center()
        .role("button")
        .label(help.split("  ").next().unwrap_or(help).to_string())
        .help(help.to_string())
        .focusable(shown)
        .focus_ring(1.5, 0.0, k.text.alpha(0.9))
        .hover_group()
        .radius(12.0)
        .on_press_tx(guarded(move |s, tx| carry(s, Event::Nudge { minutes }, tx)))
        .child(
            el().size(22.0, 22.0)
                .radius(11.0)
                .center()
                .fill(k.shade.alpha(0.55))
                .border(1.0, k.text.alpha(0.35))
                .group_color(k.text.alpha(0.2))
                .opacity(if shown { 1.0 } else { 0.0 })
                .child(line(k, glyph, 14.0, 500.0, BRIGHT)),
        )
}

/// How did that block feel? Optional: three words, offered from the bell. A word given is
/// noted for a moment and the question folds away; left unanswered, it fades into the break
/// and the lantern stays unanswered.
fn reflection(s: &State, k: Ink) -> Option<El> {
    let card = col().key("reflect").gap(6.0).transition(Transition::opacity());
    match s.asking() {
        Asking::Hidden => None,
        Asking::Noted(felt) => Some(
            card.child(line(k, format!("Felt {}. Noted.", felt.word()), SMALL, 480.0, QUIET).key("reflect-question")),
        ),
        Asking::Open => {
            let words = Felt::ALL.iter().enumerate().fold(
                row().key("rating").gap(6.0).center_y().focus_group(),
                |r, (i, &felt)| {
                    r.child(
                        row()
                            .key(format!("felt-{}", felt.word()))
                            .min_h(HIT)
                            .center_y()
                            .px(10.0)
                            .radius(12.0)
                            .fill(k.shade.alpha(0.5))
                            .border(1.0, k.text.alpha(0.45))
                            .hover(k.text.alpha(0.14))
                            .role("radio")
                            .label(felt.word())
                            .checked(false)
                            .focusable(true)
                            .help(format!("{}  {}", felt.word(), i + 1))
                            .focus_ring(1.5, 1.5, k.text.alpha(0.9))
                            .on_press_tx(guarded(move |s, tx| rate(s, felt, tx)))
                            .child(text(felt.word()).font(TextStyle::new(11.5, 540.0)).color(k.text.alpha(BRIGHT))),
                    )
                },
            );
            let dismiss = el()
                .key("reflect-dismiss")
                .size(HIT, HIT)
                .radius(12.0)
                .center()
                .role("button")
                .label("Not now")
                .help("Not now  Esc")
                .hover(k.text.alpha(0.14))
                .focusable(true)
                .focus_ring(1.5, 0.0, k.text.alpha(0.9))
                .child(line(k, "×", 15.0, 500.0, QUIET))
                .on_press_tx(guarded(|s, _| s.reflect = None));
            Some(
                card.child(line(k, "How did that block feel? Optional.", SMALL, 480.0, QUIET).key("reflect-question"))
                    .child(words.child(dismiss)),
            )
        }
    }
}
