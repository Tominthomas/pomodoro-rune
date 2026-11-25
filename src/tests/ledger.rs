//! Today and History: naming blocks, the day's strip and numbers, past days, and saving.

use super::*;

#[test]
fn history_tells_voiceover_which_day_the_page_shows() {
    let mut app = App::scene("today");
    let today = format!("day-{}", app.rt.state.today().0);
    let yesterday = format!("day-{}", app.rt.state.today().0 - 1);
    let pressed = |app: &App, key: &str| {
        let states = app.node(key).unwrap_or_else(|| panic!("{key} is in the tree"))["states"].clone();
        states.as_array().is_some_and(|s| s.contains(&"checked".into()))
    };
    assert!(app.node(&today).unwrap()["name"].as_str().unwrap().contains(", today:"));
    assert!(pressed(&app, &today) && !pressed(&app, &yesterday), "today is the day shown");
    app.voiceover_press(&yesterday);
    app.look();
    assert!(pressed(&app, &yesterday) && !pressed(&app, &today), "the day picked is the one shown");
    app.voiceover_press("back-to-today");
    app.look();
    assert!(pressed(&app, &today) && !pressed(&app, &yesterday), "and today again, after focus moved away");
}

#[test]
fn l_names_the_block_and_today_knows_what_it_was_for() {
    let mut app = App::new();
    app.char("l");
    app.typed("Reading");
    app.key(Key::Enter);
    app.look();
    assert_eq!(
        (app.read("top-text").as_deref(), app.time()),
        (Some("Reading"), "25:00".into()),
        "typing starts nothing"
    );
    app.one_minute();
    app.block();
    assert_eq!(app.read("top-text").as_deref(), Some("Reading"), "the label carries over to the next block");
    app.char("h");
    app.look();
    assert_eq!(app.read("today-title").as_deref(), Some("Today"));
    assert!(app.has("for-Reading"));
    app.key(Key::Escape);
    app.look();
    assert!(!app.has("today-title") && app.time() == "01:00", "Escape goes back to the sunset");
}

#[test]
fn l_from_today_names_the_next_block_and_its_letters_never_reach_the_timer() {
    let mut app = App::new();
    app.char("h");
    app.look();
    app.char("l");
    app.look();
    assert!(app.has("label-field") && !app.has("today-title"), "the field opens under the time, in sight");
    // S skips, H opens Today, T changes place, U undoes, L names: here they are only letters.
    app.typed("Study the last unit, slowly");
    app.key(Key::Enter);
    app.look();
    assert_eq!(app.read("top-text").as_deref(), Some("Study the last unit, slowly"));
    assert_eq!(
        (app.time(), app.phase()),
        ("25:00".into(), "The sun sets as you focus.\nWhen it's down, a bell rings.".into())
    );
    assert_eq!(app.rt.state.place, 0, "still at the coast");
    app.space();
    app.char("h");
    app.look();
    app.char("l");
    app.typed("Then the hard set");
    app.key(Key::Enter);
    app.wait(2.0);
    assert_eq!((app.phase(), app.read("top-text").as_deref()), ("focus".into(), Some("Then the hard set")));
    assert!(app.rt.state.study.log.blocks.is_empty() && !app.has("undo"), "the running block was never ended");
}

#[test]
fn both_name_fields_say_what_they_name_empty_or_filled() {
    let mut app = App::new();
    let field = |app: &App, key: &str| {
        let n = app.node(key).unwrap_or_else(|| panic!("{key} is in the accessibility tree"));
        (n["name"].as_str().unwrap_or_default().to_string(), n["value"].as_str().unwrap_or_default().to_string())
    };
    app.char("l");
    app.look();
    assert_eq!(field(&app, "label-field"), ("Name for the next block".into(), String::new()));
    app.typed("Essay");
    assert_eq!(field(&app, "label-field"), ("Name for the next block".into(), "Essay".into()));
    app.key(Key::Enter);
    let mut today = App::scene("today");
    today.tap("day-lantern-0");
    today.tap("detail-rename");
    today.look();
    assert_eq!(field(&today, "detail-label-field"), ("Name for this block".into(), "Reading & notes".into()));
}

