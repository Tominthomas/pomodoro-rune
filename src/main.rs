//! Pomodoro: one focus block is one sunset. The gap between the sun and the sea is the time
//! left; a finished block lights a lantern on the water, and four lanterns earn the night.
//! `cargo run --release` opens the window; `-- --snap out.png --scene focus-late` renders it
//! headless.

mod controls;
mod demo;
mod drawing;
mod lantern;
mod log;
mod readout;
mod scene;
mod store;
mod study;
mod theme;
mod timer;
mod today;

use std::cell::Cell;
use std::time::Duration;

use rune::prefs::Prefs;
use rune::time::{SystemTime, UNIX_EPOCH};
use rune::{
    AppHandle, AppSpec, Cx, Drag, El, Key, KeyEvent, Phase as DragPhase, TextEdit, Transition, Tx, WindowOptions, el,
};

use lantern::Lantern;
use log::{Block, Calendar, Day, Felt};
use scene::{Dawn, Scene, Sky};
use store::Store;
use study::Study;
use theme::{H, PLACES, Place, W};
use timer::{Clock, Effect, Event, Lengths, Phase, RING, Timer};

const APP_ID: &str = "dev.tomin.pomodoro";
/// Headless runs start on a fixed afternoon (Wednesday 11 November 2026, 14:00 UTC), so
/// scripts and renders read the same everywhere.
const HEADLESS_EPOCH: f64 = 1_794_405_600.0;
/// Fine motion (glitter, drift, bobbing) is drawn at this rate; the bell at full rate.
const MOTION_FPS: f64 = 20.0;
const BELL_FPS: f64 = 60.0;
/// A press that travels less than this is a tap, not a drag on the sun.
const SLOP: f32 = 4.0;
/// The hover controls stay this long after the pointer stops.
const CONTROLS_LINGER: f64 = 2.0;
/// A press this soon after the window comes forward is the click that brought it forward.
const ACTIVATING: f64 = 0.35;
/// A word given is acknowledged this long, then the question folds away.
const ACKNOWLEDGED: f64 = 2.0;
/// Left unanswered, the question fades this far into the break.
const ASK_FOR: f64 = 30.0;
/// Quitting waits this long at most for the last save.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

enum Page {
    Scene,
    Today(today::Nav),
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
    controls: bool,
    asking: Asking,
    day: Day,
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
    /// Today's card explaining lanterns, rings and knots, until it's put away.
    explain: bool,
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
    /// The press under way on the sky brought the window forward, so it does nothing else.
    activating: bool,
    visible: bool,
    /// When the last break ended, on the window's clock, and the sky it ended in: the next
    /// block's morning comes up from there.
    dawn_from: Option<(f64, Dawn)>,
    /// The system's Reduce Motion, as the last view saw it; the frame pacing reads it.
    reduced: Cell<bool>,
    store: Store,
    /// Why the last save failed or is slow, until one works again.
    save_error: Option<String>,
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
    let cal = Calendar { fixed_offset: headless.then_some(0) };
    restore(prefs, epoch, cal, |prefs| {
        if headless {
            Store::memory()
        } else {
            Store::open(prefs, move |e| handle.update(move |s: &mut State, _| s.save_error = e))
        }
    })
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
    let explain = !prefs.get::<bool>("explained").unwrap_or(false);
    let legacy: Vec<&str> = study::LEGACY.iter().copied().filter(|k| prefs.contains(k)).collect();
    let timer = study.resume(cal, now, lengths);
    let mut s = State::new(epoch, cal, study, timer, open(prefs));
    s.label = TextEdit::new(saved_label.clone());
    s.saved_label = saved_label;
    (s.hint, s.explain, s.place) = (hint, explain, place);
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
            explain: false,
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
            activating: false,
            visible: true,
            dawn_from: None,
            reduced: Cell::new(false),
            store,
            save_error: None,
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

    /// Whether the controls show: while the pointer moves over the window or rests on them, or
    /// after Tab.
    fn controls_shown(&self) -> bool {
        self.keyboard || self.over_controls || self.controls_until > self.now
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
        Timed { controls: self.controls_shown(), asking: self.asking(), day: self.today() }
    }

