//! Today and History: a day as a strip of lanterns along the water, three plain numbers, how
//! it felt and what it was for; then four weeks of tiles, one a day, any of which opens that
//! day the same way. No streak that resets, no grades. A lantern opens its block, where its
//! word and its name can still be given or changed. The page sits on the chosen place's own
//! evening, light or dark as the system is.

use rune::shape::{LineCap, Shape};
use rune::tok::TextStyle;
use rune::{El, Key, KeyEvent, TextEdit, Tx, canvas, col, el, row, text, text_input};

use crate::drawing::{disc, ring};
use crate::lantern::{self, Lantern, Pose};
use crate::log::{Block, Calendar, Day, Felt, Log, Totals, duration};
use crate::readout::{HIT, button};
use crate::theme::{H, Ink, Place, W, rgb};
use crate::{Page, State, describe_block, guarded, open_today, put_explanation_away};

const PAD: f32 = 22.0;
const WIDE: f32 = W - 2.0 * PAD;
const TILE: (f32, f32) = (40.0, 18.0);
const GAP: f32 = 5.0;
/// Lanterns on the strip keep at least this far apart, however close their blocks ended.
const SPREAD: f32 = 20.0;
/// Focused minutes a History tile steps up at: none, then each of these or more.
const STEPS: [u32; 4] = [1, 60 * 60, 2 * 60 * 60, 3 * 60 * 60];
const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = [
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
/// What the strip's marks mean, said once and then on every mark's tooltip.
const LANTERN: &str = "a lantern is a block finished";
const RING: &str = "a ring is the minutes kept after letting go";
const KNOT: &str = "a knot is a pause";

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
    /// Whether the card explaining lanterns, rings and knots still shows.
    pub explain: bool,
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

    fn back_to_today(&mut self) {
        self.viewing = None;
        self.open = None;
    }

    fn pick_day(&mut self, day: Day, today: Day) {
        self.viewing = (day != today).then_some(day);
        self.hovered = Some(day);
        self.open = None;
    }

    fn hover(&mut self, day: Day, on: bool) {
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
fn nav(s: &mut State) -> Option<&mut Nav> {
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

impl Ledger<'_> {
    fn today(&self) -> Day {
        self.cal.day_of(self.now)
    }

    /// Light ink on the night, dark ink on a light day.
    fn ink(&self) -> Ink {
        if self.dark { Ink::light() } else { Ink::dark() }
    }
}

/// The view's content, drawn on the place's evening behind it.
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
        button(k, "today-close", "Close", "Back to the sunset  Esc").on_press_tx(guarded(|s, _| open_today(s, false))),
    );
    let head = row()
        .w(WIDE)
        .justify_between()
        .child(
            col()
                .gap(1.0)
                .child(label(k, &title, 21.0, 480.0, 0.97).key("today-title"))
                .child(label(k, &subtitle, 12.0, 450.0, 0.82).key("today-date")),
        )
        .child(actions.self_start());
    let mut page = col().key("today").pin(Some(PAD), Some(PAD - 2.0), Some(PAD - 4.0), None).child(head);
    let opened = l.nav.serial().and_then(|n| blocks.iter().find(|(m, _)| *m == n)).map(|(n, b)| (*n, *b));
    let under = match opened {
        Some((n, b)) => detail(l, n, b),
        None if l.explain && !blocks.is_empty() => explanation(k),
        None if blocks.is_empty() && l.day == today => {
            label(k, "No blocks yet today. The sea is calm.", 11.5, 450.0, 0.82).key("strip-note")
        }
        None if blocks.is_empty() => label(k, "No blocks this day.", 11.5, 450.0, 0.82).key("strip-note"),
        None => label(k, "pick a lantern or a ring for its details", 11.0, 450.0, 0.82).key("strip-note"),
    };
    page = page.child(el().h(2.0)).child(strip(l, &blocks)).child(el().h(2.0)).child(under).child(el().h(10.0));
    let mut numbers = row().gap(28.0).child(stat(k, "focused", &duration(totals.focused), "focused")).child(stat(
        k,
        "lanterns",
        &totals.lanterns.to_string(),
        if totals.lanterns == 1 { "lantern lit" } else { "lanterns lit" },
    ));
    if totals.let_go() > 0 {
        numbers = numbers.child(stat(k, "let-go", &totals.let_go().to_string(), "let go"));
    }
    page = page.child(numbers);
    let labels = Log::labels(blocks.iter().map(|(_, b)| *b));
    if !labels.is_empty() {
        page = page.child(el().h(10.0)).child(worked_on(k, &labels));
    }
    if totals.lanterns > 0 {
        let said = format!("you said how {} of {} felt", totals.reflected(), totals.lanterns);
        page = page.child(el().h(10.0)).child(
            row()
                .gap(8.0)
                .center_y()
                .child(label(k, "How the blocks felt", 12.0, 520.0, 0.92))
                .child(label(k, &said, 11.0, 450.0, 0.82).key("reflected")),
        );
        if totals.reflected() > 0 {
            page = page.child(el().h(6.0)).child(felt_bar(k, &totals));
        }
    }
    page.child(el().h(12.0)).child(history(l, today))
}

