//! Saving, off the UI thread. `Prefs::set` rewrites and syncs the whole file, so the window
//! never calls it: it hands each change to one writer thread, which writes in the order keys
//! first arrived and keeps only the latest change of a key that changed again before its turn.
//! A watchdog beside it says when a write has stalled, and quitting waits for the writer only
//! so long.

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use rune::prefs::Prefs;
use serde::Serialize;
use serde_json::Value;

/// A write still running after this long is reported as stalled.
const STALL: Duration = Duration::from_secs(4);

type Encode = Box<dyn FnOnce() -> Result<Value, String> + Send>;

enum Msg {
    /// A value to save under a key, or `None` to remove the key.
    Set(String, Option<Encode>),
    /// Hears, once everything queued before it is written, why the record isn't saved, if it isn't.
    Flush(Sender<Option<String>>),
}

/// What the watchdog hears from the writer.
enum Writing {
    Began,
    Ended(Option<String>),
}

/// Where settings and the record are saved. Headless runs and tests use [`Store::memory`],
/// which keeps nothing.
pub struct Store {
    tx: Option<Sender<Msg>>,
}

impl Store {
    pub fn memory() -> Store {
        Store { tx: None }
    }

    /// Saves into `prefs` from a writer thread. `report` hears each change between saving and
    /// not: the reason once a save fails or stalls, then `None` once one works again.
    pub fn open(mut prefs: Prefs, report: impl Fn(Option<String>) + Send + Sync + 'static) -> Store {
        Store::spawn(
            move |key, value| match value {
                Some(v) => prefs.set(key, &v).map_err(|e| e.to_string()),
                None => prefs.remove(key).map_err(|e| e.to_string()),
            },
            report,
        )
    }

