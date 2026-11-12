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

    /// The window resized to `w` by `h` points, as a person drags its corner, and drawn again.
    fn resize(&mut self, w: f32, h: f32) {
        self.rt.set_viewport((w, h), 1.0);
        self.look();
    }

    /// Where the element keyed `key` is drawn: `[x, y, w, h]` in window points.
    fn rect(&self, key: &str) -> [f32; 4] {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map_or_else(|| panic!("{key} is drawn"), |e| e.rect)
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

mod dock;
mod ledger;
mod sunset;
mod window;
