//! Sandbox backend: the same operations as the real backend, against a private folder tree.
//!
//! ```text
//! <root>\Windows\Fonts                        all-users fonts folder
//! <root>\LocalAppData\Microsoft\Windows\Fonts  per-user fonts folder
//! <root>\sandbox-registry.json                 stands in for HKLM/HKCU ...\Fonts and ...\Type 1 Fonts
//! <root>\RecycleBin\                           where "deleted" files go
//! <root>\AppData\Roaming\AnyFont, <root>\LocalAppData\AnyFont, <root>\Documents\AnyFont\Content\Fonts
//! ```
//!
//! No `AddFontResourceExW`, no `WM_FONTCHANGE`, no reboot, no real registry write, ever. A new
//! sandbox is seeded by copying some fonts that ship with Windows (read only) into its all-users
//! folder; they are registered under HKLM and marked protected, so the UI shows a realistic
//! "My fonts" list with a few protected system fonts.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::engine::{Engine, Platform, RegData};
use crate::paths::{AppPaths, FontDirs};
use crate::util::{is_directly_in, same_path};
use crate::{
    Backend, DeleteReport, FontSystem, InstallReport, InstalledFont, Progress, RegKind, Scope, UninstallReport, UsedFontAction,
};

/// Fonts copied (from `%WINDIR%\Fonts`) into a new sandbox when `ANYFONT_SANDBOX_SEED` is not set.
pub const DEFAULT_SANDBOX_SEED: &[&str] = &["arial.ttf", "arialbd.ttf", "times.ttf", "cour.ttf", "segoeui.ttf", "cambria.ttc"];

const STATE_FILE: &str = "sandbox-registry.json";

/// How a new sandbox is seeded.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Font files copied into `<root>\Windows\Fonts` and registered (HKLM, protected) when the
    /// sandbox is created. Relative names are looked up in the real `%WINDIR%\Fonts`; missing
    /// files are skipped.
    pub seed: Vec<PathBuf>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        SandboxConfig { seed: DEFAULT_SANDBOX_SEED.iter().map(PathBuf::from).collect() }
    }
}

impl SandboxConfig {
    /// No seed fonts.
    pub fn empty() -> Self {
        SandboxConfig { seed: Vec::new() }
    }

    /// From `ANYFONT_SANDBOX_SEED` (`;`-separated; `none` = empty), else the default list.
    pub fn from_env() -> Self {
        match std::env::var(crate::SANDBOX_SEED_ENV) {
            Ok(v) if v.trim().eq_ignore_ascii_case("none") || v.trim().is_empty() => Self::empty(),
            Ok(v) => SandboxConfig { seed: v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from).collect() },
            Err(_) => Self::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct State {
    /// `"HKLM\\...\\Fonts"` -> value name -> data.
    registry: BTreeMap<String, BTreeMap<String, RegData>>,
    /// Seeded "Windows" fonts.
    protected: Vec<PathBuf>,
    /// Files the tests declare as held open by another application.
    in_use: Vec<PathBuf>,
    /// Files held for the next N in-use checks only (as Windows holds a font it has just loaded).
    #[serde(default)]
    in_use_briefly: Vec<(PathBuf, u32)>,
    /// `MoveFileEx(DELAY_UNTIL_REBOOT)` stand-in.
    pending_reboot: Vec<PathBuf>,
    /// (original path, stored path) of recycled files.
    recycle_bin: Vec<(PathBuf, PathBuf)>,
    reboots: u32,
    font_change_broadcasts: u32,
    opened_locations: Vec<PathBuf>,
}

fn reg_key(scope: Scope, kind: RegKind) -> String {
    let root = match scope {
        Scope::AllUsers => "HKLM",
        _ => "HKCU",
    };
    let key = match kind {
        RegKind::Type1 => crate::regname::TYPE1_KEY,
        _ => crate::regname::FONTS_KEY,
    };
    format!("{root}\\{key}")
}

pub(crate) struct SandboxPlatform {
    root: PathBuf,
    dirs: FontDirs,
    paths: AppPaths,
    state: Mutex<()>,
}

impl SandboxPlatform {
    fn state_path(&self) -> PathBuf {
        self.root.join(STATE_FILE)
    }

    fn load(&self) -> State {
        std::fs::read(self.state_path()).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
    }

    fn save(&self, s: &State) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(s).map_err(io::Error::other)?;
        crate::util::write_atomic(&self.state_path(), &data)
    }

    /// Read-modify-write the state under the lock.
    fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> io::Result<R> {
        let _g = self.state.lock().unwrap();
        let mut s = self.load();
        let r = f(&mut s);
        self.save(&s)?;
        Ok(r)
    }

    fn read<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        let _g = self.state.lock().unwrap();
        f(&self.load())
    }
}

