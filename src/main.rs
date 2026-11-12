//! Pomodoro: one focus block is one sunset. The gap between the sun and the sea is the time
//! left; a finished block lights a lantern on the water, and four lanterns earn the night.
//! `cargo run --release` opens the window; `-- --snap out.png --scene focus-late` renders it
//! headless.

mod bell;
mod controls;
mod demo;
mod drawing;
mod history;
mod input;
mod lantern;
mod log;
mod menu;
mod readout;
mod scene;
mod store;
mod study;
mod theme;
mod timer;
mod today;
mod tray;

use std::cell::{Cell, RefCell};
use std::time::Duration;

use rune::prefs::Prefs;
use rune::time::{SystemTime, UNIX_EPOCH};
use rune::{AppHandle, AppSpec, Cx, El, TextEdit, Transition, Tx, el};

use bell::Bells;
use lantern::Lantern;
use log::{Block, Calendar, Day, Felt};
use scene::{Dawn, Scene, Sky};
use store::Store;
use study::Study;
use theme::{Frame, H, PLACES, Place, W};
use timer::{Clock, Effect, Event, Lengths, Phase, RING, Timer};

const APP_ID: &str = "dev.tomin.pomodoro";
/// The name macOS saves the window's place and size under, between launches.
const FRAME: &str = "pomodoro";
/// Headless runs start on a fixed afternoon (Wednesday 11 November 2026, 14:00 UTC), so
/// scripts and renders read the same everywhere.
const HEADLESS_EPOCH: f64 = 1_794_405_600.0;
/// Fine motion (glitter, drift, bobbing) is drawn at this rate; the bell at full rate.
const MOTION_FPS: f64 = 20.0;
const BELL_FPS: f64 = 60.0;
/// A press that travels less than this is a tap, not a drag on the sun.
const SLOP: f32 = 4.0;
/// Once a hand lets go of the sun, it takes this long to ease back to the top.
const GLIDE: f64 = 0.45;
/// The hover controls stay this long after the pointer stops.
const CONTROLS_LINGER: f64 = 2.0;
/// A press this soon after the window comes forward is the click that brought it forward.
const ACTIVATING: f64 = 0.35;
/// A word given is acknowledged this long, then the question folds away.
const ACKNOWLEDGED: f64 = 2.0;
/// Left unanswered, the question fades this far into the break.
const ASK_FOR: f64 = 30.0;
/// How much bigger Larger Text draws the words and every control: the view's zoom.
const LARGER: f32 = 1.25;
/// Quitting waits this long at most for the last save.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

enum Page {
    Scene,
    Today(today::Nav),
}

/// A hand letting go of the sun at `at`, on the window's clock, `rise` points above the sea: it
/// eases back to the top from there.
#[derive(Clone, Copy, PartialEq, Debug)]
struct LetGo {
    at: f64,
    rise: f32,
}

/// How did that block feel: asked of one lantern, from its bell.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Reflect {
    /// The lantern's block.
    serial: u64,
    /// When the bell rang, on the window's clock.
    asked: f64,
    /// When a word was given.
    answered: Option<f64>,
}

/// What the reflection shows right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Asking {
    Hidden,
    Open,
    /// A word was just given; it is acknowledged before the question folds away.
    Noted(Felt),
}

