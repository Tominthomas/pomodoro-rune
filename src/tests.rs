//! The app as a person drives it: keys, taps and the window's clock go through Rune's
//! `Runtime` into the real `view`, `on_key`, `poll` and handlers, and the checks read what the
//! window draws. Relaunches read only what the store wrote. No GPU is needed, so CI runs these.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rune::{Env, Mods, Runtime};
use serde_json::Value;

use super::*;

const UTC: Calendar = Calendar::UTC;
/// One frame at 60 fps: how long a key or a tap takes to land.
const STEP: f64 = 1.0 / 60.0;
/// Text a person must read is drawn at least this opaque, its colour's alpha times the opacity
/// it inherits.
const READABLE: f32 = 0.5;

/// The prefs file, as the store left it.
#[derive(Clone, Default)]
struct Disk(Arc<Mutex<BTreeMap<String, Value>>>);

impl Disk {
    fn store(&self) -> Store {
        let disk = self.0.clone();
        Store::spawn(
            move |key, value| {
                let mut d = disk.lock().unwrap();
                match value {
                    Some(v) => d.insert(key.to_owned(), v),
                    None => d.remove(key),
                };
                Ok(())
            },
            |_| {},
        )
    }

    fn prefs(&self) -> Prefs {
        let mut prefs = Prefs::memory();
        for (k, v) in self.0.lock().unwrap().iter() {
            prefs.set(k, v).unwrap();
        }
        prefs
    }
}

/// A window, launched from `disk` and saving into it.
struct App {
    rt: Runtime<State>,
    disk: Disk,
}

impl App {
    fn new() -> App {
        App::launch(&Disk::default(), HEADLESS_EPOCH)
    }

    /// Opens the window at Unix time `unix` from what `disk` holds.
    fn launch(disk: &Disk, unix: f64) -> App {
        let s = restore(disk.prefs(), unix, UTC, |_| disk.store());
        let mut rt = Runtime::new(s, view, (W, H), Env::new(false, 1.0));
        rt.poll = Some(poll);
        rt.on_key = Some(on_key);
        rt.frame(STEP);
        App { rt, disk: disk.clone() }
    }

    /// Opens the window on one of the review scenes, as `--scene` does.
    fn scene(name: &str) -> App {
        let mut app = App::new();
        let (_, setup) = demo::SCENES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{name} is a scene"));
        app.rt.update(|s, _| setup(s));
        app.look();
        app
    }

    /// Quits, waiting for the last save, and opens the app again `later` seconds on.
    fn relaunch(&self, later: f64) -> App {
        assert_eq!(self.rt.state.store.flush(Duration::from_secs(5)), Ok(()), "the last save is written");
        App::launch(&self.disk, self.rt.state.epoch + self.rt.clock + later)
    }

    fn wait(&mut self, secs: f64) {
        self.rt.frame(secs);
        self.settle();
    }

    /// Lands every fade and glide without moving the clock, so a check reads what a person sees
    /// once the window is still.
    fn settle(&mut self) {
        self.rt.settle_all();
        self.rt.frame(0.0);
    }

    /// Waits for a page or a word that just changed to finish fading in, as a reader would.
    fn look(&mut self) {
        self.wait(0.5);
    }

    fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.wait(STEP);
    }

    fn char(&mut self, c: &str) {
        self.key(Key::Text(c.into()));
    }

    fn space(&mut self) {
        self.char(" ");
    }

    fn typed(&mut self, text: &str) {
        for c in text.chars() {
            self.char(&c.to_string());
        }
    }

    /// A click on the element keyed `key`, held as long as a person holds one.
    fn tap(&mut self, key: &str) {
        let (x, y) = self.rt.find(key).unwrap_or_else(|| panic!("{key} is on screen"));
        self.click_at(x, y);
    }

    /// A click at `x, y` in window points, wherever it lands.
    fn click_at(&mut self, x: f32, y: f32) {
        self.rt.pointer_move(x, y);
        self.rt.pointer_down(x, y);
        self.rt.frame(0.08);
        self.rt.pointer_up(x, y);
        self.wait(STEP);
    }

    /// Presses the element keyed `key` as `VoiceOver` does, through its accessibility action.
    fn voiceover_press(&mut self, key: &str) {
        assert!(self.rt.accessibility_action(key, rune::accesskit::Action::Click, None), "{key} takes a press");
        self.wait(STEP);
    }

    /// Sets the waiting block to one minute with the arrow keys, as the headless scripts do.
    fn one_minute(&mut self) {
        for _ in 0..24 {
            self.key(Key::Down);
        }
    }

    /// Starts the waiting block and runs it to its bell.
    fn run_to_bell(&mut self) {
        self.space();
        self.wait(f64::from(self.rt.state.timer.length()) + 0.5);
    }

    /// Runs a one-minute block to its bell, lets the bell ring out, and skips the break.
    fn block(&mut self) {
        self.run_to_bell();
        self.wait(RING + 0.5);
        if self.rt.state.timer.phase != Phase::Long {
            self.char("s");
        }
    }

    /// The text of the element keyed `key`, if it is on screen. Present but too faint to read
    /// fails the test: what a person must read has to be drawn readable.
    fn read(&self, key: &str) -> Option<String> {
        let e = self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)?;
        let ink = e.text_color.map_or(1.0, |c| c.a) * e.opacity;
        assert!(ink >= READABLE, "{key} {:?} is drawn too faint to read: {ink}", e.text);
        e.text
    }

    fn time(&self) -> String {
        self.read("time").unwrap_or_default()
    }

    fn phase(&self) -> String {
        self.read("phase").unwrap_or_default()
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    /// The accessibility node keyed `key`, as `VoiceOver` is given it: role, name, value, actions.
    fn node(&self, key: &str) -> Option<Value> {
        let mut stack = vec![self.rt.accessibility_json()];
        while let Some(mut n) = stack.pop() {
            if n["key"] == key {
                return Some(n);
            }
            if let Value::Array(children) = n["children"].take() {
                stack.extend(children);
            }
        }
        None
    }

    /// The bell's status `VoiceOver` announces, and whether it is news: marked to be read out now.
    fn status(&self) -> Option<(String, bool)> {
        self.announced("status")
    }

    /// What the element keyed `key` says to `VoiceOver`, and whether it is read out as it changes.
    fn announced(&self, key: &str) -> Option<(String, bool)> {
        let tree = self.rt.accessibility_tree();
        let (_, n) = tree.nodes.iter().find(|(_, n)| n.author_id() == Some(key))?;
        Some((n.label().unwrap_or_default().to_string(), n.live() == Some(rune::accesskit::Live::Polite)))
    }

    /// The element holding keyboard focus: its key, and where it is drawn.
    fn focused(&self) -> Option<(String, [f32; 4])> {
        self.rt.elements().into_iter().find(|e| e.focused).map(|e| (e.key.unwrap_or_default(), e.rect))
    }

    /// Presses Tab until focus lands on an element whose key is `wanted`, and says where it is
    /// drawn once the window is still.
    fn tab_to(&mut self, wanted: impl Fn(&str) -> bool) -> [f32; 4] {
        for _ in 0..80 {
            if self.focused().is_some_and(|(k, _)| wanted(&k)) {
                self.look();
                return self.focused().map(|(_, rect)| rect).unwrap_or_default();
            }
            self.key(Key::Tab);
        }
        panic!("Tab never reaches it");
    }

    /// The lanterns floating on the water.
    fn lanterns(&self) -> usize {
        (0..=timer::CYCLE).take_while(|i| self.has(&format!("lantern-{i}"))).count()
    }
}

#[test]
fn space_starts_pauses_and_resumes_the_sunset() {
    let mut app = App::new();
    assert_eq!(
        (app.time(), app.phase()),
        ("25:00".into(), "The sun sets as you focus. A finished block lights a lantern.".into())
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
        ("25:00".into(), "The sun sets as you focus. A finished block lights a lantern.".into())
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
    assert!(app.has("strip-key") && !app.has("strip-cue"), "and stays away once the block is closed");
    let mut back = app.relaunch(60.0);
    back.char("h");
    back.look();
    assert!(back.has("strip-key") && !back.has("strip-cue"), "and after a relaunch");
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
fn the_click_that_brings_the_window_forward_starts_nothing() {
    let mut app = App::new();
    app.rt.update(|s, tx| on_focus(s, false, tx));
    app.look();
    app.rt.update(|s, tx| on_focus(s, true, tx));
    app.tap("sky");
    app.wait(1.0);
    assert_eq!(
        (app.time(), app.phase()),
        ("25:00".into(), "The sun sets as you focus. A finished block lights a lantern.".into())
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
    assert_eq!(app.phase(), "The sun sets as you focus. A finished block lights a lantern.");
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
