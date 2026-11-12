//! The timer as a person runs it: starting, pausing, the bell, skipping, the cycle of four,
//! and the wakes a still window owes.

use super::*;

#[test]
fn space_starts_pauses_and_resumes_the_sunset() {
    let mut app = App::new();
    assert_eq!(
        (app.time(), app.phase()),
        ("25:00".into(), "The sun sets as you focus.\nA finished block lights a lantern.".into())
    );
    app.space();
    app.wait(2.0);
    assert_eq!((app.time(), app.phase()), ("24:58".into(), "focus".into()));
    app.space();
    app.wait(5.0);
    assert_eq!(
        (app.time(), app.phase()),
        ("24:58".into(), "the sky waits · space to resume".into()),
        "held where it was"
    );
    assert_eq!(app.read("time-unit").as_deref(), Some("paused"), "and says so beside the time");
    app.space();
    app.wait(1.0);
    assert_eq!((app.time(), app.phase()), ("24:57".into(), "focus".into()));
}

#[test]
fn a_tap_on_the_sky_starts_pauses_and_resumes_it_too() {
    let mut app = App::new();
    app.tap("sky");
    app.wait(2.0);
    assert_eq!((app.time(), app.phase()), ("24:58".into(), "focus".into()));
    app.tap("sky");
    app.wait(3.0);
    assert_eq!(app.phase(), "the sky waits · space to resume");
    app.tap("sky");
    app.wait(1.0);
    assert_eq!((app.time(), app.phase()), ("24:57".into(), "focus".into()));
}

#[test]
fn the_arrows_set_the_length_only_while_the_block_waits() {
    let mut app = App::new();
    app.key(Key::Up);
    assert_eq!(app.time(), "26:00");
    app.key(Key::Down);
    app.key(Key::Left);
    assert_eq!(app.time(), "24:00");
    app.key(Key::Right);
    assert_eq!(app.time(), "25:00");
    app.space();
    app.key(Key::Up);
    app.wait(1.0);
    assert_eq!(app.time(), "24:59", "a running block keeps its length");
}

#[test]
fn the_arrows_leave_the_length_alone_while_today_or_the_keys_are_open() {
    let mut app = App::new();
    for (open, arrows) in [("h", [Key::Down, Key::Left]), ("?", [Key::Up, Key::Right])] {
        app.char(open);
        for arrow in arrows {
            app.key(arrow);
        }
        app.key(Key::Escape);
        app.look();
        assert_eq!(app.time(), "25:00", "the arrows on {open} set nothing");
    }
}

#[test]
fn the_bell_lights_a_lantern_and_a_break_waits_with_the_question() {
    let mut app = App::new();
    app.one_minute();
    assert_eq!(app.time(), "01:00");
    app.run_to_bell();
    assert_eq!(
        (app.time(), app.phase()),
        ("1 min".into(), "1 of 4 lanterns · 5-minute break ready".into()),
        "the cycle, at the bell"
    );
    assert_eq!(app.read("time-unit").as_deref(), Some("focused"), "what the minute was, not what is left");
    let time = app.node("time").unwrap();
    assert_eq!((&time["name"], &time["value"]), (&"Block complete".into(), &"1 minute focused".into()));
    assert_eq!(app.lanterns(), 1);
    assert_eq!(app.read("reflect-question").as_deref(), Some("How did that block feel? Optional."));
    app.wait(RING + 0.5);
    assert_eq!((app.time(), app.phase()), ("05:00".into(), "1 of 4 lanterns · 5-minute break ready".into()));
    app.char("2");
    assert_eq!(app.read("reflect-question").as_deref(), Some("Felt steady. Noted."));
    app.wait(ACKNOWLEDGED + 0.5);
    assert!(!app.has("reflect"), "the question folds away");
}

#[test]
fn s_skips_a_block_and_u_takes_it_back_so_it_counts_once_finished() {
    let mut app = App::new();
    app.one_minute();
    app.space();
    app.wait(20.0);
    app.char("s");
    assert_eq!(app.phase(), "Skipped before a minute. No lantern.");
    assert!(app.has("undo") && app.lanterns() == 0, "no lantern, and an offer to take it back");
    app.wait(1.0);
    app.char("u");
    assert_eq!((app.time(), app.phase()), ("00:39".into(), "focus".into()), "running all along");
    app.wait(40.0);
    assert_eq!((app.phase(), app.lanterns()), ("1 of 4 lanterns · 5-minute break ready".into(), 1));
    let mut back = app.relaunch(3600.0);
    assert_eq!((back.phase(), back.lanterns()), ("1 of 4 lanterns · 5-minute break ready".into(), 1));
    back.char("h");
    back.look();
    assert_eq!((back.read("stat-lanterns-value").as_deref(), back.has("stat-skipped")), (Some("1"), false));
}

