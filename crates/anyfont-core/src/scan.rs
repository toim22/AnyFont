//! Incremental rescans of installed fonts and user folders, off the UI thread.
//!
//! The equivalent of the original's database refresh (`CFontDatabase` update + `FONTFOLDERS` monitoring):
//!
//! 1. Build the file list: the installed font files given by the caller (`anyfont-sys` enumerates the
//!    registry / `C:\Windows\Fonts`), plus every font file under the enabled user folders (recursively when
//!    the folder says so). A `.pfm` whose `.pfb`/`.pfa` exists is represented by the outline file.
//! 2. Files whose path, size and modification time match the cache are skipped.
//! 3. New or changed files are parsed in parallel ([`std::thread::scope`]); each file yields one or more
//!    faces (or a corrupt record), stored by content hash, so identical copies share their faces.
//! 4. Files that disappeared are removed, faces without any file are dropped, duplicates are recomputed.
//!
//! Progress is reported through a callback ([`scan`]) or a channel ([`spawn_scan`]); cancellation is a
//! shared flag checked between files. A cancelled scan keeps what it already stored (the next scan resumes
//! incrementally) and skips step 4.

use crate::db::{insert_font, normalize_path, FontDatabase};
use crate::error::{CoreError, Result};
use crate::model::*;
use crate::parse::{self, ParsedFile};
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// What to scan.
#[derive(Clone, Debug, Default)]
pub struct ScanRequest {
    /// Database file (the worker opens its own connection).
    pub db_path: PathBuf,
    /// Installed font files ("My fonts"); replaces the stored installed set.
    pub installed: Vec<InstalledFont>,
    /// Re-parse every file even when the cache is current.
    pub force: bool,
    /// Worker threads for parsing (0 = available parallelism).
    pub threads: usize,
}

/// Progress events.
#[derive(Clone, Debug, PartialEq)]
pub enum ScanProgress {
    /// The file list is known.
    Started {
        /// Files found.
        total: usize,
        /// Files that need parsing.
        to_parse: usize,
    },
    /// One file was parsed.
    File {
        /// Files parsed so far.
        done: usize,
        /// Files to parse.
        total: usize,
        /// The file.
        path: PathBuf,
    },
    /// Finished (also sent after a cancellation).
    Finished(ScanReport),
}

/// Summary of a scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Files found (installed + folders).
    pub files_found: usize,
    /// Files parsed (new or changed).
    pub files_parsed: usize,
    /// Files served from the cache.
    pub files_cached: usize,
    /// Files removed from the cache.
    pub files_removed: usize,
    /// Faces in the database after the scan.
    pub faces: usize,
    /// Corrupt files among the parsed ones.
    pub corrupt: usize,
    /// The scan was cancelled.
    pub cancelled: bool,
    /// Duration in milliseconds.
    pub elapsed_ms: u64,
}

/// Collects the font files of the library: installed files first, then enabled folders.
pub fn collect_files(installed: &[InstalledFont], folders: &[crate::db::Folder]) -> Vec<PathBuf> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    let mut push = |p: PathBuf, out: &mut Vec<PathBuf>| {
        let p = parse::canonical_font_path(&p);
        if seen.insert(normalize_path(&p)) {
            out.push(p);
        }
    };
    for f in installed {
        push(f.path.clone(), &mut out);
    }
    for folder in folders.iter().filter(|f| f.enabled) {
        let walker = walkdir::WalkDir::new(&folder.path)
            .max_depth(if folder.recursive { usize::MAX } else { 1 })
            .follow_links(false)
            .sort_by_file_name();
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() && parse::is_font_path(entry.path()) {
                push(entry.path().to_path_buf(), &mut out);
            }
        }
    }
    out
}

struct CachedFile {
    size: u64,
    mtime: i64,
}

fn file_stamp(p: &Path) -> Option<(u64, i64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), parse::file_mtime(&m)))
}

