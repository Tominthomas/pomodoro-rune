//! Today and History: a day as a strip of lanterns along the water, three plain numbers, how
//! it felt and what it was for; then four weeks of tiles, one a day, any of which opens that
//! day the same way. No streak that resets, no grades. A lantern opens its block, where its
//! word and its name can still be given or changed. The page sits on the chosen place's own
//! evening, light or dark as the system is.

use rune::shape::{LineCap, Shape};
use rune::tok::TextStyle;
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, canvas, col, el, row, text, text_input};

use crate::controls;
use crate::drawing::ring;
use crate::history;
use crate::lantern::{self, Lantern, Pose};
use crate::log::{Block, Calendar, Day, Felt, Log, Totals, duration};
use crate::readout::{HIT, button, mmss};
use crate::theme::{Frame, Ink, Place, rgb};
use crate::timer::{Clock, Event, Phase, Timer};
use crate::{Page, State, carry, describe_block, guarded, open_today};

const PAD: f32 = 22.0;
/// Today's column never runs wider than this, so a line stays easy to read across; a wide
/// window sets two columns side by side instead, the day beside the four weeks.
const MAX_WIDE: f32 = 460.0;
/// Two columns need this much width each, and this between them.
const SPLIT_AT: f32 = 340.0;
const GUTTER: f32 = 36.0;
/// Lanterns on the strip keep at least this far apart, however close their blocks ended: wider
/// than a target, so no two targets overlap.
const SPREAD: f32 = HIT + 2.0;
pub const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
/// The strip's viewport, which a crowded day scrolls sideways.
pub const STRIP: &str = "day-strip-view";

/// What Today and History read from: the log, and which day on the user's calendar.
pub struct Ledger<'a> {
    pub log: &'a Log,
    pub cal: Calendar,
    pub now: i64,
    /// The day shown: today, or one picked in History.
    pub day: Day,
    pub place: &'static Place,
    pub dark: bool,
    /// Which day, which tile under the pointer, which block open.
    pub nav: &'a Nav,
    /// The open block's label while it is being changed.
    pub rename: &'a TextEdit,
    /// What the last bell said, until the next phase starts.
    pub status: Option<El>,
    /// Why the record isn't saved, until a save works.
    pub unsaved: Option<El>,
    /// Whether to say that a mark opens its block: until one has been opened.
    pub cue: bool,
    /// How wide a column is, and whether the four weeks stand beside the day instead of under it.
    pub wide: f32,
    pub split: bool,
    /// The timer as it stands, and the window's clock to read it at.
    pub timer: &'a Timer,
    pub clock: f64,
}

/// Where Today is: the day it shows, the History tile under the pointer, and the block open.
/// Opening Today starts it afresh on today.
#[derive(Clone, Debug, Default)]
pub struct Nav {
    /// The History day shown; `None` is today.
    viewing: Option<Day>,
    /// The History day under the pointer or keyboard, whose exact numbers head the grid.
    hovered: Option<Day>,
    /// The block whose lantern was picked.
    open: Option<BlockDetail>,
}

/// A block whose lantern was picked, and whether its label is being changed. The field itself is
/// `State::rename`, which always exists: Rune writes to a field as focus leaves it, after the
/// handler that closed it has run.
#[derive(Clone, Copy, Debug)]
struct BlockDetail {
    serial: u64,
    renaming: bool,
}

impl Nav {
    /// The day shown, `today` unless one was picked in History.
    pub fn day(&self, today: Day) -> Day {
        self.viewing.unwrap_or(today)
    }

    /// Closes what is open inside Today, innermost first, and says whether anything was.
    pub fn escape(&mut self) -> bool {
        match &mut self.open {
            Some(d) if d.renaming => d.renaming = false,
            Some(_) => self.open = None,
            None if self.viewing.is_some() => self.viewing = None,
            None => return false,
        }
        true
    }

    /// The History day under the pointer or keyboard.
    pub fn hovered(&self) -> Option<Day> {
        self.hovered
    }

    fn back_to_today(&mut self) {
        self.viewing = None;
        self.open = None;
    }

    pub fn pick_day(&mut self, day: Day, today: Day) {
        self.viewing = (day != today).then_some(day);
        self.hovered = Some(day);
        self.open = None;
    }

    pub fn hover(&mut self, day: Day, on: bool) {
        self.hovered = if on { Some(day) } else { self.hovered.filter(|d| *d != day) };
    }

