//! The sun in the menu bar's menu, as the window's state words it, and what its items do. The
//! status item itself needs the real menu bar, so these read the words it would be given.

use super::*;
use crate::tray::{Glyph, MenuBar, Words};

fn words(app: &App) -> Words {
    Words::of(&app.rt.state, MenuBar::of_state(&app.rt.state))
}

fn pick(app: &mut App, id: &'static str) {
    app.rt.update(move |s, tx| crate::tray::act(s, id, tx));
    app.wait(STEP);
}

#[test]
fn the_menu_says_where_the_block_is_and_what_today_holds() {
    let mut app = App::new();
    let w = words(&app);
    assert_eq!(
        (w.header.as_str(), w.press.as_str(), w.skip, w.today.as_str(), w.sound),
        ("Focus · 25 min", "Start Focus", None, "Today · no blocks yet", true)
    );
    app.space();
    app.wait(90.0);
    let w = words(&app);
    assert_eq!((w.header.as_str(), w.press.as_str()), ("Focus · 24 min left", "Pause"));
    assert_eq!(w.skip.as_deref(), Some("Skip This Block"));
    app.space();
    assert_eq!(words(&app).header, "Focus · paused · 24 min left");
    assert_eq!(words(&app).press, "Resume");
    app.char("s");
    app.wait(RING + 0.5);
    assert_eq!(words(&app).today, "Today · 2 min · 0 lanterns", "a skipped block's minutes count, no lantern");
    app.char("s");
    app.one_minute();
    app.char("l");
    app.typed("Thesis");
    app.key(Key::Enter);
    app.space();
    assert_eq!(words(&app).header, "Thesis · 1 min left", "a named block goes by its name");
    app.wait(60.5);
    assert_eq!(words(&app).header, "A lantern for that one");
    app.wait(RING + 0.5);
    let w = words(&app);
    assert_eq!((w.header.as_str(), w.press.as_str()), ("Break · 5 min", "Start Break"));
    assert_eq!(w.today, "Today · 3 min · 1 lantern");
}

#[test]
fn the_menu_starts_the_block_turns_the_sound_off_and_opens_today() {
    let mut app = App::new();
    pick(&mut app, "tray-press");
    assert!(matches!(app.rt.state.timer.clock, Clock::Ticking { .. }), "Start Focus starts the block");
    pick(&mut app, "tray-press");
    assert!(matches!(app.rt.state.timer.clock, Clock::Paused { .. }), "and Pause holds it");
    pick(&mut app, "tray-skip");
    assert!(app.rt.state.timer.can_undo(), "Skip is the same skip, with its undo");
    pick(&mut app, "tray-sound");
    assert!(!app.rt.state.sound && !words(&app).sound, "Sound goes off, and its tick with it");
    assert!(
        !menu::items(&app.rt.state).into_iter().find(|i| i.id == "view-sound").unwrap().checked,
        "the View menu's Sound follows"
    );
    pick(&mut app, "tray-today");
    assert!(app.has("ledger"), "Today opens in the window");
}

#[test]
fn a_break_after_a_skip_stays_unlit_across_a_relaunch() {
    let mut app = App::new();
    app.space();
    app.wait(120.0);
    app.char("s");
    app.wait(RING + 0.5);
    let glyph = |app: &App| MenuBar::of_state(&app.rt.state).glyph;
    assert_eq!(glyph(&app), Glyph::Lantern, "the skipped block lit nothing");
    let app = app.relaunch(60.0);
    assert_eq!(app.rt.state.timer.phase, Phase::Short, "the break still waits");
    assert_eq!(glyph(&app), Glyph::Lantern, "and still lit nothing after the relaunch");
}
