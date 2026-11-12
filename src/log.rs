//! The study log: every focus block that started, kept as the timer ended it, and what the day
//! and the month look like from it. Everything here is derived on read; the only stored facts
//! are the blocks themselves, and for days older than `KEEP_DAYS`, what each day added up to.
//! Each block keeps the serial number it was recorded with, saved beside it, so neither
//! compaction nor a record set aside on reading changes it: the cycle names its first block by it.

use std::cmp::Reverse;

use rune::time::LocalTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::timer::{Ended, Outcome};

/// A study day starts at 4 a.m., so a late night counts toward the evening it began in.
pub const DAY_STARTS: i64 = 4 * 3600;
/// How far back History looks, and the window "days with a lantern" counts within.
pub const WEEKS: i64 = 4;
/// Blocks are kept one by one this many days; older days are kept as their totals.
pub const KEEP_DAYS: i64 = 120;

/// How a block felt, in one word. Only the user can say a block was deep.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Felt {
    Deep,
    Steady,
    Drifting,
}

impl Felt {
    pub const ALL: [Felt; 3] = [Felt::Deep, Felt::Steady, Felt::Drifting];

    pub fn word(self) -> &'static str {
        match self {
            Felt::Deep => "deep",
            Felt::Steady => "steady",
            Felt::Drifting => "drifting",
        }
    }
}

/// One focus block, as the log keeps it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Block {
    /// Its place in the order blocks were ever recorded; `Log::push` gives it.
    pub serial: u64,
    /// Unix seconds when it ended.
    pub ended: i64,
    pub planned: u32,
    pub focused: u32,
    pub outcome: Outcome,
    /// How far through the block each pause began, 0..1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pauses: Vec<f32>,
    #[serde(default)]
    pub paused: u32,
    /// The user's word for it; `None` until they give one, which they never have to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rated: Option<Felt>,
    /// What the block was for, if the user named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Block {
    pub fn new(ended: Ended, at: i64, label: Option<String>) -> Block {
        Block {
            serial: 0,
            ended: at,
            planned: ended.planned,
            focused: ended.focused,
            outcome: ended.outcome,
            pauses: ended.pauses,
            paused: ended.paused,
            rated: None,
            label,
        }
    }

    pub fn finished(&self) -> bool {
        self.outcome == Outcome::Finished
    }

    /// Unix seconds when it began, pauses included.
    pub fn started(&self) -> i64 {
        self.ended - i64::from(self.focused) - i64::from(self.paused)
    }

    /// The user's word for a lantern, if they gave one. The app never guesses: a question left
    /// unanswered stays unanswered.
    pub fn felt(&self) -> Option<Felt> {
        self.rated.filter(|_| self.finished())
    }

    /// Ran to the bell without a pause: the one objective measure the app shows.
    pub fn unbroken(&self) -> bool {
        self.finished() && self.pauses.is_empty()
    }
}

/// A study day, counted in days since 1970-01-01 on the local clock, starting at `DAY_STARTS`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Day(pub i64);

impl Day {
    /// 0 is Monday.
    pub fn weekday(self) -> i64 {
        (self.0 + 3).rem_euclid(7)
    }

    /// The calendar date it's named by.
    pub fn date(self) -> LocalTime {
        LocalTime::at_offset(self.0 * 86_400_000, 0)
    }
}

/// The user's wall clock: how far east of UTC it is at any moment, daylight saving included.
/// Headless runs pin it to UTC, so scripts and renders don't depend on where they run.
#[derive(Clone, Copy, Debug)]
pub struct Calendar {
    /// Seconds east of UTC at a Unix time.
    offset: fn(i64) -> i32,
}

impl Calendar {
    pub const UTC: Calendar = Calendar { offset: |_| 0 };
    /// The time zone the OS is set to.
    pub const LOCAL: Calendar = Calendar { offset: |unix| LocalTime::at(unix * 1000).offset };

