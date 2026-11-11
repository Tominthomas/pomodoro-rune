//! History: four weeks of tiles, one a day, Monday first, any of which opens that day in Today.
//! No streak that resets: a count of days with focus in the window, a missed day one fewer.

use rune::shape::Shape;
use rune::tok::TextStyle;
use rune::{El, canvas, col, el, row, text};

use crate::drawing::disc;
use crate::log::{Day, Log, duration};
use crate::theme::rgb;
use crate::today::{DAYS, Ledger, MONTHS, WIDE, label, nav};
use crate::{State, guarded};

const TILE: (f32, f32) = (40.0, 18.0);
const GAP: f32 = 5.0;
/// Focused minutes a History tile steps up at: none, then each of these or more.
const STEPS: [u32; 4] = [1, 60 * 60, 2 * 60 * 60, 3 * 60 * 60];

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
pub fn view(l: &Ledger<'_>, today: Day) -> El {
    let k = l.ink();
    let days: Vec<Day> = Log::weeks(today).collect();
    let with_focus = l.log.days_with_focus(l.cal, today);
    let aside = match l.nav.hovered() {
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
            // Arrows press the tile they reach, and the day it opens would otherwise keep the
            // grid where it was.
            tx.scroll_into_view(format!("day-{}", day.0));
        }))
        .child(canvas(sh).size(tw, th));
    match (shown, day == today) {
        (true, _) => base.border(2.0, k.text.alpha(0.95)),
        (false, true) => base.border(1.2, k.text.alpha(0.6)),
        _ => base.border(1.0, k.text.alpha(0.12)),
    }
}