/// Everything the view shows that changes with the clock alone, so `poll` can tell when the
/// window must redraw without a frame between.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Timed {
    beckoned: bool,
    asking: Asking,
    day: Day,
    gliding: bool,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent UI flags (focus, hover, open panels), not one state in disguise"
)]
pub struct State {
    timer: Timer,
    /// The window's clock as of the last frame or handler.
    now: f64,
    /// Unix seconds at window-clock zero.
    epoch: f64,
    cal: Calendar,
    /// The log and the cycle, saved together as one value.
    study: Study,
    /// What the next block is for; empty means unnamed.
    label: TextEdit,
    /// The label as last saved, for an edit that's cancelled.
    saved_label: String,
    editing: bool,
    /// The label of a block being changed in Today.
    rename: TextEdit,
    reflect: Option<Reflect>,
    /// The first-use hint for setting a block's length, until it's been used or put away.
    hint: bool,
    /// Today's cue that a mark opens its block, until one has been opened.
    cue_marks: bool,
    page: Page,
    /// The key overlay `?` opens.
    keys_open: bool,
    /// Index into `PLACES`.
    place: usize,
    /// Seconds of fine motion, advanced only while the scene moves.
    drift: f32,
    next_frame: f64,
    /// The hover controls show until this time.
    controls_until: f64,
    /// The pointer is over the controls' corner, where they stay out.
    over_controls: bool,
    /// Tab brought the controls out; they stay until the pointer moves.
    keyboard: bool,
    /// What the last view drew of the clock-driven state, so the moment it lapses redraws.
    drawn: Cell<Option<Timed>>,
    /// Whether the window is the key window, and when it last became it.
    key_window: bool,
    activated_at: f64,
    /// The press under way on the sky did its one job as it went down, bringing the window
    /// forward or putting the name field away, so it does nothing else.
    spent: bool,
    visible: bool,
    /// macOS has been asked to remember the window's frame; it can't be before the window exists.
    framed: bool,
    /// Larger Text is on: words and controls are drawn bigger, the scene as it is.
    larger: bool,
    /// "Always show time" is on: the digits stay in sight while a focus block runs.
    always_time: bool,
    /// Sound is on: the end of a block or a break rings a bell you hear, as well as see.
    sound: bool,
    sounds: Bells,
    /// The window's size in its own points, as the last view laid it out.
    window: Cell<(f32, f32)>,
    /// When the last break ended, on the window's clock, and the sky it ended in: the next
    /// block's morning comes up from there.
    dawn_from: Option<(f64, Dawn)>,
    /// How high the sun stood when a hand last took it up: the top, unless it was still easing
    /// back there. The hand moves it on from where it was.
    took_up: f32,
    /// When a hand last let go of the sun, and how high it held it.
    let_go: Option<LetGo>,
    /// The system's Reduce Motion, as the last view saw it; the frame pacing reads it.
    reduced: Cell<bool>,
    store: Store,
    /// Why the last save failed or is slow, until one works again.
    save_error: Option<String>,
    /// The phase whose bell rang last, until the next phase starts or the user moves on.
    rang: Option<Phase>,
    /// How many bells have rung, and how many of them a view has drawn as news: each is
    /// announced once, as it happens, never again when the page or the key card changes.
    bells: u32,
    heard: Cell<u32>,
    /// The running app, for its menu bar and its window, and the items the menu bar was last
    /// given. A test has none.
    app: Option<AppHandle<State>>,
    menu_shown: RefCell<Vec<menu::Item>>,
    /// The sun in the menu bar. Tests and headless runs have none.
    tray: Option<tray::Tray>,
    /// The first close has said the sun is in the menu bar now.
    told_tray: bool,
}

/// The first state, from what was saved. Headless runs keep their own settings and record in
/// memory, so a script never touches the real ones.
fn init(handle: AppHandle<State>) -> State {
    let headless = rune::tooling::headless_run().is_some();
    let prefs = if headless { Prefs::memory() } else { Prefs::open(APP_ID) };
    let epoch = if headless {
        HEADLESS_EPOCH
    } else {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
    };
    let cal = if headless { Calendar::UTC } else { Calendar::LOCAL };
    let app = handle.clone();
    if !headless {
        let menu = handle.clone();
        handle.update(move |s: &mut State, _| {
            s.tray = tray::Tray::new(menu);
            tray::sync(s);
        });
    }
    let mut s = restore(prefs, epoch, cal, |prefs| {
        if headless {
            Store::memory()
        } else {
            Store::open(prefs, move |e| handle.update(move |s: &mut State, _| s.save_error = e))
        }
    });
    s.app = Some(app);
    // A headless run has no window to remember.
    s.framed = headless;
    s
}