    pub fn local(self, unix: i64) -> LocalTime {
        LocalTime::at_offset(unix * 1000, (self.offset)(unix))
    }

    pub fn day_of(self, unix: i64) -> Day {
        let local = unix + i64::from((self.offset)(unix));
        Day((local - DAY_STARTS).div_euclid(86_400))
    }

    /// Unix seconds when the next study day begins, at the next 4 a.m. after `unix`. The offset
    /// is the one in force at that 4 a.m., so a night the clocks change still turns at 4.
    pub fn next_day(self, unix: i64) -> i64 {
        let four = (self.day_of(unix).0 + 1) * 86_400 + DAY_STARTS;
        let near = four - i64::from((self.offset)(unix));
        four - i64::from((self.offset)(near))
    }

    /// Hours since local midnight of the study day `unix` falls in, so 1 a.m. reads as 25.
    pub fn hour_of(self, unix: i64) -> f32 {
        let local = unix + i64::from((self.offset)(unix));
        (local - self.day_of(unix).0 * 86_400) as f32 / 3600.0
    }
}

/// What a set of blocks adds up to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Totals {
    pub focused: u32,
    pub lanterns: u32,
    pub unbroken: u32,
    pub blocks: u32,
    /// Lanterns by how the user said they felt, in `Felt::ALL` order; lanterns left unanswered
    /// are in none of them.
    pub felt: [u32; 3],
}

impl Totals {
    pub fn of<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Totals {
        blocks.into_iter().fold(Totals::default(), |mut t, b| {
            t.focused += b.focused;
            t.blocks += 1;
            t.lanterns += u32::from(b.finished());
            t.unbroken += u32::from(b.unbroken());
            if let Some(felt) = b.felt() {
                t.felt[Felt::ALL.iter().position(|f| *f == felt).unwrap_or(0)] += 1;
            }
            t
        })
    }

    /// Lanterns the user said how they felt.
    pub fn reflected(&self) -> u32 {
        self.felt.iter().sum()
    }

    /// Blocks skipped.
    pub fn skipped(&self) -> u32 {
        self.blocks - self.lanterns
    }

    fn add(&mut self, o: &Totals) {
        self.focused += o.focused;
        self.lanterns += o.lanterns;
        self.unbroken += o.unbroken;
        self.blocks += o.blocks;
        for (a, b) in self.felt.iter_mut().zip(o.felt) {
            *a += b;
        }
    }
}

/// A day older than `KEEP_DAYS`, kept as what it added up to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Summary {
    pub day: i64,
    #[serde(flatten)]
    pub totals: Totals,
}

/// Every block, oldest first, and the days compacted out of it.
#[derive(Clone, PartialEq, Debug, Default, Serialize)]
pub struct Log {
    pub blocks: Vec<Block>,
    pub days: Vec<Summary>,
    /// The serial the next block recorded will have.
    next: u64,
}

impl Log {
    pub fn next(&self) -> u64 {
        self.next
    }

    /// Every kept block with its serial, oldest first.
    pub fn numbered(&self) -> impl DoubleEndedIterator<Item = (u64, &Block)> + Clone {
        self.blocks.iter().map(|b| (b.serial, b))
    }