    /// Opens a block, or closes it if it is the one open.
    fn toggle(&mut self, serial: u64) {
        self.open = (self.serial() != Some(serial)).then_some(BlockDetail { serial, renaming: false });
    }

    fn serial(&self) -> Option<u64> {
        self.open.as_ref().map(|d| d.serial)
    }

    fn renaming(&mut self, on: bool) {
        if let Some(d) = &mut self.open {
            d.renaming = on;
        }
    }
}

/// Today's navigation, while Today is the page.
pub fn nav(s: &mut State) -> Option<&mut Nav> {
    match &mut s.page {
        Page::Today(nav) => Some(nav),
        Page::Scene => None,
    }
}

/// A press that moves around Today, through the guard every press goes through.
fn navigate(f: impl Fn(&mut Nav) + 'static) -> impl Fn(&mut State, &mut Tx) + 'static {
    guarded(move |s, _| {
        if let Some(n) = nav(s) {
            f(n);
        }
    })
}

/// Today's columns in a window `f`: their width, and whether there are two.
pub fn columns(f: Frame) -> (f32, bool) {
    let room = f.w - 2.0 * PAD;
    if room >= 2.0 * SPLIT_AT + GUTTER {
        (((room - GUTTER) / 2.0).min(MAX_WIDE), true)
    } else {
        (room.min(MAX_WIDE), false)
    }
}

impl Ledger<'_> {
    /// The width the timer and the day's name span: both columns when there are two.
    fn span(&self) -> f32 {
        if self.split { 2.0 * self.wide + GUTTER } else { self.wide }
    }

    pub fn today(&self) -> Day {
        self.cal.day_of(self.now)
    }

    /// Light ink on the night, dark ink on a light day.
    pub fn ink(&self) -> Ink {
        if self.dark { Ink::light() } else { Ink::dark() }
    }
}

/// The timer as it stands, held at the top of Today so time passing, or a pause, is never out of
/// sight: the phase and its time, the one press it takes now, and the bell's words until the next
/// start. A skipped block can still be taken back from here.
fn now_bar(l: &Ledger<'_>) -> El {
    let k = l.ink();
    let t = l.timer;
    let time = mmss(t.shown(l.clock));
    let words = match (&t.clock, t.phase) {
        (Clock::Wound | Clock::Scrubbing { .. }, Phase::Focus) => format!("Focus ready · {time}"),
        (Clock::Wound | Clock::Scrubbing { .. }, Phase::Short) => format!("Break ready · {time}"),
        (Clock::Wound | Clock::Scrubbing { .. }, Phase::Long) => format!("Long break ready · {time}"),
        (Clock::Ticking { .. }, Phase::Focus) => format!("Focus · {time} left"),
        (Clock::Ticking { .. }, Phase::Short) => format!("Break · {time} left"),
        (Clock::Ticking { .. }, Phase::Long) => format!("Long break · {time} left"),
        (Clock::Paused { .. }, _) => format!("Paused · {time} left"),
        (Clock::Ringing { kept: true, .. }, Phase::Focus) => "Focus complete".into(),
        (Clock::Ringing { .. }, Phase::Focus) => "Skipped".into(),
        (Clock::Ringing { .. }, _) => "Break over".into(),
    };
    let press = controls::press_words(t);
    let mut said = col().gap(3.0).shrink().child(label(k, &words, 13.0, 560.0, 0.97).key("timer-text"));
    if let Some(status) = l.status.clone() {
        said = said.child(status.wrap());
    }
    let mut acts = row().gap(6.0).self_start();
    if t.can_undo() {
        acts = acts.child(
            button(k, "timer-undo", "Undo skip", "Bring the block back  U")
                .on_press_tx(guarded(|s, tx| carry(s, Event::Undo { now: tx.clock() }, tx))),
        );
    }
    acts = acts.child(
        button(k, "timer-press", press, &format!("{press}  Space"))
            .on_press_tx(guarded(|s, tx| carry(s, Event::Press { now: tx.clock() }, tx))),
    );
    row()
        .key("timer-bar")
        .w(l.span())
        .min_h(44.0)
        .px(12.0)
        .py(8.0)
        .gap(10.0)
        .center_y()
        .justify_between()
        .radius(14.0)
        .fill(k.shade.alpha(0.42))
        .border(1.0, k.text.alpha(0.16))
        .child(said)
        .child(acts)
}

