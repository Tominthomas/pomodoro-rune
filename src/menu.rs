//! The menu bar: Timer, View and Help, the dock's actions under the names and keys the dock
//! gives them. The items follow the state, so a title says what its key does now ("Pause" while a
//! block runs) and an item that would do nothing is dimmed, as the dock leaves it out.

use rune::{MenuItem, Shortcut, Tx};

use crate::controls::{press_words, skip_words};
use crate::theme::PLACES;
use crate::timer::Event;
use crate::{Page, State, carry, open_today, set_always_time, set_larger, set_place};

/// One menu item as the state has it now. Its action is looked up by `id`.
#[derive(Clone, PartialEq, Debug)]
pub struct Item {
    pub id: &'static str,
    pub menu: &'static str,
    pub title: String,
    /// The key the item names, which the window's own keys take first: a bare letter typed into
    /// a name stays a letter.
    pub key: &'static str,
    pub enabled: bool,
    pub checked: bool,
    /// A separator goes above it.
    pub gap: bool,
}

/// Picks each place, in `PLACES` order.
const PICK: [fn(&mut State, &mut Tx); 3] =
    [|s, tx| set_place(s, 0, tx), |s, tx| set_place(s, 1, tx), |s, tx| set_place(s, 2, tx)];
const _: () = assert!(PICK.len() == PLACES.len(), "one menu item per place");
const PLACE_IDS: [&str; 3] = ["place-0", "place-1", "place-2"];

/// The items the state calls for now.
pub fn items(s: &State) -> Vec<Item> {
    let t = &s.timer;
    let item = |id, menu, title: &str, key| Item {
        id,
        menu,
        title: title_case(title),
        key,
        enabled: true,
        checked: false,
        gap: false,
    };
    let skip = skip_words(t);
    let mut out = vec![
        item("timer-press", "Timer", press_words(t), "space"),
        Item { enabled: skip.is_some(), ..item("timer-skip", "Timer", skip.map_or("Skip", |w| w.0), "s") },
        Item { enabled: t.can_undo(), gap: true, ..item("timer-undo", "Timer", "Undo skip", "cmd+z") },
        Item { checked: matches!(s.page, Page::Today(_)), ..item("view-today", "View", "Today", "h") },
    ];
    for (i, (place, id)) in PLACES.iter().zip(PLACE_IDS).enumerate() {
        out.push(Item { checked: s.place == i, gap: i == 0, ..item(id, "View", place.name, "") });
    }
    out.push(item("view-next-place", "View", "Next place", "t"));
    out.push(Item { checked: s.larger, gap: true, ..item("view-larger", "View", "Larger text", "cmd+=") });
    out.push(Item { checked: s.always_time, ..item("view-always-time", "View", "Always show time", "") });
    out.push(item("help-keys", "Help", "Keys", "slash"));
    out
}

/// "Skip this block" as a menu says it: "Skip This Block".
fn title_case(words: &str) -> String {
    words
        .split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn action(id: &str) -> fn(&mut State, &mut Tx) {
    match id {
        "timer-press" => |s, tx| carry(s, Event::Press { now: tx.clock() }, tx),
        "timer-skip" => |s, tx| carry(s, Event::Skip { now: tx.clock() }, tx),
        "timer-undo" => |s, tx| carry(s, Event::Undo { now: tx.clock() }, tx),
        "view-today" => |s, tx| open_today(s, matches!(s.page, Page::Scene), tx),
        "view-next-place" => |s, tx| set_place(s, s.place + 1, tx),
        "view-larger" => |s, tx| set_larger(s, !s.larger, tx),
        "view-always-time" => |s, _| set_always_time(s, !s.always_time),
        "help-keys" => |s, _| s.keys_open = !s.keys_open,
        place => PLACE_IDS.iter().position(|p| *p == place).map_or(|_, _| {}, |i| PICK[i]),
    }
}

/// Hands the menu bar the items the state calls for, when they changed since it was last told.
/// Tests run without a menu bar, and so without a handle.
pub fn sync(s: &State) {
    let Some(handle) = &s.menu_bar else { return };
    let now = items(s);
    if *s.menu_shown.borrow() == now {
        return;
    }
    let mut built = Vec::new();
    for i in &now {
        if i.gap {
            built.push(MenuItem::separator().menu(i.menu));
        }
        let mut m = MenuItem::new(i.title.clone(), action(i.id))
            .id(i.id)
            .menu(i.menu)
            .enabled(i.enabled)
            .checked(i.checked)
            .keys_first();
        if let Ok(key) = i.key.parse::<Shortcut>() {
            m = m.key_equivalent(key);
        }
        built.push(m);
    }
    if let Err(why) = handle.set_menu(built) {
        eprintln!("pomodoro: the menu bar wasn't updated: {why}");
    }
    *s.menu_shown.borrow_mut() = now;
}
