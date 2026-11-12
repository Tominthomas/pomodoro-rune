//! The sun in the menu bar: a small glyph by the clock that sets as the block does, with the whole
//! minutes left beside it, and a native menu to act on the timer. Closing the window keeps the
//! block going, so this is where it shows while the window is away. `tools/glyphs.py` draws the
//! glyphs.

use std::cell::RefCell;

use rune::{AppHandle, Tx};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::controls::{press_words, skip_words};
use crate::log::{Block, duration};
use crate::menu::title_case;
use crate::timer::{Clock, Phase, Timer};
use crate::{State, open_today, set_sound};

/// How many heights the sun takes from high in the sky to half set.
pub const HEIGHTS: u8 = 8;

/// What the status item shows: a glyph, and the whole minutes left while a phase runs or is held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MenuBar {
    pub glyph: Glyph,
    pub minutes: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    /// The sun over the water, 0 high in the sky to `HEIGHTS - 1` half set.
    Sun(u8),
    /// A hollow sun: the phase is paused.
    Held,
    /// A short break, or one that waits after a skipped block, which lit nothing.
    Lantern,
    /// A block ran to the end: its lantern is lit, and its break waits.
    LitLantern,
    /// The long break.
    Moon,
}

impl MenuBar {
    /// What the status item shows for `t` at `now`, where `lit` says the last block recorded ran to
    /// the end. The minutes round up, so `1` shows until the bell, and the sun moves only when they
    /// change, so the item changes at most once a minute.
    pub fn of(t: &Timer, now: f64, lit: bool) -> MenuBar {
        let minutes = t.shown(now).div_ceil(60).max(1);
        let (glyph, minutes) = match (t.phase, &t.clock) {
            (_, Clock::Paused { .. }) => (Glyph::Held, Some(minutes)),
            (Phase::Focus, Clock::Ticking { .. }) => {
                let whole = t.length().div_ceil(60);
                let set = (whole.saturating_sub(minutes) * u32::from(HEIGHTS) / whole) as u8;
                (Glyph::Sun(set.min(HEIGHTS - 1)), Some(minutes))
            }
            (Phase::Short, Clock::Ticking { .. }) => (Glyph::Lantern, Some(minutes)),
            (Phase::Long, Clock::Ticking { .. }) => (Glyph::Moon, Some(minutes)),
            (Phase::Focus, Clock::Ringing { kept: true, .. }) => (Glyph::LitLantern, None),
            (Phase::Short | Phase::Long, Clock::Wound) if lit => (Glyph::LitLantern, None),
            (Phase::Focus, Clock::Ringing { .. }) | (Phase::Short | Phase::Long, Clock::Wound) => {
                (Glyph::Lantern, None)
            }
            (Phase::Focus, Clock::Wound) | (_, Clock::Ringing { .. }) => (Glyph::Sun(0), None),
        };
        MenuBar { glyph, minutes }
    }
}

impl MenuBar {
    /// What the status item shows for the window's state now.
    pub fn of_state(s: &State) -> MenuBar {
        MenuBar::of(&s.timer, s.now, s.study.log.blocks.last().is_some_and(Block::finished))
    }
}

impl Glyph {
    /// The template image, 18 by 22 points drawn at 2x.
    fn png(self) -> &'static [u8] {
        const SUN: [&[u8]; HEIGHTS as usize] = [
            include_bytes!("../assets/menubar/sun-0.png"),
            include_bytes!("../assets/menubar/sun-1.png"),
            include_bytes!("../assets/menubar/sun-2.png"),
            include_bytes!("../assets/menubar/sun-3.png"),
            include_bytes!("../assets/menubar/sun-4.png"),
            include_bytes!("../assets/menubar/sun-5.png"),
            include_bytes!("../assets/menubar/sun-6.png"),
            include_bytes!("../assets/menubar/sun-7.png"),
        ];
        match self {
            Glyph::Sun(k) => SUN[usize::from(k.min(HEIGHTS - 1))],
            Glyph::Held => include_bytes!("../assets/menubar/held.png"),
            Glyph::Lantern => include_bytes!("../assets/menubar/lantern.png"),
            Glyph::LitLantern => include_bytes!("../assets/menubar/lantern-lit.png"),
            Glyph::Moon => include_bytes!("../assets/menubar/moon.png"),
        }
    }

    /// The image as straight RGBA rows, with its width and height in pixels.
    pub fn rgba(self) -> Result<(Vec<u8>, u32, u32), png::DecodingError> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(self.png()));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info()?;
        let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let info = reader.next_frame(&mut buf)?;
        buf.truncate(info.buffer_size());
        Ok((buf, info.width, info.height))
    }
}

