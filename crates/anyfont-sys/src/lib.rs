//! AnyFont's Windows layer.
//!
//! Everything that touches the operating system lives here, behind the [`FontSystem`] trait:
//! enumeration of the registered (installed) fonts, the protected-system-font rule, install /
//! uninstall / delete, `WM_FONTCHANGE`, the reboot prompt support, "open file location",
//! elevation for all-users installs, plus the process-level helpers the application's `main`
//! needs (command line, logging, crash reports, single instance).
//!
//! [`system()`] returns the real backend, or the **sandbox** backend when the environment variable
//! `ANYFONT_SANDBOX_ROOT` is set. The sandbox performs the same operations against a private
//! folder tree and a JSON "registry", so tests never change the fonts of the machine.
//!
//! The full contract for the UI team is in `docs/sys-api.md`; the dossier findings behind the
//! behaviour are in `docs/phases/10-behavior.md`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod cmdline;
pub mod crash;
pub mod fontfile;
pub mod helper;
pub mod logging;
pub mod paths;
pub mod protect;
pub mod regname;
pub mod single_instance;

mod engine;
mod sandbox;
mod util;
#[cfg(windows)]
mod win;

pub use cmdline::CommandLine;
pub use fontfile::{FontFileInfo, FontFormat};
pub use paths::{AppPaths, FontDirs};
pub use sandbox::{SandboxConfig, SandboxFontSystem, DEFAULT_SANDBOX_SEED};

/// Environment variable that switches [`system()`] to the sandbox backend.
pub const SANDBOX_ENV: &str = "ANYFONT_SANDBOX_ROOT";
/// Optional `;`-separated list of fonts copied into a new sandbox (file names in `%WINDIR%\Fonts`
/// or absolute paths). `none` seeds nothing. Default: [`DEFAULT_SANDBOX_SEED`].
pub const SANDBOX_SEED_ENV: &str = "ANYFONT_SANDBOX_SEED";
/// When set to `1`, the sandbox behaves as if the user cancelled the UAC prompt: installs for all users
/// report [`FontOpError::ElevationCancelled`] (to try the "Install only for me" fallback).
pub const SANDBOX_DECLINE_UAC_ENV: &str = "ANYFONT_SANDBOX_DECLINE_UAC";

/// Where a font is registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts` + `%LOCALAPPDATA%\Microsoft\Windows\Fonts`.
    /// No administrator rights needed.
    CurrentUser,
    /// `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts` + `%WINDIR%\Fonts`. Needs elevation
    /// (the real backend re-launches `anyfont.exe` elevated, see [`helper`]).
    AllUsers,
    /// Loaded for the current Windows session only (`AddFontResourceExW`, no copy, no registry):
    /// the original's "TemporaryMode" preference. Gone after sign-out.
    Session,
}

/// Registry key a font is registered under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegKind {
    /// `...\CurrentVersion\Fonts` (TrueType, OpenType, collections, bitmap fonts).
    Fonts,
    /// `...\CurrentVersion\Type 1 Installer\Type 1 Fonts` (`.pfm` + `.pfb` pairs, REG_MULTI_SZ `T`, pfm, pfb).
    Type1,
    /// Not in the registry ([`Scope::Session`]).
    None,
}

/// One registered font (one registry value).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFont {
    /// Registry value name, e.g. `Arial Bold (TrueType)`.
    pub value_name: String,
    /// Resolved font file (for Type 1: the `.pfm`).
    pub path: PathBuf,
    /// Further files of the same font (Type 1: the `.pfb`).
    pub extra_paths: Vec<PathBuf>,
    pub scope: Scope,
    pub kind: RegKind,
    /// The resolved file exists on disk.
    pub exists: bool,
    /// Protected system font (cannot be uninstalled or deleted), see [`FontSystem::is_protected`].
    pub protected: bool,
}

