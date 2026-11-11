//! Everything the app has recorded, as one value: the log, the cycle and any record this
//! version couldn't read. It is saved whole under one key, so a crash leaves the last save or the
//! one before, never a log from one moment beside a cycle from another. The cycle names its
//! first block by serial, so which lanterns belong to it never depends on a clock.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::log::{Block, Calendar, Log};
use crate::timer::{CYCLE, Clock, Ended, Lengths, Phase, Timer};

/// The key the record is saved under.
pub const KEY: &str = "study";
/// Where earlier versions kept the same facts, one key each. Read once, then removed.
pub const LEGACY: [&str; 4] = ["log", "log-days", "cycle", "log-rejected"];

/// The cycle under way: the serial of its first block, and the phase it waits at.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Cycle {
    /// Lanterns with this serial or later are this cycle's.
    pub from: u64,
    /// The phase waiting once any bell has rung out: a break earned before a quit is owed.
    pub phase: Phase,
}

/// Saved as `{ blocks, days, next, cycle, rejected }`, which `parse` reads back.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Study {
    #[serde(flatten)]
    pub log: Log,
    pub cycle: Cycle,
    /// Saved records this version can't read, kept as they were.
    pub rejected: Vec<Value>,
}

/// How 6f9f48e kept the cycle: from a moment, not a block.
#[derive(Deserialize)]
struct SinceCycle {
    since: i64,
    phase: Phase,
}

impl Study {
    pub fn new() -> Study {
        Study { log: Log::default(), cycle: Cycle { from: 0, phase: Phase::Focus }, rejected: Vec::new() }
    }

    /// Reads what was saved through `get`, from this version's key or, the first time, from the
    /// keys earlier versions used. Says whether it should be saved straight back: it came from
    /// the old keys, or something in it had to be set aside.
    pub fn load(get: impl Fn(&str) -> Option<Value>, cal: Calendar, now: i64) -> (Study, bool) {
        if let Some(saved) = get(KEY) {
            return Study::parse(saved);
        }
        let (log, mut rejected) = Log::parse(get("log"), get("log-days"), 0, None);
        if let Some(Value::Array(earlier)) = get("log-rejected") {
            rejected.splice(0..0, earlier);
        }
        let cycle = if let Some(old) = get("cycle").and_then(|v| serde_json::from_value::<SinceCycle>(v).ok()) {
            let from = log.numbered().find(|(_, b)| b.ended >= old.since).map_or(log.next(), |(n, _)| n);
            Cycle { from, phase: old.phase }
        } else {
            // ed845dc counted the cycle as today's lanterns modulo four.
            let today: Vec<u64> = log.on(cal, cal.day_of(now)).filter(|(_, b)| b.finished()).map(|(n, _)| n).collect();
            let open = today.len() % CYCLE as usize;
            Cycle { from: if open == 0 { log.next() } else { today[today.len() - open] }, phase: Phase::Focus }
        };
        let migrated = LEGACY.iter().any(|k| get(k).is_some());
        (Study { log, cycle, rejected }, migrated)
    }

    fn parse(saved: Value) -> (Study, bool) {
        let cycle = saved.get("cycle").and_then(|c| serde_json::from_value::<Cycle>(c.clone()).ok());
        let Some(cycle) = cycle else {
            // A record shaped by a newer version: keep it whole and start a fresh one beside it.
            return (Study { rejected: vec![saved], ..Study::new() }, true);
        };
        let field = |k| saved.get(k).and_then(Value::as_u64);
        let (log, mut rejected) = Log::parse(
            saved.get("blocks").cloned(),
            saved.get("days").cloned(),
            field("first").unwrap_or(0),
            field("next"),
        );
        let fresh = !rejected.is_empty();
        if let Some(Value::Array(earlier)) = saved.get("rejected") {
            rejected.splice(0..0, earlier.iter().cloned());
        }
        (Study { log, cycle, rejected }, fresh)
    }

    /// Lanterns lit in this cycle.
    pub fn lit(&self) -> u8 {
        self.log.numbered().filter(|(n, b)| *n >= self.cycle.from && b.finished()).count().min(CYCLE as usize) as u8
    }

    /// The timer a launch finds, starting a new cycle if the old one belongs to an earlier day.
    pub fn resume(&mut self, cal: Calendar, now: i64, lengths: Lengths) -> Timer {
        let timer = Timer::resume(self.cycle.phase, self.lit(), lengths);
        self.roll_over(cal, now, &timer).unwrap_or(timer)
    }

