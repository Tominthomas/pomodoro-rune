//! The pomodoro as one state machine: where you are in the cycle, and what the clock is doing.
//! `step` never reads a clock or touches the window; it takes `now` in its events and returns
//! what should happen, so the same machine runs in a window, a headless script and a test.

use rune::machine::Effects;

/// Four focus blocks earn a long break.
pub const CYCLE: u8 = 4;
/// The longest any phase can be set to, and the shortest.
pub const MAX_SECS: u32 = 60 * 60;
pub const MIN_SECS: u32 = 60;
/// How long the bell rings before the next phase winds up.
pub const RING: f64 = 3.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Focus,
    Short,
    Long,
}

/// How long each phase runs, as the user last set it: each from `MIN_SECS` to `MAX_SECS`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "SavedLengths")]
pub struct Lengths {
    focus: u32,
    short: u32,
    long: u32,
}

/// Lengths as they were saved, before their bounds are checked.
#[derive(serde::Deserialize)]
struct SavedLengths {
    focus: u32,
    short: u32,
    long: u32,
}

impl TryFrom<SavedLengths> for Lengths {
    type Error = String;

    fn try_from(saved: SavedLengths) -> Result<Lengths, String> {
        let SavedLengths { focus, short, long } = saved;
        if [focus, short, long].iter().all(|secs| (MIN_SECS..=MAX_SECS).contains(secs)) {
            Ok(Lengths { focus, short, long })
        } else {
            Err(format!("lengths out of bounds: {focus}, {short}, {long}"))
        }
    }
}

impl Default for Lengths {
    fn default() -> Self {
        Lengths { focus: 25 * 60, short: 5 * 60, long: 15 * 60 }
    }
}

impl Lengths {
    pub fn of(&self, phase: Phase) -> u32 {
        match phase {
            Phase::Focus => self.focus,
            Phase::Short => self.short,
            Phase::Long => self.long,
        }
    }

    /// Sets `phase` to `secs`, kept within bounds.
    pub fn set(&mut self, phase: Phase, secs: u32) {
        let secs = secs.clamp(MIN_SECS, MAX_SECS);
        match phase {
            Phase::Focus => self.focus = secs,
            Phase::Short => self.short = secs,
            Phase::Long => self.long = secs,
        }
    }
}

/// A phase that has started: what it was set to, and every pause so far.
#[derive(Clone, PartialEq, Debug)]
pub struct Run {
    pub planned: u32,
    /// How far through the phase each pause began, 0..1.
    pub pauses: Vec<f32>,
    /// Seconds spent paused, not counting a pause still in progress.
    pub paused: f64,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Clock {
    /// Set to this phase's length, waiting for a press.
    Wound,
    /// Running down to `ends_at` on the window's clock. The digits next change at `tick`.
    Ticking { ends_at: f64, tick: f64, run: Run },
    /// Held with `secs` left since `since`.
    Paused { secs: u32, since: f64, run: Run },
    /// The phase is over and the next winds up at `until`. `kept` is false for a focus block
    /// skipped, which doesn't count; `from` is how far through it was when it ended.
    Ringing { until: f64, kept: bool, from: f32 },
}

/// A hand setting the length of `phase`. `secs` is continuous; it shows snapped to whole
/// minutes. It is apart from the clock, so a break runs on while the next block is set.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Setting {
    pub phase: Phase,
    pub secs: f64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    /// A tap on the scene, or Space: starts, pauses or resumes, and during a bell starts what
    /// comes next.
    Press { now: f64 },
    /// The hand moved the length by `secs`. Grabs it first if needed. Only while there is a
    /// length to set: see `Timer::settable`.
    Scrub { secs: f64 },
    /// The hand lifted: the length is set, and the block waits for a press like any other.
    Release,
    /// Arrow keys: whole minutes, while wound.
    Nudge { minutes: i32 },
    /// Moves on. A focus block that has started is skipped: it rings out without counting.
    Skip { now: f64 },
    /// The clock reached the deadline `Timer::due` asked for.
    Due { now: f64 },
    /// Takes back the last skip, until the next phase starts.
    Undo { now: f64 },
}

/// How a focus block that started came to an end.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Finished,
    /// Ended early with Skip. Records from before the word changed say `let_go`.
    #[serde(alias = "let_go")]
    Skipped,
}