/// What to do with fonts whose file is held open by another application
/// (the original's `EUsedFontAction`, set by the FontInteraction dialog:
/// Reboot now = 0, Reboot later = 1, Ignore = 2, Cancel = 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UsedFontAction {
    /// First pass: leave in-use fonts untouched and list them in [`Report::in_use`] so the UI can show
    /// the "Reboot now / Reboot later / Ignore" prompt and call again for those paths.
    #[default]
    Ask,
    /// Unregister now, remove the files at the next restart; the caller then calls
    /// [`FontSystem::reboot_now`].
    RebootNow,
    /// Unregister now, remove the files at the next restart.
    RebootLater,
    /// Skip the in-use fonts (they stay installed / on disk).
    Ignore,
    /// If any font is in use, change nothing at all.
    Cancel,
}

impl UsedFontAction {
    pub(crate) fn defers_to_reboot(self) -> bool {
        matches!(self, UsedFontAction::RebootNow | UsedFontAction::RebootLater)
    }
}

/// Why an operation on one file failed or was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontOpError {
    /// Protected system font: never uninstalled or deleted.
    Protected,
    /// The file does not exist.
    NotFound,
    /// Not a font file AnyFont can install (`.ttf .otf .ttc .otc .pfm/.pfb`).
    UnsupportedFormat(String),
    /// The file could not be parsed or Windows rejected it.
    InvalidFont(String),
    /// Read-only files are never deleted ("Read-only fonts cannot be deleted.").
    ReadOnly,
    /// The file is held open by another application (see [`UsedFontAction`]).
    InUse,
    /// `uninstall` was asked for a file that is not registered.
    NotInstalled,
    /// Type 1 fonts can only be registered for all users (Windows reads the
    /// `Type 1 Installer` key from HKLM only).
    Type1NeedsAllUsers,
    /// The `.pfm` / `.pfb` partner of a Type 1 font is missing.
    MissingCompanion(PathBuf),
    /// The user declined the UAC prompt ("Insufficient Permissions").
    ElevationCancelled,
    /// Access denied by the file system or the registry.
    AccessDenied,
    /// The operation was cancelled (`UsedFontAction::Cancel` or the progress callback).
    Cancelled,
    /// Any other I/O or Win32 error (message text).
    Io(String),
}

impl std::fmt::Display for FontOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontOpError::Protected => write!(f, "protected system font"),
            FontOpError::NotFound => write!(f, "file not found"),
            FontOpError::UnsupportedFormat(e) => write!(f, "unsupported font format ({e})"),
            FontOpError::InvalidFont(e) => write!(f, "invalid font: {e}"),
            FontOpError::ReadOnly => write!(f, "read-only fonts cannot be deleted"),
            FontOpError::InUse => write!(f, "the font is in use by one or more applications"),
            FontOpError::NotInstalled => write!(f, "the font is not installed"),
            FontOpError::Type1NeedsAllUsers => write!(f, "Type 1 fonts can only be installed for all users"),
            FontOpError::MissingCompanion(p) => write!(f, "missing Type 1 companion file {}", p.display()),
            FontOpError::ElevationCancelled => write!(f, "insufficient permissions (elevation was cancelled)"),
            FontOpError::AccessDenied => write!(f, "access denied"),
            FontOpError::Cancelled => write!(f, "cancelled"),
            FontOpError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FontOpError {}

impl From<std::io::Error> for FontOpError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => FontOpError::NotFound,
            std::io::ErrorKind::PermissionDenied => FontOpError::AccessDenied,
            _ if e.raw_os_error() == Some(32) || e.raw_os_error() == Some(33) => FontOpError::InUse,
            _ => FontOpError::Io(e.to_string()),
        }
    }
}

/// What happened to the font file when a font was uninstalled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileRemoval {
    /// The copy in a fonts folder was deleted.
    Removed,
    /// The file is outside the fonts folders (registered in place) and was left alone.
    Kept,
    /// The file is in use; it is removed at the next restart ([`Report::reboot_required`]).
    PendingReboot,
}

