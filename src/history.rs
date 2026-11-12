//! History: four weeks of days, one a cell, Monday first, any of which opens that day in Today.
//! No streak that resets: a count of days with focus in the window, a missed day one fewer.

use std::f32::consts::PI;

use rune::shape::Shape;
use rune::tok::TextStyle;
use rune::{Color, El, canvas, col, el, row, text};

use crate::drawing::{disc, halo};
use crate::log::{Day, Log, duration};
use crate::theme::rgb;
use crate::today::{DAYS, Ledger, MONTHS, label, nav};
use crate::{State, guarded};

const GAP: f32 = 5.0;
/// Narrower than this, a hovered day's summary doesn't fit beside "Four weeks" and its dates, so
/// it goes under them, hovered or not, and the grid doesn't jump as the pointer moves.
const BESIDE: f32 = 320.0;
/// A day's sunset fills the width, seven to a week.
/// A day's tile across a week of `wide` points, and its height.
fn tile(wide: f32) -> (f32, f32) {
    ((wide - 6.0 * GAP) / 7.0, 22.0 * (wide / 336.0).clamp(1.0, 1.3))
}
/// Where the sea meets the sky in a tile, from its top.
const SHORE: f32 = 0.62;
/// Focused minutes a History tile steps up at: none, then each of these or more.
const STEPS: [u32; 4] = [1, 60 * 60, 2 * 60 * 60, 3 * 60 * 60];
/// How far into the evening each step's sunset has gone, on the scene's sky axis: from a golden
/// afternoon to the afterglow.
const EVENING: [f32; 5] = [0.0, 0.45, 0.8, 1.0, 1.35];

/// "19 Oct – 15 Nov", the span History shows.
fn span(days: &[Day]) -> String {
    let (a, b) = (days[0].date(), days[days.len() - 1].date());
    format!("{} {} – {} {}", a.day, &MONTHS[a.month as usize - 1][..3], b.day, &MONTHS[b.month as usize - 1][..3])
}

/// How far a day's sunset has gone, 0 to 4, by the day's focused time.
fn step(focused: u32) -> usize {
    STEPS.iter().filter(|s| focused >= **s).count()
}

/// Four weeks of days, Monday first. Each is its date over a small sunset: the longer the day's
/// focus ran, the further its sun has set, in the place's own colours, and its lanterns float on
/// its water, one each.
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
        .fold(row().gap(GAP), |r, d| r.child(el().w(tile(l.wide).0).child(label(k, &d[..1], 10.5, 500.0, 0.9))));
    // Arrows move between days without opening each one, since opening scrolls the page away
    // from the grid; Return or Space opens the day reached.
    let mut weeks = col().gap(GAP).focus_group();
    for week in days.chunks(7) {
        weeks = weeks.child(week.iter().fold(row().gap(GAP), |r, &day| r.child(cell(l, day, today))));
    }
    let samples = (1..5).fold(row().gap(3.0), |r, i| r.child(sunset(l, i, 0, (14.0, 10.0)).radius(3.0)));
    let scale = row()
        .key("history-scale")
        .gap(5.0)
        .center_y()
        .child(label(k, "less", 10.5, 450.0, 0.9))
        .child(samples)
        .child(label(k, "more focus · a dot is a lantern", 10.5, 450.0, 0.9));
    let heading = row()
        .gap(8.0)
        .center_y()
        .child(label(k, "Four weeks", 12.0, 520.0, 0.92))
        .child(label(k, &span(&days), 10.5, 450.0, 0.8).key("history-span"));
    let aside = text(aside).key("evenings").font(TextStyle::new(11.0, 480.0)).color(k.text.alpha(0.9));
    let top = if l.wide < BESIDE {
        col().gap(4.0).child(heading).child(aside)
    } else {
        row().w(l.wide).justify_between().center_y().child(heading).child(aside)
    };
    col()
        .child(top)
        .child(el().h(6.0))
        .child(col().key("history").gap(GAP).child(head).child(weeks))
        .child(el().h(8.0))
        .child(scale)
}

