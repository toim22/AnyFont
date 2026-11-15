//! Test seams. Nothing here changes what AnyFont does for a user: the functions only act when a test (the
//! headless end-to-end tests in `crates/anyfont/tests/`, or the `/SelfTest:` run on a GitHub runner) turns
//! them on.
//!
//! - **Requests to the outside world** (a browser, Explorer, the printer): with [`enable`] (or the
//!   `ANYFONT_TEST_INTERCEPT` environment variable) they are recorded instead of carried out, so a test can
//!   assert the requested URL or path and nothing opens on the desktop of the computer running the tests.
//! - **Native file dialogs** (`rfd`): a test queues the answer of the next dialog with [`queue_pick`]; with
//!   interception on and no queued answer, a dialog counts as cancelled instead of opening.
//! - [`selftest`]: `anyfont.exe /SelfTest:<script.json>`, the scripted run of the real font system used by
//!   `tests/real-system/run-real.ps1` on a disposable GitHub-hosted runner (`.github/workflows/e2e-real.yml`).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub mod selftest;

/// Environment variable that turns interception on (for a whole process, e.g. a test run of `anyfont.exe`).
pub const INTERCEPT_ENV: &str = "ANYFONT_TEST_INTERCEPT";

/// Something AnyFont asked the operating system to open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum External {
    /// A URL (or a local file) for the default browser (`dlg::open_url`).
    Url(String),
    /// Explorer with this file selected (bug report fallback).
    Explorer(PathBuf),
    /// This text file sent to the printer (Legal Notices ▸ Print).
    Print(PathBuf),
}

static INTERCEPT: AtomicBool = AtomicBool::new(false);
static REQUESTS: Mutex<Vec<External>> = Mutex::new(Vec::new());

thread_local! {
    static PICKS: RefCell<VecDeque<Option<PathBuf>>> = const { RefCell::new(VecDeque::new()) };
}

/// Turns interception on for this process (the tests call it before building a window).
pub fn enable() {
    INTERCEPT.store(true, Ordering::SeqCst);
}

/// True when requests to the outside world are recorded instead of carried out.
pub fn enabled() -> bool {
    INTERCEPT.load(Ordering::SeqCst) || std::env::var_os(INTERCEPT_ENV).is_some_and(|v| !v.is_empty())
}

/// Called where AnyFont would open a browser, Explorer or the printer. With interception on, the request is
/// recorded and `true` returned: the caller then does nothing else. Otherwise `false` (go ahead).
pub fn intercept(request: External) -> bool {
    if !enabled() {
        return false;
    }
    log::info!("test seam: intercepted {request:?}");
    REQUESTS.lock().unwrap_or_else(|e| e.into_inner()).push(request);
    true
}

/// Every request recorded so far in this process, oldest first.
pub fn requests() -> Vec<External> {
    REQUESTS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The URLs recorded so far.
pub fn urls() -> Vec<String> {
    requests().into_iter().filter_map(|r| if let External::Url(u) = r { Some(u) } else { None }).collect()
}

/// Queues the answer of the next native file dialog opened on this thread (`None` = the user cancelled).
pub fn queue_pick(answer: Option<PathBuf>) {
    PICKS.with(|q| q.borrow_mut().push_back(answer));
}

/// Wraps a native file dialog: a queued answer if there is one; with interception on and nothing queued,
/// "cancelled"; otherwise the real dialog.
pub fn pick(dialog: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    if let Some(answer) = PICKS.with(|q| q.borrow_mut().pop_front()) {
        log::info!("test seam: file dialog answered with {answer:?}");
        return answer;
    }
    if enabled() {
        log::info!("test seam: file dialog with no queued answer: cancelled");
        return None;
    }
    dialog()
}

// --- network redirection (Get More) ---

static REDIRECTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Sends requests whose URL starts with `from` to `to` + the rest of the URL (e.g. `https://fonts.google.com/`
/// → `http://127.0.0.1:<port>/google/`). Process-wide; the latest mapping of a prefix wins.
pub fn redirect_prefix(from: &str, to: &str) {
    let mut r = REDIRECTS.lock().unwrap_or_else(|e| e.into_inner());
    r.retain(|(f, _)| f != from);
    r.push((from.to_string(), to.to_string()));
}

/// Removes every mapping of [`redirect_prefix`].
pub fn clear_redirects() {
    REDIRECTS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// The address AnyFont really fetches: with interception off, `url` itself; with it on, the mapped address,
/// or an unreachable local one for any other host than this computer (a test never reaches the internet).
pub fn redirect(url: &str) -> String {
    if !enabled() {
        return url.to_string();
    }
    let r = REDIRECTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((from, to)) = r.iter().find(|(f, _)| url.starts_with(f.as_str())) {
        return format!("{to}{}", &url[from.len()..]);
    }
    let local = ["http://127.0.0.1", "http://localhost", "http://[::1]"].iter().any(|p| url.starts_with(p));
    if local {
        url.to_string()
    } else {
        log::info!("test seam: blocked {url}");
        "http://127.0.0.1:9/blocked".to_string()
    }
}
