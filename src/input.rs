//! What keys and the pointer on the sky do: the shortcuts, Escape closing the innermost thing
//! open, and the sky's tap and drag.

use rune::{Drag, Key, KeyEvent, Phase as DragPhase, Tx};

use crate::log::Felt;
use crate::timer::{Clock, Event};
use crate::{
    Asking, Page, SLOP, State, carry, edit_label, finish_label, open_today, rate, scene, set_larger, set_place,
};

/// Escape closes whatever is open, innermost first.
pub fn escape(s: &mut State, tx: &mut Tx) -> bool {
    if s.editing {
        finish_label(s, false, tx);
    } else if s.keys_open {
        s.keys_open = false;
    } else if let Page::Today(nav) = &mut s.page {
        if !nav.escape() {
            open_today(s, false, tx);
        }
    } else if s.asking() != Asking::Hidden {
        s.reflect = None;
    } else if s.keyboard {
        s.keyboard = false;
    } else {
        return false;
    }
    true
}

pub fn on_key(s: &mut State, e: &KeyEvent, tx: &mut Tx) -> bool {
    let now = tx.clock();
    if e.key == Key::Escape {
        return escape(s, tx);
    }
    if e.is_command('=') || e.is_command('+') || e.is_command('-') || e.is_command('0') {
        set_larger(s, e.is_command('=') || e.is_command('+'), tx);
        return true;
    }
    if e.is_command('z') {
        carry(s, Event::Undo { now }, tx);
        return true;
    }
    if let (true, Key::Text(t)) = (s.editing, &e.key) {
        // A name is being typed, so letters are words, never shortcuts. Only a field that lost
        // focus lets one reach here: it takes the letter and focus back.
        s.label.insert(t);
        s.label.focused = true;
        tx.focus("label-field", false);
        return true;
    }
    if e.key == Key::Tab && tx.focused().is_none() && matches!(s.page, Page::Scene) && !s.keyboard {
        // Tab brings the controls out first, so focus never lands on something unseen.
        s.keyboard = true;
        tx.focus("control-play", true);
        return true;
    }
    let on_sky = matches!(s.page, Page::Scene) && !s.keys_open && !s.editing;
    let event = match &e.key {
        Key::Text(t) if t == " " => Event::Press { now },
        Key::Up | Key::Right if on_sky => Event::Nudge { minutes: 1 },
        Key::Down | Key::Left if on_sky => Event::Nudge { minutes: -1 },
        Key::Text(t) => match t.to_ascii_lowercase().as_str() {
            "s" => Event::Skip { now },
            "u" => Event::Undo { now },
            "h" => {
                open_today(s, matches!(s.page, Page::Scene), tx);
                return true;
            }
            "t" => {
                set_place(s, s.place + 1, tx);
                return true;
            }
            "l" => {
                edit_label(s, tx);
                return true;
            }
            "?" | "/" => {
                s.keys_open = !s.keys_open;
                return true;
            }
            n @ ("1" | "2" | "3") if s.asking() == Asking::Open => {
                rate(s, Felt::ALL[n.parse::<usize>().unwrap_or(1) - 1], tx);
                return true;
            }
            _ => return false,
        },
        _ => return false,
    };
    carry(s, event, tx);
    true
}

/// A tap on the sky starts or pauses; dragging up or down while the block waits raises or
/// lowers the sun, which sets the block's length, and letting go keeps it, waiting for a press
/// as the arrow keys do. The click that brings the window forward does neither.
pub fn drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    let now = tx.clock();
    match d.phase {
        DragPhase::Down => {
            d.data = d.y;
            s.spent = s.activating_press(tx);
            if s.editing {
                // A click on the sky while naming keeps the name.
                finish_label(s, true, tx);
                s.spent = true;
            }
        }
        DragPhase::Move => {
            let scrubbing = matches!(s.timer.clock, Clock::Scrubbing { .. });
            if !scrubbing && (s.spent || d.dx.hypot(d.dy) < SLOP || s.timer.clock != Clock::Wound) {
                return;
            }
            let from = match s.timer.clock {
                Clock::Scrubbing { secs } => secs,
                _ => f64::from(s.timer.length()),
            };
            // The pointer moves in the view's points; the sun in the window's.
            let to = scene::length_at(scene::rise(from) + f64::from((d.data - d.y) * s.zoom()));
            d.data = d.y;
            carry(s, Event::Scrub { secs: to - from }, tx);
        }
        DragPhase::Up if matches!(s.timer.clock, Clock::Scrubbing { .. }) => carry(s, Event::Release, tx),
        DragPhase::Up if !s.spent && !d.cancelled() && d.dx.hypot(d.dy) < SLOP => {
            carry(s, Event::Press { now }, tx);
        }
        DragPhase::Up => {}
    }
}