#[test]
fn four_lanterns_earn_the_long_break_and_quitting_during_its_bell_keeps_it() {
    let mut app = App::new();
    app.one_minute();
    for _ in 0..3 {
        app.block();
    }
    app.run_to_bell();
    assert_eq!(app.lanterns(), 4);
    let back = app.relaunch(4.0 * 3600.0);
    assert_eq!((back.phase(), back.time()), ("4 of 4 lanterns · 15-minute long break ready".into(), "15:00".into()));
    assert_eq!(back.lanterns(), 4, "the four are on the water, waiting to rise");
    app.wait(RING + 0.5);
    assert_eq!(app.phase(), "4 of 4 lanterns · 15-minute long break ready");
    app.space();
    app.wait(1.0);
    assert_eq!((app.phase(), app.lanterns()), ("long rest, four lanterns up".into(), 4));
}

#[test]
fn skipping_the_long_break_in_the_second_it_starts_begins_a_new_cycle_once() {
    let mut app = App::new();
    app.one_minute();
    for _ in 0..3 {
        app.block();
    }
    app.run_to_bell();
    app.space();
    app.char("s");
    assert_eq!((app.phase(), app.lanterns()), ("focus · space to begin".into(), 0), "a new cycle, live");
    let back = app.relaunch(0.0);
    assert_eq!((back.phase(), back.lanterns()), ("focus · space to begin".into(), 0), "and after a relaunch");
}

#[test]
fn a_saved_length_out_of_bounds_is_not_loaded() {
    let disk = Disk::default();
    let lengths = serde_json::json!({ "focus": 0, "short": 300, "long": 900 });
    disk.0.lock().unwrap().insert("lengths".into(), lengths);
    let app = App::launch(&disk, HEADLESS_EPOCH);
    assert_eq!((app.time(), app.rt.state.timer.progress(0.0)), ("25:00".into(), 0.0), "the defaults, not a zero block");
}

#[test]
fn an_idle_window_wakes_at_four_in_the_morning_and_turns_the_day() {
    let mut study = Study::new();
    study.log.push(Block {
        serial: 0,
        ended: HEADLESS_EPOCH as i64 - 3600,
        planned: 1500,
        focused: 1500,
        outcome: timer::Outcome::Finished,
        pauses: vec![],
        paused: 0,
        rated: None,
        label: None,
    });
    let timer = study.resume(UTC, HEADLESS_EPOCH as i64, Lengths::default());
    let mut s = State::new(HEADLESS_EPOCH, UTC, study, timer, Store::memory());
    let before = s.timed();
    let (water, stars) = s.lanterns();
    assert_eq!((stars.len(), water.len()), (0, 1), "the afternoon's lantern floats");
    // Idle at 2 p.m.: nothing is due but the turn of the study day, 14 hours on.
    let wake = s.next_change();
    assert_eq!(wake, 14.0 * 3600.0, "the only wake is 4 a.m.");
    s.now = wake;
    assert_ne!(s.timed(), before, "at 4 a.m. the window has something new to draw");
    assert!(s.study.roll_over(s.cal, s.unix(), &s.timer).is_some(), "and a new cycle begins");
    let (water, stars) = s.lanterns();
    assert_eq!((stars.len(), water.len()), (0, 0), "yesterday's lantern has left today's sky");
}

#[test]
fn a_skipped_block_can_be_taken_back_until_the_next_start_even_from_today() {
    let mut app = App::new();
    app.one_minute();
    app.space();
    app.wait(20.0);
    app.char("s");
    // One ten-minute step with no frames in it: only the wakes the window asked for run.
    app.rt.frame(600.0);
    app.settle();
    assert_eq!(app.phase(), "Skipped before a minute. No lantern.", "the quiet ring ended on its own wake");
    assert_eq!(app.rt.state.timer.clock, Clock::Wound);
    assert!(app.has("undo"), "ten minutes on, the offer stands");
    app.char("h");
    app.look();
    app.char("u");
    app.key(Key::Escape);
    app.look();
    assert_eq!(app.lanterns(), 1, "it ran on all along, so it is over, and lit");
    let mut started = App::new();
    started.one_minute();
    started.space();
    started.wait(20.0);
    started.char("s");
    started.wait(RING + 0.5);
    started.space();
    started.char("u");
    assert!(!started.has("undo") && started.phase() == "rest", "once the break starts, it can't be");
}

#[test]
fn a_still_window_wakes_for_the_bell_it_owes() {
    let mut app = App::new();
    app.one_minute();
    app.space();
    // An hour in one step: if the bell's wake were missing, the block would end at the hour.
    app.rt.frame(3600.0);
    app.settle();
    app.char("h");
    app.look();
    let lantern = app.node("day-lantern-0").expect("the block is in Today");
    assert!(lantern["name"].as_str().unwrap().starts_with("Open block: 14:00 – 14:01 · 1 min"), "{lantern}");
}
