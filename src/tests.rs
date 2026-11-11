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
        self.rt.pointer_move(x, y);
        self.rt.pointer_down(x, y);
        self.rt.frame(0.08);
        self.rt.pointer_up(x, y);
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

    /// The lanterns floating on the water.
    fn lanterns(&self) -> usize {
        (0..=timer::CYCLE).take_while(|i| self.has(&format!("lantern-{i}"))).count()
    }
}

#[test]
fn space_starts_pauses_and_resumes_the_sunset() {
    let mut app = App::new();
    assert_eq!((app.time(), app.phase()), ("25:00".into(), "focus · space to begin".into()));
    app.space();
    app.wait(2.0);
    assert_eq!((app.time(), app.phase()), ("24:58".into(), "focus".into()));
    app.space();
    app.wait(5.0);
    assert_eq!((app.time(), app.phase()), ("24:58".into(), "paused · space to resume".into()), "held where it was");
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
    assert_eq!(app.phase(), "paused · space to resume");
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
    assert_eq!((app.time(), app.phase()), ("1 min".into(), "a lantern for that one".into()));
    assert_eq!(app.lanterns(), 1);
    assert_eq!(app.read("reflect-question").as_deref(), Some("How did that block feel? Optional."));
    app.wait(RING + 0.5);
    assert_eq!((app.time(), app.phase()), ("05:00".into(), "rest · space when you're ready".into()));
    app.char("2");
    assert_eq!(app.read("reflect-question").as_deref(), Some("Felt steady. Noted."));
    app.wait(ACKNOWLEDGED + 0.5);
    assert!(!app.has("reflect"), "the question folds away");
}

#[test]
fn s_lets_a_block_go_and_u_takes_it_back_so_it_counts_once_finished() {
    let mut app = App::new();
    app.one_minute();
    app.space();
    app.wait(20.0);
    app.char("s");
    assert_eq!(app.phase(), "let go; the minutes still count");
    assert!(app.has("undo") && app.lanterns() == 0, "no lantern, and an offer to take it back");
    app.wait(1.0);
    app.char("u");
    assert_eq!((app.time(), app.phase()), ("00:39".into(), "focus".into()), "running all along");
    app.wait(40.0);
    assert_eq!((app.phase(), app.lanterns()), ("a lantern for that one".into(), 1));
    let mut back = app.relaunch(3600.0);
    assert_eq!((back.phase(), back.lanterns()), ("rest · space when you're ready".into(), 1));
    back.char("h");
    back.look();
    assert_eq!((back.read("stat-lanterns-value").as_deref(), back.has("stat-let-go")), (Some("1"), false));
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
    app.tap(&format!("day-{yesterday}"));
    app.look();
    assert_eq!(app.read("today-title").as_deref(), Some("Tuesday"));
    app.tap("back-to-today");
    app.look();
    assert_eq!(app.read("today-title").as_deref(), Some("Today"));
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
    assert_eq!((app.time(), app.phase()), ("25:00".into(), "focus · space to begin".into()));
    app.tap("sky");
    app.wait(1.0);
    assert_eq!(app.phase(), "focus", "the next click does");
    app.rt.update(|s, tx| on_focus(s, false, tx));
    app.rt.update(|s, tx| on_focus(s, true, tx));
    app.space();
    assert_eq!(app.phase(), "paused · space to resume", "keys always act");
}

#[test]
fn the_activating_click_on_a_hover_control_does_nothing_either() {
    let mut app = App::new();
    app.rt.pointer_move(300.0, 510.0);
    app.rt.update(|s, tx| on_focus(s, false, tx));
    app.look();
    app.rt.update(|s, tx| on_focus(s, true, tx));
    app.tap("control-play");
    app.wait(1.0);
    assert_eq!(app.phase(), "focus · space to begin");
    app.tap("control-play");
    app.wait(1.0);
    assert_eq!(app.phase(), "focus");
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
    assert_eq!((back.phase(), back.time()), ("long rest · space when you're ready".into(), "15:00".into()));
    assert_eq!(back.lanterns(), 4, "the four are on the water, waiting to rise");
    app.wait(RING + 0.5);
    assert_eq!(app.phase(), "long rest · space when you're ready");
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
fn the_offer_to_name_a_block_shows_only_with_the_controls() {
    let mut app = App::new();
    let ink = |app: &App| {
        let e = app.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("top-text")).unwrap();
        e.text_color.map_or(1.0, |c| c.a) * e.opacity
    };
    assert!(ink(&app) < 0.01, "a hover offer is hidden while the pointer is away");
    app.rt.pointer_move(200.0, 300.0);
    app.wait(STEP);
    assert!(ink(&app) >= READABLE);
    assert_eq!(app.read("top-text").as_deref(), Some("+ name this block"));
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