/// What the status item's menu says now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Words {
    /// The disabled first line: `Focus · 18 min left`, with the block's name for "Focus".
    pub header: String,
    /// Start, Pause or Resume, as the window's primary control says it.
    pub press: String,
    /// Skip as it applies now; dimmed when there is nothing to skip.
    pub skip: Option<String>,
    /// `Today · 1 h 15 min · 3 lanterns`.
    pub today: String,
    pub sound: bool,
}

impl Words {
    pub fn of(s: &State, bar: MenuBar) -> Words {
        let t = &s.timer;
        let label = s.label();
        let name = match t.phase {
            Phase::Focus => label.as_deref().unwrap_or("Focus"),
            Phase::Short => "Break",
            Phase::Long => "Long break",
        };
        let left = bar.minutes.map_or_else(String::new, |m| format!("{m} min left"));
        let header = match (t.phase, &t.clock) {
            (Phase::Focus, Clock::Ringing { kept: true, .. }) => "A lantern for that one".into(),
            (Phase::Focus, Clock::Ringing { .. }) => "Skipped".into(),
            (_, Clock::Ringing { .. }) => "Break's over".into(),
            (_, Clock::Paused { .. }) => format!("{name} · paused · {left}"),
            (_, Clock::Ticking { .. }) => format!("{name} · {left}"),
            (_, Clock::Wound) => format!("{name} · {} min", t.length() / 60),
        };
        let totals = s.study.log.totals_on(s.cal, s.today());
        let today = match totals.lanterns {
            _ if totals.blocks == 0 => "Today · no blocks yet".into(),
            1 => format!("Today · {} · 1 lantern", duration(totals.focused)),
            n => format!("Today · {} · {n} lanterns", duration(totals.focused)),
        };
        Words {
            header,
            press: title_case(press_words(t)),
            skip: skip_words(t).map(|w| title_case(w.0)),
            today,
            sound: s.sound,
        }
    }
}

/// What the menu item `id` does when it is picked.
pub fn act(s: &mut State, id: &str, tx: &mut Tx) {
    let open = |s: &State| {
        if let Some(app) = &s.app {
            app.open();
        }
    };
    match id {
        "tray-press" => crate::menu::action("timer-press")(s, tx),
        "tray-skip" => crate::menu::action("timer-skip")(s, tx),
        "tray-today" => {
            open(s);
            open_today(s, true, tx);
        }
        "tray-open" => open(s),
        "tray-sound" => set_sound(s, !s.sound),
        "tray-quit" => tx.quit(),
        _ => {}
    }
}

/// The status item and the menu items that change, with what they were last told.
pub struct Tray {
    icon: TrayIcon,
    header: MenuItem,
    press: MenuItem,
    skip: MenuItem,
    today: MenuItem,
    sound: CheckMenuItem,
    shown: RefCell<Option<(MenuBar, Words)>>,
}

