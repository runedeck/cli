//! One native copy per harness at a time. The lock is an atomic `mkdir`
//! under the state directory, with the name the `session-sync` script
//! used, so the script and rune never copy the same harness at once while
//! both exist. A lock older than thirty minutes is a crash and is taken.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const STALE: Duration = Duration::from_mins(30);
const STEP: Duration = Duration::from_millis(200);

/// A held lock. Dropping it releases the directory.
#[derive(Debug)]
pub struct Lock {
    dir: PathBuf,
}

/// The lock directory of a harness.
#[must_use]
pub fn dir(state_dir: &Path, harness: &str) -> PathBuf {
    state_dir.join(format!("session-sync.lock.{harness}.d"))
}

/// Take the lock, waiting up to `wait`. `None` when the wait runs out.
pub fn acquire(state_dir: &Path, harness: &str, wait: Duration) -> Option<Lock> {
    let dir = dir(state_dir, harness);
    let _ = std::fs::create_dir_all(state_dir);
    let started = Instant::now();
    loop {
        if std::fs::create_dir(&dir).is_ok() {
            return Some(Lock { dir });
        }
        if is_stale(&dir) {
            // A lock this old is a crash. The wait still counts, so a lock
            // that cannot be removed ends the wait instead of spinning.
            let _ = std::fs::remove_dir(&dir);
        }
        if started.elapsed() >= wait {
            return None;
        }
        std::thread::sleep(
            STEP.min(
                wait.saturating_sub(started.elapsed())
                    .max(Duration::from_millis(1)),
            ),
        );
    }
}

fn is_stale(dir: &Path) -> bool {
    std::fs::metadata(dir)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age > STALE)
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.dir);
    }
}
