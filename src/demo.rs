//! The review renders: a day of study and four weeks behind it, and the moments of an evening
//! the review pack shoots (`--scene focus-late`). Headless only; nothing here is saved.

use crate::log::{Block, Felt};
use crate::timer::{self, CYCLE, Clock, Outcome, Phase};
use crate::{Page, Reflect, State, drawing};

/// A block for `seed`: the hour it ends, how, its pauses, how it felt, and its name.
type Seeded<'a> = (f32, Outcome, &'a [f32], Option<Felt>, &'a str);

/// A day of study, for the review renders: blocks ending at these hours, with their pauses
/// and words.
fn seed(s: &mut State, day_offset: i64, blocks: &[Seeded<'_>]) {
    let day = (s.epoch as i64).div_euclid(86_400) + day_offset;
    for &(hour, outcome, pauses, rated, label) in blocks {
        let focused = if outcome == Outcome::Finished { 1500 } else { 540 };
        s.study.log.push(Block {
            serial: 0,
            ended: day * 86_400 + (hour * 3600.0) as i64,
            planned: 1500,
            focused,
            outcome,
            pauses: pauses.to_vec(),
            paused: 50 * pauses.len() as u32,
            rated,
            label: (!label.is_empty()).then(|| label.into()),
        });
    }
}

/// Today so far in the review renders: three lanterns this morning, one let go.
fn morning(s: &mut State) {
    use Outcome::{Finished, LetGo};
    seed(
        s,
        0,
        &[
            (9.4, Finished, &[], Some(Felt::Deep), "Reading & notes"),
            (10.0, Finished, &[0.4], None, "Reading & notes"),
            (10.55, Finished, &[], Some(Felt::Deep), "Problem set"),
            (11.2, LetGo, &[0.2], None, "Problem set"),
            (13.6, Finished, &[0.3, 0.72], Some(Felt::Steady), "Problem set"),
        ],
    );
    s.label.replace_text("Problem set");
}

/// A long day: nineteen blocks from half past four, about every half hour, every fifth let go.
fn busy(s: &mut State) {
    let blocks: Vec<Seeded<'_>> = (0..19)
        .map(|i| {
            let outcome = if i % 5 == 3 { Outcome::LetGo } else { Outcome::Finished };
            (4.6 + i as f32 * 0.49, outcome, &[][..], None, ["Lecture notes", "Problem set", "Reading"][i % 3])
        })
        .collect();
    seed(s, 0, &blocks);
}

/// Four weeks of history behind today, deterministic, with rest days among them.
fn month(s: &mut State) {
    for back in 1..24i64 {
        let n = (drawing::hash(back as u32 * 3) * 7.0) as usize;
        if (back % 7 == 4 && drawing::hash(back as u32 + 40) > 0.3) || drawing::hash(back as u32 + 7) < 0.15 {
            continue;
        }
        let blocks: Vec<Seeded<'_>> = (0..n.max(1))
            .map(|i| {
                let paused: &[f32] = if drawing::hash(back as u32 * 11 + i as u32) > 0.7 { &[0.5] } else { &[] };
                let felt = (drawing::hash(back as u32 * 5 + i as u32) > 0.5).then_some(Felt::Deep);
                let label = ["Reading & notes", "Problem set", "Essay draft"][(back as usize + i) % 3];
                (9.0 + i as f32 * 0.6, Outcome::Finished, paused, felt, label)
            })
            .collect();
        seed(s, -back, &blocks);
    }
}

/// Puts the last `n` lanterns lit on this cycle's water; the day's earlier ones are stars.
fn on_water(s: &mut State, n: usize) {
    let lit: Vec<u64> = s.study.log.numbered().filter(|(_, b)| b.finished()).map(|(n, _)| n).collect();
    s.study.cycle.from = if n == 0 { s.study.log.next() } else { lit[lit.len().saturating_sub(n)] };
}

/// The reflection open on the last lantern, asked `ago` seconds before the scene's clock.
fn asking(s: &mut State, ago: f64) {
    if let Some(serial) = s.study.log.last_lit() {
        s.reflect = Some(Reflect { serial, asked: -ago, answered: None });
    }
}

fn ticking(s: &mut State, progress: f64) {
    let secs = f64::from(s.timer.length());
    s.timer.clock = Clock::Ticking {
        ends_at: secs * (1.0 - progress),
        tick: 0.0,
        run: timer::Run { planned: secs as u32, pauses: vec![], paused: 0.0 },
    };
}

/// Sets a fresh state up as one review scene.
type Setup = fn(&mut State);

pub const SCENES: &[(&str, Setup)] = &[
    ("focus-early", |s| {
        morning(s);
        s.timer.done = 0;
        on_water(s, 0);
        ticking(s, 0.12);
    }),
    ("focus-late", |s| {
        morning(s);
        s.timer.done = 3;
        on_water(s, 3);
        ticking(s, 0.84);
    }),
    ("paused", |s| {
        morning(s);
        s.timer.done = 2;
        on_water(s, 2);
        ticking(s, 0.5);
        let Clock::Ticking { run, .. } = s.timer.clock.clone() else { return };
        s.timer.clock = Clock::Paused { secs: 750, since: 0.0, run };
    }),
    ("bell", |s| {
        morning(s);
        seed(s, 0, &[(14.4, Outcome::Finished, &[], None, "Problem set")]);
        s.timer.done = 2;
        on_water(s, 3);
        asking(s, 0.0);
        s.timer.clock = Clock::Ringing { until: timer::RING, kept: true, from: 1.0 };
    }),
    ("let-go", |s| {
        morning(s);
        s.timer.done = 3;
        s.study.log.take_back();
        seed(s, 0, &[(14.2, Outcome::LetGo, &[], None, "Problem set")]);
        on_water(s, 3);
        s.timer.clock = Clock::Ringing { until: timer::RING, kept: false, from: 0.55 };
    }),
    ("short", |s| {
        morning(s);
        s.timer.done = 3;
        on_water(s, 3);
        asking(s, timer::RING + 5.0);
        s.timer.phase = Phase::Short;
        ticking(s, 0.35);
    }),
    ("long", |s| {
        morning(s);
        seed(s, 0, &[(14.1, Outcome::Finished, &[], None, "Problem set")]);
        s.timer.done = CYCLE;
        on_water(s, CYCLE as usize);
        s.timer.phase = Phase::Long;
        ticking(s, 0.45);
    }),
    ("evening", |s| {
        morning(s);
        seed(
            s,
            0,
            &[
                (14.1, Outcome::Finished, &[], None, "Problem set"),
                (15.0, Outcome::Finished, &[0.6], Some(Felt::Deep), "Essay draft"),
            ],
        );
        s.timer.done = 1;
        on_water(s, 1);
        ticking(s, 0.97);
    }),
    ("last-minute", |s| {
        morning(s);
        s.timer.done = 2;
        on_water(s, 2);
        ticking(s, 0.96);
    }),
    ("night-minute", |s| {
        morning(s);
        seed(s, 0, &[(14.1, Outcome::Finished, &[], None, "Problem set")]);
        s.timer.done = CYCLE;
        on_water(s, CYCLE as usize);
        s.timer.phase = Phase::Long;
        s.timer.lengths.set(Phase::Long, 60);
    }),
    ("today", |s| {
        morning(s);
        month(s);
        s.page = Page::Today(crate::today::Nav::default());
    }),
    ("busy", |s| {
        busy(s);
        month(s);
        s.page = Page::Today(crate::today::Nav::default());
    }),
    ("controls", |s| {
        morning(s);
        s.timer.done = 0;
        on_water(s, 0);
        s.controls_until = f64::INFINITY;
    }),
];