impl Platform for SandboxPlatform {
    fn dirs(&self) -> &FontDirs {
        &self.dirs
    }
    fn app_paths(&self) -> &AppPaths {
        &self.paths
    }
    fn reg_list(&self, scope: Scope, kind: RegKind) -> io::Result<Vec<(String, RegData)>> {
        Ok(self.read(|s| s.registry.get(&reg_key(scope, kind)).map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()))
    }
    fn reg_set(&self, scope: Scope, kind: RegKind, name: &str, data: &RegData) -> io::Result<()> {
        self.update(|s| {
            let key = s.registry.entry(reg_key(scope, kind)).or_default();
            // Registry value names are case-insensitive.
            let existing = key.keys().find(|k| k.eq_ignore_ascii_case(name)).cloned();
            if let Some(k) = existing {
                key.remove(&k);
            }
            key.insert(name.to_string(), data.clone());
        })
    }
    fn reg_delete(&self, scope: Scope, kind: RegKind, name: &str) -> io::Result<bool> {
        self.update(|s| {
            let Some(key) = s.registry.get_mut(&reg_key(scope, kind)) else { return false };
            match key.keys().find(|k| k.eq_ignore_ascii_case(name)).cloned() {
                Some(k) => key.remove(&k).is_some(),
                None => false,
            }
        })
    }
    fn add_font_resource(&self, spec: &str) -> bool {
        spec.split('|').all(|p| Path::new(p).is_file())
    }
    fn remove_font_resource(&self, _spec: &str) -> bool {
        true
    }
    fn font_change(&self) {
        let _ = self.update(|s| s.font_change_broadcasts += 1);
    }
    fn is_protected(&self, path: &Path) -> bool {
        if self.read(|s| s.protected.iter().any(|p| same_path(p, path))) {
            return true;
        }
        is_directly_in(&self.dirs.all_users, path) && crate::protect::is_critical_font(path)
    }
    fn is_in_use(&self, path: &Path) -> bool {
        if self.read(|s| s.in_use_briefly.iter().any(|(p, n)| *n > 0 && same_path(p, path))) {
            let _ = self.update(|s| {
                for (_, n) in s.in_use_briefly.iter_mut().filter(|(p, _)| same_path(p, path)) {
                    *n = n.saturating_sub(1);
                }
                s.in_use_briefly.retain(|(_, n)| *n > 0);
            });
            return true;
        }
        self.read(|s| s.in_use.iter().any(|p| same_path(p, path))) || crate::protect::is_open_elsewhere(path)
    }
    fn in_use_retry_ms(&self) -> &'static [u64] {
        // the same retries, without the wait
        &[1, 1, 1, 1, 1, 1]
    }
    fn delete_on_reboot(&self, path: &Path) -> io::Result<()> {
        // Like the real backend: MOVEFILE_DELAY_UNTIL_REBOOT needs administrator rights, which the
        // per-user path does not have (AnyFont then remembers the file in pending-deletes.json).
        if is_directly_in(&self.dirs.current_user, path) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "MOVEFILE_DELAY_UNTIL_REBOOT needs elevation"));
        }
        self.update(|s| {
            if !s.pending_reboot.iter().any(|p| same_path(p, path)) {
                s.pending_reboot.push(path.to_path_buf());
            }
        })
    }
    fn recycle(&self, paths: &[PathBuf]) -> io::Result<()> {
        let bin = self.root.join("RecycleBin");
        std::fs::create_dir_all(&bin)?;
        let mut moved = Vec::new();
        let mut first_err = None;
        for p in paths {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let stored = bin.join(format!("{}_{}", crate::util::unique_id(), name));
            match std::fs::rename(p, &stored).or_else(|_| std::fs::copy(p, &stored).and_then(|_| std::fs::remove_file(p))) {
                Ok(()) => moved.push((p.clone(), stored)),
                Err(e) => {
                    let _ = std::fs::remove_file(&stored);
                    first_err.get_or_insert(e);
                }
            }
        }
        self.update(|s| s.recycle_bin.extend(moved))?;
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// The sandbox [`FontSystem`].
pub struct SandboxFontSystem {
    engine: Engine<SandboxPlatform>,
    /// Behave like a process that is not elevated and whose UAC prompt the user cancels: every
    /// all-users install reports `ElevationCancelled` (see [`SandboxFontSystem::set_decline_elevation`]).
    decline_elevation: std::sync::atomic::AtomicBool,
}

impl SandboxFontSystem {
    /// Open (and on first use create and seed) the sandbox at `root`.
    pub fn open(root: PathBuf, config: SandboxConfig) -> io::Result<SandboxFontSystem> {
        std::fs::create_dir_all(&root)?;
        let root = std::fs::canonicalize(&root).map(|p| strip_verbatim(&p)).unwrap_or(root);
        let p = SandboxPlatform { dirs: FontDirs::sandbox(&root), paths: AppPaths::sandbox(&root), root, state: Mutex::new(()) };
        std::fs::create_dir_all(&p.dirs.all_users)?;
        std::fs::create_dir_all(&p.dirs.current_user)?;
        let fresh = !p.state_path().exists();
        let decline = std::env::var_os(crate::SANDBOX_DECLINE_UAC_ENV).is_some_and(|v| !v.is_empty() && v != "0");
        let sb = SandboxFontSystem { engine: Engine::new(p), decline_elevation: std::sync::atomic::AtomicBool::new(decline) };
        if fresh {
            sb.engine.p.save(&State::default())?;
            sb.seed(&config)?;
        }
        Ok(sb)
    }

    /// Simulates a cancelled UAC prompt: from now on, installs for all users fail with
    /// `FontOpError::ElevationCancelled` (nothing is copied), as the real backend reports when the user
    /// answers No to the elevated helper. Also set by `ANYFONT_SANDBOX_DECLINE_UAC=1`.
    pub fn set_decline_elevation(&self, decline: bool) {
        self.decline_elevation.store(decline, std::sync::atomic::Ordering::Relaxed);
    }

    fn declines_elevation(&self) -> bool {
        self.decline_elevation.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn seed(&self, config: &SandboxConfig) -> io::Result<()> {
        let windir_fonts = crate::paths::windows_dir().join("Fonts");
        let p = &self.engine.p;
        for item in &config.seed {
            let src = if item.is_absolute() { item.clone() } else { windir_fonts.join(item) };
            let Ok(info) = crate::fontfile::inspect(&src) else {
                log::debug!("sandbox seed: skipping {}", src.display());
                continue;
            };
            let files: Vec<PathBuf> = match &info.type1_files {
                Some((m, b)) => vec![m.clone(), b.clone()],
                None => vec![src.clone()],
            };
            let mut dests = Vec::new();
            for f in &files {
                let dest = p.dirs.all_users.join(f.file_name().unwrap_or_default());
                if !dest.exists() {
                    std::fs::copy(f, &dest)?;
                }
                // Seed copies come from a read-only system folder; keep them writable in the sandbox.
                if let Ok(meta) = std::fs::metadata(&dest) {
                    let mut perm = meta.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    perm.set_readonly(false);
                    let _ = std::fs::set_permissions(&dest, perm);
                }
                dests.push(dest);
            }
            let names: Vec<String> = dests.iter().map(|d| d.file_name().unwrap_or_default().to_string_lossy().into_owned()).collect();
            let (kind, data) = match info.format {
                crate::FontFormat::Type1 => (RegKind::Type1, RegData::MultiSz(crate::regname::type1_data(&names[0], &names[1]))),
                _ => (RegKind::Fonts, RegData::Sz(names[0].clone())),
            };
            p.reg_set(Scope::AllUsers, kind, &info.registry_value_name(), &data)?;
            p.update(|s| s.protected.extend(dests.iter().cloned()))?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.engine.p.root
    }

    /// Declare a file as held open by another application (or release it).
    pub fn set_in_use(&self, path: &Path, in_use: bool) {
        let _ = self.engine.p.update(|s| {
            s.in_use.retain(|p| !same_path(p, path));
            if in_use {
                s.in_use.push(path.to_path_buf());
            }
        });
    }

    /// Declare a file as held for the next `checks` in-use checks only, as Windows (its font cache) holds a
    /// font it has just loaded for about a second.
    pub fn set_in_use_briefly(&self, path: &Path, checks: u32) {
        let _ = self.engine.p.update(|s| {
            s.in_use_briefly.retain(|(p, _)| !same_path(p, path));
            s.in_use_briefly.push((path.to_path_buf(), checks));
        });
    }

    /// Mark a file as a protected system font (tests).
    pub fn set_protected(&self, path: &Path) {
        let _ = self.engine.p.update(|s| s.protected.push(path.to_path_buf()));
    }

    /// Files scheduled for deletion at the next restart.
    pub fn pending_reboot(&self) -> Vec<PathBuf> {
        self.engine.p.read(|s| s.pending_reboot.clone())
    }

    /// Pretend Windows restarted: deletes the files scheduled with `MOVEFILE_DELAY_UNTIL_REBOOT`
    /// and releases every in-use marker.
    pub fn simulate_reboot(&self) {
        let pending = self.pending_reboot();
        for f in &pending {
            let _ = std::fs::remove_file(f);
        }
        let _ = self.engine.p.update(|s| {
            s.pending_reboot.clear();
            s.in_use.clear();
            s.in_use_briefly.clear();
        });
    }

    /// How many times `reboot_now` was called.
    pub fn reboot_count(&self) -> u32 {
        self.engine.p.read(|s| s.reboots)
    }

    /// How many `WM_FONTCHANGE` broadcasts the real backend would have sent.
    pub fn font_change_count(&self) -> u32 {
        self.engine.p.read(|s| s.font_change_broadcasts)
    }

    /// (original path, stored path) of every file sent to the sandbox Recycle Bin.
    pub fn recycle_bin(&self) -> Vec<(PathBuf, PathBuf)> {
        self.engine.p.read(|s| s.recycle_bin.clone())
    }

    /// Paths passed to `open_location`.
    pub fn opened_locations(&self) -> Vec<PathBuf> {
        self.engine.p.read(|s| s.opened_locations.clone())
    }
}

fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

impl FontSystem for SandboxFontSystem {
    fn backend(&self) -> Backend {
        Backend::Sandbox { root: self.root().to_path_buf() }
    }
    fn font_dirs(&self) -> FontDirs {
        self.engine.p.dirs.clone()
    }
    fn paths(&self) -> &AppPaths {
        &self.engine.p.paths
    }
    fn installed_fonts(&self) -> io::Result<Vec<InstalledFont>> {
        self.engine.list()
    }
    fn is_protected(&self, path: &Path) -> bool {
        self.engine.p.is_protected(path)
    }
    fn needs_elevation(&self, scope: Scope) -> bool {
        scope == Scope::AllUsers && self.declines_elevation()
    }
    fn install_with(&self, files: &[PathBuf], scope: Scope, used: UsedFontAction, progress: Progress<'_>) -> InstallReport {
        if scope == Scope::AllUsers && self.declines_elevation() {
            log::info!("sandbox: UAC prompt declined ({} file(s))", files.len());
            return InstallReport {
                results: files.iter().map(|p| crate::FileResult { path: p.clone(), result: Err(crate::FontOpError::ElevationCancelled) }).collect(),
                ..Default::default()
            };
        }
        self.engine.install(files, scope, used, progress)
    }
    fn uninstall_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> UninstallReport {
        self.engine.uninstall(paths, used, progress)
    }
    fn delete_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> DeleteReport {
        self.engine.delete(paths, used, progress)
    }
    fn open_location(&self, path: &Path) -> io::Result<()> {
        self.engine.p.update(|s| s.opened_locations.push(path.to_path_buf()))?;
        if std::env::var_os("ANYFONT_SANDBOX_OPEN_EXPLORER").is_some() {
            #[cfg(windows)]
            return crate::win::shell::open_location(path);
        }
        Ok(())
    }
    fn reboot_now(&self) -> io::Result<()> {
        log::info!("sandbox: reboot requested (not performed)");
        self.engine.p.update(|s| s.reboots += 1)
    }
    fn process_pending(&self) -> usize {
        self.engine.process_pending()
    }
}