/// Successful install of one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallOutcome {
    Installed {
        value_name: String,
        /// Where the registered copy lives (inside the fonts folder of the scope).
        installed_path: PathBuf,
        /// Previously registered file of the same name that this install replaced.
        replaced: Option<PathBuf>,
    },
    /// The same font (same registry name and identical file) is already registered in that scope.
    AlreadyInstalled { value_name: String, installed_path: PathBuf },
}

/// Successful uninstall of one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallOutcome {
    /// Registry values removed (one file can be registered in both scopes).
    pub value_names: Vec<String>,
    pub file: FileRemoval,
}

/// Successful delete of one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteOutcome {
    /// Moved to the Recycle Bin (after being uninstalled when it was installed).
    Recycled { uninstalled: Option<UninstallOutcome> },
    /// In use: unregistered now, deleted at the next restart (not recoverable from the Recycle Bin).
    PendingReboot { uninstalled: Option<UninstallOutcome> },
}

/// Result for one input path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileResult<T> {
    pub path: PathBuf,
    pub result: Result<T, FontOpError>,
}

/// Result of a batch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report<T> {
    /// One entry per input path, in input order.
    pub results: Vec<FileResult<T>>,
    /// Files that are in use and were left alone ([`UsedFontAction::Ask`] / `Ignore`): show the
    /// FontInteraction prompt for them.
    pub in_use: Vec<PathBuf>,
    /// Some file removal was deferred to the next restart.
    pub reboot_required: bool,
}

impl<T> Default for Report<T> {
    fn default() -> Self {
        Report { results: Vec::new(), in_use: Vec::new(), reboot_required: false }
    }
}

impl<T> Report<T> {
    pub fn succeeded(&self) -> usize {
        self.results.iter().filter(|r| r.result.is_ok()).count()
    }
    pub fn failed(&self) -> impl Iterator<Item = &FileResult<T>> {
        self.results.iter().filter(|r| r.result.is_err())
    }
    /// True when at least one result is [`FontOpError::ElevationCancelled`] (show "Insufficient Permissions").
    pub fn elevation_cancelled(&self) -> bool {
        self.results.iter().any(|r| r.result.as_ref().err() == Some(&FontOpError::ElevationCancelled))
    }
}

pub type InstallReport = Report<InstallOutcome>;
pub type UninstallReport = Report<UninstallOutcome>;
pub type DeleteReport = Report<DeleteOutcome>;

/// Progress callback: `(done, total, current file)`; return `false` to cancel the remaining files.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize, &Path) -> bool;

/// Which implementation is behind a [`FontSystem`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    Real,
    Sandbox { root: PathBuf },
}

/// AnyFont's view of the Windows font system. All mutating calls block (they may wait for an
/// elevated helper process) - call them from a worker thread, not the UI thread.
pub trait FontSystem: Send + Sync {
    fn backend(&self) -> Backend;

    /// Fonts folders of both scopes.
    fn font_dirs(&self) -> FontDirs;

    /// AnyFont's own folders (settings, database/logs, crash reports, content folder).
    fn paths(&self) -> &AppPaths;

    /// `%USERPROFILE%\Documents\AnyFont\Content\Fonts` (created on demand): where downloaded
    /// fonts are stored as not-installed fonts.
    fn content_folder(&self) -> PathBuf {
        let dir = self.paths().content_fonts_dir.clone();
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Every registered font (HKLM + HKCU, `Fonts` and `Type 1 Installer` keys) plus the fonts this
    /// process loaded for the session.
    fn installed_fonts(&self) -> std::io::Result<Vec<InstalledFont>>;

    /// Protected system font: a font Windows ships and that must never be uninstalled or deleted.
    /// Rule (dossier `CdrFnt` 0x1802028E0 + Windows): the file's DACL does not give
    /// `BUILTIN\Administrators` full control (TrustedInstaller-owned), or Windows Resource
    /// Protection covers it (`SfcIsFileProtected`), or it is one of the shell's critical fonts in
    /// `%WINDIR%\Fonts`. See `docs/phases/10-behavior.md`.
    fn is_protected(&self, path: &Path) -> bool;

    /// All protected font files currently registered.
    fn protected_fonts(&self) -> Vec<PathBuf> {
        self.installed_fonts()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| f.protected)
            .map(|f| f.path)
            .collect()
    }