/// A window's state from what `prefs` holds, as of Unix `epoch`; `open` turns the prefs into the
/// store that saves from then on.
fn restore(prefs: Prefs, epoch: f64, cal: Calendar, open: impl FnOnce(Prefs) -> Store) -> State {
    let now = epoch as i64;
    let (mut study, mut rewrite) = Study::load(|k| prefs.get(k), cal, now);
    rewrite |= study.log.compact(cal, cal.day_of(now));
    let place = prefs.get::<String>("place").and_then(|name| PLACES.iter().position(|p| p.name == name)).unwrap_or(0);
    let lengths: Lengths = prefs.get("lengths").unwrap_or_default();
    let saved_label = prefs.get::<String>("label").unwrap_or_default();
    let hint = !prefs.get::<bool>("hinted").unwrap_or(false) && study.log.blocks.is_empty();
    let cue_marks = !prefs.get::<bool>("opened-block").unwrap_or(false);
    let larger = prefs.get::<bool>("larger-text").unwrap_or(false);
    let always_time = prefs.get::<bool>("always-show-time").unwrap_or(false);
    let told_tray = prefs.get::<bool>("told-tray").unwrap_or(false);
    let sound = prefs.get::<bool>("sound").unwrap_or(true);
    let legacy: Vec<&str> = study::LEGACY.iter().copied().filter(|k| prefs.contains(k)).collect();
    let timer = study.resume(cal, now, lengths);
    let mut s = State::new(epoch, cal, study, timer, open(prefs));
    s.label = TextEdit::new(saved_label.clone());
    s.saved_label = saved_label;
    (s.hint, s.cue_marks, s.place, s.larger, s.always_time, s.sound, s.told_tray) =
        (hint, cue_marks, place, larger, always_time, sound, told_tray);
    // The new key is written before the old ones go, so a crash between finds it and ignores them.
    if rewrite {
        s.save();
    }
    for key in legacy {
        s.store.remove(key);
    }
    s
}

impl State {
    /// A window's state before it has shown anything: settings at their defaults.
    fn new(epoch: f64, cal: Calendar, study: Study, timer: Timer, store: Store) -> State {
        State {
            timer,
            now: 0.0,
            epoch,
            cal,
            study,
            label: TextEdit::new(String::new()),
            saved_label: String::new(),
            editing: false,
            rename: TextEdit::new(String::new()),
            reflect: None,
            hint: false,
            cue_marks: false,
            page: Page::Scene,
            keys_open: false,
            place: 0,
            drift: 0.0,
            next_frame: 0.0,
            controls_until: f64::NEG_INFINITY,
            over_controls: false,
            keyboard: false,
            drawn: Cell::new(None),
            key_window: true,
            activated_at: f64::NEG_INFINITY,
            spent: false,
            visible: true,
            framed: false,
            larger: false,
            always_time: false,
            sound: true,
            sounds: Bells::new(),
            window: Cell::new((W, H)),
            dawn_from: None,
            took_up: scene::RISE,
            let_go: None,
            reduced: Cell::new(false),
            store,
            save_error: None,
            rang: None,
            bells: 0,
            heard: Cell::new(0),
            app: None,
            menu_shown: RefCell::new(Vec::new()),
            tray: None,
            told_tray: false,
        }
    }

    fn unix(&self) -> i64 {
        (self.epoch + self.now) as i64
    }

    fn today(&self) -> Day {
        self.cal.day_of(self.unix())
    }

    fn label(&self) -> Option<String> {
        Some(self.label.text.trim().to_string()).filter(|l| !l.is_empty())
    }

    /// The lanterns on this cycle's water, and today's earlier ones, now stars.
    fn lanterns(&self) -> (Vec<Lantern>, Vec<Lantern>) {
        let (stars, water) = self.study.log.lanterns(self.cal, self.today(), self.study.cycle.from);
        let looks = |blocks: Vec<&Block>| blocks.into_iter().map(Lantern::of).collect();
        (looks(water), looks(stars))
    }

    /// Whether the controls show: always while a block or a break waits to start, so the first
    /// thing to do is in sight, and once it runs, only while beckoned.
    fn controls_shown(&self) -> bool {
        self.timer.clock == Clock::Wound || self.beckoned()
    }

    /// Whether the pointer or the keyboard has called the hover controls out: the pointer moves
    /// over the window or rests on them, or Tab was pressed.
    fn beckoned(&self) -> bool {
        self.keyboard || self.over_controls || self.controls_until > self.now
    }

    /// Whether the first-use hint is out: before the first block, until it is put away.
    pub fn teaching(&self) -> bool {
        self.hint
            && self.study.log.blocks.is_empty()
            && self.timer.phase == Phase::Focus
            && self.timer.clock == Clock::Wound
            && self.timer.setting.is_none()
    }

