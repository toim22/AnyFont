//! Small shared helpers (path comparison, hashing, time stamps, atomic writes).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicIsize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Owner window for UAC prompts / shell windows (see `set_owner_window`).
pub(crate) static OWNER_HWND: AtomicIsize = AtomicIsize::new(0);

/// Case-insensitive, separator-normalised key for comparing Windows paths.
pub(crate) fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let mut k = s.replace('/', "\\").to_lowercase();
    while k.len() > 3 && k.ends_with('\\') {
        k.pop();
    }
    k
}

/// `path` is directly inside `dir` (not in a sub-folder).
pub(crate) fn is_directly_in(dir: &Path, path: &Path) -> bool {
    match path.parent() {
        Some(parent) => path_key(parent) == path_key(dir),
        None => false,
    }
}

/// `path` is inside `dir` (any depth).
#[allow(dead_code)]
pub(crate) fn is_under(dir: &Path, path: &Path) -> bool {
    let d = path_key(dir);
    let p = path_key(path);
    p.len() > d.len() && p.starts_with(&d) && p.as_bytes()[d.len()] == b'\\'
}

pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

/// 64-bit FNV-1a (stable across runs; used for sandbox / pipe names, not for security).
pub(crate) fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Civil date-time (UTC) from seconds since the Unix epoch.
pub(crate) fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = ((rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32);
    // Howard Hinnant's days_from_civil inverse.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d, h, mi, s)
}

pub(crate) fn now_unix() -> (i64, u32) {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (d.as_secs() as i64, d.subsec_millis())
}

/// `2026-11-15 13:04:05.123Z`
pub(crate) fn timestamp() -> String {
    let (secs, ms) = now_unix();
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

/// `20261115-130405` (file-name safe).
pub(crate) fn file_stamp() -> String {
    let (secs, _) = now_unix();
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Write a file atomically (temp file + rename).
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_sibling(path);
    std::fs::write(&tmp, data)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Random-ish hex id (time + pid + counter mixed through FNV); unique enough for request files.
pub(crate) fn unique_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let (secs, ms) = now_unix();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = format!("{secs}.{ms}.{nanos}.{}.{c}", std::process::id());
    format!("{:016x}{:08x}", fnv1a64(seed.as_bytes()), nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0, 0));
        // 2026-11-15 12:34:56 UTC
        assert_eq!(civil_from_unix(1_794_746_096), (2026, 11, 15, 12, 34, 56));
        assert_eq!(civil_from_unix(951_782_400), (2000, 2, 29, 0, 0, 0));
    }

    #[test]
    fn path_keys() {
        assert_eq!(path_key(Path::new(r"C:\Windows\Fonts\")), r"c:\windows\fonts");
        assert!(same_path(Path::new(r"\\?\C:\A\b.TTF"), Path::new("c:/a/B.ttf")));
        assert!(is_directly_in(Path::new(r"C:\Windows\Fonts"), Path::new(r"c:\windows\fonts\arial.ttf")));
        assert!(!is_directly_in(Path::new(r"C:\Windows\Fonts"), Path::new(r"c:\windows\fonts\x\arial.ttf")));
        assert!(is_under(Path::new(r"C:\Windows"), Path::new(r"c:\windows\fonts\arial.ttf")));
        assert!(!is_under(Path::new(r"C:\Win"), Path::new(r"c:\windows\fonts\arial.ttf")));
    }
}
