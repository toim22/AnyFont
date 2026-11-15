//! A cheap watcher of the font folders: a background thread compares the modification times of the watched
//! folders (and their direct sub-folders) every few seconds. When something changed, View ▸ Refresh the Font
//! List and the information strip above the list become available (UI-SPEC §2.7, §11 #5).

use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

struct Shared {
    folders: Mutex<Vec<PathBuf>>,
    snapshot: Mutex<HashMap<PathBuf, Option<SystemTime>>>,
    changed: AtomicBool,
    rebase: AtomicBool,
    stop: AtomicBool,
}

pub struct Watcher {
    shared: Arc<Shared>,
}

fn stamps(folders: &[PathBuf]) -> HashMap<PathBuf, Option<SystemTime>> {
    let mut m = HashMap::new();
    for f in folders {
        m.insert(f.clone(), std::fs::metadata(f).and_then(|m| m.modified()).ok());
        if let Ok(rd) = std::fs::read_dir(f) {
            for e in rd.flatten().take(256) {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let p = e.path();
                    let t = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
                    m.insert(p, t);
                }
            }
        }
    }
    m
}

impl Watcher {
    /// Starts the polling thread (`interval` between checks).
    pub fn start(ctx: egui::Context, interval: Duration) -> Watcher {
        let shared = Arc::new(Shared {
            folders: Mutex::new(Vec::new()),
            snapshot: Mutex::new(HashMap::new()),
            changed: AtomicBool::new(false),
            rebase: AtomicBool::new(true),
            stop: AtomicBool::new(false),
        });
        let s = shared.clone();
        let _ = std::thread::Builder::new().name("anyfont-folder-watch".into()).spawn(move || {
            while !s.stop.load(Ordering::Relaxed) {
                let folders = s.folders.lock().clone();
                let now = stamps(&folders);
                if s.rebase.swap(false, Ordering::Relaxed) {
                    *s.snapshot.lock() = now;
                } else {
                    let differs = *s.snapshot.lock() != now;
                    if differs && !s.changed.swap(true, Ordering::Relaxed) {
                        log::info!("font folders changed on disk");
                        ctx.request_repaint();
                    }
                }
                let mut slept = Duration::ZERO;
                while slept < interval && !s.stop.load(Ordering::Relaxed) && !s.rebase.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(100));
                    slept += Duration::from_millis(100);
                }
            }
        });
        Watcher { shared }
    }

    /// The folders to watch; takes a new snapshot (after a scan, the current state is the reference).
    pub fn watch(&self, folders: Vec<PathBuf>) {
        *self.shared.folders.lock() = folders;
        self.rebase();
    }

    /// Accepts the current state of the folders as the reference.
    pub fn rebase(&self) {
        self.shared.changed.store(false, Ordering::Relaxed);
        self.shared.rebase.store(true, Ordering::Relaxed);
    }

    /// True when the folders changed since the last snapshot.
    pub fn changed(&self) -> bool {
        self.shared.changed.load(Ordering::Relaxed)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}
