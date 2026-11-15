//! The real Windows backend.

pub(crate) mod acl;
pub(crate) mod registry;
pub(crate) mod shell;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{AddFontResourceExW, RemoveFontResourceExW, FONT_RESOURCE_CHARACTERISTICS};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, HWND_BROADCAST, WM_FONTCHANGE};

use crate::engine::{Engine, Platform, RegData};
use crate::helper::{HelperRequest, HelperResponse};
use crate::paths::{AppPaths, FontDirs};
use crate::util::{is_directly_in, path_key};
use crate::{
    Backend, DeleteReport, FileResult, FontOpError, FontSystem, InstallReport, InstalledFont, Progress, RegKind, Report, Scope,
    UninstallReport, UsedFontAction,
};

/// Tests must never touch the machine's font registration: every real write panics in this
/// crate's unit tests and in any build with the `forbid-real-writes` feature (enabled for this
/// crate's integration tests through its dev-dependency on itself; other crates enable it in their
/// `[dev-dependencies]`).
macro_rules! forbid_in_tests {
    ($what:expr) => {
        if cfg!(any(test, feature = "forbid-real-writes")) {
            panic!("anyfont-sys test tried to {} on the real system - use the sandbox backend", $what);
        }
    };
}
pub(crate) use forbid_in_tests;

pub(crate) fn wide(p: &Path) -> HSTRING {
    HSTRING::from(p.as_os_str())
}

pub(crate) struct WinPlatform {
    dirs: FontDirs,
    paths: AppPaths,
    protected_cache: Mutex<HashMap<String, bool>>,
}

impl Platform for WinPlatform {
    fn dirs(&self) -> &FontDirs {
        &self.dirs
    }
    fn app_paths(&self) -> &AppPaths {
        &self.paths
    }
    fn reg_list(&self, scope: Scope, kind: RegKind) -> io::Result<Vec<(String, RegData)>> {
        registry::list(scope, kind)
    }
    fn reg_set(&self, scope: Scope, kind: RegKind, name: &str, data: &RegData) -> io::Result<()> {
        forbid_in_tests!("write the font registry");
        registry::set(scope, kind, name, data)
    }
    fn reg_delete(&self, scope: Scope, kind: RegKind, name: &str) -> io::Result<bool> {
        forbid_in_tests!("delete a font registry value");
        registry::delete(scope, kind, name)
    }
    fn add_font_resource(&self, spec: &str) -> bool {
        forbid_in_tests!("call AddFontResourceExW");
        // Not FR_PRIVATE: the font becomes visible to every application of the session.
        unsafe { AddFontResourceExW(&HSTRING::from(spec), FONT_RESOURCE_CHARACTERISTICS(0), None) > 0 }
    }
    fn remove_font_resource(&self, spec: &str) -> bool {
        forbid_in_tests!("call RemoveFontResourceExW");
        // The original loops until the call fails, dropping every reference (CdrFnt 0x180202390).
        let name = HSTRING::from(spec);
        let mut n = 0;
        while n < 64 && unsafe { RemoveFontResourceExW(&name, 0, None) }.as_bool() {
            n += 1;
        }
        n > 0
    }
    fn font_change(&self) {
        forbid_in_tests!("broadcast WM_FONTCHANGE");
        // CdrFnt 0x1802010D0: PostMessageW(HWND_BROADCAST, WM_FONTCHANGE, 0, 0) - non-blocking.
        unsafe {
            let _ = PostMessageW(Some(HWND_BROADCAST), WM_FONTCHANGE, WPARAM(0), LPARAM(0));
        }
    }
    fn is_protected(&self, path: &Path) -> bool {
        let key = path_key(path);
        if let Some(v) = self.protected_cache.lock().unwrap().get(&key) {
            return *v;
        }
        let v = acl::is_protected(path, &self.dirs.all_users);
        self.protected_cache.lock().unwrap().insert(key, v);
        v
    }
    fn is_in_use(&self, path: &Path) -> bool {
        crate::protect::is_open_elsewhere(path)
    }
    fn delete_on_reboot(&self, path: &Path) -> io::Result<()> {
        forbid_in_tests!("schedule a delete on reboot");
        unsafe { MoveFileExW(&wide(path), PCWSTR::null(), MOVEFILE_DELAY_UNTIL_REBOOT) }.map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))
    }
    fn recycle(&self, paths: &[PathBuf]) -> io::Result<()> {
        forbid_in_tests!("move files to the Recycle Bin");
        shell::recycle(paths)
    }
    fn guard_file_writes(&self, what: &str) {
        forbid_in_tests!(what);
    }
}