/// The view's content, drawn on the place's evening behind it: the timer and the day's name held
/// at the top, and under them the day and the four weeks, which scroll when they outgrow the window.
pub fn view(l: &Ledger<'_>) -> El {
    let k = l.ink();
    let today = l.today();
    let blocks: Vec<(u64, &Block)> = l.log.on(l.cal, l.day).collect();
    let totals = Totals::of(blocks.iter().map(|(_, b)| *b));
    let date = l.day.date();
    let weekday = DAYS[l.day.weekday() as usize];
    let (title, subtitle) = if l.day == today {
        ("Today".to_string(), format!("{weekday} {} {}", date.day, MONTHS[date.month as usize - 1]))
    } else {
        (weekday.to_string(), format!("{} {}, from History", date.day, MONTHS[date.month as usize - 1]))
    };
    let mut actions = row().gap(6.0);
    if l.day != today {
        actions = actions
            .child(button(k, "back-to-today", "Today", "Back to today").on_press_tx(navigate(Nav::back_to_today)));
    }
    actions = actions.child(
        button(k, "today-close", "Back to sunset", "Back to the sunset  Esc")
            .on_press_tx(guarded(|s, tx| open_today(s, false, tx))),
    );
    let head = row()
        .w(l.span())
        .justify_between()
        .flex_wrap()
        .gaps(8.0, 10.0)
        .child(
            col()
                .gap(1.0)
                .child(label(k, &title, 21.0, 480.0, 0.97).key("today-title"))
                .child(label(k, &subtitle, 12.0, 450.0, 0.82).key("today-date")),
        )
        .child(actions.self_start());
    let mut page =
        col().key("today").pin(Some(PAD), Some(PAD - 6.0), Some(PAD - 4.0), Some(0.0)).items_center().child(now_bar(l));
    if let Some(unsaved) = l.unsaved.clone() {
        page = page.child(el().h(8.0)).child(unsaved.w(l.span()));
    }
    let page = page.child(el().h(14.0)).child(head).child(el().h(6.0));
    let mut body = col().key("today-day").w(l.wide);
    let opened = l.nav.serial().and_then(|n| blocks.iter().find(|(m, _)| *m == n)).map(|(n, b)| (*n, *b));
    let under = match opened {
        Some((n, b)) => detail(l, n, b),
        None if blocks.is_empty() && l.day == today => {
            label(k, "No blocks yet today. The sea is calm.", 11.5, 450.0, 0.82).key("strip-note")
        }
        None if blocks.is_empty() => label(k, "No blocks this day.", 11.5, 450.0, 0.82).key("strip-note"),
        None if l.cue => col().gap(6.0).child(key_row(k, l.wide)).child(
            label(k, "Select a lantern or ring to open the block", 10.5, 460.0, 0.82)
                .key("strip-cue")
                .max_w(l.wide)
                .wrap(),
        ),
        None => key_row(k, l.wide),
    };
    body = body.child(el().h(2.0)).child(strip(l, &blocks)).child(under).child(el().h(14.0));
    let mut numbers =
        row().gaps(10.0, 28.0).flex_wrap().child(stat(k, "focused", &duration(totals.focused), "focus logged")).child(
            stat(
                k,
                "lanterns",
                &totals.lanterns.to_string(),
                if totals.lanterns == 1 { "lantern lit" } else { "lanterns lit" },
            ),
        );
    if totals.skipped() > 0 {
        numbers = numbers.child(stat(k, "skipped", &totals.skipped().to_string(), "skipped"));
    }
    body = body.child(numbers);
    let labels = Log::labels(blocks.iter().map(|(_, b)| *b));
    if !labels.is_empty() {
        body = body.child(el().h(14.0)).child(worked_on(l, &labels, totals.focused));
    }
    if totals.lanterns > 0 {
        body = body.child(el().h(14.0)).child(felt(k, &totals, l.wide));
    }
    let weeks = history::view(l, today);
    let body = if l.split {
        row().key("today-body").pb(PAD).gap(GUTTER).items_start().child(body).child(weeks)
    } else {
        col().key("today-body").pb(PAD).child(body.child(el().h(12.0)).child(weeks))
    };
    // Below the held top, the day scrolls in what is left of the window, and a tile or a lantern
    // reached by the keyboard scrolls into view.
    page.child(
        col()
            .key("today-scroll")
            .w_full()
            .items_center()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec { stick_bottom: false })
            .scroll_edge_fade(18.0)
            .scroll_margin(12.0)
            .child(body),
    )
}

