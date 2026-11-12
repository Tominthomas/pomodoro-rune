//! The dock and the menu bar: what each control and item says and does, the click that
//! brings the window forward, and what `VoiceOver` finds and hears.

use super::*;

#[test]
fn the_menus_name_what_each_key_does_now_and_dim_what_would_do_nothing() {
    let mut app = App::new();
    let menu = |app: &App| -> Vec<(String, bool, bool)> {
        menu::items(&app.rt.state).into_iter().map(|i| (i.title, i.enabled, i.checked)).collect()
    };
    let item = |app: &App, id: &str| {
        let i = menu::items(&app.rt.state).into_iter().find(|i| i.id == id).unwrap();
        (i.title, i.enabled, i.checked)
    };
    let titles: Vec<String> = menu(&app).into_iter().map(|i| i.0).collect();
    assert_eq!(
        titles,
        ["Start Focus", "Skip", "Undo Skip", "Today", "Coast", "Lake", "Mesa", "Next Place", "Larger Text", "Keys"]
    );
    assert_eq!(
        (item(&app, "timer-skip"), item(&app, "timer-undo")),
        (("Skip".into(), false, false), ("Undo Skip".into(), false, false)),
        "nothing to skip or undo yet"
    );
    assert_eq!(item(&app, "place-0"), ("Coast".into(), true, true), "the place shown is ticked");
    app.space();
    assert_eq!(
        (item(&app, "timer-press").0, item(&app, "timer-skip")),
        ("Pause".into(), ("Skip This Block".into(), true, false))
    );
    app.char("s");
    assert_eq!((item(&app, "timer-press").0, item(&app, "timer-undo").1), ("Start Break".into(), true));
    app.char("h");
    assert!(item(&app, "view-today").2, "Today is ticked while it is open");
    app.char("u");
    assert_eq!(item(&app, "timer-press").0, "Pause", "and the menu follows an undo from Today");
}

#[test]
fn the_click_that_brings_the_window_forward_starts_nothing() {
    let mut app = App::new();
    app.rt.update(|s, tx| on_focus(s, false, tx));
    app.look();
    app.rt.update(|s, tx| on_focus(s, true, tx));
    app.tap("sky");
    app.wait(1.0);
    assert_eq!(
        (app.time(), app.phase()),
        ("25:00".into(), "The sun sets as you focus.\nA finished block lights a lantern.".into())
    );
    app.tap("sky");
    app.wait(1.0);
    assert_eq!(app.phase(), "focus", "the next click does");
    app.rt.update(|s, tx| on_focus(s, false, tx));
    app.rt.update(|s, tx| on_focus(s, true, tx));
    app.space();
    assert_eq!(app.phase(), "the sky waits · space to resume", "keys always act");
}

#[test]
fn the_activating_click_on_a_control_does_nothing_either() {
    let mut app = App::new();
    app.rt.pointer_move(300.0, 510.0);
    app.rt.update(|s, tx| on_focus(s, false, tx));
    app.look();
    app.rt.update(|s, tx| on_focus(s, true, tx));
    app.tap("control-play");
    app.wait(1.0);
    assert_eq!(app.phase(), "The sun sets as you focus.\nA finished block lights a lantern.");
    app.tap("control-play");
    app.wait(1.0);
    assert_eq!(app.phase(), "focus");
}

#[test]
fn two_quick_clicks_on_start_start_then_pause_and_never_end_the_block() {
    let mut app = App::new();
    let slot =
        |app: &App| app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("control-play")).unwrap().rect;
    let waiting = slot(&app);
    // Where the round-one critique double-clicked: inside Start, left of its middle.
    let (x, y) = (waiting[0] + 0.4 * waiting[2], waiting[1] + waiting[3] / 2.0);
    app.click_at(x, y);
    app.rt.frame(0.2);
    assert_eq!(slot(&app), waiting, "running, Pause takes Start's whole place");
    app.click_at(x, y);
    app.look();
    assert_eq!(app.phase(), "the sky waits · space to resume", "the second click pauses");
    assert_eq!(slot(&app), waiting, "and Resume keeps it");
    assert!(app.rt.state.study.log.blocks.is_empty() && !app.has("undo"), "nothing was ended");
}

#[test]
fn voiceover_finds_every_control_and_presses_play_while_the_dock_is_hidden() {
    let mut app = App::new();
    let control = |app: &App, key: &str| {
        let n = app.node(key).unwrap_or_else(|| panic!("{key} is in the accessibility tree"));
        assert_eq!(n["role"], "button", "{n}");
        assert!(n["actions"].as_array().is_some_and(|a| a.contains(&"click".into())), "{key} can be pressed: {n}");
        n["name"].as_str().unwrap_or_default().to_string()
    };
    let names: Vec<String> =
        ["control-play", "control-today", "control-place", "control-keys"].iter().map(|k| control(&app, k)).collect();
    assert_eq!(names, ["Start focus", "Today", "Place: Coast", "Keys"]);
    app.voiceover_press("control-play");
    app.wait(2.0);
    assert_eq!((app.time(), app.phase()), ("24:58".into(), "focus".into()), "pressed as a click would");
    assert_eq!(
        (control(&app, "control-play"), control(&app, "control-skip")),
        ("Pause".into(), "Skip this block".into())
    );
    app.voiceover_press("control-play");
    app.wait(1.0);
    assert_eq!(app.phase(), "the sky waits · space to resume");
    assert_eq!(control(&app, "control-play"), "Resume");
}

#[test]
fn each_bell_is_announced_once_politely_whatever_page_is_open() {
    let done = "Focus complete. 1 of 4 lanterns. 5-minute break ready.".to_string();
    let mut app = App::new();
    app.one_minute();
    app.space();
    app.wait(30.0);
    assert_eq!(app.status(), None, "a running block's seconds are never announced");
    app.char("h");
    app.look();
    app.rt.frame(31.0);
    assert_eq!(app.status(), Some((done.clone(), true)), "the bell is news while Today is open");
    app.settle();
    assert_eq!(app.status(), Some((done.clone(), false)), "and said once");
    assert_eq!(app.read("status"), Some(done.clone()), "Today shows it too");
    app.char("?");
    app.look();
    assert_eq!(app.status(), Some((done.clone(), false)), "the key card shows it without saying it again");
    app.key(Key::Escape);
    app.key(Key::Escape);
    app.look();
    assert_eq!(app.status(), Some((done, false)), "back on the scene it is still quiet");
    app.wait(RING);
    app.space();
    assert_eq!(app.status(), None, "the break has started");
    app.rt.frame(5.0 * 60.0);
    assert_eq!(app.status(), Some(("Break complete. Focus ready.".into(), true)));
}