#[test]
fn a_failed_save_is_shown_where_the_record_changes_and_can_be_tried_again() {
    let mut app = App::new();
    app.one_minute();
    app.block();
    // What the store reports from its writer thread when a write fails.
    app.rt.update(|s, _| s.save_error = Some("the disk is full".into()));
    app.look();
    let said = Some("Not saved: the disk is full".to_string());
    assert_eq!(app.read("save-error-text"), said, "on the sunset");
    assert_eq!(app.announced("save-error-text"), said.clone().map(|s| (s, true)), "and said as it appears");
    app.char("h");
    app.look();
    assert_eq!(app.read("save-error-text"), said, "and in Today, where names and words are changed");
    app.disk.0.lock().unwrap().clear();
    app.tap("save-retry");
    assert_eq!(app.rt.state.store.flush(Duration::from_secs(5)), Ok(()));
    assert!(app.disk.0.lock().unwrap().contains_key(study::KEY), "Try again writes the record again");
}

#[test]
fn a_name_being_typed_keeps_its_letters_even_when_its_field_lost_focus() {
    let mut app = App::new();
    app.char("l");
    app.typed("Re");
    app.rt.update(|_, tx| tx.blur());
    app.wait(STEP);
    app.typed("ad");
    app.key(Key::Enter);
    app.look();
    assert_eq!(app.read("top-text").as_deref(), Some("Read"), "the field took its letters back");
    assert_eq!((app.time(), app.rt.state.timer.phase), ("25:00".into(), Phase::Focus), "and the timer took none");
}

#[test]
fn todays_marks_say_they_open_their_block_and_the_cue_goes_once_one_is_opened() {
    let mut app = App::new();
    app.one_minute();
    app.block();
    app.char("h");
    app.look();
    let name = |app: &App| app.node("day-lantern-0").unwrap()["name"].as_str().unwrap().to_string();
    assert!(name(&app).starts_with("Open block: 14:00 – 14:01 · 1 min"), "{}", name(&app));
    assert_eq!(app.read("strip-cue").as_deref(), Some("Select a lantern or ring to open the block"));
    app.tap("day-lantern-0");
    app.look();
    assert!(name(&app).starts_with("Close block: "), "open, the mark closes it: {}", name(&app));
    assert!(!app.has("strip-cue"), "the cue has done its work");
    app.tap("day-lantern-0");
    app.look();
    assert!(!app.has("strip-cue"), "and stays away once the block is closed");
    let mut back = app.relaunch(60.0);
    back.char("h");
    back.look();
    assert!(!back.has("strip-cue"), "and after a relaunch");
}

#[test]
fn today_opens_a_block_and_a_past_day_and_escape_closes_the_innermost_first() {
    let mut app = App::new();
    app.one_minute();
    app.block();
    app.char("h");
    app.look();
    app.tap("day-lantern-0");
    app.look();
    assert!(app.read("detail-about").unwrap().contains("feeling not given"));
    app.tap("detail-rename");
    app.typed("Essay");
    app.key(Key::Enter);
    app.look();
    assert!(app.read("detail-about").unwrap().starts_with("Essay"), "typing in the field lets nothing go");
    app.tap("detail-rename");
    app.key(Key::Escape);
    app.look();
    assert!(!app.has("detail-label-field") && app.has("detail"), "Escape puts the field away first");
    app.key(Key::Escape);
    app.look();
    assert!(!app.has("detail") && app.has("today-title"), "then the block");
    let yesterday = UTC.day_of(HEADLESS_EPOCH as i64).0 - 1;
    app.rt.wheel(190.0, 400.0, -2000.0);
    app.look();
    app.tap(&format!("day-{yesterday}"));
    app.look();
    assert_eq!(app.read("today-title").as_deref(), Some("Tuesday"));
    app.tap("back-to-today");
    app.look();
    assert_eq!(app.read("today-title").as_deref(), Some("Today"));
    app.rt.wheel(190.0, 400.0, -2000.0);
    app.look();
    app.tap(&format!("day-{yesterday}"));
    app.key(Key::Escape);
    app.look();
    assert_eq!(app.read("today-title").as_deref(), Some("Today"), "then the day picked");
    app.key(Key::Escape);
    app.look();
    assert!(!app.has("today-title") && app.time() == "01:00", "then Today itself");
    app.char("h");
    app.look();
    app.tap("day-lantern-0");
    app.look();
    app.tap("detail-rename");
    app.tap("today-close");
    app.look();
    assert!(!app.has("today-title"), "Today closes in the middle of a rename");
}