/// A focus block's record, as the timer knows it: no wall-clock time or label.
#[derive(Clone, PartialEq, Debug)]
pub struct Ended {
    pub outcome: Outcome,
    pub planned: u32,
    pub focused: u32,
    pub pauses: Vec<f32>,
    pub paused: u32,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Effect {
    WakeAt(f64),
    /// A phase ran to the end (not skipped).
    Rang(Phase),
    /// A focus block that had started is over, finished or skipped.
    Ended(Ended),
    /// The user set a new length for a phase.
    Lengths(Lengths),
    /// The cycle moved on to `to`, from `from`.
    Advanced {
        from: Phase,
        to: Phase,
    },
    /// The block skipped last is running again: its record should go.
    Unended,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Timer {
    pub phase: Phase,
    /// Focus blocks finished in this cycle, 0..=CYCLE.
    pub done: u8,
    pub lengths: Lengths,
    pub clock: Clock,
    pub setting: Option<Setting>,
    /// The block skipped last, as it was, until the next phase starts.
    undo: Option<Clock>,
}

impl Default for Timer {
    fn default() -> Self {
        Timer::resume(Phase::Focus, 0, Lengths::default())
    }
}

impl Timer {
    /// A timer waiting at `phase` with `done` blocks in the cycle, as a relaunch finds it.
    pub fn resume(phase: Phase, done: u8, lengths: Lengths) -> Timer {
        Timer { phase, done: done.min(CYCLE), lengths, clock: Clock::Wound, setting: None, undo: None }
    }

    /// Whether the block skipped last can still be taken back: until the next phase starts, so
    /// the offer lasts as long as it takes to notice, through the break and Today too.
    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    /// The length the current phase is set to, live while a hand is setting it.
    pub fn length(&self) -> u32 {
        match &self.clock {
            Clock::Ticking { run, .. } | Clock::Paused { run, .. } => run.planned,
            _ => self.length_of(self.phase),
        }
    }

    /// The length `phase` is set to, live while a hand is setting it.
    pub fn length_of(&self, phase: Phase) -> u32 {
        match self.setting {
            Some(setting) if setting.phase == phase => snap(setting.secs),
            _ => self.lengths.of(phase),
        }
    }

    /// The phase whose length a hand can set now: the one waiting to start, or during a break,
    /// the focus block after it. Never a focus block that has started.
    pub fn settable(&self) -> Option<Phase> {
        match (&self.clock, self.phase) {
            (Clock::Wound, phase) => Some(phase),
            (_, Phase::Short | Phase::Long) => Some(Phase::Focus),
            (_, Phase::Focus) => None,
        }
    }

    /// Seconds left, continuous while ticking.
    pub fn remaining(&self, now: f64) -> f64 {
        match &self.clock {
            Clock::Wound => f64::from(self.length()),
            Clock::Paused { secs, .. } => f64::from(*secs),
            Clock::Ticking { ends_at, .. } => (ends_at - now).max(0.0),
            Clock::Ringing { .. } => 0.0,
        }
    }

    /// How far through the phase, 0 at the start and 1 at the bell.
    pub fn progress(&self, now: f64) -> f32 {
        match &self.clock {
            Clock::Ringing { from, .. } => *from,
            _ => (1.0 - self.remaining(now) / f64::from(self.length())).clamp(0.0, 1.0) as f32,
        }
    }

    /// Whole seconds left, as the digits show them: 24:59 until a full second has gone.
    pub fn shown(&self, now: f64) -> u32 {
        self.remaining(now).ceil() as u32
    }

    /// The deadline this state is waiting for: the next tick of the digits, or the bell's end.
    pub fn due(&self) -> Option<f64> {
        match &self.clock {
            Clock::Ticking { tick, .. } => Some(*tick),
            Clock::Ringing { until, .. } => Some(*until),
            _ => None,
        }
    }

    pub fn step(&mut self, event: Event) -> Effects<Effect> {
        let mut fx = Effects::none();
        match event {
            Event::Undo { now } => self.undo(now, &mut fx),
            Event::Scrub { secs } => self.scrub(secs),
            Event::Release => {
                if let Some(Setting { phase, secs }) = self.setting.take() {
                    self.set_length(phase, snap(secs), &mut fx);
                }
            }
            Event::Press { .. } if self.setting.is_some() => {}
            _ => self.step_clock(event, &mut fx),
        }
        if self.setting.is_some_and(|setting| self.settable() != Some(setting.phase)) {
            self.setting = None;
        }
        fx
    }

    fn step_clock(&mut self, event: Event, fx: &mut Effects<Effect>) {
        match (self.clock.clone(), event) {
            // The bell offers the next phase by name, so a press starts it rather than only cutting
            // the bell short.
            (Clock::Ringing { kept, .. }, Event::Press { now }) => {
                self.advance_by(kept, fx);
                self.start(self.length(), now, fx);
            }
            (Clock::Ringing { until, kept, .. }, Event::Due { now }) if now >= until => self.advance_by(kept, fx),
            (Clock::Ringing { .. }, _) => {}

            (Clock::Wound, Event::Press { now }) => self.start(self.length(), now, fx),
            (Clock::Paused { secs, since, mut run }, Event::Press { now }) => {
                run.paused += (now - since).max(0.0);
                self.tick(now + f64::from(secs), now, run, fx);
            }
            (Clock::Ticking { ends_at, mut run, .. }, Event::Press { now }) => {
                let secs = (ends_at - now).max(0.0).ceil() as u32;
                run.pauses.push(1.0 - secs as f32 / run.planned as f32);
                self.clock = Clock::Paused { secs, since: now, run };
            }

            (Clock::Wound, Event::Nudge { minutes }) => {
                let secs = i64::from(self.length() / 60 * 60) + i64::from(minutes) * 60;
                self.set_length(self.phase, secs.clamp(0, i64::from(MAX_SECS)) as u32, fx);
            }

            (Clock::Ticking { ends_at, run, .. }, Event::Due { now }) if now >= ends_at => {
                let until = now + RING;
                self.clock = Clock::Ringing { until, kept: true, from: 1.0 };
                if self.phase == Phase::Focus {
                    fx.push(Effect::Ended(Ended::of(Outcome::Finished, run.planned, run)));
                }
                fx.push(Effect::Rang(self.phase));
                fx.push(Effect::WakeAt(until));
            }
            (Clock::Ticking { ends_at, tick, run }, Event::Due { now }) if now >= tick => {
                self.tick(ends_at, now, run, fx);
            }

            (Clock::Ticking { .. } | Clock::Paused { .. }, Event::Skip { now }) if self.phase == Phase::Focus => {
                self.skip_block(now, fx);
            }
            (_, Event::Skip { .. }) => self.advance_by(false, fx),
            _ => {}
        }
    }

    fn scrub(&mut self, by: f64) {
        let Some(phase) = self.settable() else { return };
        let from = self.setting.map_or(f64::from(self.lengths.of(phase)), |setting| setting.secs);
        self.setting = Some(Setting { phase, secs: (from + by).clamp(f64::from(MIN_SECS), f64::from(MAX_SECS)) });
    }

    fn start(&mut self, secs: u32, now: f64, fx: &mut Effects<Effect>) {
        self.undo = None;
        self.tick(now + f64::from(secs), now, Run { planned: secs, pauses: Vec::new(), paused: 0.0 }, fx);
    }

    /// Ticks on toward `ends_at`, due again when the digits next change: once the time left
    /// drops below the next whole second, strictly after `now`.
    fn tick(&mut self, ends_at: f64, now: f64, run: Run, fx: &mut Effects<Effect>) {
        let left = ends_at - now;
        let tick = if left <= 1.0 { ends_at } else { ends_at - (left.ceil() - 1.0) };
        self.clock = Clock::Ticking { ends_at, tick, run };
        fx.push(Effect::WakeAt(tick));
    }

    /// Ends a focus block early. Its minutes are kept, but it doesn't count toward the cycle,
    /// and it rings out quietly before the break. A pause still in progress counts as paused.
    fn skip_block(&mut self, now: f64, fx: &mut Effects<Effect>) {
        let from = self.progress(now);
        let left = self.remaining(now).ceil() as u32;
        let before = std::mem::replace(&mut self.clock, Clock::Wound);
        let run = match before.clone() {
            Clock::Ticking { run, .. } => run,
            Clock::Paused { since, mut run, .. } => {
                run.paused += (now - since).max(0.0);
                run
            }
            other => {
                self.clock = other;
                return;
            }
        };
        let focused = run.planned.saturating_sub(left);
        fx.push(Effect::Ended(Ended::of(Outcome::Skipped, focused, run)));
        let until = now + RING;
        self.clock = Clock::Ringing { until, kept: false, from };
        self.undo = Some(before);
        fx.push(Effect::WakeAt(until));
    }

    /// Puts the block skipped last back as it was. A running block kept running all along, so
    /// the seconds since count, and one taken back after its end rings at once; one that was
    /// paused is still paused.
    fn undo(&mut self, now: f64, fx: &mut Effects<Effect>) {
        let Some(clock) = self.undo.take() else { return };
        if self.phase != Phase::Focus {
            fx.push(Effect::Advanced { from: self.phase, to: Phase::Focus });
        }
        self.phase = Phase::Focus;
        fx.push(Effect::Unended);
        match clock {
            Clock::Ticking { ends_at, run, .. } => self.tick(ends_at, now, run, fx),
            clock => self.clock = clock,
        }
    }

    fn set_length(&mut self, phase: Phase, secs: u32, fx: &mut Effects<Effect>) {
        self.lengths.set(phase, secs);
        fx.push(Effect::Lengths(self.lengths));
    }

    /// Where the cycle stands once any bell has rung out: the phase that waits next and the
    /// lanterns in the cycle. A bell already owes its break, so this is what a relaunch resumes.
    pub fn settled(&self) -> (Phase, u8) {
        match self.clock {
            Clock::Ringing { kept, .. } => self.next(kept),
            _ => (self.phase, self.done),
        }
    }

    /// The phase after this one. Only a focus block that ran to the end counts toward the long
    /// break.
    fn next(&self, finished: bool) -> (Phase, u8) {
        match self.phase {
            Phase::Focus if finished && self.done + 1 >= CYCLE => (Phase::Long, CYCLE),
            Phase::Focus if finished => (Phase::Short, self.done + 1),
            Phase::Focus => (Phase::Short, self.done),
            Phase::Short => (Phase::Focus, self.done),
            Phase::Long => (Phase::Focus, 0),
        }
    }

    /// Moves to the next phase, wound to its length.
    fn advance_by(&mut self, finished: bool, fx: &mut Effects<Effect>) {
        let (phase, done) = self.next(finished);
        fx.push(Effect::Advanced { from: self.phase, to: phase });
        self.phase = phase;
        self.done = done;
        self.clock = Clock::Wound;
    }
}

impl Ended {
    fn of(outcome: Outcome, focused: u32, run: Run) -> Ended {
        Ended { outcome, planned: run.planned, focused, pauses: run.pauses, paused: run.paused.round() as u32 }
    }
}

/// Seconds to the nearest whole minute, within the lengths a phase can take.
pub fn snap(secs: f64) -> u32 {
    ((secs / 60.0).round() * 60.0).clamp(f64::from(MIN_SECS), f64::from(MAX_SECS)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_end(t: &mut Timer, now: &mut f64) -> Vec<Effect> {
        let mut fx = Vec::new();
        while let Some(at) = t.due() {
            *now = at;
            fx.extend(t.step(Event::Due { now: at }));
        }
        fx
    }

    fn ended(fx: &[Effect]) -> Vec<Ended> {
        fx.iter().filter_map(|e| if let Effect::Ended(e) = e { Some(e.clone()) } else { None }).collect()
    }

    #[test]
    fn a_press_starts_the_block_and_digits_count_whole_seconds() {
        let mut t = Timer::default();
        let fx: Vec<_> = t.step(Event::Press { now: 10.0 }).into_iter().collect();
        assert!(matches!(t.clock, Clock::Ticking { ends_at: 1510.0, tick: 11.0, .. }));
        assert_eq!(fx, vec![Effect::WakeAt(11.0)], "first redraw when 25:00 becomes 24:59");
        assert_eq!(t.shown(10.0), 1500);
        assert_eq!(t.shown(10.4), 1500);
        assert_eq!(t.shown(11.0), 1499);
    }

    #[test]
    fn a_wake_that_lands_exactly_on_a_second_still_ticks_on() {
        let mut t = Timer::default();
        t.step(Event::Press { now: 0.3 });
        let fx: Vec<_> = t.step(Event::Due { now: 1.3 }).into_iter().collect();
        assert!(matches!(fx[..], [Effect::WakeAt(at)] if (at - 2.3).abs() < 1e-9), "{fx:?}");
        assert_eq!(t.shown(1.3), 1499);
        assert!(t.step(Event::Due { now: 2.0 }).into_iter().next().is_none(), "an early wake changes nothing");
    }

    #[test]
    fn pausing_keeps_the_seconds_left_and_resuming_continues_from_them() {
        let mut t = Timer::default();
        t.step(Event::Press { now: 0.0 });
        t.step(Event::Press { now: 60.5 });
        assert!(matches!(t.clock, Clock::Paused { secs: 1440, .. }));
        t.step(Event::Press { now: 100.0 });
        assert!(matches!(t.clock, Clock::Ticking { ends_at: 1540.0, tick: 101.0, .. }));
    }

    #[test]
    fn a_finished_block_rings_records_itself_and_winds_a_short_break() {
        let mut t = Timer::default();
        let mut now = 0.0;
        t.step(Event::Press { now });
        let fx = run_to_end(&mut t, &mut now);
        assert_eq!(now, 1500.0 + RING);
        assert!(fx.contains(&Effect::Rang(Phase::Focus)));
        assert_eq!(
            ended(&fx),
            vec![Ended { outcome: Outcome::Finished, planned: 1500, focused: 1500, pauses: vec![], paused: 0 }]
        );
        assert_eq!((t.phase, t.done, &t.clock, t.length()), (Phase::Short, 1, &Clock::Wound, 300));
    }

    #[test]
    fn a_block_remembers_where_it_was_paused_and_for_how_long() {
        let mut t = Timer::default();
        let mut now = 0.0;
        t.step(Event::Press { now });
        t.step(Event::Press { now: 375.0 });
        t.step(Event::Press { now: 435.0 });
        t.step(Event::Press { now: 1185.0 });
        t.step(Event::Press { now: 1215.0 });
        let fx = run_to_end(&mut t, &mut now);
        let [block] = &ended(&fx)[..] else { panic!("{fx:?}") };
        assert_eq!(block.outcome, Outcome::Finished);
        assert_eq!(block.pauses, vec![0.25, 0.75], "a quarter in, then three quarters in");
        assert_eq!(block.paused, 90);
        assert_eq!(block.focused, 1500, "paused time isn't focus time");
        assert_eq!(now, 1500.0 + 90.0 + RING);
    }

    #[test]
    fn letting_a_block_go_keeps_its_minutes_but_not_its_place_in_the_cycle() {
        let mut t = Timer { done: 2, ..Timer::default() };
        let mut now = 0.0;
        t.step(Event::Press { now });
        let mut fx: Vec<_> = t.step(Event::Skip { now: 540.0 }).into_iter().collect();
        assert!(matches!(t.clock, Clock::Ringing { kept: false, .. }), "it rings out quietly first");
        assert!(!fx.contains(&Effect::Rang(Phase::Focus)), "no bell, no notification");
        fx.extend(run_to_end(&mut t, &mut now));
        assert_eq!(
            ended(&fx),
            vec![Ended { outcome: Outcome::Skipped, planned: 1500, focused: 540, pauses: vec![], paused: 0 }]
        );
        assert_eq!((t.phase, t.done), (Phase::Short, 2));
    }

    #[test]
    fn letting_go_while_paused_counts_the_pause_still_in_progress() {
        let mut t = Timer::default();
        t.step(Event::Press { now: 0.0 });
        t.step(Event::Press { now: 60.0 });
        let fx: Vec<_> = t.step(Event::Skip { now: 660.0 }).into_iter().collect();
        let [block] = &ended(&fx)[..] else { panic!("{fx:?}") };
        assert_eq!((block.outcome, block.focused, block.paused, block.pauses.len()), (Outcome::Skipped, 60, 600, 1));
    }

    #[test]
    fn a_skipped_block_can_be_taken_back_through_the_break_until_it_starts() {
        let mut t = Timer::default();
        let mut now = 0.0;
        t.step(Event::Press { now });
        t.step(Event::Skip { now: 600.0 });
        t.step(Event::Due { now: 600.0 + RING });
        t.step(Event::Nudge { minutes: 2 });
        assert_eq!(t.phase, Phase::Short, "the break waits after the quiet ring");
        now = 900.0;
        assert!(t.can_undo(), "minutes later, and after setting the break's length");
        let fx: Vec<_> = t.step(Event::Undo { now }).into_iter().collect();
        assert!(fx.contains(&Effect::Unended), "{fx:?}");
        assert_eq!(t.phase, Phase::Focus);
        assert!(matches!(t.clock, Clock::Ticking { ends_at: 1500.0, .. }), "it ran on all along: {:?}", t.clock);
        let fx = run_to_end(&mut t, &mut now);
        assert_eq!(ended(&fx).len(), 1, "and finishes as one block");
        assert_eq!((t.phase, t.done), (Phase::Short, 1));
    }

    #[test]
    fn a_skip_cant_be_taken_back_once_the_break_starts() {
        let mut t = Timer::default();
        t.step(Event::Press { now: 0.0 });
        t.step(Event::Skip { now: 200.0 });
        let mut now = 200.0;
        run_to_end(&mut t, &mut now);
        assert_eq!(now, 200.0 + RING, "nothing is due once the quiet ring is over");
        t.step(Event::Press { now });
        assert!(!t.can_undo());
        let before = t.clone();
        assert!(t.step(Event::Undo { now }).into_iter().next().is_none());
        assert_eq!(t, before);
        let mut paused = Timer::default();
        paused.step(Event::Press { now: 0.0 });
        paused.step(Event::Press { now: 100.0 });
        paused.step(Event::Skip { now: 200.0 });
        paused.step(Event::Undo { now: 201.0 });
        assert!(
            matches!(paused.clock, Clock::Paused { secs: 1400, since: 100.0, .. }),
            "a paused block comes back paused"
        );
    }

    #[test]
    fn a_relaunch_resumes_an_earned_break() {
        let mut t = Timer::resume(Phase::Long, CYCLE, Lengths::default());
        let mut now = 0.0;
        assert_eq!(t.length(), 900);
        t.step(Event::Press { now });
        let fx = run_to_end(&mut t, &mut now);
        assert!(fx.contains(&Effect::Advanced { from: Phase::Long, to: Phase::Focus }), "{fx:?}");
        assert_eq!((t.phase, t.done), (Phase::Focus, 0));
    }

    #[test]
    fn four_focus_blocks_earn_a_long_break_and_the_cycle_starts_over() {
        use Phase::*;
        let mut t = Timer::default();
        let mut now = 0.0;
        let mut phases = Vec::new();
        for _ in 0..9 {
            t.step(Event::Press { now });
            run_to_end(&mut t, &mut now);
            phases.push((t.phase, t.done));
        }
        assert_eq!(
            phases,
            vec![
                (Short, 1),
                (Focus, 1),
                (Short, 2),
                (Focus, 2),
                (Short, 3),
                (Focus, 3),
                (Long, 4),
                (Focus, 0),
                (Short, 1),
            ]
        );
    }

    #[test]
    fn a_press_while_ringing_starts_the_break_without_waiting_for_the_bell() {
        let mut t = Timer::default();
        t.step(Event::Press { now: 0.0 });
        t.step(Event::Due { now: 1500.0 });
        assert!(matches!(t.clock, Clock::Ringing { .. }));
        t.step(Event::Press { now: 1500.3 });
        assert_eq!(t.phase, Phase::Short);
        assert!(matches!(t.clock, Clock::Ticking { ends_at: 1800.3, .. }), "{:?}", t.clock);
    }

    #[test]
    fn dragging_sets_whole_minutes_and_letting_go_only_sets_the_length() {
        let mut t = Timer::default();
        t.step(Event::Scrub { secs: -20.0 * 60.0 + 25.0 });
        assert_eq!(t.shown(0.0), 5 * 60, "5:25 of travel lands on 5 minutes");
        t.step(Event::Scrub { secs: -60.0 });
        assert_eq!(t.shown(0.0), 4 * 60);
        let fx: Vec<_> = t.step(Event::Release).into_iter().collect();
        assert_eq!(t.clock, Clock::Wound, "it waits for a press, as after the arrow keys");
        assert!(fx.contains(&Effect::Lengths(Lengths { focus: 240, ..Lengths::default() })), "{fx:?}");
        t.step(Event::Press { now: 2.0 });
        assert!(matches!(t.clock, Clock::Ticking { ends_at: 242.0, tick: 3.0, .. }));
    }

    #[test]
    fn a_drag_during_a_break_sets_the_next_block_and_the_break_runs_on() {
        let mut t = Timer { phase: Phase::Short, ..Timer::default() };
        t.step(Event::Press { now: 0.0 });
        assert_eq!(t.settable(), Some(Phase::Focus));
        t.step(Event::Scrub { secs: 5.0 * 60.0 + 20.0 });
        t.step(Event::Press { now: 1.0 });
        assert!(matches!(t.clock, Clock::Ticking { .. }), "a press waits for the hand to let go");
        assert_eq!((t.length(), t.length_of(Phase::Focus)), (300, 30 * 60), "the break keeps its own length");
        let fx: Vec<_> = t.step(Event::Release).into_iter().collect();
        assert!(fx.contains(&Effect::Lengths(Lengths { focus: 30 * 60, ..Lengths::default() })), "{fx:?}");
        let mut now = 0.0;
        run_to_end(&mut t, &mut now);
        assert_eq!((t.phase, t.length()), (Phase::Focus, 30 * 60));
    }

    #[test]
    fn a_break_that_ends_under_the_hand_keeps_the_length_it_is_setting() {
        let mut t = Timer { phase: Phase::Short, ..Timer::default() };
        let mut now = 0.0;
        t.step(Event::Press { now });
        t.step(Event::Scrub { secs: -10.0 * 60.0 });
        run_to_end(&mut t, &mut now);
        assert_eq!((t.phase, t.length()), (Phase::Focus, 15 * 60), "the next block waits, still in hand");
        t.step(Event::Release);
        assert_eq!((t.setting, t.lengths.of(Phase::Focus)), (None, 15 * 60));
    }

    #[test]
    fn a_bell_already_owes_the_break_it_rings_in() {
        let mut t = Timer { done: 3, ..Timer::default() };
        t.step(Event::Press { now: 0.0 });
        t.step(Event::Due { now: 1500.0 });
        assert_eq!((t.phase, t.settled()), (Phase::Focus, (Phase::Long, CYCLE)), "the fourth bell owes the long break");
        let mut skipped = Timer { done: 3, ..Timer::default() };
        skipped.step(Event::Press { now: 0.0 });
        skipped.step(Event::Skip { now: 10.0 });
        assert_eq!(skipped.settled(), (Phase::Short, 3));
    }

    #[test]
    fn lengths_stay_between_a_minute_and_an_hour() {
        let mut t = Timer::default();
        t.step(Event::Scrub { secs: -1e6 });
        assert_eq!(t.shown(0.0), 60);
        t.step(Event::Scrub { secs: 1e6 });
        assert_eq!(t.shown(0.0), MAX_SECS);
        let mut short = Timer { phase: Phase::Short, ..Timer::default() };
        short.step(Event::Nudge { minutes: -10 });
        assert_eq!(short.length(), 60);
    }

    #[test]
    fn a_set_length_is_remembered_for_the_next_block() {
        let mut t = Timer::default();
        let mut now = 0.0;
        t.step(Event::Nudge { minutes: 5 });
        t.step(Event::Press { now });
        run_to_end(&mut t, &mut now);
        t.step(Event::Skip { now });
        assert_eq!((t.phase, t.length()), (Phase::Focus, 30 * 60));
    }

    #[test]
    fn nudges_and_drags_leave_a_running_block_alone() {
        let mut t = Timer::default();
        t.step(Event::Press { now: 0.0 });
        let before = t.clone();
        t.step(Event::Nudge { minutes: 2 });
        t.step(Event::Scrub { secs: 600.0 });
        assert_eq!(t, before);
    }

    #[test]
    fn skipping_a_block_that_never_started_records_nothing() {
        let mut t = Timer::default();
        let fx: Vec<_> = t.step(Event::Skip { now: 0.0 }).into_iter().collect();
        assert_eq!(ended(&fx), []);
        assert_eq!((t.phase, t.done, &t.clock), (Phase::Short, 0, &Clock::Wound));
        t.step(Event::Skip { now: 0.0 });
        assert_eq!((t.phase, t.length()), (Phase::Focus, 1500));
    }

    #[test]
    fn breaks_are_never_recorded() {
        let mut t = Timer { phase: Phase::Short, ..Timer::default() };
        let mut now = 0.0;
        t.step(Event::Press { now });
        let mut fx = run_to_end(&mut t, &mut now);
        t.step(Event::Skip { now });
        t.phase = Phase::Long;
        t.step(Event::Press { now });
        fx.extend(t.step(Event::Skip { now: now + 10.0 }));
        assert!(ended(&fx).is_empty(), "{fx:?}");
    }

    #[test]
    fn a_block_saved_as_let_go_before_the_rename_loads_as_skipped() {
        let old: Outcome = serde_json::from_str("\"let_go\"").unwrap();
        assert_eq!((old, serde_json::to_string(&old).unwrap()), (Outcome::Skipped, "\"skipped\"".into()));
    }
}