/// The real [`FontSystem`].
pub(crate) struct RealFontSystem {
    engine: Engine<WinPlatform>,
    /// False inside the elevated helper (never re-launch).
    allow_elevation: bool,
}

impl RealFontSystem {
    pub fn new(allow_elevation: bool) -> Self {
        let p = WinPlatform { dirs: FontDirs::real(), paths: AppPaths::real(), protected_cache: Mutex::new(HashMap::new()) };
        RealFontSystem { engine: Engine::new(p), allow_elevation }
    }

    fn must_elevate(&self) -> bool {
        self.allow_elevation && !is_elevated()
    }

    /// Paths whose operation needs administrator rights: registered for all users, or the file
    /// lives in `%WINDIR%\Fonts`.
    fn needs_admin(&self, entries: &[crate::engine::Entry], path: &Path) -> bool {
        is_directly_in(&self.engine.p.dirs().all_users, path)
            || entries.iter().any(|e| e.scope == Scope::AllUsers && e.files.iter().any(|f| crate::util::same_path(f, path)))
    }

    fn helper(&self, req: HelperRequest) -> Result<HelperResponse, FontOpError> {
        crate::helper::run_elevated(&req, &self.engine.p.app_paths().helper_dir())
    }

    /// Split `paths` into local and elevated work, run both, merge in input order.
    fn split_run<T: Clone>(
        &self,
        paths: &[PathBuf],
        local: impl FnOnce(&[PathBuf]) -> Report<T>,
        remote: impl FnOnce(Vec<PathBuf>) -> Result<Report<T>, FontOpError>,
    ) -> Report<T> {
        let entries = self.engine.entries().unwrap_or_default();
        type Indexed<'a> = Vec<(usize, &'a PathBuf)>;
        let (admin, user): (Indexed<'_>, Indexed<'_>) = paths.iter().enumerate().partition(|(_, p)| self.needs_admin(&entries, p));
        let mut slots: Vec<Option<FileResult<T>>> = vec![None; paths.len()];
        let mut out = Report::default();
        if !user.is_empty() {
            let list: Vec<PathBuf> = user.iter().map(|(_, p)| (*p).clone()).collect();
            let r = local(&list);
            for ((i, _), res) in user.iter().zip(r.results) {
                slots[*i] = Some(res);
            }
            out.in_use.extend(r.in_use);
            out.reboot_required |= r.reboot_required;
        }
        if !admin.is_empty() {
            let list: Vec<PathBuf> = admin.iter().map(|(_, p)| (*p).clone()).collect();
            match remote(list.clone()) {
                Ok(r) => {
                    for ((i, _), res) in admin.iter().zip(r.results) {
                        slots[*i] = Some(res);
                    }
                    out.in_use.extend(r.in_use);
                    out.reboot_required |= r.reboot_required;
                }
                Err(e) => {
                    for (i, p) in &admin {
                        slots[*i] = Some(FileResult { path: (*p).clone(), result: Err(e.clone()) });
                    }
                }
            }
        }
        out.results = slots
            .into_iter()
            .zip(paths)
            .map(|(s, p)| s.unwrap_or_else(|| FileResult { path: p.clone(), result: Err(FontOpError::Io("no result".into())) }))
            .collect();
        out
    }
}

fn unexpected<T>(paths: &[PathBuf], what: &str) -> Report<T> {
    Report {
        results: paths.iter().map(|p| FileResult { path: p.clone(), result: Err(FontOpError::Io(format!("unexpected helper response ({what})"))) }).collect(),
        in_use: Vec::new(),
        reboot_required: false,
    }
}