    /// What the reflection shows now: open from the bell until a word is given (then noted
    /// briefly), it is put away, the next block starts, or the break has gone on a while.
    pub fn asking(&self) -> Asking {
        let Some(r) = self.reflect else { return Asking::Hidden };
        match r.answered {
            Some(at) if self.now < at + ACKNOWLEDGED => self
                .study
                .log
                .numbered()
                .find(|(n, _)| *n == r.serial)
                .and_then(|(_, b)| b.felt())
                .map_or(Asking::Hidden, Asking::Noted),
            None if self.now < r.asked + RING + ASK_FOR => Asking::Open,
            _ => Asking::Hidden,
        }
    }

    fn timed(&self) -> Timed {
        Timed { beckoned: self.beckoned(), asking: self.asking(), day: self.today(), gliding: self.gliding() }
    }

    /// The next moment the window must wake by the clock alone, on the window's clock: when
    /// `timed` can change, or the timer has a deadline.
    fn next_change(&self) -> f64 {
        let mut at = vec![self.cal.next_day(self.unix()) as f64 - self.epoch];
        at.extend(self.timer.due());
        if self.controls_until > self.now {
            at.push(self.controls_until);
        }
        if let Some(let_go) = self.let_go {
            at.push(let_go.at + GLIDE);
        }
        if let Some(r) = self.reflect {
            at.push(r.answered.map_or(r.asked + RING + ASK_FOR, |a| a + ACKNOWLEDGED));
        }
        at.into_iter().filter(|t| *t > self.now).fold(f64::INFINITY, f64::min)
    }

    /// The dawn under way after a break, if it hasn't finished.
    fn dawn(&self) -> Option<Dawn> {
        let (at, d) = self.dawn_from?;
        let k = (self.now - at) as f32 / Dawn::secs(d.night);
        (self.timer.phase == Phase::Focus && k < 1.0).then_some(Dawn { k: k.max(0.0), ..d })
    }

    /// How high the sun's path starts, in points above the sea: the top for every block. While a
    /// hand sets the waiting block, the sun rides as far above or below where it was taken up as
    /// the hand has moved the length, and once let go it eases back to the top.
    fn sun_rise(&self) -> f32 {
        let t = &self.timer;
        let from_top = |secs: f64| (scene::rise(secs) - scene::rise(f64::from(t.lengths.of(Phase::Focus)))) as f32;
        match (t.setting, self.let_go) {
            (Some(setting), _) if t.phase == Phase::Focus => self.took_up + from_top(setting.secs),
            (_, Some(LetGo { at, rise })) if self.gliding() => {
                rise + (scene::RISE - rise) * scene::ease(((self.now - at) / GLIDE) as f32)
            }
            _ => scene::RISE,
        }
    }

    /// Whether the sun is still easing back to the top after a hand let go of it.
    fn gliding(&self) -> bool {
        self.let_go.is_some_and(|let_go| self.now - let_go.at < GLIDE) && !self.reduced.get()
    }

    /// How often the scene redraws on its own right now, if at all. A still scene costs nothing
    /// between the digits' once-a-second change.
    fn motion(&self) -> Option<f64> {
        if !self.visible || self.reduced.get() || matches!(self.page, Page::Today(_)) {
            return None;
        }
        if self.dawn().is_some() || self.gliding() {
            return Some(BELL_FPS);
        }
        match self.timer.clock {
            Clock::Ringing { .. } => Some(BELL_FPS),
            Clock::Ticking { .. } => Some(MOTION_FPS),
            _ => None,
        }
    }

    /// Queues a copy of the whole record for the writer thread, which encodes and writes it as
    /// one value. Only a block ending, being described or taken back, or the cycle moving on calls
    /// this, never a frame.
    fn save(&self) {
        self.store.set(study::KEY, self.study.clone());
    }

    /// How much the view is zoomed: Larger Text, or not.
    fn zoom(&self) -> f32 {
        if self.larger { LARGER } else { 1.0 }
    }

    /// A block was opened from Today, so its marks no longer need the cue.
    fn learned_marks(&mut self) {
        if self.cue_marks {
            self.cue_marks = false;
            self.store.set("opened-block", true);
        }
    }

    fn put_hint_away(&mut self) {
        if self.hint {
            self.hint = false;
            self.store.set("hinted", true);
        }
    }

    /// Whether a pointer press should do nothing because it is the click that brought the window
    /// forward. Keys always act.
    fn activating_press(&self, tx: &Tx) -> bool {
        !tx.by_keys() && (!self.key_window || tx.clock() - self.activated_at < ACTIVATING)
    }
}

