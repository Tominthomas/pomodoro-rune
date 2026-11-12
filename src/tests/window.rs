//! The window at the sizes a person drags it to: the scene fills it, the readout and the dock
//! keep their corners, and Today uses the width.

use super::*;

/// The sizes the checks take: the smallest, a tall one and a wide one.
const SIZES: [(f32, f32); 3] = [(W, H), (420.0, 780.0), (900.0, 600.0)];

#[test]
fn the_scene_fills_every_size_and_the_readout_and_dock_keep_their_corners() {
    for (w, h) in SIZES {
        let mut app = App::new();
        app.resize(w, h);
        assert_eq!(app.rect("sky"), [0.0, 0.0, w, h], "the sky fills {w} x {h}");
        let time = app.rect("time");
        assert_eq!(
            (time[0], (h - time[1] - 184.0).abs() < 25.0),
            (24.0, true),
            "the time keeps the bottom left at {w} x {h}"
        );
        let dock = app.rect("controls");
        assert_eq!(
            (dock[0] + dock[2], dock[1] + dock[3]),
            (w - 14.0, h - 14.0),
            "the dock keeps the bottom right at {w} x {h}"
        );
        let play = app.rect("control-play");
        app.space();
        app.wait(1.0);
        assert_eq!((app.time(), app.rect("control-play")), ("24:59".into(), play), "and Pause takes its place");
    }
}

#[test]
fn today_sets_the_four_weeks_beside_the_day_when_the_window_is_wide() {
    let mut app = App::scene("today");
    let (day, weeks) = (app.rect("today-day"), app.rect("history"));
    assert!(weeks[1] > day[1] + 200.0 && (weeks[0] - day[0]).abs() < 1.0, "narrow, the weeks are under the day");
    app.resize(900.0, 600.0);
    let (day, weeks) = (app.rect("today-day"), app.rect("history"));
    assert!(weeks[0] >= day[0] + day[2] + 20.0, "wide, they stand beside it: {day:?} {weeks:?}");
    assert!(weeks[0] + weeks[2] <= 900.0 - 20.0, "and inside the window");
    app.resize(420.0, 780.0);
    let (day, weeks) = (app.rect("today-day"), app.rect("history"));
    assert!(weeks[1] > day[1] + 200.0 && day[2] > 370.0, "tall and narrow, one column as wide as the window allows");
}

#[test]
fn larger_text_draws_words_and_controls_bigger_keeps_the_scene_and_is_remembered() {
    let mut app = App::new();
    let (sun, time) = (app.rect("sun"), app.rect("time"));
    app.command('=');
    app.look();
    assert_eq!(app.rt.zoom(), 1.25, "the view is zoomed");
    let item = menu::items(&app.rt.state).into_iter().find(|i| i.id == "view-larger").unwrap();
    assert_eq!((item.title.as_str(), item.checked), ("Larger Text", true));
    let to_window = |r: [f32; 4]| r.map(|v| v * 1.25);
    assert_eq!(to_window(app.rect("sun")), sun, "the sun is where it was, the size it was");
    assert!(to_window(app.rect("time"))[3] > time[3] * 1.2, "the time is bigger");
    let mut back = app.relaunch(60.0);
    assert_eq!(back.rt.zoom(), 1.25, "and stays on across a launch");
    back.command('-');
    back.look();
    assert_eq!((back.rt.zoom(), back.rect("sun")), (1.0, sun), "⌘− puts it back");
}