    /// A store whose writer thread hands each change to `write`.
    pub fn spawn(
        mut write: impl FnMut(&str, Option<Value>) -> Result<(), String> + Send + 'static,
        report: impl Fn(Option<String>) + Send + Sync + 'static,
    ) -> Store {
        let (tx, rx) = mpsc::channel::<Msg>();
        let (watch, heard) = mpsc::channel::<Writing>();
        let report = Arc::new(report);
        let told = report.clone();
        let watchdog = std::thread::Builder::new().name("store-watch".into()).spawn(move || {
            let mut said: Option<String> = None;
            let mut tell = |now: Option<String>| {
                if now != said {
                    said.clone_from(&now);
                    told(now);
                }
            };
            while let Ok(Writing::Began) = heard.recv() {
                let ended = match heard.recv_timeout(STALL) {
                    Ok(Writing::Ended(e)) => e,
                    Err(RecvTimeoutError::Timeout) => {
                        tell(Some("still saving".into()));
                        match heard.recv() {
                            Ok(Writing::Ended(e)) => e,
                            _ => return,
                        }
                    }
                    _ => return,
                };
                tell(ended);
            }
        });
        let writer = std::thread::Builder::new().name("store".into()).spawn(move || {
            // Why the last write failed, until a write works again.
            let mut failed: Option<String> = None;
            while let Ok(first) = rx.recv() {
                let mut pending: Vec<(String, Option<Encode>)> = Vec::new();
                let mut acks = Vec::new();
                for msg in std::iter::once(first).chain(rx.try_iter()) {
                    match msg {
                        Msg::Set(key, value) => match pending.iter_mut().find(|(k, _)| *k == key) {
                            Some(slot) => slot.1 = value,
                            None => pending.push((key, value)),
                        },
                        Msg::Flush(ack) => acks.push(ack),
                    }
                }
                // A flush with nothing queued writes nothing, so it can't say a failure is over.
                if !pending.is_empty() {
                    #[expect(
                        clippy::let_underscore_must_use,
                        reason = "the watchdog only reports; saving goes on without it"
                    )]
                    let _ = watch.send(Writing::Began);
                    let mut error = None;
                    for (key, value) in pending {
                        let result = match value {
                            Some(encode) => encode().and_then(|v| write(&key, Some(v))),
                            None => write(&key, None),
                        };
                        if let Err(e) = result {
                            error = Some(e);
                        }
                    }
                    failed.clone_from(&error);
                    #[expect(
                        clippy::let_underscore_must_use,
                        reason = "the watchdog only reports; saving goes on without it"
                    )]
                    let _ = watch.send(Writing::Ended(error));
                }
                for ack in acks {
                    #[expect(clippy::let_underscore_must_use, reason = "a flush that timed out has stopped listening")]
                    let _ = ack.send(failed.clone());
                }
            }
        });
        match (writer, watchdog) {
            (Ok(_), Ok(_)) => Store { tx: Some(tx) },
            (Err(e), _) | (_, Err(e)) => {
                report(Some(format!("can't start saving: {e}")));
                Store::memory()
            }
        }
    }

    /// Queues `value` to be saved under `key`; it is encoded and written on the writer thread.
    pub fn set<T: Serialize + Send + 'static>(&self, key: &str, value: T) {
        self.send(key, Some(Box::new(move || serde_json::to_value(&value).map_err(|e| e.to_string()))));
    }

    pub fn remove(&self, key: &str) {
        self.send(key, None);
    }

    fn send(&self, key: &str, value: Option<Encode>) {
        if let Some(tx) = &self.tx {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "only a panicked writer drops this, and flush reports that"
            )]
            let _ = tx.send(Msg::Set(key.to_owned(), value));
        }
    }

    /// Waits up to `limit` until everything queued so far is written, for quitting, and says
    /// why it isn't saved if it isn't: the last write failed, or it is still being written. A
    /// disk that stalls never holds the app open. Each file write replaces the file whole, so
    /// one cut short leaves the last save.
    pub fn flush(&self, limit: Duration) -> Result<(), String> {
        let Some(tx) = &self.tx else { return Ok(()) };
        let (ack, done) = mpsc::channel();
        tx.send(Msg::Flush(ack)).map_err(|_| "the writer has stopped".to_owned())?;
        match done.recv_timeout(limit) {
            Ok(None) => Ok(()),
            Ok(Some(why)) => Err(why),
            Err(_) => Err("still being written".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn saving_returns_at_once_while_the_writer_is_busy() {
        let (open, gate) = mpsc::channel::<()>();
        let gate = Mutex::new(gate);
        let written = Arc::new(Mutex::new(Vec::new()));
        let seen = written.clone();
        let store = Store::spawn(
            move |key, value| {
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "a write past the three opened goes ahead after the timeout"
                )]
                let _ = gate.lock().unwrap().recv_timeout(Duration::from_secs(2));
                seen.lock().unwrap().push((key.to_owned(), value.unwrap_or_default()));
                Ok(())
            },
            |_| {},
        );
        let started = Instant::now();
        store.set("log", 1);
        store.set("log", 2);
        store.set("place", "Lake");
        store.set("log", 3);
        assert!(started.elapsed() < Duration::from_millis(200), "the caller never waits on the disk");
        for _ in 0..3 {
            open.send(()).unwrap();
        }
        assert_eq!(store.flush(Duration::from_secs(5)), Ok(()));
        let written = written.lock().unwrap().clone();
        let logs: Vec<&Value> = written.iter().filter(|(k, _)| k == "log").map(|(_, v)| v).collect();
        assert_eq!(logs.last(), Some(&&Value::from(3)), "the last value wins: {written:?}");
        assert!(logs.len() <= 2, "values queued behind a busy write fold into one: {written:?}");
        assert!(written.iter().any(|(k, _)| k == "place"));
    }

    /// Every report `Store::spawn` gives, in order, once the store is dropped and its threads
    /// have ended.
    fn all(heard: &mpsc::Receiver<Option<String>>) -> Vec<Option<String>> {
        std::iter::from_fn(|| heard.recv_timeout(Duration::from_secs(5)).ok()).collect()
    }

    #[test]
    fn a_failing_save_is_reported_once_and_its_recovery_too() {
        let fail = Arc::new(Mutex::new(true));
        let (said, heard) = mpsc::channel();
        let f = fail.clone();
        let store = Store::spawn(
            move |_, _| if *f.lock().unwrap() { Err("disk full".into()) } else { Ok(()) },
            move |e| said.send(e).unwrap(),
        );
        let flush = || store.flush(Duration::from_secs(5));
        let full = Err("disk full".to_owned());
        store.set("log", 1);
        assert_eq!(flush(), full, "the flush says the save failed");
        assert_eq!(flush(), full, "and a flush with nothing queued doesn't forget it");
        store.set("log", 2);
        assert_eq!(flush(), full);
        *fail.lock().unwrap() = false;
        assert_eq!(flush(), full, "only a write that works ends the failure");
        store.set("log", 3);
        assert_eq!(flush(), Ok(()));
        drop(store);
        assert_eq!(all(&heard), vec![Some("disk full".to_owned()), None]);
    }

    #[test]
    fn a_stalled_disk_is_reported_and_never_holds_quitting() {
        let (open, gate) = mpsc::channel::<()>();
        let (entered, writing) = mpsc::channel::<()>();
        let gate = Mutex::new(gate);
        let (said, heard) = mpsc::channel();
        let store = Store::spawn(
            move |_, _| {
                entered.send(()).unwrap();
                gate.lock().unwrap().recv().unwrap();
                Ok(())
            },
            move |e| said.send(e).unwrap(),
        );
        store.set("study", 1);
        writing.recv_timeout(Duration::from_secs(5)).unwrap();
        let started = Instant::now();
        assert_eq!(store.flush(Duration::from_millis(300)), Err("still being written".into()), "the flush gives up");
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(heard.recv_timeout(STALL * 2), Ok(Some("still saving".to_owned())));
        open.send(()).unwrap();
        assert_eq!(store.flush(Duration::from_secs(2)), Ok(()));
        assert_eq!(heard.recv_timeout(Duration::from_secs(5)), Ok(None), "and the recovery is heard");
        let started = Instant::now();
        drop(store);
        assert!(started.elapsed() < Duration::from_millis(100), "dropping the store never joins the writer");
    }

    #[test]
    fn values_reach_the_file_by_the_time_the_store_is_flushed() {
        let dir = std::env::temp_dir().join(format!("pomodoro-store-{}", std::process::id()));
        let path = dir.join("prefs.json");
        let store = Store::open(Prefs::at(&path), |_| {});
        store.set("lengths", (25, 5, 15));
        store.set("log", 1);
        store.remove("log");
        assert_eq!(store.flush(Duration::from_secs(5)), Ok(()));
        let back = Prefs::at(&path);
        assert_eq!(back.get::<(u32, u32, u32)>("lengths"), Some((25, 5, 15)));
        assert!(!back.contains("log"), "a removal queued after a value wins");
        drop(store);
        #[expect(clippy::let_underscore_must_use, reason = "cleanup of a temporary directory is best effort")]
        let _ = std::fs::remove_dir_all(dir);
    }
}
