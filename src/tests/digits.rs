//! The digits during focus: the sunset is the timer, so they step back while a block runs and
//! come out when called, held, set or rung, or always once the View menu says so.

use super::*;
use crate::readout::digits_shown;

#[test]
fn the_digits_hide_only_while_a_focus_block_runs_uncalled() {
    let run = timer::Run { planned: 60, pauses: Vec::new(), paused: 0.0 };
    let ticking = Clock::Ticking { ends_at: 60.0, tick: 1.0, run: run.clone() };
    let paused = Clock::Paused { secs: 60, since: 0.0, run };
    let ringing = Clock::Ringing { until: 9.0, kept: true, from: 1.0 };
    let table = [
        (Phase::Focus, &ticking, false, false, false),
        (Phase::Focus, &ticking, true, false, true),
        (Phase::Focus, &ticking, false, true, true),
        (Phase::Focus, &Clock::Wound, false, false, true),
        (Phase::Focus, &paused, false, false, true),
        (Phase::Focus, &ringing, false, false, true),
        (Phase::Short, &ticking, false, false, true),
        (Phase::Long, &ticking, false, false, true),
        (Phase::Short, &ringing, false, false, true),
    ];
    for (phase, clock, beckoned, always, shown) in table {
        assert_eq!(digits_shown(phase, clock, beckoned, always), shown, "{phase:?} {clock:?} {beckoned} {always}");
    }
}

#[test]
fn a_running_block_hides_its_digits_until_the_pointer_or_tab_calls_them_and_voiceover_still_hears_them() {
    let mut app = App::new();
    assert!(app.digits(), "a waiting block shows its length");
    app.space();
    app.wait(3.0);
    assert!(!app.digits(), "the sunset is the timer");
    assert_eq!(app.node("time").unwrap()["value"], "24 minutes 57 seconds", "VoiceOver still reads it");
    app.rt.pointer_move(200.0, 300.0);
    app.look();
    assert!(app.digits(), "the pointer calls them out");
    app.wait(CONTROLS_LINGER + 0.5);
    assert!(!app.digits(), "and they step back once it rests");
    app.key(Key::Tab);
    app.look();
    assert!(app.digits(), "Tab calls them out too");
    app.rt.pointer_move(200.0, 300.0);
    app.wait(CONTROLS_LINGER + 0.5);
    app.space();
    app.look();
    assert_eq!((app.digits(), app.phase()), (true, "the sky waits · space to resume".into()), "paused, they show");
}

#[test]
fn always_show_time_keeps_the_digits_through_a_block_and_is_remembered() {
    let mut app = App::new();
    app.rt.update(|s, _| set_always_time(s, true));
    let item = menu::items(&app.rt.state).into_iter().find(|i| i.id == "view-always-time").unwrap();
    assert_eq!((item.title.as_str(), item.checked), ("Always Show Time", true));
    app.space();
    app.wait(3.0);
    assert!(app.digits(), "the digits stay");
    let mut back = app.relaunch(60.0);
    back.space();
    back.wait(3.0);
    assert!(back.digits(), "and stay on across a launch");
}