impl FontSystem for RealFontSystem {
    fn backend(&self) -> Backend {
        Backend::Real
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
        scope == Scope::AllUsers && !is_elevated()
    }
    fn install_with(&self, files: &[PathBuf], scope: Scope, used: UsedFontAction, progress: Progress<'_>) -> InstallReport {
        if scope == Scope::AllUsers && self.must_elevate() {
            return match self.helper(HelperRequest::Install { files: files.to_vec(), scope, used }) {
                Ok(HelperResponse::Install(r)) => r,
                Ok(HelperResponse::Error(e)) => Report { results: files.iter().map(|p| FileResult { path: p.clone(), result: Err(FontOpError::Io(e.clone())) }).collect(), ..Default::default() },
                Ok(_) => unexpected(files, "install"),
                Err(e) => Report { results: files.iter().map(|p| FileResult { path: p.clone(), result: Err(e.clone()) }).collect(), ..Default::default() },
            };
        }
        self.engine.install(files, scope, used, progress)
    }
    fn uninstall_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> UninstallReport {
        if !self.must_elevate() {
            return self.engine.uninstall(paths, used, progress);
        }
        self.split_run(
            paths,
            |p| self.engine.uninstall(p, used, progress),
            |p| match self.helper(HelperRequest::Uninstall { paths: p.clone(), used })? {
                HelperResponse::Uninstall(r) => Ok(r),
                HelperResponse::Error(e) => Err(FontOpError::Io(e)),
                _ => Ok(unexpected(&p, "uninstall")),
            },
        )
    }
    fn delete_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> DeleteReport {
        if !self.must_elevate() {
            return self.engine.delete(paths, used, progress);
        }
        self.split_run(
            paths,
            |p| self.engine.delete(p, used, progress),
            |p| match self.helper(HelperRequest::Delete { paths: p.clone(), used })? {
                HelperResponse::Delete(r) => Ok(r),
                HelperResponse::Error(e) => Err(FontOpError::Io(e)),
                _ => Ok(unexpected(&p, "delete")),
            },
        )
    }
    fn open_location(&self, path: &Path) -> io::Result<()> {
        shell::open_location(path)
    }
    fn reboot_now(&self) -> io::Result<()> {
        shell::reboot_now()
    }
    fn process_pending(&self) -> usize {
        crate::helper::clean_stale(&self.engine.p.app_paths().helper_dir());
        self.engine.process_pending()
    }
}

/// `TokenElevation` of the process token.
pub(crate) fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elev = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elev.TokenIsElevated != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reading the real registration is allowed (no writes).
    #[test]
    fn real_listing_is_readable() {
        let sys = RealFontSystem::new(false);
        let fonts = sys.installed_fonts().unwrap();
        assert!(fonts.iter().any(|f| f.scope == Scope::AllUsers), "HKLM fonts expected");
        let arial = fonts.iter().find(|f| f.value_name.eq_ignore_ascii_case("Arial (TrueType)"));
        if let Some(a) = arial {
            assert!(a.exists);
            assert!(a.protected, "a shipped font must be protected: {}", a.path.display());
        }
    }

    #[test]
    #[should_panic(expected = "use the sandbox backend")]
    fn real_writes_are_refused_in_tests() {
        let sys = RealFontSystem::new(false);
        let _ = sys.engine.p.reg_set(Scope::CurrentUser, RegKind::Fonts, "x", &RegData::Sz("x".into()));
    }

    /// The engine's own file copies / removals are guarded too. Only non-existent paths are used, so
    /// nothing could change even if a guard were missing.
    #[test]
    fn real_engine_operations_are_refused_in_tests() {
        let sys = RealFontSystem::new(false);
        let ghost = vec![std::env::temp_dir().join("anyfont-guard-does-not-exist.ttf")];
        let calls: Vec<Box<dyn Fn() + '_>> = vec![
            Box::new(|| drop(sys.install(&ghost, Scope::CurrentUser, UsedFontAction::Ask))),
            Box::new(|| drop(sys.uninstall(&ghost, UsedFontAction::Ask))),
            Box::new(|| drop(sys.delete(&ghost, UsedFontAction::Ask))),
            Box::new(|| sys.engine.defer_delete(&ghost[0])),
        ];
        for call in calls {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).is_err());
        }
    }
}