/// The marks explained under the strip the first times it shows them; put away for good with
/// "got it". After that every mark still says what it means under the pointer.
fn explanation(k: Ink) -> El {
    let words = format!("{}; {}; {} on its cord. Four lanterns bring the long rest.", cap(LANTERN), RING, KNOT);
    row()
        .key("explain")
        .w(WIDE)
        .gap(10.0)
        .items_center()
        .child(
            text(words)
                .key("explain-text")
                .font(TextStyle::new(11.0, 460.0))
                .color(k.text.alpha(0.9))
                .wrap()
                .w(WIDE - 74.0),
        )
        .child(
            button(k, "explain-dismiss", "got it", "Hide this explanation")
                .on_press_tx(guarded(|s, _| put_explanation_away(s))),
        )
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
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

/// When a block ran and for how long: "10:40 – 11:05 · 25 min", or "· let go, 9 min kept".
fn when(cal: Calendar, b: &Block) -> String {
    let (from, to) = (cal.local(b.started()), cal.local(b.ended));
    let span = format!("{}:{:02} – {}:{:02}", from.hour, from.minute, to.hour, to.minute);
    if b.finished() {
        format!("{span} · {}", duration(b.focused))
    } else {
        format!("{span} · let go, {} kept", duration(b.focused))
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
        .w(WIDE - HIT - 8.0)
        .child(label(k, &when(l.cal, b), 11.5, 520.0, 0.95).key("detail-when"))
        .child(
            label(k, &about(b).join(" · "), 11.0, 460.0, 0.86).key("detail-about").max_w(WIDE - HIT - 8.0).truncate(),
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
        .w(WIDE)
        .gap(6.0)
        .child(row().w(WIDE).justify_between().items_start().child(lines).child(close))
        .child(edit.child(name))
}

/// The day along a waterline, with each block where it ended: a lantern for one that ran to
/// the bell, a ring for one let go. Blocks that ended close together are spread apart. On
/// today, a labelled tick marks now.
fn strip(l: &Ledger<'_>, blocks: &[(u64, &Block)]) -> El {
    let k = l.ink();
    let h = 58.0;
    let base = 42.0;
    let hours = |b: &Block| l.cal.hour_of(b.ended);
    let from = blocks.iter().map(|(_, b)| hours(b).floor() - 1.0).fold(8.0f32, f32::min);
    let to = blocks.iter().map(|(_, b)| hours(b).ceil() + 1.0).fold(20.0f32, f32::max);
    let x_of = |hr: f32| (hr - from) / (to - from) * WIDE;

    let mut sh = Shape::new().line([0.0, base], [WIDE, base], 1.0, LineCap::Butt, k.text.alpha(0.3));
    let mut marks = el().w(WIDE).h(14.0);
    let now = (l.day == l.today()).then(|| x_of(l.cal.hour_of(l.now))).filter(|x| (0.0..=WIDE).contains(x));
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

    let mut xs: Vec<f32> = blocks.iter().map(|(_, b)| x_of(hours(b))).collect();
    for i in 1..xs.len() {
        xs[i] = xs[i].max(xs[i - 1] + SPREAD);
    }
    if let Some(over) = xs.last().map(|x| x - (WIDE - 12.0)).filter(|o| *o > 0.0) {
        for x in &mut xs {
            *x -= over;
        }
    }
    let mut art = el().key("day-strip").size(WIDE, h);
    let (mut lit, mut rings) = (0, 0);
    for (&(serial, b), x) in blocks.iter().zip(xs) {
        let x = x.round();
        let spoken = format!("{}, {}", when(l.cal, b), about(b).join(", "));
        let picked = l.nav.serial() == Some(serial);
        // Every mark takes a press over at least `HIT` square, and lights under the pointer.
        let target = |key: String, y: f32, meaning: &str| {
            el().key(key)
                .absolute(x - HIT / 2.0, y - HIT / 2.0)
                .size(HIT, HIT)
                .radius(HIT / 2.0)
                .hover(k.text.alpha(0.16))
                .fill(k.text.alpha(if picked { 0.16 } else { 0.0 }))
                .role("button")
                .label(spoken.clone())
                .help(format!("{}: {meaning}", when(l.cal, b)))
                .focusable(true)
                .focus_ring(1.5, 0.0, k.text.alpha(0.9))
                .on_press_tx(navigate(move |n| n.toggle(serial)))
        };
        if b.finished() {
            let pose = Pose { scale: 1.15, t: 0.0, seed: 0, bloom: 1.0, glow: 0.4, reflect: true, soft: 4.0 };
            let half = pose.reach() / 2.0;
            let y = base - 14.0 - (lit % 2) as f32 * 7.0;
            // The glow reaches far; only the target around the lantern takes the press.
            art = art.child(target(format!("day-lantern-{lit}"), y, LANTERN)).child(
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
            art = art.child(target(format!("day-ring-{rings}"), base - 2.0, RING));
            rings += 1;
        }
    }
    col().gap(2.0).child(art.child(canvas(sh).absolute(0.0, 0.0).size(WIDE, h).inert())).child(marks)
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

/// What the day's time went to, most first, wrapping onto a second line when it must; a very
/// long name is cut, not spilled, and says itself in full under the pointer.
fn worked_on(k: Ink, labels: &[(&str, u32)]) -> El {
    labels.iter().take(4).fold(row().key("worked-on").gap(12.0).flex_wrap().w(WIDE), |r, (name, secs)| {
        let time = duration(*secs);
        r.child(
            row()
                .key(format!("for-{name}"))
                .gap(5.0)
                .help(format!("{name}: {time}"))
                .child(label(k, name, 11.5, 500.0, 0.95).key(format!("for-name-{name}")).max_w(170.0).truncate())
                .child(label(k, &time, 11.5, 450.0, 0.82)),
        )
    })
}

/// How the lanterns the user described felt, each word named on its own part of the bar. A
/// part is never too narrow for its name. Lanterns left unanswered are in none of them.
fn felt_bar(k: Ink, t: &Totals) -> El {
    let parts: Vec<(Felt, u32)> = Felt::ALL.iter().copied().zip(t.felt).filter(|(_, n)| *n > 0).collect();
    let total: u32 = parts.iter().map(|(_, n)| n).sum();
    let gaps = 4.0 * parts.len().saturating_sub(1) as f32;
    let min = 74.0;
    let raw: Vec<f32> = parts.iter().map(|(_, n)| (WIDE - gaps) * *n as f32 / total as f32).collect();
    let short: f32 = raw.iter().map(|w| (min - w).max(0.0)).sum();
    let roomy: f32 = raw.iter().filter(|w| **w > min).sum();
    let widths = raw.iter().map(|w| if *w <= min { min } else { w - short * w / roomy });
    parts.iter().zip(widths).fold(row().key("felt").gap(4.0), |r, (&(felt, n), w)| {
        let tone = Lantern::light(Some(felt)).0;
        r.child(
            col()
                .key(format!("felt-part-{}", felt.word()))
                .w(w)
                .gap(4.0)
                .child(el().w_full().h(6.0).radius(3.0).fill(tone))
                .child(label(k, &format!("{n} {}", felt.word()), 11.0, 480.0, 0.9)),
        )
    })
}

/// "19 Oct – 15 Nov", the span History shows.
fn span(days: &[Day]) -> String {
    let (a, b) = (days[0].date(), days[days.len() - 1].date());
    format!("{} {} – {} {}", a.day, &MONTHS[a.month as usize - 1][..3], b.day, &MONTHS[b.month as usize - 1][..3])
}

/// How deep a tile is drawn, 0 to 4, by the day's focused time.
fn step(focused: u32) -> usize {
    STEPS.iter().filter(|s| focused >= **s).count()
}

/// Four weeks of tiles, Monday first. A tile is one channel, how long the day's focus ran, in
/// five steps of the place's own dusk; its dots are its lanterns, one each.
fn history(l: &Ledger<'_>, today: Day) -> El {
    let k = l.ink();
    let days: Vec<Day> = Log::weeks(today).collect();
    let with_focus = l.log.days_with_focus(l.cal, today);
    let aside = match l.nav.hovered {
        Some(day) => {
            let t = l.log.totals_on(l.cal, day);
            let d = day.date();
            let lit = if t.lanterns == 1 { "1 lantern".to_string() } else { format!("{} lanterns", t.lanterns) };
            format!(
                "{} {} {} · {} · {lit}",
                &DAYS[day.weekday() as usize][..3],
                d.day,
                &MONTHS[d.month as usize - 1][..3],
                duration(t.focused)
            )
        }
        None => format!("{with_focus} {} with focus", if with_focus == 1 { "day" } else { "days" }),
    };
    let head = DAYS
        .iter()
        .fold(row().gap(GAP), |r, d| r.child(el().w(TILE.0).center().child(label(k, &d[..1], 10.5, 500.0, 0.8))));
    let mut weeks = col().gap(GAP).focus_group().focus_follows();
    for week in days.chunks(7) {
        weeks = weeks.child(week.iter().fold(row().gap(GAP), |r, &day| r.child(tile(l, day, today))));
    }
    let scale = (0..5)
        .fold(row().key("history-scale").gap(3.0).center_y().child(label(k, "less", 10.5, 450.0, 0.8)), |r, i| {
            r.child(el().size(12.0, 10.0).radius(3.0).fill(depth(l, i)).border(1.0, k.text.alpha(0.18)))
        })
        .child(label(k, "more focus · a dot is a lantern", 10.5, 450.0, 0.8));
    col()
        .child(
            row()
                .w(WIDE)
                .justify_between()
                .center_y()
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(label(k, "Four weeks", 12.0, 520.0, 0.92))
                        .child(label(k, &span(&days), 10.5, 450.0, 0.8).key("history-span")),
                )
                .child(text(aside).key("evenings").font(TextStyle::new(11.0, 480.0)).color(k.text.alpha(0.9))),
        )
        .child(el().h(6.0))
        .child(col().key("history").gap(GAP).child(head).child(weeks))
        .child(el().h(7.0))
        .child(scale)
}

/// The tile colour for a step of focus: one ramp in the place's own sunset colour, from a
/// quiet empty tile to its deepest glow, in steps far enough apart to tell at a glance.
fn depth(l: &Ledger<'_>, i: usize) -> rune::Color {
    let glow = l.place.pal(1.0).glow;
    if l.dark {
        rgb(0x2a2e48).mix(glow, [0.0, 0.3, 0.55, 0.8, 1.0][i])
    } else {
        rgb(0xf1ebe3).mix(glow.mix(rgb(0x5a1a10), 0.3), [0.0, 0.3, 0.52, 0.75, 1.0][i])
    }
}

fn tile(l: &Ledger<'_>, day: Day, today: Day) -> El {
    let k = l.ink();
    let (tw, th) = TILE;
    let key = format!("day-{}", day.0);
    let d = day.date();
    let named = format!("{} {} {}", DAYS[day.weekday() as usize], d.day, MONTHS[d.month as usize - 1]);
    if day > today {
        // Days still to come are only an outline, unlike a past day with nothing in it.
        return el()
            .key(key)
            .size(tw, th)
            .radius(6.0)
            .border(1.0, k.text.alpha(0.25))
            .role("img")
            .label(format!("{named}, still to come"));
    }
    let t = l.log.totals_on(l.cal, day);
    // Dots are the day's lanterns exactly, in rows of eight.
    let mut sh = Shape::new();
    let dot = if l.dark || step(t.focused) >= 2 { rgb(0xfff8ec) } else { rgb(0x3a2018) };
    for i in 0..t.lanterns.min(16) {
        let (col, rowi) = ((i % 8) as f32, (i / 8) as f32);
        sh = disc(sh, [5.0 + col * 4.2, th - 4.5 - rowi * 4.0], [1.3, 1.3], dot.alpha(0.95));
    }
    let spoken = if t.blocks == 0 {
        format!("{named}: nothing")
    } else {
        format!("{named}: {} focused, {} lit", duration(t.focused), t.lanterns)
    };
    let shown = l.day == day;
    let base = el()
        .key(key)
        .size(tw, th)
        .radius(6.0)
        .clip_rounded(6.0)
        .fill(depth(l, step(t.focused)))
        .hover(k.text.alpha(0.18))
        .role("button")
        .label(spoken.clone())
        .help(spoken)
        .focusable(true)
        .focus_ring(1.5, 1.5, k.text.alpha(0.95))
        .on_hover_change(move |s: &mut State, on, _| {
            if let Some(n) = nav(s) {
                n.hover(day, on);
            }
        })
        .on_press_tx(guarded(move |s, tx| {
            let today = s.cal.day_of((s.epoch + tx.clock()) as i64);
            if let Some(n) = nav(s) {
                n.pick_day(day, today);
            }
        }))
        .child(canvas(sh).size(tw, th));
    match (shown, day == today) {
        (true, _) => base.border(2.0, k.text.alpha(0.95)),
        (false, true) => base.border(1.2, k.text.alpha(0.6)),
        _ => base.border(1.0, k.text.alpha(0.12)),
    }
}

fn label(k: Ink, s: &str, size: f32, weight: f32, a: f32) -> El {
    text(s.to_string()).font(TextStyle::new(size, weight)).color(k.text.alpha(a))
}

/// The page the Today view sits on: the place's own sky, blurred back under a veil of its own
/// colour, deep in dark mode and pale in light.
pub fn backdrop(scene: El, place: &Place, dark: bool) -> El {
    let (ground, veil) = if dark {
        (place.pal(1.45).zen.mix(rgb(0x05060f), 0.35), 0.55)
    } else {
        (place.pal(0.5).low.mix(rgb(0xfbf8f3), 0.72), 0.62)
    };
    el().size(W, H)
        .fill(ground)
        .child(scene.blur(14.0))
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(ground.alpha(veil)))
}