    /// The one rollover rule: a cycle belongs to the study day its first lantern was lit in, and
    /// a new one begins once that day is over and the cycle waits for a focus block. A break
    /// already earned, or a block running across 4 a.m., finishes first. Returns the timer for
    /// the new cycle when one began.
    pub fn roll_over(&mut self, cal: Calendar, now: i64, timer: &Timer) -> Option<Timer> {
        let waiting = timer.phase == Phase::Focus && matches!(timer.clock, Clock::Wound);
        let began =
            self.log.numbered().find(|(n, b)| *n >= self.cycle.from && b.finished()).map(|(_, b)| cal.day_of(b.ended));
        if !waiting || began.is_none_or(|day| day >= cal.day_of(now)) {
            return None;
        }
        self.cycle = Cycle { from: self.log.next(), phase: Phase::Focus };
        Some(Timer::resume(Phase::Focus, 0, timer.lengths))
    }

    /// Records a focus block the timer ended, and returns its serial.
    pub fn record(&mut self, ended: Ended, at: i64, label: Option<String>) -> u64 {
        self.log.push(Block::new(ended, at, label))
    }

    /// Takes back the block recorded last, which the timer has put back.
    pub fn unrecord(&mut self) {
        self.log.take_back();
    }

    /// Brings the cycle in line with the timer once it has stepped: the phase it will wait at,
    /// and, when the timer has started a new cycle, a boundary after every block so far. Says
    /// whether anything changed.
    pub fn settle(&mut self, timer: &Timer) -> bool {
        let (phase, done) = timer.settled();
        let from = if done == 0 && self.lit() > 0 { self.log.next() } else { self.cycle.from };
        let cycle = Cycle { from, phase };
        std::mem::replace(&mut self.cycle, cycle) != cycle
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::log::{Felt, Summary, Totals};
    use crate::timer::{Event, Outcome};

    const UTC: Calendar = Calendar { fixed_offset: Some(0) };
    /// 2026-11-11 00:00 UTC.
    const WED: i64 = 1_794_355_200;
    const H: i64 = 3600;

    fn lit(ended: i64) -> Block {
        Block {
            serial: 0,
            ended,
            planned: 1500,
            focused: 1500,
            outcome: Outcome::Finished,
            pauses: vec![],
            paused: 0,
            rated: None,
            label: None,
        }
    }

    /// Blocks as versions before serials were saved wrote them.
    fn unnumbered(blocks: &[Block]) -> Value {
        let mut v = serde_json::to_value(blocks).unwrap();
        for b in v.as_array_mut().unwrap() {
            b.as_object_mut().unwrap().remove("serial");
        }
        v
    }

    #[test]
    fn a_block_set_aside_never_renumbers_the_ones_after_it() {
        let block = |ended: i64| json!({ "ended": ended, "planned": 1500, "focused": 1500, "outcome": "finished" });
        let saved = json!({
            "first": 0,
            "blocks": [block(WED + 9 * H), { "ended": "garbled" }, block(WED + 10 * H)],
            "cycle": { "from": 2, "phase": "short" },
        });
        let (study, rewrite) = Study::load(|k| (k == KEY).then(|| saved.clone()), UTC, WED + 11 * H);
        assert!(rewrite, "the garbled record is set aside");
        assert_eq!((study.lit(), study.log.next()), (1, 3), "the cycle's lantern is still its own");
        let saved = serde_json::to_value(&study).unwrap();
        let (back, rewrite) = Study::load(|k| (k == KEY).then(|| saved.clone()), UTC, WED + 11 * H);
        assert!(!rewrite);
        assert_eq!((back.lit(), back.log.next(), back.rejected.len()), (1, 3, 1), "and stays so once saved");
    }

    #[test]
    fn the_whole_record_is_one_value_so_no_crash_can_split_it() {
        let today = WED + 12 * H;
        let old = today - 130 * 24 * H;
        let prefs = json!({
            "log": unnumbered(&[lit(old), lit(today - 2 * H), lit(today - H)]),
            "log-days": [Summary { day: 1, totals: Totals { focused: 60, ..Totals::default() } }],
            "cycle": { "since": today - 2 * H - 10, "phase": "short" },
        });
        let (mut study, migrated) = Study::load(|k| prefs.get(k).cloned(), UTC, today);
        assert!(migrated);
        assert!(study.log.compact(UTC, UTC.day_of(today)));
        let before = (study.log.totals_on(UTC, UTC.day_of(old)), study.lit(), study.cycle);
        // Compaction and the cycle travel in the same value: there is no moment where the
        // shortened log is saved and the day it folded into is not.
        let saved = serde_json::to_value(&study).unwrap();
        let keys: Vec<&str> = saved.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["blocks", "cycle", "days", "next", "rejected"], "the layout `parse` reads");
        let (back, _) = Study::load(|k| (k == KEY).then(|| saved.clone()), UTC, today);
        assert_eq!(back, study);
        assert_eq!((back.log.totals_on(UTC, UTC.day_of(old)), back.lit(), back.cycle), before);
        assert_eq!((before.0.lanterns, before.1), (1, 2), "the old day still counts its lantern, the cycle its two");
    }

