//! Crash reports.
//!
//! The original initialises Crashpad (`CrlConfig` 0x18000F8A0) with a crash-log directory and a
//! `crashpad_handler.exe`, and shows `CrashReportDialogDS` afterwards. AnyFont writes a plain text
//! report from a panic hook into `%LOCALAPPDATA%\AnyFont\crash-reports\crash-<time>.txt`; on the
//! next start the UI lists [`pending_reports`] and offers to send them with the bug reporter, then
//! calls [`mark_handled`]. Reports never leave the machine without the user's action.
//!
//! The user profile path is replaced by `%USERPROFILE%` in the report text.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Lines of the log appended to a report.
pub const LOG_TAIL_LINES: usize = 60;

static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install the panic hook (idempotent). The previous hook still runs afterwards.
pub fn install_panic_hook() {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        log::error!("panic in thread '{thread}' at {location}: {message}");
        let text = render_report(&message, &location, &thread, &backtrace, &crate::logging::tail(LOG_TAIL_LINES));
        match write_report_in(&crate::paths::app_paths().crash_dir, &text) {
            Ok(p) => log::error!("crash report written to {}", p.display()),
            Err(e) => log::error!("could not write the crash report: {e}"),
        }
        previous(info);
    }));
}

/// Build the report text.
pub fn render_report(message: &str, location: &str, thread: &str, backtrace: &str, log_tail: &str) -> String {
    let text = format!(
        "AnyFont crash report\r\n\
         ====================\r\n\
         Version: {version}\r\n\
         Time (UTC): {time}\r\n\
         OS: {os}\r\n\
         Architecture: {arch}\r\n\
         Thread: {thread}\r\n\
         Location: {location}\r\n\
         Message: {message}\r\n\
         \r\n\
         Backtrace\r\n\
         ---------\r\n\
         {backtrace}\r\n\
         \r\n\
         Recent log\r\n\
         ----------\r\n\
         {log_tail}\r\n",
        version = env!("CARGO_PKG_VERSION"),
        time = crate::util::timestamp(),
        os = os_version(),
        arch = std::env::consts::ARCH,
    );
    redact(&text)
}

/// Replace the user profile folder (and user name) with placeholders.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let p = profile.to_string_lossy().to_string();
        if p.len() > 3 {
            out = replace_ci(&out, &p, "%USERPROFILE%");
            out = replace_ci(&out, &p.replace('\\', "/"), "%USERPROFILE%");
        }
    }
    out
}

fn replace_ci(haystack: &str, needle: &str, with: &str) -> String {
    let lower_h = haystack.to_lowercase();
    let lower_n = needle.to_lowercase();
    if lower_n.is_empty() || lower_h.len() != haystack.len() {
        return haystack.replace(needle, with);
    }
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    while let Some(pos) = lower_h[i..].find(&lower_n) {
        out.push_str(&haystack[i..i + pos]);
        out.push_str(with);
        i += pos + needle.len();
    }
    out.push_str(&haystack[i..]);
    out
}

/// Write a report into `dir`; returns its path.
pub fn write_report_in(dir: &Path, text: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut path = dir.join(format!("crash-{}.txt", crate::util::file_stamp()));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("crash-{}-{n}.txt", crate::util::file_stamp()));
        n += 1;
    }
    std::fs::write(&path, text.as_bytes())?;
    Ok(path)
}

/// Reports not yet shown to the user (oldest first).
pub fn pending_reports() -> Vec<PathBuf> {
    pending_reports_in(&crate::paths::app_paths().crash_dir)
}

pub fn pending_reports_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name().map(|n| n.to_string_lossy().starts_with("crash-")).unwrap_or(false)
                        && crate::fontfile::extension(p) == "txt"
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Move a report to `crash-reports\handled\` (sent or dismissed by the user).
pub fn mark_handled(report: &Path) -> io::Result<()> {
    let dir = report.parent().ok_or_else(|| io::Error::other("no parent"))?.join("handled");
    std::fs::create_dir_all(&dir)?;
    std::fs::rename(report, dir.join(report.file_name().unwrap_or_default()))
}

/// "Windows 11 Pro 24H2 (build 26200)" from the registry (read only).
pub fn os_version() -> String {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
        let read = |name: &str| -> Option<String> {
            let mut buf = vec![0u16; 256];
            let mut len = (buf.len() * 2) as u32;
            let r = unsafe {
                RegGetValueW(
                    HKEY_LOCAL_MACHINE,
                    &HSTRING::from(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
                    &HSTRING::from(name),
                    RRF_RT_REG_SZ,
                    None,
                    Some(buf.as_mut_ptr() as *mut _),
                    Some(&mut len),
                )
            };
            if r.is_ok() {
                let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
                Some(String::from_utf16_lossy(&buf[..end]))
            } else {
                None
            }
        };
        let product = read("ProductName").unwrap_or_else(|| "Windows".into());
        let display = read("DisplayVersion").unwrap_or_default();
        let build = read("CurrentBuild").unwrap_or_default();
        // ProductName still says "Windows 10" on Windows 11; the build number tells.
        let product = if build.parse::<u32>().map(|b| b >= 22000).unwrap_or(false) { product.replace("Windows 10", "Windows 11") } else { product };
        format!("{product} {display} (build {build})").replace("  ", " ")
    }
    #[cfg(not(windows))]
    {
        std::env::consts::OS.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_files_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let text = render_report("boom", "src/x.rs:1:2", "main", "bt", "log line");
        assert!(text.contains("Message: boom"));
        assert!(text.contains("AnyFont crash report"));
        let a = write_report_in(dir.path(), &text).unwrap();
        let b = write_report_in(dir.path(), &text).unwrap();
        assert_ne!(a, b);
        assert_eq!(pending_reports_in(dir.path()).len(), 2);
        mark_handled(&a).unwrap();
        assert_eq!(pending_reports_in(dir.path()), vec![b]);
        assert!(dir.path().join("handled").join(a.file_name().unwrap()).exists());
    }

    #[test]
    fn redacts_profile() {
        if let Some(p) = std::env::var_os("USERPROFILE") {
            let p = p.to_string_lossy().to_string();
            let t = redact(&format!("file {}\\Fonts\\x.ttf and {}", p.to_uppercase(), p));
            assert!(!t.to_lowercase().contains(&p.to_lowercase()));
            assert!(t.contains("%USERPROFILE%\\Fonts"));
        }
    }

    #[test]
    fn os_version_is_not_empty() {
        assert!(!os_version().trim().is_empty());
    }
}