/// A pointer action that the click bringing the window forward never runs. Every control,
/// button and chip goes through it; keys run as usual.
pub fn guarded(f: impl Fn(&mut State, &mut Tx) + 'static) -> impl Fn(&mut State, &mut Tx) + 'static {
    move |s, tx| {
        if !s.activating_press(tx) {
            f(s, tx);
        }
    }
}

/// Steps the timer and carries its effects out: wakes become frame deadlines, a finished
/// block goes into the record, a finished phase rings the bell (when Sound is on) and posts a
/// notification when the window isn't in front. The cycle then follows the timer, and the record
/// is saved once if anything changed.
pub fn carry(s: &mut State, event: Event, tx: &mut Tx) {
    s.now = tx.clock();
    let rested = s.timer.progress(s.now);
    let mut changed = false;
    for effect in s.timer.step(event) {
        match effect {
            Effect::WakeAt(at) => tx.wake_at(at),
            Effect::Rang(phase) => {
                if s.sound {
                    s.sounds.ring(phase);
                }
                notify(phase);
                s.rang = Some(phase);
                s.bells += 1;
            }
            Effect::Ended(ended) => {
                let finished = ended.outcome == timer::Outcome::Finished;
                let serial = s.study.record(ended, s.unix(), s.label());
                s.reflect = finished.then_some(Reflect { serial, asked: s.now, answered: None });
                changed = true;
            }
            Effect::Unended => {
                s.study.unrecord();
                changed = true;
            }
            Effect::Lengths(lengths) => {
                s.store.set("lengths", lengths);
                s.put_hint_away();
            }
            Effect::Advanced { from: from @ (Phase::Short | Phase::Long), to: Phase::Focus } => {
                s.dawn_from = Some((s.now, Dawn { k: 0.0, night: from == Phase::Long, from: rested }));
            }
            Effect::Advanced { .. } => {}
        }
    }
    if matches!(event, Event::Skip { .. } | Event::Undo { .. }) || matches!(s.timer.clock, Clock::Ticking { .. }) {
        s.rang = None;
    }
    changed |= s.study.settle(&s.timer);
    if changed {
        s.save();
    }
    if s.timer.phase == Phase::Focus && matches!(s.timer.clock, Clock::Ticking { .. }) {
        s.reflect = None;
        s.put_hint_away();
    }
    tx.wake_at(s.next_change());
}

fn notify(finished: Phase) {
    if !rune_mac::notify::available() || rune_mac::notify::app_focused() {
        return;
    }
    let (title, body) = match finished {
        Phase::Focus => ("A lantern for that one", "The sun's down. Rest a while."),
        Phase::Short | Phase::Long => ("Break's over", "Press start when you're ready for the next sunset."),
    };
    rune_mac::notify::post(&rune_mac::notify::Notification::new("phase", title).body(body));
}

/// Waits a little for the last save, as the window closes or the app quits from the menu bar
/// with the window already closed.
fn flush(s: &State) {
    if let Err(why) = s.store.flush(CLOSE_WAIT) {
        eprintln!("pomodoro: closing with the last save not written: {why}");
    }
}

/// The first time the window closes, says once where the block went: closing hides the window,
/// and the sun in the menu bar keeps the time. It waits for a close that can say it.
fn tell_tray(s: &mut State) {
    if s.told_tray || s.tray.is_none() || !rune_mac::notify::available() {
        return;
    }
    rune_mac::notify::post(
        &rune_mac::notify::Notification::new("tray", "The sun's in the menu bar now")
            .body("Pomodoro keeps the time up there while the window is closed. Click the sun to open it again."),
    );
    s.told_tray = true;
    s.store.set("told-tray", true);
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    s.now = now;
    if !s.framed {
        s.framed = rune_mac::remember_frame(FRAME);
    }
    let mut changed = false;
    if let Some(timer) = s.study.roll_over(s.cal, s.unix(), &s.timer) {
        s.timer = timer;
        s.save();
        changed = true;
    }
    if s.timer.due().is_some_and(|at| now >= at) {
        carry(s, Event::Due { now }, tx);
        changed = true;
    }
    if s.drawn.get() != Some(s.timed()) {
        changed = true;
    }
    tx.wake_at(s.next_change());
    // A bell can ring with the window hidden, where no view runs; the menus still follow it.
    menu::sync(s);
    tray::sync(s);
    if let Some(fps) = s.motion() {
        if now >= s.next_frame {
            s.drift += (1.0 / fps) as f32;
            s.next_frame = now + 1.0 / fps;
            changed = true;
        }
        tx.wake_at(s.next_frame);
    }
    changed
}

/// Gives the lantern the reflection asks about the user's word.
fn rate(s: &mut State, felt: Felt, tx: &mut Tx) {
    let Some(r) = s.reflect.filter(|_| s.asking() != Asking::Hidden) else { return };
    if let Some(b) = s.study.log.block_mut(r.serial) {
        b.rated = Some(felt);
        s.reflect = Some(Reflect { answered: Some(tx.clock()), ..r });
        s.save();
        tx.wake_at(tx.clock() + ACKNOWLEDGED);
    }
}

/// Changes what a recorded block says about itself, from its detail in Today.
fn describe_block(s: &mut State, serial: u64, change: impl FnOnce(&mut Block)) {
    if let Some(b) = s.study.log.block_mut(serial) {
        change(b);
        s.save();
    }
}

/// Turns Larger Text on or off. On, a window too small to hold the bigger words grows until it
/// does; it can still be made smaller again, and the words then wrap.
fn set_larger(s: &mut State, on: bool, tx: &mut Tx) {
    s.larger = on;
    s.store.set("larger-text", on);
    let (w, h) = s.window.get();
    let fits = (W * LARGER, H * LARGER);
    if on && (w < fits.0 || h < fits.1) {
        tx.resize_window((w.max(fits.0), h.max(fits.1)));
    }
}

fn set_always_time(s: &mut State, on: bool) {
    s.always_time = on;
    s.store.set("always-show-time", on);
}

/// Turns the bell's sound on or off. The ripples on the water ring either way.
fn set_sound(s: &mut State, on: bool) {
    s.sound = on;
    s.store.set("sound", on);
}

fn set_place(s: &mut State, index: usize, tx: &mut Tx) {
    s.place = index % PLACES.len();
    tx.color_motion(rune::tok::motion::CALM);
    s.store.set("place", Place::at(s.place).name);
}

/// Opens the field for the next block's name. It lives under the time on the sunset, so Today
/// and the key card close first: a field that isn't drawn can't take the keys.
fn edit_label(s: &mut State, tx: &mut Tx) {
    s.keys_open = false;
    open_today(s, false, tx);
    s.editing = true;
    s.label.focused = true;
    // The name it has is selected, so typing replaces it and an arrow keeps it.
    s.label.set_selection(0, s.label.text.len());
    tx.focus("label-field", false);
}

fn finish_label(s: &mut State, keep: bool, tx: &mut Tx) {
    if keep {
        s.saved_label = s.label().unwrap_or_default();
        s.store.set("label", s.saved_label.clone());
    }
    s.label.replace_text(s.saved_label.clone());
    s.editing = false;
    tx.blur();
}

/// Opens Today on today, its strip scrolled to the newest block, or goes back to the sunset.
fn open_today(s: &mut State, open: bool, tx: &mut Tx) {
    s.page = if open { Page::Today(today::Nav::default()) } else { Page::Scene };
    if open {
        tx.scroll_to_x(today::STRIP, f32::MAX);
    }
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    s.reduced.set(cx.is_reduced_motion());
    s.drawn.set(Some(s.timed()));
    menu::sync(s);
    tray::sync(s);
    let news = s.heard.replace(s.bells) != s.bells;
    let dark = cx.system_dark();
    // The window as the last layout found it; before the first, the size it opens at.
    // Larger Text zooms the whole view; the scene alone is drawn back at the window's own scale.
    let zoom = s.zoom();
    cx.zoom(zoom);
    let frame = cx.layout_size("window").map_or(Frame::of(W / zoom, H / zoom), |[w, h]| Frame::of(w, h));
    s.window.set((frame.w * zoom, frame.h * zoom));
    let (water, stars) = s.lanterns();
    // Reduce Motion: a bell or a dawn shows only where it ends.
    let mut sky = Sky::of(&s.timer, s.now, s.dawn().filter(|_| !s.reduced.get()));
    if let Sky::Bell { since, .. } = &mut sky
        && s.reduced.get()
    {
        *since = RING as f32;
    }
    let scene = Scene {
        sky,
        place: Place::at(s.place),
        dark,
        rise: s.sun_rise(),
        drift: s.drift,
        water: &water,
        stars: &stars,
        held: matches!(s.timer.clock, Clock::Paused { .. }),
        frame: Frame::of(frame.w * zoom, frame.h * zoom),
    };
    let root = el().key("window").w_full().h_full().clip_rounded(0.0).window_drag().on_pointer_move_local(
        |s: &mut State, _, tx: &mut Tx| {
            let shown = s.beckoned();
            s.keyboard = false;
            s.controls_until = tx.clock() + CONTROLS_LINGER;
            tx.wake_at(s.controls_until);
            if shown {
                tx.live_only();
            }
        },
    );
    let page = match &s.page {
        Page::Scene => {
            // One sky through every phase: each boundary is continuous, the bell easing into the
            // break it rings in and every break opening into a dawn, so nothing cross-fades.
            let sky = el()
                .key("sky")
                .role("none")
                .absolute(0.0, 0.0)
                .size(frame.w, frame.h)
                .on_drag(input::drag)
                .child(scene::view(&scene).key("scene").absolute(0.0, 0.0).origin(0.0, 0.0).scale(1.0 / zoom));
            let k = theme::Ink::on(scene::ground(&scene));
            // The phase line already says it on the scene, so here the status is only heard.
            let status =
                readout::status(s, k, news).filter(|_| !s.keys_open).map(|e| e.opacity(1.0).color(k.text.alpha(0.0)));
            let root = root
                .child(sky)
                .child(readout::scrim(k, frame))
                .child(readout::view(s, k, frame))
                .child(controls::view(s, k, frame));
            match status {
                Some(e) => root.child(e.absolute(24.0, frame.h - readout::top(frame))),
                None => root,
            }
        }
        Page::Today(nav) => {
            let day = nav.day(s.today());
            let ink = if dark { theme::Ink::light() } else { theme::Ink::dark() };
            let (wide, split) = today::columns(frame);
            let ledger = today::Ledger {
                log: &s.study.log,
                cal: s.cal,
                now: s.unix(),
                day,
                place: Place::at(s.place),
                dark,
                nav,
                rename: &s.rename,
                status: readout::status(s, ink, news).filter(|_| !s.keys_open),
                unsaved: readout::save_status(s, ink),
                cue: s.cue_marks,
                wide,
                split,
                timer: &s.timer,
                clock: s.now,
            };
            root.child(today::backdrop(Place::at(s.place), dark))
                .child(today::view(&ledger).key("ledger").transition(Transition::blur_replace()))
        }
    };
    if s.keys_open { page.child(controls::keys(readout::status(s, theme::Ink::light(), news), frame)) } else { page }
}

/// The window became the key window or stopped being it.
fn on_focus(s: &mut State, key: bool, tx: &mut Tx) {
    s.key_window = key;
    if key {
        s.activated_at = tx.clock();
    }
}

/// The size the window opens at: the size it last had, so it doesn't open small and then grow.
/// A headless run never reads it and renders at the size it is given. A saved size below the
/// smallest window is one the window can't have, so it is dropped.
fn opening_size(headless: bool, saved: impl FnOnce() -> Option<(f32, f32)>) -> (f32, f32) {
    if headless {
        return (W, H);
    }
    saved().filter(|&(w, h)| w >= W && h >= H).unwrap_or((W, H))
}

fn main() {
    rune::prewarm_gpu();
    let size = opening_size(rune::is_headless(), || rune_mac::saved_size(FRAME));
    let app = rune::App::new(AppSpec::<State> {
        scenes: demo::SCENES,
        fonts: &[include_bytes!("../assets/fonts/Inter.ttf")],
        families: ("Inter", "Inter"),
        poll: Some(poll),
        on_key: Some(input::on_key),
        min_size: Some((W, H)),
        ..AppSpec::new("Pomodoro", size, || unreachable!("App::init builds the state"), view)
    });
    let handle = app.handle();
    app.init(move || init(handle))
        .on_visibility(|s: &mut State, shown, _| s.visible = shown)
        .on_focus(on_focus)
        .keep_running()
        .on_close(|s: &mut State, _| {
            if !rune::quitting() {
                tell_tray(s);
            }
            flush(s);
        })
        .on_quit(|s: &mut State, _| flush(s))
        .run();
}

#[cfg(test)]
mod tests;