impl Tray {
    /// Puts the sun in the menu bar. It must be called on the main thread, from a turn of the
    /// window's loop; its menu's picks come back to the window through `handle`.
    pub fn new(handle: AppHandle<State>) -> Option<Tray> {
        let header = MenuItem::new("", false, None);
        let press = MenuItem::with_id("tray-press", "", true, None);
        let skip = MenuItem::with_id("tray-skip", "Skip", false, None);
        let today = MenuItem::with_id("tray-today", "", true, None);
        let sound = CheckMenuItem::with_id("tray-sound", "Sound", true, true, None);
        let gap = PredefinedMenuItem::separator;
        let menu = Menu::with_items(&[
            &header,
            &gap(),
            &press,
            &skip,
            &gap(),
            &today,
            &MenuItem::with_id("tray-open", "Open Pomodoro", true, None),
            &sound,
            &gap(),
            &MenuItem::with_id("tray-quit", "Quit Pomodoro", true, None),
        ]);
        let icon = menu.map_err(|e| e.to_string()).and_then(|menu| {
            TrayIconBuilder::new()
                .with_id("pomodoro")
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(true)
                .build()
                .map_err(|e| e.to_string())
        });
        let icon = match icon {
            Ok(icon) => icon,
            Err(why) => {
                eprintln!("pomodoro: the sun can't go in the menu bar: {why}");
                return None;
            }
        };
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let id = e.id().0.clone();
            handle.update(move |s, tx| act(s, &id, tx));
        }));
        Some(Tray { icon, header, press, skip, today, sound, shown: RefCell::new(None) })
    }

    fn show(&self, s: &State) {
        let bar = MenuBar::of_state(s);
        let words = Words::of(s, bar);
        let mut shown = self.shown.borrow_mut();
        let before = shown.as_ref();
        if before.map(|(b, _)| b.glyph) != Some(bar.glyph) {
            let icon = bar
                .glyph
                .rgba()
                .map_err(|e| e.to_string())
                .and_then(|(rgba, w, h)| Icon::from_rgba(rgba, w, h).map_err(|e| e.to_string()));
            if let Err(why) = icon.and_then(|i| self.icon.set_icon_templated(Some(i)).map_err(|e| e.to_string())) {
                eprintln!("pomodoro: the menu bar's sun wasn't updated: {why}");
            }
        }
        if before.map(|(b, _)| b.minutes) != Some(bar.minutes) {
            self.icon.set_title(bar.minutes.map(|m| m.to_string()));
        }
        if before.map(|(_, w)| w) != Some(&words) {
            self.header.set_text(&words.header);
            self.press.set_text(&words.press);
            self.skip.set_text(words.skip.as_deref().unwrap_or("Skip"));
            self.skip.set_enabled(words.skip.is_some());
            self.today.set_text(&words.today);
            self.sound.set_checked(words.sound);
            if let Err(why) = self.icon.set_tooltip(Some(&words.header)) {
                eprintln!("pomodoro: the menu bar's sun has no tooltip: {why}");
            }
        }
        *shown = Some((bar, words));
    }
}