/// What the strip's marks mean, in one row under it: a lantern, a ring and a knot, each drawn
/// as the strip draws it, beside its word.
fn key_row(k: Ink, wide: f32) -> El {
    let ink = k.text.alpha(0.82);
    let (lantern, _) = Lantern::light(Some(Felt::Deep));
    let mark = |key: &str, glyph: Shape, words: &str| {
        row()
            .key(key.to_string())
            .gap(5.0)
            .center_y()
            .child(canvas(glyph).size(14.0, 12.0).a11y_hidden())
            .child(label(k, words, 10.5, 460.0, 0.82))
    };
    let lit = Shape::new().fill(vec![[4.0, 2.5], [10.0, 2.5], [10.5, 9.5], [3.5, 9.5]], lantern).line(
        [5.5, 1.0],
        [8.5, 1.0],
        1.2,
        LineCap::Round,
        ink,
    );
    let skipped = Shape::new().stroke(ring([7.0, 7.0], [6.0, 2.0], 24, 0.0, std::f32::consts::TAU), 1.3, ink);
    let knot = Shape::new()
        .line([7.0, 0.5], [7.0, 11.5], 1.0, LineCap::Round, ink)
        .fill(ring([7.0, 6.0], [1.9, 1.9], 10, 0.0, std::f32::consts::TAU), ink);
    row()
        .key("strip-key")
        .w(wide)
        .gaps(6.0, 14.0)
        .flex_wrap()
        .center_y()
        .child(mark("key-lantern", lit, "a block finished"))
        .child(mark("key-ring", skipped, "skipped, minutes kept"))
        .child(mark("key-knot", knot, "a pause"))
}

/// "paused twice", as the detail and `VoiceOver` say it.
fn pauses(n: usize) -> String {
    match n {
        0 => "no pauses".into(),
        1 => "paused once".into(),
        2 => "paused twice".into(),
        n => format!("paused {n} times"),
    }
}

/// When a block ran and for how long: "10:40 – 11:05 · 25 min", or "· skipped, 9 min kept".
fn when(cal: Calendar, b: &Block) -> String {
    let (from, to) = (cal.local(b.started()), cal.local(b.ended));
    let span = format!("{}:{:02} – {}:{:02}", from.hour, from.minute, to.hour, to.minute);
    if b.finished() {
        format!("{span} · {}", duration(b.focused))
    } else {
        format!("{span} · skipped, {} kept", duration(b.focused))
    }
}

/// What else the record says: its name, its word, its pauses.
fn about(b: &Block) -> Vec<String> {
    let mut parts: Vec<String> = b.label.clone().into_iter().collect();
    if b.finished() {
        parts.push(b.felt().map_or("feeling not given".into(), |f| format!("felt {}", f.word())));
    }
    parts.push(pauses(b.pauses.len()));
    parts
}

