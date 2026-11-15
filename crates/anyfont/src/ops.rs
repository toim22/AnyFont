//! Install / Uninstall / Delete on a worker thread (`anyfont_sys::FontSystem`), with progress, cancel and the
//! fonts-in-use two-pass flow (sys-api.md): the first pass (`UsedFontAction::Ask`) leaves fonts that are in
//! use untouched; the UI then shows the reboot prompt and runs a second pass for those files with the chosen
//! action.

use anyfont_sys::{FontOpError, FontSystem, Scope, UsedFontAction};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Install,
    Uninstall,
    Delete,
}

impl OpKind {
    /// The progress line format (`docs/strings-en.tsv`).
    pub fn progress_format(self) -> &'static str {
        match self {
            OpKind::Install => "Installing {}... {}%",
            OpKind::Uninstall => "Uninstalling {}... {}%",
            OpKind::Delete => "Deleting {}... {}%",
        }
    }
}

/// What a finished operation reports.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub succeeded: usize,
    pub failures: Vec<(PathBuf, FontOpError)>,
    pub in_use: Vec<PathBuf>,
    pub reboot_required: bool,
    pub elevation_cancelled: bool,
    /// The files that were not processed because the UAC prompt was cancelled.
    pub elevation_cancelled_files: Vec<PathBuf>,
    pub cancelled: bool,
}

enum Msg {
    Progress { done: usize, total: usize, path: PathBuf },
    Done(Outcome),
}

/// A running (or just finished) operation.
pub struct Running {
    pub kind: OpKind,
    /// Install only: where the fonts go.
    pub scope: Scope,
    pub used: UsedFontAction,
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub outcome: Option<Outcome>,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Msg>,
    names: HashMap<PathBuf, String>,
}

impl Running {
    /// Starts `kind` for `files` on a worker thread. `names` maps a file to the font name shown in the
    /// progress line; `scope` is where Install puts the fonts (see [`install_scope`]).
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        fs: Arc<dyn FontSystem>,
        kind: OpKind,
        scope: Scope,
        files: Vec<PathBuf>,
        used: UsedFontAction,
        names: HashMap<PathBuf, String>,
        ctx: egui::Context,
    ) -> Running {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let total = files.len();
        let c = cancel.clone();
        std::thread::Builder::new()
            .name("anyfont-font-op".into())
            .spawn(move || {
                let mut progress = |done: usize, total: usize, p: &std::path::Path| -> bool {
                    let _ = tx.send(Msg::Progress { done, total, path: p.to_path_buf() });
                    ctx.request_repaint();
                    !c.load(Ordering::Relaxed)
                };
                log::info!("{kind:?} {} file(s), {used:?}", files.len());
                let outcome = match kind {
                    OpKind::Install => {
                        let r = fs.install_with(&files, scope, used, &mut progress);
                        outcome_of(&r.results, &r.in_use, r.reboot_required, r.elevation_cancelled())
                    }
                    OpKind::Uninstall => {
                        let r = fs.uninstall_with(&files, used, &mut progress);
                        outcome_of(&r.results, &r.in_use, r.reboot_required, r.elevation_cancelled())
                    }
                    OpKind::Delete => {
                        let r = fs.delete_with(&files, used, &mut progress);
                        outcome_of(&r.results, &r.in_use, r.reboot_required, r.elevation_cancelled())
                    }
                };
                log::info!(
                    "{kind:?} finished: {} ok, {} failed, {} in use",
                    outcome.succeeded,
                    outcome.failures.len(),
                    outcome.in_use.len()
                );
                let _ = tx.send(Msg::Done(outcome));
                ctx.request_repaint();
            })
            .expect("operation thread");
        Running { kind, scope, used, total, done: 0, current: String::new(), outcome: None, cancel, rx, names }
    }

    /// Takes the worker's messages. Returns true when the operation just finished.
    pub fn poll(&mut self) -> bool {
        let mut finished = false;
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Progress { done, total, path } => {
                    self.done = done;
                    self.total = total;
                    self.current = self.names.get(&path).cloned().unwrap_or_else(|| {
                        path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                    });
                }
                Msg::Done(o) => {
                    self.done = self.total;
                    self.outcome = Some(o);
                    finished = true;
                }
            }
        }
        finished
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn percent(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        ((self.done as f32 / self.total as f32) * 100.0).round() as u32
    }

    /// "Installing Arial... 40%".
    pub fn progress_text(&self) -> String {
        crate::fmt(self.kind.progress_format(), &[&self.current, &self.percent().to_string()])
    }
}

/// Install for all users, like the original: one UAC prompt per operation through the elevated helper when
/// AnyFont is not elevated. When the prompt is cancelled the shell offers "Install only for me"
/// ([`Scope::CurrentUser`], no administrator rights), see `dialogs::finish`.
pub fn install_scope(_fs: &dyn FontSystem) -> Scope {
    Scope::AllUsers
}

fn outcome_of<T>(
    results: &[anyfont_sys::FileResult<T>],
    in_use: &[PathBuf],
    reboot_required: bool,
    elevation_cancelled: bool,
) -> Outcome {
    let mut o = Outcome { reboot_required, elevation_cancelled, in_use: in_use.to_vec(), ..Default::default() };
    for r in results {
        match &r.result {
            Ok(_) => o.succeeded += 1,
            Err(FontOpError::InUse) => {
                if !o.in_use.contains(&r.path) {
                    o.in_use.push(r.path.clone());
                }
            }
            Err(FontOpError::Cancelled) => o.cancelled = true,
            Err(FontOpError::ElevationCancelled) => {
                o.elevation_cancelled = true;
                o.elevation_cancelled_files.push(r.path.clone());
            }
            Err(e) => o.failures.push((r.path.clone(), e.clone())),
        }
    }
    o
}