#[test]
fn the_offer_to_name_a_block_shows_while_it_waits() {
    let mut app = App::new();
    let ink = |app: &App| {
        let e = app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("top-text")).unwrap();
        e.text_color.map_or(1.0, |c| c.a) * e.opacity
    };
    assert_eq!(app.read("top-text").as_deref(), Some("+ name this block"), "in sight with the block, pointer or not");
    app.space();
    app.look();
    assert!(ink(&app) < 0.01, "and gone once the block runs unnamed");
}

/// Whether `rect` is wholly inside `within`.
fn inside(rect: [f32; 4], within: [f32; 4]) -> bool {
    let [x, y, w, h] = rect;
    let [a, b, c, d] = within;
    x >= a - 0.5 && y >= b - 0.5 && x + w <= a + c + 0.5 && y + h <= b + d + 0.5
}

#[test]
fn a_long_day_keeps_every_lantern_apart_and_reachable_from_the_keyboard() {
    let mut app = App::scene("busy");
    app.key(Key::Escape);
    app.char("h");
    app.look();
    let targets: Vec<[f32; 4]> = app
        .rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("day-lantern-") || k.starts_with("day-ring-")))
        .filter(|e| !e.key.as_deref().is_some_and(|k| k.contains("art")))
        .map(|e| e.rect)
        .collect();
    assert_eq!(targets.len(), 19);
    for (i, a) in targets.iter().enumerate() {
        for b in &targets[i + 1..] {
            assert!(a[0] + a[2] <= b[0] || b[0] + b[2] <= a[0], "targets overlap: {a:?} {b:?}");
        }
    }
    let view = app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(today::STRIP)).unwrap().rect;
    let newest = app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("day-lantern-14")).unwrap().rect;
    assert!(inside(newest, view), "Today opens on the newest: {newest:?} in {view:?}");
    for key in ["day-lantern-0", "day-ring-3", "day-lantern-14"] {
        let at = app.tab_to(|k| k == key);
        assert!(inside(at, view), "{key} scrolls into view: {at:?} in {view:?}");
    }
}

#[test]
fn four_long_names_push_history_down_and_the_page_scrolls_to_it() {
    let mut app = App::scene("today");
    let names =
        ["Draft literature review", "Calculus practice problems", "Read research paper", "Write seminar summary"];
    for (i, name) in names.iter().enumerate() {
        app.tap(&format!("day-lantern-{i}"));
        app.tap("detail-rename");
        app.key(Key::Command('a'));
        app.typed(name);
        app.key(Key::Enter);
        app.look();
    }
    let window = [0.0, 0.0, W, H];
    for key in ["timer-bar", "today-title", "today-close"] {
        let at = app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap().rect;
        assert!(inside(at, window), "{key} stays put: {at:?}");
    }
    app.tab_to(|k| k.strip_prefix("day-").is_some_and(|n| n.parse::<i64>().is_ok()));
    app.key(Key::End);
    app.look();
    let today = format!("day-{}", UTC.day_of(HEADLESS_EPOCH as i64).0);
    let (key, at) = app.focused().unwrap();
    assert_eq!(key, today, "End reaches the last day in the grid");
    assert!(inside(at, window), "and today's tile scrolls into view: {at:?}");
    app.rt.wheel(190.0, 400.0, -2000.0);
    app.look();
    let legend = app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("history-scale")).unwrap().rect;
    assert!(inside(legend, window), "and the wheel reaches the legend: {legend:?}");
}

#[test]
fn today_keeps_the_running_block_in_sight_and_in_hand() {
    let mut app = App::new();
    app.space();
    app.wait(4.0);
    app.char("h");
    app.look();
    let bar = |app: &App| app.read("timer-text").unwrap_or_default();
    assert_eq!(bar(&app), "Focus · 24:56 left", "the block runs on in sight");
    app.tap("timer-press");
    app.look();
    assert!(bar(&app).starts_with("Paused · 24:5"), "paused from Today: {}", bar(&app));
    app.wait(10.0);
    app.space();
    app.look();
    assert!(bar(&app).starts_with("Focus · 24:5"), "and Space there resumes it where you can see: {}", bar(&app));
    app.char("s");
    app.wait(RING + 0.5);
    assert_eq!(bar(&app), "Break ready · 05:00");
    app.tap("timer-undo");
    app.look();
    assert!(bar(&app).starts_with("Focus · 24:"), "a skip comes back from Today: {}", bar(&app));
    app.tap("today-close");
    app.look();
    assert_eq!(app.phase(), "focus", "and Back to sunset finds it still running");
}