    #[test]
    fn upgrading_from_ed845dc_keeps_the_cycle_it_counted() {
        let today = WED + 15 * H;
        let mut blocks: Vec<Block> = (0..6).map(|i| lit(WED + (8 + i) * H)).collect();
        blocks.insert(0, lit(WED - 10 * H));
        blocks[3].rated = Some(Felt::Deep);
        let prefs = json!({ "log": unnumbered(&blocks), "place": "Lake" });
        let (mut study, migrated) = Study::load(|k| prefs.get(k).cloned(), UTC, today);
        assert!(migrated, "the old key is rewritten into the new one");
        let timer = study.resume(UTC, today, Lengths::default());
        assert_eq!(
            (timer.phase, timer.done),
            (Phase::Focus, 2),
            "six lanterns today: one cycle done, two into the next"
        );
        let (stars, water) = study.log.lanterns(UTC, UTC.day_of(today), study.cycle.from);
        assert_eq!((stars.len(), water.len()), (4, 2));
        assert_eq!(study.log.blocks[3].rated, Some(Felt::Deep), "words given are kept");
    }

    #[test]
    fn upgrading_from_6f9f48e_reads_its_cycle_by_time_once() {
        let today = WED + 15 * H;
        let prefs = json!({
            "log": unnumbered(&(0..4).map(|i| lit(WED + (10 + i) * H)).collect::<Vec<_>>()),
            "cycle": { "since": WED + 10 * H, "phase": "long" },
            "log-rejected": [{ "ended": "garbled" }],
        });
        let (mut study, _) = Study::load(|k| prefs.get(k).cloned(), UTC, today);
        let timer = study.resume(UTC, today, Lengths::default());
        assert_eq!((timer.phase, timer.done, study.cycle.from), (Phase::Long, CYCLE, 0));
        assert_eq!(study.rejected.len(), 1, "records set aside before stay set aside");
    }

    #[test]
    fn a_record_from_a_newer_version_is_kept_whole_not_overwritten() {
        let future = json!({ "format": 9, "entries": [] });
        let (study, rewrite) = Study::load(|k| (k == KEY).then(|| future.clone()), UTC, WED);
        assert!(rewrite);
        assert_eq!((study.log.blocks.len(), study.rejected), (0, vec![future]));
    }

    #[test]
    fn a_cycle_running_across_four_in_the_morning_stays_one_cycle() {
        let mut study = Study::new();
        for ended in [WED + 26 * H, WED + 26 * H + 1800, WED + 27 * H] {
            study.log.push(lit(ended));
        }
        let mut timer = Timer::resume(Phase::Focus, 3, Lengths::default());
        timer.step(Event::Press { now: 0.0 });
        assert!(
            study.roll_over(UTC, WED + 28 * H + 60, &timer).is_none(),
            "a block running at 4 a.m. finishes in its cycle"
        );
        study.log.push(lit(WED + 28 * H + 600));
        study.cycle.phase = Phase::Long;
        let after = WED + 28 * H + 700;
        let timer = study.resume(UTC, after, Lengths::default());
        assert_eq!((timer.phase, timer.done), (Phase::Long, 4), "a relaunch agrees with the live state");
        let (stars, water) = study.log.lanterns(UTC, UTC.day_of(after), study.cycle.from);
        assert_eq!((stars.len(), water.len()), (0, 4), "all four lanterns on the water together");
    }

    #[test]
    fn a_cycle_waiting_for_focus_starts_over_with_the_new_study_day() {
        let mut study = Study::new();
        study.log.push(lit(WED + 20 * H));
        assert_eq!(
            study.clone().resume(UTC, WED + 23 * H, Lengths::default()).done,
            1,
            "the same evening, it carries on"
        );
        let next_morning = WED + 33 * H;
        let timer = study.resume(UTC, next_morning, Lengths::default());
        assert_eq!((study.cycle.from, timer.done), (1, 0));
        let (stars, water) = study.log.lanterns(UTC, UTC.day_of(next_morning), study.cycle.from);
        assert_eq!((stars.len(), water.len()), (0, 0), "yesterday's lantern is in neither of today's skies");
    }
}