/// The open block on two lines, with room kept for its close button, and under them its word
/// and its name, both changeable.
fn detail(l: &Ledger<'_>, serial: u64, b: &Block) -> El {
    let k = l.ink();
    let close = el()
        .key("detail-close")
        .size(HIT, HIT)
        .radius(12.0)
        .center()
        .role("button")
        .label("Close details")
        .help("Close  Esc")
        .hover(k.text.alpha(0.14))
        .focusable(true)
        .focus_ring(1.5, 0.0, k.text.alpha(0.9))
        .child(label(k, "×", 15.0, 500.0, 0.9))
        .on_press_tx(navigate(|n| n.open = None));
    let lines = col()
        .key("detail-text")
        .gap(2.0)
        .w(l.wide - HIT - 8.0)
        .child(label(k, &when(l.cal, b), 11.5, 520.0, 0.95).key("detail-when"))
        .child(
            label(k, &about(b).join(" · "), 11.0, 460.0, 0.86).key("detail-about").max_w(l.wide - HIT - 8.0).truncate(),
        );
    let mut edit = row().key("detail-edit").gap(5.0).center_y().flex_wrap();
    if b.finished() {
        for felt in Felt::ALL {
            let on = b.rated == Some(felt);
            let ink = if on { Ink { text: k.shade, shade: k.text } } else { k };
            edit = edit.child(
                row()
                    .key(format!("detail-felt-{}", felt.word()))
                    .min_h(HIT)
                    .center_y()
                    .px(9.0)
                    .radius(12.0)
                    .fill(if on { k.text.alpha(0.9) } else { k.shade.alpha(0.4) })
                    .border(1.0, k.text.alpha(0.4))
                    .hover(k.text.alpha(0.12))
                    .role("radio")
                    .label(felt.word())
                    .checked(on)
                    .focusable(true)
                    .focus_ring(1.5, 1.5, k.text.alpha(0.9))
                    .help(if on { "Clear this word".to_string() } else { format!("It felt {}", felt.word()) })
                    .on_press_tx(guarded(move |s, _| {
                        describe_block(s, serial, |b| b.rated = if b.rated == Some(felt) { None } else { Some(felt) });
                    }))
                    .child(label(ink, felt.word(), 11.0, 540.0, 0.97)),
            );
        }
    }
    let name = if l.nav.open.is_some_and(|d| d.renaming) {
        text_input(l.rename, TextStyle::new(11.5, 480.0))
            .key("detail-label-field")
            .label("Name for this block")
            .single_line()
            .w(150.0)
            .color(k.text.alpha(0.97))
            .placeholder("what was it for?", k.text.alpha(0.7))
            .on_edit(|s: &mut State| &mut s.rename)
            .on_input_key(|s: &mut State, e: &KeyEvent, tx: &mut Tx| {
                if e.key != Key::Escape {
                    return false;
                }
                if let Some(n) = nav(s) {
                    n.renaming(false);
                }
                tx.blur();
                true
            })
            .on_submit(move |s: &mut State, tx: &mut Tx| {
                let named = Some(s.rename.text.trim().to_string()).filter(|t| !t.is_empty());
                if let Some(n) = nav(s) {
                    n.renaming(false);
                }
                describe_block(s, serial, |b| b.label = named);
                tx.blur();
            })
    } else {
        button(k, "detail-rename", if b.label.is_some() { "rename" } else { "name it" }, "What this block was for")
            .on_press_tx(guarded(move |s, tx| {
                let current = s.study.log.block_mut(serial).and_then(|b| b.label.clone()).unwrap_or_default();
                s.rename = TextEdit::new(current);
                s.rename.focused = true;
                if let Some(n) = nav(s) {
                    n.renaming(true);
                }
                tx.focus("detail-label-field", false);
            }))
    };
    col()
        .key("detail")
        .w(l.wide)
        .gap(6.0)
        .child(row().w(l.wide).justify_between().items_start().child(lines).child(close))
        .child(edit.child(name))
}