    /// Registrations of this file (any scope); empty when not installed.
    fn registrations(&self, path: &Path) -> Vec<InstalledFont> {
        let key = util::path_key(path);
        self.installed_fonts()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| util::path_key(&f.path) == key || f.extra_paths.iter().any(|p| util::path_key(p) == key))
            .collect()
    }

    /// True when installing for `scope` will show a UAC prompt.
    fn needs_elevation(&self, scope: Scope) -> bool;

    /// Install font files. TrueType/OpenType/collections go to the `Fonts` key; a `.pfm`/`.pfb`
    /// pair (either file may be given) goes to the `Type 1 Installer` key (all users only).
    fn install_with(&self, files: &[PathBuf], scope: Scope, used: UsedFontAction, progress: Progress<'_>) -> InstallReport;

    fn install(&self, files: &[PathBuf], scope: Scope, used: UsedFontAction) -> InstallReport {
        self.install_with(files, scope, used, &mut |_, _, _| true)
    }

    /// Uninstall the fonts registered for these files (every scope they are registered in).
    fn uninstall_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> UninstallReport;

    fn uninstall(&self, paths: &[PathBuf], used: UsedFontAction) -> UninstallReport {
        self.uninstall_with(paths, used, &mut |_, _, _| true)
    }

    /// Delete font files: protected and read-only files are refused; installed fonts are
    /// uninstalled first; the files go to the Recycle Bin.
    fn delete_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> DeleteReport;

    fn delete(&self, paths: &[PathBuf], used: UsedFontAction) -> DeleteReport {
        self.delete_with(paths, used, &mut |_, _, _| true)
    }

    /// Open Explorer on the folder of `path` with the file selected.
    fn open_location(&self, path: &Path) -> std::io::Result<()>;

    /// Restart Windows now (only on explicit user request: "Reboot now").
    fn reboot_now(&self) -> std::io::Result<()>;

    /// Finish deletions that were deferred because the file was in use (call once at start-up).
    /// Returns the number of files removed.
    fn process_pending(&self) -> usize;
}

/// The font system for this process: the sandbox when `ANYFONT_SANDBOX_ROOT` is set, the real
/// Windows backend otherwise.
pub fn system() -> Box<dyn FontSystem> {
    if let Some(root) = std::env::var_os(SANDBOX_ENV).filter(|v| !v.is_empty()) {
        let config = SandboxConfig::from_env();
        match SandboxFontSystem::open(PathBuf::from(root), config) {
            Ok(s) => return Box::new(s),
            Err(e) => {
                // Never fall back to the real backend when a sandbox was requested.
                panic!("{SANDBOX_ENV} is set but the sandbox cannot be opened: {e}");
            }
        }
    }
    real_system()
}

#[cfg(windows)]
fn real_system() -> Box<dyn FontSystem> {
    Box::new(win::RealFontSystem::new(true))
}

/// The backend used inside the elevated helper: never re-launches itself.
pub(crate) fn system_for_helper() -> Box<dyn FontSystem> {
    if std::env::var_os(SANDBOX_ENV).filter(|v| !v.is_empty()).is_some() {
        return system();
    }
    #[cfg(windows)]
    {
        Box::new(win::RealFontSystem::new(false))
    }
    #[cfg(not(windows))]
    {
        real_system()
    }
}

#[cfg(not(windows))]
fn real_system() -> Box<dyn FontSystem> {
    let root = std::env::temp_dir().join("anyfont-sandbox");
    Box::new(SandboxFontSystem::open(root, SandboxConfig::default()).expect("sandbox"))
}

/// True when the process runs with an elevated (administrator) token.
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        win::is_elevated()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Sets the window that owns UAC prompts and Explorer windows (the main window's HWND as `isize`).
pub fn set_owner_window(hwnd: isize) {
    util::OWNER_HWND.store(hwnd, std::sync::atomic::Ordering::Relaxed);
}