    /// The next moment `timed` can change by the clock alone, on the window's clock.
    fn next_change(&self) -> f64 {
        let mut at = vec![self.cal.next_day(self.unix()) as f64 - self.epoch];
        if self.controls_until > self.now {
            at.push(self.controls_until);
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

    /// How often the scene redraws on its own right now, if at all. A still scene costs nothing
    /// between the digits' once-a-second change.
    fn motion(&self) -> Option<f64> {
        if !self.visible || self.reduced.get() || matches!(self.page, Page::Today(_)) {
            return None;
        }
        if self.dawn().is_some() {
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
/// block goes into the record, a finished phase posts a notification when the window isn't in
/// front. The cycle then follows the timer, and the record is saved once if anything changed.
pub fn carry(s: &mut State, event: Event, tx: &mut Tx) {
    s.now = tx.clock();
    let rested = s.timer.progress(s.now);
    let mut changed = false;
    for effect in s.timer.step(event) {
        match effect {
            Effect::WakeAt(at) => tx.wake_at(at),
            Effect::Rang(phase) => notify(phase),
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
    if !rune_notify::available() || rune_notify::app_focused() {
        return;
    }
    let (title, body) = match finished {
        Phase::Focus => ("A lantern for that one", "The sun's down. Rest a while."),
        Phase::Short | Phase::Long => ("Break's over", "Press start when you're ready for the next sunset."),
    };
    rune_notify::post(&rune_notify::Notification::new("phase", title).body(body));
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    s.now = now;
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

fn set_place(s: &mut State, index: usize, tx: &mut Tx) {
    s.place = index % PLACES.len();
    tx.color_motion(rune::tok::motion::CALM);
    s.store.set("place", Place::at(s.place).name);
}

fn edit_label(s: &mut State, tx: &mut Tx) {
    s.editing = true;
    s.label.focused = true;
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

fn open_today(s: &mut State, open: bool) {
    s.page = if open { Page::Today(today::Nav::default()) } else { Page::Scene };
}

fn put_explanation_away(s: &mut State) {
    s.explain = false;
    s.store.set("explained", true);
}

/// Escape closes whatever is open, innermost first.
fn escape(s: &mut State) -> bool {
    if s.keys_open {
        s.keys_open = false;
    } else if let Page::Today(nav) = &mut s.page {
        if !nav.escape() {
            open_today(s, false);
        }
    } else if s.asking() != Asking::Hidden {
        s.reflect = None;
    } else if s.keyboard {
        s.keyboard = false;
    } else {
        return false;
    }
    true
}

fn on_key(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    let now = tx.clock();
    if e.key == Key::Escape {
        return escape(s);
    }
    if e.is_command('z') {
        carry(s, Event::Undo { now }, tx);
        return true;
    }
    if e.key == Key::Tab && tx.focused().is_none() && matches!(s.page, Page::Scene) && !s.keyboard {
        // Tab brings the controls out first, so focus never lands on something unseen.
        s.keyboard = true;
        tx.focus("control-play", true);
        return true;
    }
    let event = match &e.key {
        Key::Text(t) if t == " " => Event::Press { now },
        Key::Up | Key::Right => Event::Nudge { minutes: 1 },
        Key::Down | Key::Left => Event::Nudge { minutes: -1 },
        Key::Text(t) => match t.to_ascii_lowercase().as_str() {
            "s" => Event::Skip { now },
            "u" => Event::Undo { now },
            "h" => {
                open_today(s, matches!(s.page, Page::Scene));
                return true;
            }
            "t" => {
                set_place(s, s.place + 1, tx);
                return true;
            }
            "l" => {
                edit_label(s, tx);
                return true;
            }
            "?" | "/" => {
                s.keys_open = !s.keys_open;
                return true;
            }
            n @ ("1" | "2" | "3") if s.asking() == Asking::Open => {
                rate(s, Felt::ALL[n.parse::<usize>().unwrap_or(1) - 1], tx);
                return true;
            }
            _ => return false,
        },
        _ => return false,
    };
    carry(s, event, tx);
    true
}

/// A tap on the sky starts or pauses; dragging up or down while the block waits raises or
/// lowers the sun, which sets the block's length, and letting go keeps it, waiting for a press
/// as the arrow keys do. The click that brings the window forward does neither.
fn drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    let now = tx.clock();
    match d.phase {
        DragPhase::Down => {
            d.data = d.y;
            s.activating = s.activating_press(tx);
        }
        DragPhase::Move => {
            let scrubbing = matches!(s.timer.clock, Clock::Scrubbing { .. });
            if !scrubbing && (s.activating || d.dx.hypot(d.dy) < SLOP || s.timer.clock != Clock::Wound) {
                return;
            }
            let from = match s.timer.clock {
                Clock::Scrubbing { secs } => secs,
                _ => f64::from(s.timer.length()),
            };
            let to = scene::length_at(scene::rise(from) + f64::from(d.data - d.y));
            d.data = d.y;
            carry(s, Event::Scrub { secs: to - from }, tx);
        }
        DragPhase::Up if matches!(s.timer.clock, Clock::Scrubbing { .. }) => carry(s, Event::Release, tx),
        DragPhase::Up if !s.activating && !d.cancelled() && d.dx.hypot(d.dy) < SLOP => {
            carry(s, Event::Press { now }, tx);
        }
        DragPhase::Up => {}
    }
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    s.reduced.set(cx.is_reduced_motion());
    s.drawn.set(Some(s.timed()));
    let dark = cx.system_dark();
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
        length: s.timer.length(),
        drift: s.drift,
        water: &water,
        stars: &stars,
        held: matches!(s.timer.clock, Clock::Paused { .. }),
    };
    let root =
        el().size(W, H).clip_rounded(0.0).window_drag().on_pointer_move_local(|s: &mut State, _, tx: &mut Tx| {
            let shown = s.controls_shown();
            s.keyboard = false;
            s.controls_until = tx.clock() + CONTROLS_LINGER;
            tx.wake_at(s.controls_until);
            if shown {
                tx.live_only();
            }
        });
    let page = match &s.page {
        Page::Scene => {
            // One sky through every phase: each boundary is continuous, the bell easing into the
            // break it rings in and every break opening into a dawn, so nothing cross-fades.
            let sky = el()
                .key("sky")
                .absolute(0.0, 0.0)
                .size(W, H)
                .on_drag(drag)
                .child(scene::view(&scene).key("scene").absolute(0.0, 0.0));
            let k = theme::Ink::on(scene::ground(&scene));
            root.child(sky).child(readout::scrim(k)).child(readout::view(s, k)).child(controls::view(s, k))
        }
        Page::Today(nav) => {
            let day = nav.day(s.today());
            let ledger = today::Ledger {
                log: &s.study.log,
                cal: s.cal,
                now: s.unix(),
                day,
                place: Place::at(s.place),
                dark,
                nav,
                rename: &s.rename,
                explain: s.explain,
            };
            // The page sits on the place's own sky: a golden afternoon in light mode, blue hour in dark.
            let sky = if dark { Sky::BlueHour { progress: 0.4 } } else { Sky::Sunset { progress: 0.6, dawn: None } };
            let evening = scene::view(&Scene { sky, water: &[], stars: &[], held: false, length: 25 * 60, ..scene });
            root.child(today::backdrop(evening, Place::at(s.place), dark))
                .child(today::view(&ledger).key("ledger").transition(Transition::blur_replace()))
        }
    };
    if s.keys_open { page.child(controls::keys()) } else { page }
}

/// The window became the key window or stopped being it.
fn on_focus(s: &mut State, key: bool, tx: &mut Tx) {
    s.key_window = key;
    if key {
        s.activated_at = tx.clock();
    }
}

fn main() {
    rune::prewarm_gpu();
    let app = rune::App::new(AppSpec::<State> {
        scenes: demo::SCENES,
        fonts: &[include_bytes!("../assets/fonts/Inter.ttf")],
        families: ("Inter", "Inter"),
        poll: Some(poll),
        on_key: Some(on_key),
        min_size: Some((W, H)),
        ..AppSpec::new("Pomodoro", (W, H), || unreachable!("App::init builds the state"), view)
    });
    let handle = app.handle();
    app.init(move || init(handle))
        .window_options(WindowOptions { resizable: false, ..WindowOptions::default() })
        .on_visibility(|s: &mut State, shown, _| s.visible = shown)
        .on_focus(on_focus)
        .on_close(|s: &mut State, _| {
            if let Err(why) = s.store.flush(CLOSE_WAIT) {
                eprintln!("pomodoro: closing with the last save not written: {why}");
            }
        })
        .run();
}

#[cfg(test)]
mod tests;