/// The day along a waterline, with each block where it ended: a lantern for one that ran to
/// the bell, a ring for one skipped. Blocks that ended close together are spread apart, and a day
/// with more than the window holds runs on to the right, scrolled to its newest when Today
/// opens. On today, a labelled tick marks now.
fn strip(l: &Ledger<'_>, blocks: &[(u64, &Block)]) -> El {
    let k = l.ink();
    let h = 58.0;
    let base = 42.0;
    let hours = |b: &Block| l.cal.hour_of(b.ended);
    let from = blocks.iter().map(|(_, b)| hours(b).floor() - 1.0).fold(8.0f32, f32::min);
    let to = blocks.iter().map(|(_, b)| hours(b).ceil() + 1.0).fold(20.0f32, f32::max);
    let wide = l.wide.max(blocks.len() as f32 * SPREAD + 24.0);
    let x_of = |hr: f32| (hr - from) / (to - from) * wide;
    let mut xs: Vec<f32> = blocks.iter().map(|(_, b)| x_of(hours(b))).collect();
    for i in 1..xs.len() {
        xs[i] = xs[i].max(xs[i - 1] + SPREAD);
    }
    let wide = xs.last().map_or(wide, |x| wide.max(x + 14.0));

    let mut sh = Shape::new().line([0.0, base], [wide, base], 1.0, LineCap::Butt, k.text.alpha(0.3));
    let mut marks = el().w(wide).h(14.0);
    let now = (l.day == l.today()).then(|| x_of(l.cal.hour_of(l.now))).filter(|x| (0.0..=wide).contains(x));
    for hr in (from.ceil() as i32..=to.floor() as i32).filter(|h| h % 3 == 0) {
        let x = x_of(hr as f32);
        sh = sh.line([x, base + 4.0], [x, base + 8.0], 1.0, LineCap::Butt, k.text.alpha(0.5));
        if now.is_some_and(|n| (n - x).abs() < 26.0) {
            continue;
        }
        let name = match hr % 24 {
            0 => "midnight".to_string(),
            12 => "noon".to_string(),
            h => (if h > 12 { h - 12 } else { h }).to_string(),
        };
        marks = marks.child(el().absolute(x - 30.0, 0.0).w(60.0).center().child(label(k, &name, 10.5, 450.0, 0.8)));
    }
    if let Some(x) = now {
        sh = sh.line([x, base + 3.0], [x, base + 12.0], 1.4, LineCap::Round, k.text.alpha(0.9));
        marks = marks
            .child(el().key("now").absolute(x - 30.0, 0.0).w(60.0).center().child(label(k, "now", 10.5, 600.0, 0.95)));
    }

    let mut art = el().key("day-strip").size(wide, h);
    let (mut lit, mut rings) = (0, 0);
    for (&(serial, b), x) in blocks.iter().zip(xs) {
        let x = x.round();
        let spoken = format!("{}, {}", when(l.cal, b), about(b).join(", "));
        let picked = l.nav.serial() == Some(serial);
        // Every mark takes a press over at least `HIT` square, lights under the pointer, and says
        // that it opens its block, or closes it once open.
        let verb = if picked { "Close block" } else { "Open block" };
        let target = |key: String, y: f32| {
            el().key(key)
                .absolute(x - HIT / 2.0, y - HIT / 2.0)
                .size(HIT, HIT)
                .radius(HIT / 2.0)
                .hover(k.text.alpha(0.16))
                .fill(k.text.alpha(if picked { 0.16 } else { 0.0 }))
                .role("button")
                .label(format!("{verb}: {spoken}"))
                .help(format!("{verb}: {}", when(l.cal, b)))
                .focusable(true)
                .focus_ring(1.5, 0.0, k.text.alpha(0.9))
                .on_press_tx(guarded(move |s, _| {
                    if let Some(n) = nav(s) {
                        n.toggle(serial);
                    }
                    s.learned_marks();
                }))
        };
        if b.finished() {
            let pose = Pose { scale: 1.15, t: 0.0, seed: 0, bloom: 1.0, glow: 0.4, reflect: true, soft: 4.0 };
            let half = pose.reach() / 2.0;
            let y = base - 14.0 - (lit % 2) as f32 * 7.0;
            // The glow reaches far; only the target around the lantern takes the press.
            art = art.child(target(format!("day-lantern-{lit}"), y)).child(
                lantern::view(format!("day-lantern-art-{lit}"), String::new(), &Lantern::of(b), pose)
                    .absolute(x - half, y - half)
                    .inert()
                    .a11y_hidden(),
            );
            lit += 1;
        } else {
            sh = sh.stroke(
                ring([x, base - 2.0], [6.5, 2.0], 30, 0.0, std::f32::consts::TAU),
                1.4,
                k.text.alpha(if picked { 0.95 } else { 0.75 }),
            );
            art = art.child(target(format!("day-ring-{rings}"), base - 2.0));
            rings += 1;
        }
    }
    el().key(STRIP).w(l.wide).scroll_x().child(
        // Room under the hours, so the viewport never cuts the lanterns' reflections short.
        col()
            .w(wide)
            .gap(2.0)
            .pb(10.0)
            .child(art.child(canvas(sh).absolute(0.0, 0.0).size(wide, h).inert().a11y_hidden()))
            .child(marks),
    )
}

fn stat(k: Ink, key: &str, n: &str, what: &str) -> El {
    col()
        .key(format!("stat-{key}"))
        .gap(1.0)
        .child(
            text(n.to_string())
                .key(format!("stat-{key}-value"))
                .font(TextStyle { features: "'tnum' 1", ..TextStyle::new(22.0, 340.0) })
                .color(k.text.alpha(0.97)),
        )
        .child(label(k, what, 11.0, 450.0, 0.82))
}