    /// The block with this serial, if it's still kept whole.
    pub fn block_mut(&mut self, serial: u64) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| b.serial == serial)
    }

    /// Reads the saved blocks and summaries one record at a time: a record this version can't
    /// read is handed back, untouched, instead of costing the rest of the log. Saves from before
    /// serials were stored numbered blocks by their place in the list, counting from `first`.
    pub fn parse(blocks: Option<Value>, days: Option<Value>, first: u64, next: Option<u64>) -> (Log, Vec<Value>) {
        let mut rejected = Vec::new();
        let mut each = |v: Option<Value>| match v {
            Some(Value::Array(items)) => items,
            Some(other) => {
                rejected.push(other);
                Vec::new()
            }
            None => Vec::new(),
        };
        let (raw_blocks, raw_days) = (each(blocks), each(days));
        let mut log = Log { next: next.unwrap_or(first + raw_blocks.len() as u64), ..Log::default() };
        for (place, v) in (first..).zip(raw_blocks) {
            let mut numbered = v.clone();
            if let Value::Object(fields) = &mut numbered {
                fields.entry("serial").or_insert(place.into());
            }
            match serde_json::from_value::<Block>(numbered) {
                Ok(b) => {
                    log.next = log.next.max(b.serial + 1);
                    log.blocks.push(b);
                }
                Err(_) => rejected.push(v),
            }
        }
        for v in raw_days {
            match serde_json::from_value(v.clone()) {
                Ok(d) => log.days.push(d),
                Err(_) => rejected.push(v),
            }
        }
        (log, rejected)
    }

    /// Folds the oldest blocks, those more than `KEEP_DAYS` before `today`, into their days'
    /// totals. Only a run from the front goes, so every kept block keeps its serial. Says whether
    /// anything moved.
    pub fn compact(&mut self, cal: Calendar, today: Day) -> bool {
        let keep_from = Day(today.0 - KEEP_DAYS);
        let n = self.blocks.iter().take_while(|b| cal.day_of(b.ended) < keep_from).count();
        let old: Vec<Block> = self.blocks.drain(..n).collect();
        for b in &old {
            let day = cal.day_of(b.ended).0;
            let at = if let Some(i) = self.days.iter().position(|d| d.day == day) {
                i
            } else {
                self.days.push(Summary { day, totals: Totals::default() });
                self.days.len() - 1
            };
            self.days[at].totals.add(&Totals::of([b]));
        }
        self.days.sort_by_key(|d| d.day);
        !old.is_empty()
    }

    /// What `day` added up to, from its blocks or, for an old day, its summary.
    pub fn totals_on(&self, cal: Calendar, day: Day) -> Totals {
        let mut t = Totals::of(self.on(cal, day).map(|(_, b)| b));
        if let Some(d) = self.days.iter().find(|d| d.day == day.0) {
            t.add(&d.totals);
        }
        t
    }

    /// Records `block` under the next serial, and returns it.
    pub fn push(&mut self, block: Block) -> u64 {
        let serial = self.next;
        self.blocks.push(Block { serial, ..block });
        self.next += 1;
        serial
    }

    /// Takes back the block recorded last, as if it never was: its serial goes to the next one.
    pub fn take_back(&mut self) {
        if let Some(b) = self.blocks.pop() {
            self.next = b.serial;
        }
    }

    /// The serial of the last lantern lit, which the reflection asks about.
    pub fn last_lit(&self) -> Option<u64> {
        self.numbered().rev().find(|(_, b)| b.finished()).map(|(n, _)| n)
    }

    /// `day`'s blocks with their serials, oldest first.
    pub fn on(&self, cal: Calendar, day: Day) -> impl DoubleEndedIterator<Item = (u64, &Block)> + Clone {
        self.numbered().filter(move |(_, b)| cal.day_of(b.ended) == day)
    }

    /// The lanterns of the cycle whose first block is serial `from`, still on the water whatever
    /// day they were lit, and `day`'s earlier lanterns, now stars; each oldest first.
    pub fn lanterns(&self, cal: Calendar, day: Day, from: u64) -> (Vec<&Block>, Vec<&Block>) {
        let lit = self.numbered().filter(|(_, b)| b.finished());
        let stars = lit.clone().filter(|(n, b)| *n < from && cal.day_of(b.ended) == day).map(|(_, b)| b).collect();
        (stars, lit.filter(|(n, _)| *n >= from).map(|(_, b)| b).collect())
    }

    /// The four weeks History shows, Monday first, ending with the week `today` is in.
    pub fn weeks(today: Day) -> impl Iterator<Item = Day> {
        let first = today.0 - today.weekday() - 7 * (WEEKS - 1);
        (first..first + 7 * WEEKS).map(Day)
    }

    /// Days in the last four weeks with any focused minutes: a count within a window, so a
    /// missed day is one fewer this month, never a run broken.
    pub fn days_with_focus(&self, cal: Calendar, today: Day) -> usize {
        (today.0 - 7 * WEEKS + 1..=today.0).filter(|&d| self.totals_on(cal, Day(d)).focused > 0).count()
    }

    /// What the time on these blocks went to, most first. Unnamed blocks aren't listed.
    pub fn labels<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Vec<(&'a str, u32)> {
        let mut out: Vec<(&str, u32)> = Vec::new();
        for b in blocks {
            let Some(label) = b.label.as_deref() else { continue };
            match out.iter_mut().find(|(l, _)| *l == label) {
                Some((_, secs)) => *secs += b.focused,
                None => out.push((label, b.focused)),
            }
        }
        out.sort_by_key(|&(_, secs)| Reverse(secs));
        out
    }
}

