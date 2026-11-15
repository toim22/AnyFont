//! File logging with rotation.
//!
//! The original writes no log file (only `OutputDebugStringW`, dossier
//! `10-behavior/errors-logging-installer.md`). AnyFont keeps a small rotating text log in
//! `%LOCALAPPDATA%\AnyFont\logs\anyfont.log` (`anyfont.1.log` ... `anyfont.4.log` are the older
//! ones) so the bug reporter and crash reports can include recent activity, and still mirrors every
//! line to `OutputDebugStringW` in debug builds.
//!
//! Level: `ANYFONT_LOG=error|warn|info|debug|trace` (default `info`).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use log::{LevelFilter, Log, Metadata, Record};

/// Rotate when the current file reaches this size.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
/// Rotated files kept besides the current one.
pub const KEEP_ROTATED: usize = 4;
pub const LOG_FILE_NAME: &str = "anyfont.log";

struct FileLogger {
    path: PathBuf,
    level: LevelFilter,
    inner: Mutex<Inner>,
}

struct Inner {
    file: Option<File>,
    written: u64,
}

static LOGGER: OnceLock<FileLogger> = OnceLock::new();

/// Path of the `n`-th rotated file (`0` = current).
pub fn rotated_path(dir: &Path, n: usize) -> PathBuf {
    if n == 0 {
        dir.join(LOG_FILE_NAME)
    } else {
        dir.join(format!("anyfont.{n}.log"))
    }
}

/// Shift `anyfont.log` -> `anyfont.1.log` -> ... dropping the oldest.
pub fn rotate(dir: &Path) -> io::Result<()> {
    let _ = std::fs::remove_file(rotated_path(dir, KEEP_ROTATED));
    for n in (0..KEEP_ROTATED).rev() {
        let from = rotated_path(dir, n);
        if from.exists() {
            std::fs::rename(&from, rotated_path(dir, n + 1))?;
        }
    }
    Ok(())
}

fn open_append(path: &Path) -> io::Result<(File, u64)> {
    let f = OpenOptions::new().create(true).append(true).open(path)?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    Ok((f, len))
}

impl FileLogger {
    fn write_line(&self, line: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.written >= MAX_LOG_BYTES {
            inner.file = None;
            if let Some(dir) = self.path.parent() {
                let _ = rotate(dir);
            }
            match open_append(&self.path) {
                Ok((f, len)) => {
                    inner.file = Some(f);
                    inner.written = len;
                }
                Err(_) => return,
            }
        }
        if let Some(f) = inner.file.as_mut() {
            if f.write_all(line.as_bytes()).is_ok() {
                inner.written += line.len() as u64;
            }
        }
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let thread = std::thread::current();
        let line = format!(
            "{} {:<5} [{}] {}: {}\r\n",
            crate::util::timestamp(),
            record.level(),
            thread.name().unwrap_or("?"),
            record.target(),
            record.args()
        );
        self.write_line(&line);
        if cfg!(debug_assertions) {
            debug_output(&line);
        }
    }
    fn flush(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(f) = inner.file.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

/// `OutputDebugStringW` (what the original uses for diagnostics).
pub fn debug_output(text: &str) {
    #[cfg(windows)]
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(&windows::core::HSTRING::from(text));
    }
    #[cfg(not(windows))]
    let _ = text;
}

/// Level from `ANYFONT_LOG`.
pub fn level_from_env() -> LevelFilter {
    match std::env::var("ANYFONT_LOG").map(|v| v.to_ascii_lowercase()).as_deref() {
        Ok("off") => LevelFilter::Off,
        Ok("error") => LevelFilter::Error,
        Ok("warn") => LevelFilter::Warn,
        Ok("debug") => LevelFilter::Debug,
        Ok("trace") => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

/// Install the file logger writing to `dir\anyfont.log` (rotating first when the file is already
/// large). Returns the log file path. Calling it again returns the first path.
pub fn init_in(dir: &Path, level: LevelFilter) -> io::Result<PathBuf> {
    if let Some(l) = LOGGER.get() {
        return Ok(l.path.clone());
    }
    std::fs::create_dir_all(dir)?;
    let path = rotated_path(dir, 0);
    if std::fs::metadata(&path).map(|m| m.len() >= MAX_LOG_BYTES).unwrap_or(false) {
        rotate(dir)?;
    }
    let (file, written) = open_append(&path)?;
    let logger = FileLogger { path: path.clone(), level, inner: Mutex::new(Inner { file: Some(file), written }) };
    if LOGGER.set(logger).is_ok() {
        let l = LOGGER.get().expect("set");
        if log::set_logger(l).is_ok() {
            log::set_max_level(level);
        }
    }
    log::info!("AnyFont {} log started (pid {})", env!("CARGO_PKG_VERSION"), std::process::id());
    Ok(path)
}

/// Install the logger in AnyFont's log folder (`%LOCALAPPDATA%\AnyFont\logs`, or the sandbox's).
pub fn init() -> io::Result<PathBuf> {
    init_in(&crate::paths::app_paths().logs_dir, level_from_env())
}

/// Current log file, when [`init`] ran.
pub fn log_file() -> Option<PathBuf> {
    LOGGER.get().map(|l| l.path.clone())
}

/// The last `lines` lines of the current log (for crash and bug reports).
pub fn tail(lines: usize) -> String {
    let Some(path) = log_file() else { return String::new() };
    if let Some(l) = LOGGER.get() {
        log::Log::flush(l);
    }
    tail_of(&path, lines)
}

pub fn tail_of(path: &Path, lines: usize) -> String {
    let Ok(data) = std::fs::read(path) else { return String::new() };
    let start = data.len().saturating_sub(256 * 1024);
    let text = String::from_utf8_lossy(&data[start..]);
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_four_old_files() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..7 {
            std::fs::write(rotated_path(dir.path(), 0), format!("gen {i}")).unwrap();
            rotate(dir.path()).unwrap();
        }
        assert!(!rotated_path(dir.path(), 0).exists());
        for n in 1..=KEEP_ROTATED {
            assert!(rotated_path(dir.path(), n).exists(), "{n}");
        }
        assert!(!rotated_path(dir.path(), KEEP_ROTATED + 1).exists());
        assert_eq!(std::fs::read_to_string(rotated_path(dir.path(), 1)).unwrap(), "gen 6");
        assert_eq!(std::fs::read_to_string(rotated_path(dir.path(), 4)).unwrap(), "gen 3");
    }

    #[test]
    fn tail_returns_last_lines() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.log");
        std::fs::write(&p, "a\r\nb\r\nc\r\nd\r\n").unwrap();
        assert_eq!(tail_of(&p, 2), "c\r\nd");
        assert_eq!(tail_of(&p, 10), "a\r\nb\r\nc\r\nd");
    }

    #[test]
    fn logger_writes_and_rotates_when_large() {
        let dir = tempfile::tempdir().unwrap();
        let l = FileLogger {
            path: rotated_path(dir.path(), 0),
            level: LevelFilter::Info,
            inner: Mutex::new(Inner { file: Some(open_append(&rotated_path(dir.path(), 0)).unwrap().0), written: MAX_LOG_BYTES }),
        };
        l.write_line("after rotation\r\n");
        assert!(std::fs::read_to_string(rotated_path(dir.path(), 0)).unwrap().contains("after rotation"));
        assert!(rotated_path(dir.path(), 1).exists());
    }
}
