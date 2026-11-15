//! Install / uninstall / delete logic shared by the real and the sandbox backends.
//!
//! The backends only differ in the [`Platform`] primitives (registry access, GDI font resources,
//! file protection, Recycle Bin). Everything the dossier describes about *what* happens lives
//! here, so the sandbox tests exercise the same code path the real backend runs.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::fontfile::{self, FontFormat};
use crate::paths::{AppPaths, FontDirs};
use crate::regname;
use crate::util::{is_directly_in, path_key, same_path};
use crate::{
    DeleteOutcome, DeleteReport, FileRemoval, FileResult, FontOpError, InstallOutcome, InstallReport, InstalledFont, Progress,
    RegKind, Scope, UninstallOutcome, UninstallReport, UsedFontAction,
};

/// Registry value data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RegData {
    Sz(String),
    MultiSz(Vec<String>),
}

/// OS primitives a backend provides.
pub(crate) trait Platform: Send + Sync {
    fn dirs(&self) -> &FontDirs;
    fn app_paths(&self) -> &AppPaths;
    /// Values of the `Fonts` / `Type 1 Installer\Type 1 Fonts` key of a scope.
    fn reg_list(&self, scope: Scope, kind: RegKind) -> io::Result<Vec<(String, RegData)>>;
    fn reg_set(&self, scope: Scope, kind: RegKind, name: &str, data: &RegData) -> io::Result<()>;
    /// Returns whether a value was deleted.
    fn reg_delete(&self, scope: Scope, kind: RegKind, name: &str) -> io::Result<bool>;
    /// `AddFontResourceExW(spec, 0)`; spec = path or `pfm|pfb`.
    fn add_font_resource(&self, spec: &str) -> bool;
    /// `RemoveFontResourceExW` until it fails; returns whether something was removed.
    fn remove_font_resource(&self, spec: &str) -> bool;
    /// Notify applications (`WM_FONTCHANGE`).
    fn font_change(&self);
    fn is_protected(&self, path: &Path) -> bool;
    /// Held open by another process (`SystemIO::File::IsOpenElsewhere`).
    fn is_in_use(&self, path: &Path) -> bool;
    fn is_read_only(&self, path: &Path) -> bool {
        std::fs::metadata(path).map(|m| m.permissions().readonly()).unwrap_or(false)
    }
    /// `MoveFileExW(path, NULL, MOVEFILE_DELAY_UNTIL_REBOOT)`.
    fn delete_on_reboot(&self, path: &Path) -> io::Result<()>;
    /// Move files to the Recycle Bin.
    fn recycle(&self, paths: &[PathBuf]) -> io::Result<()>;
    /// Called before the engine itself changes files in the fonts folders (copy, remove). The real
    /// backend panics here in test builds; the sandbox allows it.
    fn guard_file_writes(&self, _what: &str) {}
}

/// One registry value, resolved.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub scope: Scope,
    pub kind: RegKind,
    pub name: String,
    /// `[file]`, or `[pfm, pfb]` for Type 1.
    pub files: Vec<PathBuf>,
}

impl Entry {
    fn spec(&self) -> String {
        spec_of(&self.files)
    }
    fn contains(&self, path: &Path) -> bool {
        self.files.iter().any(|f| same_path(f, path))
    }
}

