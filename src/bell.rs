//! The bell you hear: a low, soft one when the sun reaches the water, a lighter, higher one when a
//! break is over. `tools/bell.py` makes both. The ripples on the water are the same bell, seen.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rune::Player;

use crate::timer::Phase;

const SUNSET: (&str, &[u8]) = ("sunset.m4a", include_bytes!("../assets/sounds/sunset.m4a"));
const RESTED: (&str, &[u8]) = ("rested.m4a", include_bytes!("../assets/sounds/rested.m4a"));

/// The two bells, ready to ring. Headless runs and tests have them too, and Rune keeps them silent.
pub struct Bells {
    pub sunset: Player,
    pub rested: Player,
}

impl Bells {
    /// The bells, written out of the binary into the temporary folder, so they ring however the
    /// app was started and wherever it was moved.
    pub fn new() -> Bells {
        let dir = std::env::temp_dir().join(crate::APP_ID);
        Bells { sunset: Player::audio(write_out(&dir, SUNSET)), rested: Player::audio(write_out(&dir, RESTED)) }
    }

    /// Rings the bell for the end of `finished`.
    pub fn ring(&self, finished: Phase) {
        match finished {
            Phase::Focus => &self.sunset,
            Phase::Short | Phase::Long => &self.rested,
        }
        .play();
    }
}

/// Where the sound `bytes` named `name` is on disk, written there first unless it already is.
/// Two windows (or test threads) can start at once, so each writes a file of its own and renames
/// it into place: whoever reads the path finds a whole file.
fn write_out(dir: &Path, (name, bytes): (&str, &[u8])) -> PathBuf {
    static WRITES: AtomicU32 = AtomicU32::new(0);
    let path = dir.join(name);
    if fs::read(&path).is_ok_and(|on_disk| on_disk == bytes) {
        return path;
    }
    let part = dir.join(format!("{name}.{}.{}", std::process::id(), WRITES.fetch_add(1, Ordering::Relaxed)));
    let written = fs::create_dir_all(dir)
        .and_then(|()| fs::File::create_new(&part))
        .and_then(|mut f| f.write_all(bytes))
        .and_then(|()| fs::rename(&part, &path));
    if let Err(why) = written {
        eprintln!("pomodoro: the bell {name} can't be written to {}, so it won't sound: {why}", dir.display());
    }
    path
}
