//! The bell you hear: rung by the end of a block or a break, through the timer and its effects,
//! into the app's own players. Rune keeps them silent here and runs their clocks on the window's,
//! so a check reads where each bell is in its sound.

use rune::PlayerState;

use super::*;

/// Where each bell is: whether it sounds now, and how far into its sound it is.
fn heard(app: &App) -> [(bool, f64); 2] {
    let b = &app.rt.state.sounds;
    [&b.sunset, &b.rested].map(|p| (p.is_playing(), (p.time() * 10.0).round() / 10.0))
}

fn sound_item(app: &App) -> bool {
    menu::items(&app.rt.state).into_iter().find(|i| i.id == "view-sound").unwrap().checked
}

fn pick_sound(app: &mut App) {
    app.rt.update(|s, tx| menu::action("view-sound")(s, tx));
    app.wait(STEP);
}

#[test]
fn a_block_rings_the_low_bell_once_and_its_break_the_light_one() {
    let mut app = App::new();
    assert!(sound_item(&app), "Sound is on from the first launch");
    app.one_minute();
    app.space();
    app.wait(59.0);
    assert_eq!(heard(&app), [(false, 0.0); 2], "silent while the sun sets");
    app.wait(1.5);
    assert_eq!(heard(&app), [(true, 0.5), (false, 0.0)], "the sun is down: the low bell, struck at the water");
    app.wait(1.5);
    assert_eq!(app.rt.state.sounds.sunset.state(), PlayerState::Ended, "it dies away after one strike");
    app.wait(RING);
    assert!(!heard(&app)[0].0, "and isn't struck again while the bell's ripples run out");
    app.space();
    app.wait(300.5);
    assert_eq!(heard(&app), [(false, 1.6), (true, 0.5)], "the break is over: the lighter bell, once");
    app.wait(RING + 2.0);
    assert_eq!(
        [&app.rt.state.sounds.sunset, &app.rt.state.sounds.rested].map(rune::Player::state),
        [PlayerState::Ended, PlayerState::Ended],
        "nothing rings again while the next block waits"
    );
}

#[test]
fn a_skipped_block_rings_no_bell() {
    let mut app = App::new();
    app.one_minute();
    app.space();
    app.wait(20.0);
    app.char("s");
    app.wait(RING + 1.0);
    assert_eq!(heard(&app), [(false, 0.0); 2]);
}

#[test]
fn with_sound_off_in_the_view_menu_no_bell_sounds_and_it_stays_off() {
    let mut app = App::new();
    pick_sound(&mut app);
    assert!(!sound_item(&app), "Sound is unticked");
    app.one_minute();
    app.run_to_bell();
    assert_eq!(app.lanterns(), 1, "the block ends and lights its lantern as ever");
    assert_eq!(heard(&app), [(false, 0.0); 2], "but nothing sounds");
    let mut back = app.relaunch(60.0);
    assert!(!sound_item(&back), "still off after a relaunch");
    back.space();
    back.wait(300.5);
    assert_eq!(heard(&back), [(false, 0.0); 2], "the break ends silently too");
    pick_sound(&mut back);
    back.space();
    back.wait(60.5);
    assert_eq!(heard(&back), [(true, 0.5), (false, 0.0)], "on again, the next sunset rings");
}