fn spec_of(files: &[PathBuf]) -> String {
    files.iter().map(|f| f.to_string_lossy().into_owned()).collect::<Vec<_>>().join("|")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PendingList {
    files: Vec<PathBuf>,
}

/// Phase A of a removal for one path: the registry entries, whether a reboot is needed, the files and
/// what was unloaded.
type PlannedRemoval = (Vec<Entry>, bool, Vec<PathBuf>, Unloaded);

pub(crate) struct Engine<P: Platform> {
    pub p: P,
    /// Fonts loaded for the session by this process: (files, value name).
    session: Mutex<Vec<(Vec<PathBuf>, String)>>,
}

impl<P: Platform> Engine<P> {
    pub fn new(p: P) -> Self {
        Engine { p, session: Mutex::new(Vec::new()) }
    }

    // ----- enumeration ----------------------------------------------------------------------

    pub fn resolve(&self, scope: Scope, data: &str) -> PathBuf {
        let data = data.trim().trim_matches('"');
        let p = Path::new(data);
        if p.has_root() || data.get(1..2) == Some(":") {
            return p.to_path_buf();
        }
        let dirs = self.p.dirs();
        match scope {
            Scope::CurrentUser => {
                let user = dirs.current_user.join(data);
                if user.exists() {
                    user
                } else {
                    dirs.all_users.join(data)
                }
            }
            _ => dirs.all_users.join(data),
        }
    }

    pub fn entries(&self) -> io::Result<Vec<Entry>> {
        let mut out = Vec::new();
        for scope in [Scope::AllUsers, Scope::CurrentUser] {
            for (name, data) in self.p.reg_list(scope, RegKind::Fonts)? {
                let file = match &data {
                    RegData::Sz(s) => s.clone(),
                    RegData::MultiSz(v) => v.iter().find(|s| !s.is_empty()).cloned().unwrap_or_default(),
                };
                if file.is_empty() {
                    continue;
                }
                out.push(Entry { scope, kind: RegKind::Fonts, name, files: vec![self.resolve(scope, &file)] });
            }
            for (name, data) in self.p.reg_list(scope, RegKind::Type1)? {
                let items = match data {
                    RegData::MultiSz(v) => v,
                    RegData::Sz(s) => s.split('|').map(str::to_string).collect(),
                };
                if let Some((pfm, pfb)) = regname::parse_type1_data(&items) {
                    out.push(Entry {
                        scope,
                        kind: RegKind::Type1,
                        name,
                        files: vec![self.resolve(scope, &pfm), self.resolve(scope, &pfb)],
                    });
                }
            }
        }
        Ok(out)
    }

    pub fn list(&self) -> io::Result<Vec<InstalledFont>> {
        let mut fonts: Vec<InstalledFont> = self
            .entries()?
            .into_iter()
            .map(|e| {
                let path = e.files[0].clone();
                InstalledFont {
                    exists: path.is_file(),
                    protected: e.scope == Scope::AllUsers && self.p.is_protected(&path),
                    value_name: e.name,
                    extra_paths: e.files[1..].to_vec(),
                    path,
                    scope: e.scope,
                    kind: e.kind,
                }
            })
            .collect();
        for (files, name) in self.session.lock().unwrap().iter() {
            fonts.push(InstalledFont {
                value_name: name.clone(),
                path: files[0].clone(),
                extra_paths: files[1..].to_vec(),
                scope: Scope::Session,
                kind: RegKind::None,
                exists: files[0].is_file(),
                protected: false,
            });
        }
        Ok(fonts)
    }

    /// Registrations (any scope) of a file; true when loaded for the session by this process.
    fn registrations(&self, entries: &[Entry], path: &Path) -> (Vec<Entry>, bool) {
        let regs = entries.iter().filter(|e| e.contains(path)).cloned().collect();
        let session = self.session.lock().unwrap().iter().any(|(f, _)| f.iter().any(|x| same_path(x, path)));
        (regs, session)
    }

    /// File lives directly in one of the two fonts folders (only such copies are deleted on uninstall).
    fn in_fonts_dir(&self, path: &Path) -> bool {
        let d = self.p.dirs();
        is_directly_in(&d.all_users, path) || is_directly_in(&d.current_user, path)
    }

    // ----- install --------------------------------------------------------------------------

    pub fn install(&self, files: &[PathBuf], scope: Scope, used: UsedFontAction, progress: Progress<'_>) -> InstallReport {
        if !files.is_empty() {
            self.p.guard_file_writes("install fonts");
        }
        let mut report = InstallReport::default();
        let mut entries = match self.entries() {
            Ok(e) => e,
            Err(e) => {
                report.results = files.iter().map(|f| FileResult { path: f.clone(), result: Err(e_to_op(&e)) }).collect();
                return report;
            }
        };
        let mut changed = false;
        let mut cancelled = false;
        // A .pfm and its .pfb given together are one font: install once.
        let mut done_type1: Vec<String> = Vec::new();
        for (i, src) in files.iter().enumerate() {
            if !cancelled && !progress(i, files.len(), src) {
                cancelled = true;
            }
            let result = if cancelled {
                Err(FontOpError::Cancelled)
            } else {
                let ext = fontfile::extension(src);
                let dup_type1 = (ext == "pfm" || ext == "pfb")
                    && fontfile::type1_pair(src).map(|(m, _)| done_type1.contains(&path_key(&m))).unwrap_or(false);
                if dup_type1 {
                    fontfile::type1_pair(src).ok().and_then(|(m, _)| {
                        report.results.iter().find(|r| {
                            r.result.is_ok() && fontfile::type1_pair(&r.path).map(|(m2, _)| same_path(&m, &m2)).unwrap_or(false)
                        })
                    }).map(|r| r.result.clone()).unwrap_or(Err(FontOpError::Cancelled))
                } else {
                    let r = self.install_one(src, scope, used, &mut entries, &mut report);
                    if let Ok(InstallOutcome::Installed { .. }) = &r {
                        changed = true;
                    }
                    if r.is_ok() && (ext == "pfm" || ext == "pfb") {
                        if let Ok((m, _)) = fontfile::type1_pair(src) {
                            done_type1.push(path_key(&m));
                        }
                    }
                    r
                }
            };
            report.results.push(FileResult { path: src.clone(), result });
        }
        progress(files.len(), files.len(), Path::new(""));
        if changed {
            self.p.font_change();
        }
        report
    }

    fn install_one(
        &self,
        src: &Path,
        scope: Scope,
        used: UsedFontAction,
        entries: &mut Vec<Entry>,
        report: &mut InstallReport,
    ) -> Result<InstallOutcome, FontOpError> {
        let info = fontfile::inspect(src)?;
        if info.format == FontFormat::Bitmap {
            return Err(FontOpError::UnsupportedFormat(fontfile::extension(src)));
        }
        let src_files: Vec<PathBuf> = match &info.type1_files {
            Some((m, b)) => vec![m.clone(), b.clone()],
            None => vec![src.to_path_buf()],
        };
        let value_name = info.registry_value_name();

        if scope == Scope::Session {
            let mut session = self.session.lock().unwrap();
            if session.iter().any(|(f, _)| same_path(&f[0], &src_files[0])) {
                return Ok(InstallOutcome::AlreadyInstalled { value_name, installed_path: src_files[0].clone() });
            }
            if !self.p.add_font_resource(&spec_of(&src_files)) {
                return Err(FontOpError::InvalidFont("Windows could not load the font".into()));
            }
            session.push((src_files.clone(), value_name.clone()));
            return Ok(InstallOutcome::Installed { value_name, installed_path: src_files[0].clone(), replaced: None });
        }

        let kind = if info.format == FontFormat::Type1 { RegKind::Type1 } else { RegKind::Fonts };
        if kind == RegKind::Type1 && scope != Scope::AllUsers {
            return Err(FontOpError::Type1NeedsAllUsers);
        }
        let target_dir = self.p.dirs().for_scope(scope).expect("persistent scope").to_path_buf();

        // Already registered in this scope (same file): reuse that registration (dossier 0x180202BC0).
        if let Some(e) = entries.iter().find(|e| e.scope == scope && e.kind == kind && e.contains(&src_files[0])) {
            return Ok(InstallOutcome::AlreadyInstalled { value_name: e.name.clone(), installed_path: e.files[0].clone() });
        }

        // Same registry name already taken in this scope: identical file -> nothing to do; protected -> refuse;
        // otherwise the new file replaces the old registration (master duplicate, dossier 0x1800F1200).
        let existing = entries.iter().position(|e| e.scope == scope && e.kind == kind && e.name.eq_ignore_ascii_case(&value_name));
        if let Some(idx) = existing {
            let e = &entries[idx];
            if e.files.len() == src_files.len() && e.files.iter().zip(&src_files).all(|(a, b)| files_identical(a, b)) {
                return Ok(InstallOutcome::AlreadyInstalled { value_name: e.name.clone(), installed_path: e.files[0].clone() });
            }
            if self.p.is_protected(&e.files[0]) {
                return Err(FontOpError::Protected);
            }
        }

        // Copy into the fonts folder unless the file already lives there ("%s%s_%d%s" on collision).
        std::fs::create_dir_all(&target_dir)?;
        let mut dests: Vec<PathBuf> = Vec::new();
        let mut copied: Vec<PathBuf> = Vec::new();
        for f in &src_files {
            if is_directly_in(&target_dir, f) {
                dests.push(f.clone());
                continue;
            }
            let dest = unique_dest(&target_dir, f, &dests);
            if let Err(e) = std::fs::copy(f, &dest) {
                for c in &copied {
                    let _ = std::fs::remove_file(c);
                }
                return Err(e.into());
            }
            // Copies must not stay read-only (they could never be uninstalled otherwise).
            if let Ok(meta) = std::fs::metadata(&dest) {
                let mut perm = meta.permissions();
                if perm.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perm.set_readonly(false);
                    let _ = std::fs::set_permissions(&dest, perm);
                }
            }
            copied.push(dest.clone());
            dests.push(dest);
        }
        let rollback_copies = |copied: &[PathBuf]| {
            for c in copied {
                let _ = std::fs::remove_file(c);
            }
        };

        // Replace the previous registration of the same name.
        let mut replaced = None;
        if let Some(idx) = existing {
            let old = entries[idx].clone();
            let removed = self.p.remove_font_resource(&old.spec());
            let old_owned: Vec<PathBuf> =
                old.files.iter().filter(|f| self.in_fonts_dir(f) && !dests.iter().any(|d| same_path(d, f))).cloned().collect();
            let busy = old_owned.iter().any(|f| self.p.is_in_use(f));
            if busy {
                match used {
                    UsedFontAction::RebootNow | UsedFontAction::RebootLater => {
                        for f in &old_owned {
                            self.defer_delete(f);
                        }
                        report.reboot_required = true;
                    }
                    UsedFontAction::Cancel => {
                        if removed {
                            self.p.add_font_resource(&old.spec());
                        }
                        rollback_copies(&copied);
                        return Err(FontOpError::Cancelled);
                    }
                    UsedFontAction::Ask | UsedFontAction::Ignore => {
                        if removed {
                            self.p.add_font_resource(&old.spec());
                        }
                        rollback_copies(&copied);
                        report.in_use.push(src.to_path_buf());
                        return Err(FontOpError::InUse);
                    }
                }
            } else {
                for f in &old_owned {
                    let _ = std::fs::remove_file(f);
                }
            }
            let _ = self.p.reg_delete(old.scope, old.kind, &old.name);
            entries.remove(idx);
            replaced = Some(old.files[0].clone());
        }

        let sys_dir = (scope == Scope::AllUsers).then(|| self.p.dirs().all_users.clone());
        let data = match kind {
            RegKind::Type1 => RegData::MultiSz(regname::type1_data(
                &regname::data_for(&dests[0], sys_dir.as_deref()),
                &regname::data_for(&dests[1], sys_dir.as_deref()),
            )),
            _ => RegData::Sz(regname::data_for(&dests[0], sys_dir.as_deref())),
        };
        if let Err(e) = self.p.reg_set(scope, kind, &value_name, &data) {
            rollback_copies(&copied);
            return Err(e_to_op(&e));
        }
        if !self.p.add_font_resource(&spec_of(&dests)) {
            let _ = self.p.reg_delete(scope, kind, &value_name);
            rollback_copies(&copied);
            return Err(FontOpError::InvalidFont("Windows could not load the font".into()));
        }
        entries.push(Entry { scope, kind, name: value_name.clone(), files: dests.clone() });
        Ok(InstallOutcome::Installed { value_name, installed_path: dests[0].clone(), replaced })
    }

    // ----- uninstall / delete -----------------------------------------------------------------

    /// Phase A of uninstall/delete for one path: unload its font resources and decide whether
    /// the files it would remove are in use.
    fn unload(&self, regs: &[Entry], session: bool, path: &Path, removing: &[PathBuf]) -> Unloaded {
        let mut reloaded = Vec::new();
        for e in regs {
            if self.p.remove_font_resource(&e.spec()) {
                reloaded.push(e.spec());
            }
        }
        if session {
            let session_specs: Vec<String> = self
                .session
                .lock()
                .unwrap()
                .iter()
                .filter(|(f, _)| f.iter().any(|x| same_path(x, path)))
                .map(|(f, _)| spec_of(f))
                .collect();
            for s in session_specs {
                if self.p.remove_font_resource(&s) {
                    reloaded.push(s);
                }
            }
        }
        let busy = removing.iter().any(|f| self.p.is_in_use(f));
        Unloaded { reload: reloaded, busy }
    }

    fn reload(&self, u: &Unloaded) {
        for s in &u.reload {
            self.p.add_font_resource(s);
        }
    }

    /// Phase B: drop registrations. Returns the removed value names.
    fn unregister(&self, regs: &[Entry], session: bool, path: &Path) -> Vec<String> {
        let mut names = Vec::new();
        for e in regs {
            if self.p.reg_delete(e.scope, e.kind, &e.name).unwrap_or(false) {
                names.push(e.name.clone());
            }
        }
        if session {
            let mut s = self.session.lock().unwrap();
            s.retain(|(f, n)| {
                let hit = f.iter().any(|x| same_path(x, path));
                if hit {
                    names.push(n.clone());
                }
                !hit
            });
        }
        names
    }

    pub fn uninstall(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> UninstallReport {
        if !paths.is_empty() {
            self.p.guard_file_writes("uninstall fonts");
        }
        let mut report = UninstallReport::default();
        let entries = match self.entries() {
            Ok(e) => e,
            Err(e) => {
                report.results = paths.iter().map(|f| FileResult { path: f.clone(), result: Err(e_to_op(&e)) }).collect();
                return report;
            }
        };
        // Phase A for every path.
        let mut plan: Vec<Result<PlannedRemoval, FontOpError>> = Vec::new();
        let mut cancelled_at = None;
        for (i, path) in paths.iter().enumerate() {
            if cancelled_at.is_none() && !progress(i, paths.len(), path) {
                cancelled_at = Some(i);
            }
            if cancelled_at.is_some() {
                plan.push(Err(FontOpError::Cancelled));
                continue;
            }
            let (regs, session) = self.registrations(&entries, path);
            if regs.is_empty() && !session {
                plan.push(Err(FontOpError::NotInstalled));
                continue;
            }
            if regs.iter().any(|e| e.scope == Scope::AllUsers && self.p.is_protected(&e.files[0])) {
                plan.push(Err(FontOpError::Protected));
                continue;
            }
            let removing: Vec<PathBuf> = regs.iter().flat_map(|e| e.files.iter()).filter(|f| self.in_fonts_dir(f)).cloned().collect();
            let u = self.unload(&regs, session, path, &removing);
            plan.push(Ok((regs, session, removing, u)));
        }
        let any_busy = plan.iter().any(|p| matches!(p, Ok((_, _, _, u)) if u.busy));
        if used == UsedFontAction::Cancel && any_busy {
            for (path, p) in paths.iter().zip(&plan) {
                if let Ok((_, _, _, u)) = p {
                    self.reload(u);
                    if u.busy {
                        report.in_use.push(path.clone());
                    }
                }
                report.results.push(FileResult { path: path.clone(), result: Err(FontOpError::Cancelled) });
            }
            return report;
        }
        let mut changed = false;
        for (path, p) in paths.iter().zip(plan) {
            let result = match p {
                Err(e) => Err(e),
                Ok((regs, session, removing, u)) => {
                    if u.busy && !used.defers_to_reboot() {
                        self.reload(&u);
                        report.in_use.push(path.clone());
                        Err(FontOpError::InUse)
                    } else {
                        let names = self.unregister(&regs, session, path);
                        changed = true;
                        let file = if removing.is_empty() {
                            FileRemoval::Kept
                        } else if u.busy {
                            for f in &removing {
                                self.defer_delete(f);
                            }
                            report.reboot_required = true;
                            FileRemoval::PendingReboot
                        } else {
                            let mut state = FileRemoval::Removed;
                            for f in &removing {
                                if let Err(e) = std::fs::remove_file(f) {
                                    if e.kind() != io::ErrorKind::NotFound {
                                        self.defer_delete(f);
                                        report.reboot_required = true;
                                        state = FileRemoval::PendingReboot;
                                    }
                                }
                            }
                            state
                        };
                        Ok(UninstallOutcome { value_names: names, file })
                    }
                }
            };
            report.results.push(FileResult { path: path.clone(), result });
        }
        progress(paths.len(), paths.len(), Path::new(""));
        if changed {
            self.p.font_change();
        }
        report
    }

    pub fn delete(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> DeleteReport {
        if !paths.is_empty() {
            self.p.guard_file_writes("delete fonts");
        }
        let mut report = DeleteReport::default();
        let entries = match self.entries() {
            Ok(e) => e,
            Err(e) => {
                report.results = paths.iter().map(|f| FileResult { path: f.clone(), result: Err(e_to_op(&e)) }).collect();
                return report;
            }
        };
        type Plan = (Vec<Entry>, bool, Vec<PathBuf>, Unloaded);
        let mut plan: Vec<Result<Plan, FontOpError>> = Vec::new();
        let mut cancelled = false;
        for (i, path) in paths.iter().enumerate() {
            if !cancelled && !progress(i, paths.len(), path) {
                cancelled = true;
            }
            if cancelled {
                plan.push(Err(FontOpError::Cancelled));
                continue;
            }
            if !path.is_file() {
                plan.push(Err(FontOpError::NotFound));
                continue;
            }
            // Only font files are ever deleted (the elevated helper runs this on paths read from a
            // request file, so this also bounds what a tampered request could reach).
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !fontfile::INSTALLABLE_EXTENSIONS.contains(&ext.as_str()) && ext != "fon" && ext != "fnt" {
                plan.push(Err(FontOpError::UnsupportedFormat(ext)));
                continue;
            }
            let (regs, session) = self.registrations(&entries, path);
            if self.p.is_protected(path) || regs.iter().any(|e| e.scope == Scope::AllUsers && self.p.is_protected(&e.files[0])) {
                plan.push(Err(FontOpError::Protected));
                continue;
            }
            // The files that go: the path itself, plus the partner of a registered Type 1 pair.
            let mut files = vec![path.clone()];
            for e in regs.iter().filter(|e| e.kind == RegKind::Type1) {
                for f in &e.files {
                    if !files.iter().any(|x| same_path(x, f)) && f.is_file() {
                        files.push(f.clone());
                    }
                }
            }
            if files.iter().any(|f| self.p.is_read_only(f)) {
                plan.push(Err(FontOpError::ReadOnly));
                continue;
            }
            let u = self.unload(&regs, session, path, &files);
            plan.push(Ok((regs, session, files, u)));
        }
        let any_busy = plan.iter().any(|p| matches!(p, Ok((_, _, _, u)) if u.busy));
        if used == UsedFontAction::Cancel && any_busy {
            for (path, p) in paths.iter().zip(&plan) {
                if let Ok((_, _, _, u)) = p {
                    self.reload(u);
                    if u.busy {
                        report.in_use.push(path.clone());
                    }
                }
                report.results.push(FileResult { path: path.clone(), result: Err(FontOpError::Cancelled) });
            }
            return report;
        }
        let mut changed = false;
        let mut to_recycle: Vec<(usize, Vec<PathBuf>)> = Vec::new();
        for (i, (path, p)) in paths.iter().zip(plan).enumerate() {
            let result = match p {
                Err(e) => Err(e),
                Ok((regs, session, files, u)) => {
                    if u.busy && !used.defers_to_reboot() {
                        self.reload(&u);
                        report.in_use.push(path.clone());
                        Err(FontOpError::InUse)
                    } else {
                        let uninstalled = if regs.is_empty() && !session {
                            None
                        } else {
                            changed = true;
                            let names = self.unregister(&regs, session, path);
                            let file = if u.busy { FileRemoval::PendingReboot } else { FileRemoval::Removed };
                            Some(UninstallOutcome { value_names: names, file })
                        };
                        if u.busy {
                            for f in &files {
                                self.defer_delete(f);
                            }
                            report.reboot_required = true;
                            Ok(DeleteOutcome::PendingReboot { uninstalled })
                        } else {
                            to_recycle.push((i, files));
                            Ok(DeleteOutcome::Recycled { uninstalled })
                        }
                    }
                }
            };
            report.results.push(FileResult { path: path.clone(), result });
        }
        if !to_recycle.is_empty() {
            let all: Vec<PathBuf> = to_recycle.iter().flat_map(|(_, f)| f.iter().cloned()).collect();
            let res = self.p.recycle(&all);
            for (i, files) in &to_recycle {
                if files.iter().any(|f| f.exists()) {
                    let err = match &res {
                        Err(e) => e_to_op(e),
                        Ok(()) => FontOpError::Io("the file could not be moved to the Recycle Bin".into()),
                    };
                    report.results[*i].result = Err(err);
                }
            }
        }
        progress(paths.len(), paths.len(), Path::new(""));
        if changed {
            self.p.font_change();
        }
        report
    }

    // ----- deferred deletions ---------------------------------------------------------------

    /// Schedule `path` for deletion at the next restart; when the OS refuses (per-user process
    /// without admin rights) the file is remembered and deleted by [`Self::process_pending`].
    pub fn defer_delete(&self, path: &Path) {
        self.p.guard_file_writes("schedule a font deletion");
        if self.p.delete_on_reboot(path).is_ok() {
            return;
        }
        let file = self.p.app_paths().pending_file();
        let mut list: PendingList =
            std::fs::read(&file).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
        if !list.files.iter().any(|f| same_path(f, path)) {
            list.files.push(path.to_path_buf());
        }
        if let Ok(data) = serde_json::to_vec_pretty(&list) {
            let _ = crate::util::write_atomic(&file, &data);
        }
    }

    pub fn process_pending(&self) -> usize {
        let file = self.p.app_paths().pending_file();
        let Some(list) = std::fs::read(&file).ok().and_then(|d| serde_json::from_slice::<PendingList>(&d).ok()) else {
            return 0;
        };
        if !list.files.is_empty() {
            self.p.guard_file_writes("delete pending font files");
        }
        let entries = self.entries().unwrap_or_default();
        let mut removed = 0;
        let mut keep = Vec::new();
        for f in list.files {
            if !f.exists() {
                continue;
            }
            // Re-installed meanwhile: not ours to delete any more.
            if entries.iter().any(|e| e.contains(&f)) {
                continue;
            }
            match std::fs::remove_file(&f) {
                Ok(()) => removed += 1,
                Err(_) => keep.push(f),
            }
        }
        if keep.is_empty() {
            let _ = std::fs::remove_file(&file);
        } else if let Ok(data) = serde_json::to_vec_pretty(&PendingList { files: keep }) {
            let _ = crate::util::write_atomic(&file, &data);
        }
        removed
    }
}

pub(crate) struct Unloaded {
    /// Resource specs to re-add if the operation does not go ahead.
    reload: Vec<String>,
    /// A file that would be removed is held open elsewhere.
    busy: bool,
}

/// `<dir>\<name>`, or `<dir>\<stem>_<n><ext>` (n = 1, 2, ...) when taken.
fn unique_dest(dir: &Path, src: &Path, reserved: &[PathBuf]) -> PathBuf {
    let name = src.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    let first = dir.join(&name);
    let taken = |p: &Path| p.exists() || reserved.iter().any(|r| same_path(r, p));
    if !taken(&first) {
        return first;
    }
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1u32..)
        .map(|n| dir.join(format!("{stem}_{n}{ext}")))
        .find(|p| !taken(p))
        .expect("unbounded")
}

fn files_identical(a: &Path, b: &Path) -> bool {
    if same_path(a, b) {
        return true;
    }
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) if ma.len() == mb.len() => match (std::fs::read(a), std::fs::read(b)) {
            (Ok(da), Ok(db)) => da == db,
            _ => false,
        },
        _ => false,
    }
}

pub(crate) fn e_to_op(e: &io::Error) -> FontOpError {
    match e.kind() {
        io::ErrorKind::NotFound => FontOpError::NotFound,
        io::ErrorKind::PermissionDenied => FontOpError::AccessDenied,
        _ if e.raw_os_error() == Some(5) => FontOpError::AccessDenied,
        _ if e.raw_os_error() == Some(32) || e.raw_os_error() == Some(33) => FontOpError::InUse,
        _ => FontOpError::Io(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_follow_dossier_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let src = Path::new(r"D:\x\Font.TTF");
        assert_eq!(unique_dest(dir.path(), src, &[]), dir.path().join("Font.TTF"));
        std::fs::write(dir.path().join("Font.TTF"), b"a").unwrap();
        assert_eq!(unique_dest(dir.path(), src, &[]), dir.path().join("Font_1.TTF"));
        std::fs::write(dir.path().join("Font_1.TTF"), b"a").unwrap();
        assert_eq!(unique_dest(dir.path(), src, &[]), dir.path().join("Font_2.TTF"));
        assert_eq!(unique_dest(dir.path(), src, &[dir.path().join("font_2.ttf")]), dir.path().join("Font_3.TTF"));
    }
}