/// A small sunset `step` far into the evening, with `lit` lanterns on its water: sky over sea in
/// the place's colours, the sun lower at each step until it has gone and only its glow is left.
fn sunset(l: &Ledger<'_>, step: usize, lit: u32, (w, h): (f32, f32)) -> El {
    let p = l.place.pal(EVENING[step]);
    // At night the whole calendar sits a little further into the dark, as the scene does.
    let dusk = |c: Color| if l.dark { c.mix(rgb(0x0a0c22), 0.22) } else { c };
    let shore = (h * SHORE).round();
    let mut sky = Shape::new();
    let sun = [w * 0.62, shore * [0.0, 0.3, 0.62, 1.0, 1.3][step]];
    sky = halo(sky, [sun[0], shore], w * 0.55, 0.5, p.glow, 0.25 + 0.15 * step as f32);
    if step < 4 {
        sky = disc(sky, sun, [3.0, 3.0], dusk(p.sun));
    }
    let mut sea = Shape::new();
    for i in 0..lit.min(16) {
        let (c, r) = ((i % 8) as f32, (i / 8) as f32);
        let at = [4.5 + c * (w - 9.0) / 7.0, shore + 2.6 + r * 3.4];
        sea = disc(sea, at, [1.25, 1.25], rgb(0xfff1d6).alpha(0.95));
    }
    el().size(w, h)
        .clip_rounded(6.0)
        .linear_gradient(
            PI,
            [
                (0.0, dusk(p.zen.mix(p.mid, 0.4))),
                (SHORE * 0.7, dusk(p.mid)),
                (SHORE, dusk(p.low)),
                (SHORE, dusk(p.sea_hi)),
                (1.0, dusk(p.sea_lo)),
            ],
        )
        .child(canvas(sky).absolute(0.0, 0.0).size(w, shore).blur(1.2))
        .child(canvas(sea).absolute(0.0, 0.0).size(w, h))
}

/// A day: its date, quiet over its sunset. A past day with no focus is still water under a pale
/// sky; a day still to come is only an outline, so the two never read alike.
fn cell(l: &Ledger<'_>, day: Day, today: Day) -> El {
    let k = l.ink();
    let (tw, th) = tile(l.wide);
    let key = format!("day-{}", day.0);
    let d = day.date();
    let named = format!("{} {} {}", DAYS[day.weekday() as usize], d.day, MONTHS[d.month as usize - 1]);
    let named = if day == today { format!("{named}, today") } else { named };
    let number = label(k, &d.day.to_string(), 9.5, if day == today { 640.0 } else { 480.0 }, 0.95);
    let column = |tile: El| col().gap(2.0).child(number).child(tile);
    if day > today {
        return column(
            el().key(key)
                .size(tw, th)
                .radius(6.0)
                .border(1.0, k.text.alpha(0.24))
                .role("img")
                .label(format!("{named}, still to come")),
        );
    }
    let t = l.log.totals_on(l.cal, day);
    let spoken = if t.blocks == 0 {
        format!("{named}: nothing")
    } else {
        let lit = if t.lanterns == 1 { "1 lantern".to_string() } else { format!("{} lanterns", t.lanterns) };
        format!("{named}: {} focused, {lit}", duration(t.focused))
    };
    let tile = if t.focused == 0 {
        let still = Shape::new().line(
            [6.0, th * SHORE],
            [tw - 6.0, th * SHORE],
            1.0,
            rune::shape::LineCap::Round,
            k.text.alpha(0.2),
        );
        el().size(tw, th).radius(6.0).fill(k.text.alpha(0.07)).child(canvas(still).size(tw, th))
    } else {
        sunset(l, step(t.focused), t.lanterns, tile(l.wide))
    };
    let base = tile
        .key(key)
        .radius(6.0)
        .hover(k.text.alpha(0.18))
        .role("button")
        .label(spoken.clone())
        .help(spoken)
        // The day the page above shows is the pressed one, for VoiceOver as for the eye.
        .checked(l.day == day)
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
            // The answer to picking a day is that day: its strip and numbers at the top of the
            // page, under the date it now shows.
            tx.scroll_to("today-scroll", 0.0, 0.0);
        }));
    column(match (l.day == day, day == today) {
        (true, _) => base.border(2.0, k.text.alpha(0.95)),
        (false, true) => base.border(1.2, k.text.alpha(0.6)),
        _ => base.border(1.0, k.text.alpha(0.12)),
    })
}