/// Hands the status item and its menu what the state calls for, when it changed since they were
/// last told. Tests and headless runs have no status item.
pub fn sync(s: &State) {
    if let Some(tray) = &s.tray {
        tray.show(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::{Event, Lengths, RING};

    /// A timer driven through `events` from a fresh 25-minute block.
    fn after(events: &[Event]) -> Timer {
        let mut t = Timer::resume(Phase::Focus, 0, Lengths::default());
        for e in events {
            t.step(*e);
        }
        t
    }

    fn press(now: f64) -> Event {
        Event::Press { now }
    }

    fn due(now: f64) -> Event {
        Event::Due { now }
    }

    fn bar(glyph: Glyph, minutes: Option<u32>) -> MenuBar {
        MenuBar { glyph, minutes }
    }

    #[test]
    fn the_menu_bar_follows_the_timer_in_whole_minutes() {
        let block = 25.0 * 60.0;
        let rows: &[(&str, Vec<Event>, f64, MenuBar)] = &[
            ("waiting: the sun high, no number", vec![], 0.0, bar(Glyph::Sun(0), None)),
            ("just started", vec![press(0.0)], 0.0, bar(Glyph::Sun(0), Some(25))),
            ("one second in, still 25: the number rounds up", vec![press(0.0)], 1.0, bar(Glyph::Sun(0), Some(25))),
            ("a minute in", vec![press(0.0)], 60.0, bar(Glyph::Sun(0), Some(24))),
            ("four minutes in, the sun's a step lower", vec![press(0.0)], 240.0, bar(Glyph::Sun(1), Some(21))),
            ("half way", vec![press(0.0)], block / 2.0, bar(Glyph::Sun(3), Some(13))),
            ("the last minute: half set", vec![press(0.0)], block - 59.0, bar(Glyph::Sun(7), Some(1))),
            ("one second left still says 1", vec![press(0.0)], block - 1.0, bar(Glyph::Sun(7), Some(1))),
            ("paused: a hollow sun", vec![press(0.0), press(390.0)], 500.0, bar(Glyph::Held, Some(19))),
            ("the bell: a lit lantern", vec![press(0.0), due(block)], block, bar(Glyph::LitLantern, None)),
            (
                "the break waits under the lit lantern",
                vec![press(0.0), due(block), due(block + RING)],
                block + RING,
                bar(Glyph::LitLantern, None),
            ),
            (
                "the short break: a lantern and its minutes",
                vec![press(0.0), due(block), press(block + 1.0)],
                block + 1.0,
                bar(Glyph::Lantern, Some(5)),
            ),
            (
                "a skipped block lights nothing",
                vec![press(0.0), Event::Skip { now: 120.0 }],
                120.0,
                bar(Glyph::Lantern, None),
            ),
            (
                "and its break waits unlit",
                vec![press(0.0), Event::Skip { now: 120.0 }, due(120.0 + RING)],
                120.0 + RING,
                bar(Glyph::Lantern, None),
            ),
        ];
        for (why, events, now, want) in rows {
            let t = after(events);
            let lit = !t.can_undo();
            assert_eq!(MenuBar::of(&t, *now, lit), *want, "{why}");
        }
    }

    #[test]
    fn the_long_break_is_the_moon_and_its_end_brings_the_sun_back_up() {
        let mut t = Timer::resume(Phase::Long, 4, Lengths::default());
        assert_eq!(MenuBar::of(&t, 0.0, true), bar(Glyph::LitLantern, None), "four lanterns lit, the night waits");
        t.step(press(0.0));
        assert_eq!(MenuBar::of(&t, 61.0, true), bar(Glyph::Moon, Some(14)));
        t.step(press(70.0));
        assert_eq!(MenuBar::of(&t, 80.0, true), bar(Glyph::Held, Some(14)), "a held break is hollow too");
        t.step(press(80.0));
        t.step(due(15.0 * 60.0 + 10.0));
        assert_eq!(MenuBar::of(&t, 15.0 * 60.0 + 10.0, true), bar(Glyph::Sun(0), None), "the night is over");
    }

    #[test]
    fn a_short_block_still_sets_only_once_a_minute() {
        let mut t = Timer::resume(Phase::Focus, 0, Lengths::default());
        t.step(Event::Nudge { minutes: -22 });
        t.step(press(0.0));
        let seen: Vec<MenuBar> = (0..180).map(|s| MenuBar::of(&t, f64::from(s), false)).collect();
        let changes = seen.windows(2).filter(|w| w[0] != w[1]).count();
        assert_eq!(changes, 2, "three minutes, three looks: {seen:?}");
        assert_eq!(seen.last(), Some(&bar(Glyph::Sun(5), Some(1))));
    }

    #[test]
    fn every_glyph_is_a_template_the_menu_bar_can_hold() {
        let glyphs = (0..HEIGHTS).map(Glyph::Sun).chain([Glyph::Held, Glyph::Lantern, Glyph::LitLantern, Glyph::Moon]);
        for glyph in glyphs {
            let (rgba, w, h) = glyph.rgba().unwrap();
            assert_eq!((w, h), (36, 44), "{glyph:?} is 18 by 22 points at 2x");
            assert_eq!(rgba.len(), 36 * 44 * 4);
            assert!(rgba.chunks(4).all(|p| p[..3] == [0, 0, 0]), "{glyph:?} is black on clear, so macOS can tint it");
            assert!(rgba.chunks(4).any(|p| p[3] == 255), "{glyph:?} has ink");
        }
    }
}