/// "2 h 40 min", "45 min", as Today and History say a length of time.
pub fn duration(secs: u32) -> String {
    let mins = (secs + 30) / 60;
    match (mins / 60, mins % 60) {
        (0, 0) if secs > 0 => "under a minute".into(),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTC: Calendar = Calendar::UTC;
    /// 2026-11-11 00:00 UTC, a Wednesday.
    const WED: i64 = 1_794_355_200;
    const H: i64 = 3600;

    fn block(ended: i64, outcome: Outcome, pauses: &[f32]) -> Block {
        let focused = if outcome == Outcome::Finished { 1500 } else { 600 };
        Block {
            serial: 0,
            ended,
            planned: 1500,
            focused,
            outcome,
            pauses: pauses.to_vec(),
            paused: 40 * pauses.len() as u32,
            rated: None,
            label: None,
        }
    }

    #[test]
    fn an_unanswered_reflection_stays_unanswered_and_out_of_the_proportions() {
        let straight = block(WED, Outcome::Finished, &[]);
        let paused = block(WED, Outcome::Finished, &[0.3]);
        assert_eq!((straight.felt(), paused.felt()), (None, None), "the app never guesses a word");
        let deep = Block { rated: Some(Felt::Deep), ..paused.clone() };
        assert_eq!(deep.felt(), Some(Felt::Deep));
        let t = Totals::of([&straight, &paused, &deep]);
        assert_eq!(
            (t.lanterns, t.reflected(), t.felt),
            (3, 1, [1, 0, 0]),
            "two unanswered lanterns are in no proportion"
        );
    }

    #[test]
    fn a_skipped_block_has_no_word_and_lights_no_lantern_but_keeps_its_minutes() {
        let t = Totals::of(&[block(WED + 10 * H, Outcome::Finished, &[]), block(WED + 11 * H, Outcome::Skipped, &[])]);
        assert_eq!(block(WED, Outcome::Skipped, &[]).felt(), None);
        assert_eq!((t.focused, t.lanterns, t.blocks, t.unbroken, t.skipped()), (2100, 1, 2, 1, 1));
    }

    #[test]
    fn the_day_turns_over_at_four_in_the_morning() {
        assert_eq!(UTC.day_of(WED + 10 * H), UTC.day_of(WED + 27 * H), "1 a.m. belongs to the evening before");
        assert_ne!(UTC.day_of(WED + 10 * H), UTC.day_of(WED + 28 * H));
        assert_eq!(UTC.day_of(WED + 10 * H).weekday(), 2);
        assert_eq!(UTC.hour_of(WED + 27 * H), 27.0);
        assert_eq!(UTC.next_day(WED + 14 * H), WED + 28 * H, "an afternoon's next day begins at 4 a.m.");
        assert_eq!(UTC.next_day(WED + 27 * H), WED + 28 * H, "and so does 3 a.m.'s");
        assert_eq!(UTC.next_day(WED + 28 * H), WED + 52 * H);
        let date = UTC.day_of(WED + 2 * H).date();
        assert_eq!((date.month, date.day), (11, 10));
    }

    /// New York in 2027: daylight saving from 2 a.m. on 14 March to 2 a.m. on 7 November.
    const NEW_YORK: Calendar =
        Calendar { offset: |unix| if (1_805_007_600..1_825_567_200).contains(&unix) { -4 * 3600 } else { -5 * 3600 } };

    #[test]
    fn the_day_still_turns_at_four_on_the_nights_the_clocks_change() {
        let spring_eve = 1_804_964_400;
        let spring_turn = 1_805_011_200;
        assert_eq!(NEW_YORK.next_day(spring_eve), spring_turn, "2 p.m. EST on the 13th turns at 4 a.m. EDT");
        assert_eq!(NEW_YORK.local(spring_turn).hour, 4);
        let autumn_eve = 1_825_527_600;
        let autumn_turn = 1_825_578_000;
        assert_eq!(NEW_YORK.next_day(autumn_eve), autumn_turn, "3 p.m. EDT on the 6th turns at 4 a.m. EST");
        assert_eq!(NEW_YORK.local(autumn_turn).hour, 4);
        assert_ne!(
            NEW_YORK.day_of(autumn_turn - 1),
            NEW_YORK.day_of(autumn_turn),
            "and the day is new from that second"
        );
    }

    #[test]
    fn todays_lanterns_float_on_the_water_until_the_night_makes_them_stars() {
        let mut log = Log::default();
        log.push(block(WED - 5 * H, Outcome::Finished, &[]));
        for (hour, outcome) in (9..).zip([Outcome::Finished, Outcome::Skipped, Outcome::Finished, Outcome::Finished]) {
            log.push(block(WED + hour * H, outcome, &[]));
        }
        let today = UTC.day_of(WED + 12 * H);
        let hours = |blocks: Vec<&Block>| blocks.iter().map(|b| (b.ended - WED) / H).collect::<Vec<_>>();
        let (stars, water) = log.lanterns(UTC, today, 3);
        assert_eq!((hours(stars), hours(water)), (vec![9], vec![11, 12]), "a block skipped is neither");
        let (stars, water) = log.lanterns(UTC, today, log.next());
        assert_eq!((stars.len(), water.len()), (3, 0), "yesterday's lantern isn't in today's sky");
    }

    #[test]
    fn the_reflection_asks_about_the_last_lantern_not_a_skipped_block() {
        let mut log = Log::default();
        log.push(block(WED + 9 * H, Outcome::Finished, &[]));
        log.push(block(WED + 10 * H, Outcome::Skipped, &[]));
        assert_eq!(log.last_lit(), Some(0));
        assert_eq!(Log::default().last_lit(), None);
    }

    #[test]
    fn days_with_focus_count_days_in_four_weeks_and_never_reset() {
        let mut log = Log::default();
        let today = UTC.day_of(WED + 12 * H);
        for days_ago in [40, 28, 27, 5, 1, 0, 0] {
            log.push(block(WED + 12 * H - days_ago * 24 * H, Outcome::Finished, &[]));
        }
        log.push(block(WED + 12 * H - 2 * 24 * H, Outcome::Skipped, &[]));
        assert_eq!(
            log.days_with_focus(UTC, today),
            5,
            "today, yesterday, two days ago (skipped, still focused), five and 27 days ago"
        );
    }

    #[test]
    fn history_is_four_whole_weeks_ending_with_this_one() {
        let today = UTC.day_of(WED + 12 * H);
        let days: Vec<Day> = Log::weeks(today).collect();
        assert_eq!(days.len(), 28);
        assert_eq!(days[0].weekday(), 0);
        assert_eq!(days[21 + 2], today);
    }

    #[test]
    fn labels_add_up_what_the_time_went_to() {
        let named = |label: &str, outcome| Block { label: Some(label.into()), ..block(WED, outcome, &[]) };
        let blocks = [
            named("Reading", Outcome::Finished),
            named("Problem set", Outcome::Finished),
            named("Problem set", Outcome::Skipped),
            block(WED, Outcome::Finished, &[]),
        ];
        assert_eq!(Log::labels(&blocks), vec![("Problem set", 2100), ("Reading", 1500)]);
    }

    #[test]
    fn a_saved_log_reads_back_and_tolerates_old_fields_missing() {
        let mut log = Log::default();
        log.push(Block {
            rated: Some(Felt::Deep),
            label: Some("Reading".into()),
            ..block(WED, Outcome::Finished, &[0.5])
        });
        log.days.push(Summary { day: 3, totals: Totals { focused: 60, ..Totals::default() } });
        let saved = (serde_json::to_value(&log.blocks).unwrap(), serde_json::to_value(&log.days).unwrap());
        assert_eq!(Log::parse(Some(saved.0), Some(saved.1), 0, Some(log.next())), (log, vec![]));
        let first_version =
            serde_json::json!([{ "ended": WED, "planned": 1500, "focused": 1500, "outcome": "finished" }]);
        let (read, _) = Log::parse(Some(first_version), None, 0, None);
        assert_eq!(
            (read.blocks[0].pauses.len(), read.blocks[0].rated, read.blocks[0].label.as_deref()),
            (0, None, None)
        );
    }

    #[test]
    fn one_record_it_cant_read_costs_only_that_record() {
        let saved = serde_json::json!([
            { "ended": WED, "planned": 1500, "focused": 1500, "outcome": "finished" },
            { "ended": WED + H, "planned": 1500, "focused": 900, "outcome": "abandoned_by_a_newer_version" },
            { "ended": WED + 2 * H, "planned": 1500, "focused": 1500, "outcome": "finished" },
        ]);
        let (log, rejected) = Log::parse(Some(saved.clone()), None, 5, None);
        assert_eq!(log.numbered().map(|(n, b)| (n, b.ended)).collect::<Vec<_>>(), vec![(5, WED), (7, WED + 2 * H)]);
        assert_eq!(log.next(), 8, "and the next block comes after all three");
        assert_eq!(rejected, vec![saved[1].clone()], "kept as it was, to be set aside before the next save");
    }

    #[test]
    fn days_past_the_keeping_window_fold_into_totals_history_still_reads() {
        let mut log = Log::default();
        let today = UTC.day_of(WED + 12 * H);
        let old = WED + 12 * H - (KEEP_DAYS + 3) * 24 * H;
        log.push(Block { rated: Some(Felt::Deep), ..block(old, Outcome::Finished, &[]) });
        log.push(block(old + H, Outcome::Skipped, &[]));
        log.push(block(WED + 10 * H, Outcome::Finished, &[]));
        let before = log.totals_on(UTC, UTC.day_of(old));
        assert!(log.compact(UTC, today));
        assert_eq!(log.blocks.len(), 1, "only the recent block is kept whole");
        assert_eq!(log.numbered().next().map(|(n, _)| n), Some(2), "and it keeps its serial");
        assert_eq!(log.totals_on(UTC, UTC.day_of(old)), before, "the old day still adds up the same");
        assert_eq!(before.felt, [1, 0, 0]);
        assert!(!log.compact(UTC, today), "compacting again changes nothing");
    }

    #[test]
    fn durations_read_as_hours_and_minutes() {
        assert_eq!(duration(45 * 60), "45 min");
        assert_eq!(duration(160 * 60), "2 h 40 min");
        assert_eq!(duration(120 * 60 + 10), "2 h");
        assert_eq!(duration(109 * 60), "1 h 49 min");
        assert_eq!(duration(25), "under a minute", "a short block still counts for something");
        assert_eq!(duration(0), "0 min");
    }
}