/// Runs a scan on `db` (blocking). `progress` receives every event; `cancel` stops it between files.
pub fn scan(
    db: &mut FontDatabase,
    request: &ScanRequest,
    progress: &mut dyn FnMut(ScanProgress),
    cancel: &AtomicBool,
) -> Result<ScanReport> {
    let started = std::time::Instant::now();
    db.set_installed(&request.installed)?;
    let folders = db.folders()?;
    let files = collect_files(&request.installed, &folders);
    let mut report = ScanReport { files_found: files.len(), ..Default::default() };

    let cache: HashMap<String, CachedFile> = {
        let mut st = db.connection().prepare("SELECT path, size, mtime FROM files")?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, CachedFile { size: r.get::<_, i64>(1)? as u64, mtime: r.get(2)? }))
        })?;
        let mut m = HashMap::new();
        for row in rows {
            let (p, c) = row?;
            m.insert(normalize_path(Path::new(&p)), c);
        }
        m
    };
    let to_parse: Vec<PathBuf> = files
        .iter()
        .filter(|p| {
            if request.force {
                return true;
            }
            match (cache.get(&normalize_path(p)), file_stamp(p)) {
                (Some(c), Some((size, mtime))) => c.size != size || c.mtime != mtime,
                _ => true,
            }
        })
        .cloned()
        .collect();
    report.files_cached = files.len() - to_parse.len();
    progress(ScanProgress::Started { total: files.len(), to_parse: to_parse.len() });

    // parse in parallel, store on this thread in file order chunks
    let threads = if request.threads == 0 {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16)
    } else {
        request.threads
    };
    let next = AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel::<(usize, ParsedFile)>();
    let mut store_error: Option<CoreError> = None;
    std::thread::scope(|s| {
        for _ in 0..threads.max(1).min(to_parse.len().max(1)) {
            let tx = tx.clone();
            let next = &next;
            let to_parse = &to_parse;
            s.spawn(move || loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = to_parse.get(i) else { break };
                let parsed = std::panic::catch_unwind(|| parse::parse_font_file(path)).unwrap_or_else(|_| ParsedFile {
                    format: FileFormat::Unknown,
                    sha: String::new(),
                    faces: Vec::new(),
                });
                if tx.send((i, parsed)).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        let mut batch: Vec<(usize, ParsedFile)> = Vec::new();
        for (n, item) in rx.into_iter().enumerate() {
            progress(ScanProgress::File { done: n + 1, total: to_parse.len(), path: to_parse[item.0].clone() });
            batch.push(item);
            if batch.len() >= 64 {
                if let Err(e) = store_batch(db, &to_parse, &mut batch, &mut report) {
                    store_error.get_or_insert(e);
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        }
        if store_error.is_none() {
            if let Err(e) = store_batch(db, &to_parse, &mut batch, &mut report) {
                store_error = Some(e);
            }
        }
    });
    if let Some(e) = store_error {
        return Err(e);
    }

    report.cancelled = cancel.load(Ordering::Relaxed);
    if !report.cancelled {
        report.files_removed = remove_stale(db, &files)?;
        let mut fonts = db.load_fonts()?;
        crate::catalog::apply_installed(&mut fonts, &request.installed);
        let states = crate::duplicates::compute(&fonts);
        let changed: Vec<(FontId, DuplicateState)> =
            fonts.iter().zip(states).filter(|(f, s)| f.duplicate != *s).map(|(f, s)| (f.id, s)).collect();
        db.store_duplicates(&changed)?;
        db.set_property("last_scan", &chrono_like_now())?;
    }
    report.faces = db.font_count()?;
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    progress(ScanProgress::Finished(report.clone()));
    Ok(report)
}

fn chrono_like_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

fn store_batch(
    db: &mut FontDatabase,
    paths: &[PathBuf],
    batch: &mut Vec<(usize, ParsedFile)>,
    report: &mut ScanReport,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let tx = db.connection_mut().transaction()?;
    for (i, parsed) in batch.drain(..) {
        let path = &paths[i];
        let (size, mtime) = file_stamp(path).unwrap_or((0, 0));
        let error = parsed.faces.first().and_then(|f| f.corrupt.clone());
        if error.is_some() || parsed.faces.is_empty() {
            report.corrupt += 1;
        }
        report.files_parsed += 1;
        let sha = if parsed.sha.is_empty() { format!("unreadable:{}", normalize_path(path)) } else { parsed.sha.clone() };
        let old_sha: Option<String> = tx
            .query_row("SELECT sha FROM files WHERE path = ?1", [path.to_string_lossy()], |r| r.get(0))
            .ok();
        tx.execute(
            "INSERT INTO files(path, size, mtime, sha, format, error, scanned_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime, sha = excluded.sha, \
             format = excluded.format, error = excluded.error, scanned_at = excluded.scanned_at",
            params![
                path.to_string_lossy(),
                size as i64,
                mtime,
                sha,
                parsed.format.db_name(),
                error,
                chrono_like_now().parse::<i64>().unwrap_or(0)
            ],
        )?;
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM fonts WHERE sha = ?1)", [&sha], |r| r.get(0))?;
        if !exists {
            for face in &parsed.faces {
                let mut f = face.clone();
                f.sha = sha.clone();
                if parsed.sha.is_empty() || f.key.0.is_empty() || f.key.0.starts_with(':') {
                    f.key = FontKey::new(&sha, f.face_index, f.instance_index);
                }
                insert_font(&tx, &f)?;
            }
        }
        if let Some(old) = old_sha.filter(|o| *o != sha) {
            delete_orphan_faces(&tx, &old)?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn delete_orphan_faces(conn: &rusqlite::Connection, sha: &str) -> Result<()> {
    let still_used: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM files WHERE sha = ?1)", [sha], |r| r.get(0))?;
    if !still_used {
        conn.execute("DELETE FROM fonts WHERE sha = ?1 AND remote_asset_url IS NULL", [sha])?;
    }
    Ok(())
}

fn remove_stale(db: &mut FontDatabase, files: &[PathBuf]) -> Result<usize> {
    let keep: HashSet<String> = files.iter().map(|p| normalize_path(p)).collect();
    let tx = db.connection_mut().transaction()?;
    let stale: Vec<(i64, String)> = {
        let mut st = tx.prepare("SELECT id, path, sha FROM files")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut v = Vec::new();
        for row in rows {
            let (id, p, sha) = row?;
            if !keep.contains(&normalize_path(Path::new(&p))) {
                v.push((id, sha));
            }
        }
        v
    };
    for (id, _) in &stale {
        tx.execute("DELETE FROM files WHERE id = ?1", [id])?;
    }
    let shas: HashSet<&String> = stale.iter().map(|(_, s)| s).collect();
    for sha in shas {
        delete_orphan_faces(&tx, sha)?;
    }
    tx.commit()?;
    Ok(stale.len())
}

/// Handle of a scan running on a worker thread.
#[derive(Debug)]
pub struct ScanHandle {
    progress: Receiver<ScanProgress>,
    cancel: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<Result<ScanReport>>>,
}

impl ScanHandle {
    /// Requests cancellation (the worker stops after the files in flight).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    /// The cancellation flag (to share with other code).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
    /// Drains the progress events received so far (non-blocking; call once per UI frame).
    pub fn poll(&self) -> Vec<ScanProgress> {
        self.progress.try_iter().collect()
    }
    /// True when the worker has finished.
    pub fn is_finished(&self) -> bool {
        self.join.as_ref().map(|j| j.is_finished()).unwrap_or(true)
    }
    /// Waits for the worker and returns its result.
    pub fn join(mut self) -> Result<ScanReport> {
        match self.join.take() {
            Some(j) => j.join().unwrap_or_else(|_| Err(CoreError::NotAllowed("the scan thread panicked".into()))),
            None => Err(CoreError::NotAllowed("scan already joined".into())),
        }
    }
}

/// Starts a scan on a new thread with its own database connection.
pub fn spawn_scan(request: ScanRequest) -> ScanHandle {
    let (tx, rx): (Sender<ScanProgress>, Receiver<ScanProgress>) = std::sync::mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let join = std::thread::Builder::new()
        .name("anyfont-scan".into())
        .spawn(move || {
            let mut db = FontDatabase::open(&request.db_path)?;
            let mut send = |p: ScanProgress| {
                let _ = tx.send(p);
            };
            scan(&mut db, &request, &mut send, &flag)
        })
        .ok();
    ScanHandle { progress: rx, cancel, join }
}