/// What the day's time went to: the three names it went to most, then everything else as
/// "other", each a row with its time and a thin bar for its share of the day. A very long name is
/// cut, not spilled, and says itself in full under the pointer.
fn worked_on(l: &Ledger<'_>, labels: &[(&str, u32)], total: u32) -> El {
    let k = l.ink();
    let mut rows: Vec<(&str, u32)> = labels.iter().take(3).copied().collect();
    let other = total.saturating_sub(rows.iter().map(|(_, secs)| secs).sum());
    if other > 0 {
        rows.push(("other", other));
    }
    rows.into_iter().fold(col().key("worked-on").gap(9.0).w(l.wide), |c, (name, secs)| {
        let time = duration(secs);
        let share = secs as f32 / total.max(1) as f32;
        c.child(
            col()
                .key(format!("for-{name}"))
                .gap(4.0)
                .help(format!("{name}: {time}"))
                .child(
                    row()
                        .w(l.wide)
                        .justify_between()
                        .child(
                            label(k, name, 11.5, 500.0, 0.95)
                                .key(format!("for-name-{name}"))
                                .max_w(l.wide - 80.0)
                                .truncate(),
                        )
                        .child(label(k, &time, 11.5, 480.0, 0.95)),
                )
                .child(
                    el().w(l.wide)
                        .h(3.0)
                        .radius(1.5)
                        .fill(k.text.alpha(0.12))
                        .child(el().w(l.wide * share).h(3.0).radius(1.5).fill(k.text.alpha(0.62))),
                ),
        )
    })
}

/// How the day's lanterns felt, in plain words, each with the lantern's own tone beside it: "2 deep,
/// 1 steady, 1 not marked". A lantern left unanswered is counted as not marked, never guessed.
fn felt(k: Ink, t: &Totals, wide: f32) -> El {
    let unmarked = t.lanterns - t.reflected();
    let parts: Vec<(&str, u32, Option<Felt>)> = Felt::ALL
        .iter()
        .zip(t.felt)
        .map(|(f, n)| (f.word(), n, Some(*f)))
        .chain([("not marked", unmarked, None)])
        .filter(|(_, n, _)| *n > 0)
        .collect();
    let said = parts.iter().map(|(word, n, _)| format!("{n} {word}")).collect::<Vec<_>>().join(", ");
    let bar = parts.iter().fold(row().w(wide).gap(2.0), |r, &(_, n, felt)| {
        let w = (wide - 2.0 * (parts.len() - 1) as f32) * n as f32 / t.lanterns as f32;
        let part = el().w(w).h(5.0).radius(2.5);
        r.child(match felt {
            Some(f) => part.fill(Lantern::light(Some(f)).0),
            None => part.border(1.0, k.text.alpha(0.35)),
        })
    });
    let words = parts.iter().fold(row().gap(14.0).flex_wrap().a11y_hidden(), |r, &(word, n, felt)| {
        let dot = match felt {
            Some(f) => el().size(8.0, 8.0).radius(4.0).fill(Lantern::light(Some(f)).0),
            None => el().size(8.0, 8.0).radius(4.0).border(1.0, k.text.alpha(0.6)),
        };
        let key = format!("felt-part-{}", felt.map_or("unmarked", Felt::word));
        r.child(row().key(key).gap(5.0).center_y().child(dot).child(label(k, &format!("{n} {word}"), 11.0, 480.0, 0.9)))
    });
    col()
        .key("felt-said")
        .role("group")
        .label(format!("How the blocks felt: {said}"))
        .gap(6.0)
        .child(label(k, "How the blocks felt", 12.0, 520.0, 0.92))
        .child(bar)
        .child(words)
}

pub fn label(k: Ink, s: &str, size: f32, weight: f32, a: f32) -> El {
    text(s.to_string()).font(TextStyle::new(size, weight)).color(k.text.alpha(a))
}

/// The page Today sits on: the place's own sky, an afternoon in light mode and blue hour in dark,
/// drawn as its light alone, with no land or clouds to blur into shapes behind the words, and
/// softened toward paper or night so the words always read.
pub fn backdrop(place: &Place, dark: bool) -> El {
    let (p, ground, veil, glow) =
        if dark { (place.pal(1.42), rgb(0x05060f), 0.42, 0.22) } else { (place.pal(0.32), rgb(0xfbf8f3), 0.7, 0.3) };
    let tone = |c: rune::Color| c.mix(ground, veil);
    el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .a11y_hidden()
        .linear_gradient(std::f32::consts::PI, [(0.0, tone(p.zen)), (0.55, tone(p.mid)), (1.0, tone(p.low))])
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radial_gradient(
            [0.82, 0.12],
            [300.0, 240.0],
            [(0.0, p.glow.alpha(glow)), (0.6, p.glow.alpha(glow * 0.3)), (1.0, p.glow.alpha(0.0))],
        ))
}
