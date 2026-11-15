//! Folders: Windows font folders and AnyFont's own data folders.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The fonts folder of each registration scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontDirs {
    /// `%WINDIR%\Fonts` (HKLM registrations).
    pub all_users: PathBuf,
    /// `%LOCALAPPDATA%\Microsoft\Windows\Fonts` (HKCU registrations).
    pub current_user: PathBuf,
}

impl FontDirs {
    pub fn for_scope(&self, scope: crate::Scope) -> Option<&Path> {
        match scope {
            crate::Scope::AllUsers => Some(&self.all_users),
            crate::Scope::CurrentUser => Some(&self.current_user),
            crate::Scope::Session => None,
        }
    }

    /// The real folders of this machine / user.
    pub fn real() -> FontDirs {
        FontDirs { all_users: windows_dir().join("Fonts"), current_user: local_app_data().join(r"Microsoft\Windows\Fonts") }
    }

    /// Sandbox folders under `root`.
    pub fn sandbox(root: &Path) -> FontDirs {
        FontDirs { all_users: root.join(r"Windows\Fonts"), current_user: root.join(r"LocalAppData\Microsoft\Windows\Fonts") }
    }
}

/// AnyFont's own folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPaths {
    /// `%APPDATA%\AnyFont`: settings (roaming).
    pub settings_dir: PathBuf,
    /// `%LOCALAPPDATA%\AnyFont`: font database, caches.
    pub data_dir: PathBuf,
    /// `%LOCALAPPDATA%\AnyFont\logs`.
    pub logs_dir: PathBuf,
    /// `%LOCALAPPDATA%\AnyFont\crash-reports`.
    pub crash_dir: PathBuf,
    /// `%USERPROFILE%\Documents\AnyFont\Content\Fonts`: downloaded (not installed) fonts.
    pub content_fonts_dir: PathBuf,
}

impl AppPaths {
    pub fn real() -> AppPaths {
        let docs = dirs::document_dir().unwrap_or_else(|| user_profile().join("Documents"));
        Self::from_roots(&roaming_app_data(), &local_app_data(), &docs)
    }

    pub fn sandbox(root: &Path) -> AppPaths {
        Self::from_roots(&root.join(r"AppData\Roaming"), &root.join("LocalAppData"), &root.join("Documents"))
    }

    fn from_roots(roaming: &Path, local: &Path, documents: &Path) -> AppPaths {
        let data_dir = local.join("AnyFont");
        AppPaths {
            settings_dir: roaming.join("AnyFont"),
            logs_dir: data_dir.join("logs"),
            crash_dir: data_dir.join("crash-reports"),
            data_dir,
            content_fonts_dir: documents.join(r"AnyFont\Content\Fonts"),
        }
    }

    /// Deletions deferred to the next start (per-user fonts that were in use).
    pub(crate) fn pending_file(&self) -> PathBuf {
        self.data_dir.join("pending-deletes.json")
    }

    /// Request/response files of the elevated helper.
    pub(crate) fn helper_dir(&self) -> PathBuf {
        self.data_dir.join("helper")
    }

    /// Create every folder (ignores errors).
    pub fn ensure(&self) {
        for d in [&self.settings_dir, &self.data_dir, &self.logs_dir, &self.crash_dir] {
            let _ = std::fs::create_dir_all(d);
        }
    }
}

/// AnyFont's folders for this process: the sandbox's when `ANYFONT_SANDBOX_ROOT` is set.
/// Usable before a [`crate::FontSystem`] exists (logging, crash hook).
pub fn app_paths() -> AppPaths {
    match std::env::var_os(crate::SANDBOX_ENV).filter(|v| !v.is_empty()) {
        Some(root) => AppPaths::sandbox(Path::new(&root)),
        None => AppPaths::real(),
    }
}

pub(crate) fn windows_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

pub(crate) fn user_profile() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from(r"C:\Users\Default"))
}

pub(crate) fn local_app_data() -> PathBuf {
    dirs::data_local_dir()
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .unwrap_or_else(|| user_profile().join(r"AppData\Local"))
}

pub(crate) fn roaming_app_data() -> PathBuf {
    dirs::data_dir()
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| user_profile().join(r"AppData\Roaming"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_layout() {
        let root = Path::new(r"C:\sb");
        let d = FontDirs::sandbox(root);
        assert_eq!(d.all_users, Path::new(r"C:\sb\Windows\Fonts"));
        assert_eq!(d.current_user, Path::new(r"C:\sb\LocalAppData\Microsoft\Windows\Fonts"));
        let p = AppPaths::sandbox(root);
        assert_eq!(p.settings_dir, Path::new(r"C:\sb\AppData\Roaming\AnyFont"));
        assert_eq!(p.data_dir, Path::new(r"C:\sb\LocalAppData\AnyFont"));
        assert_eq!(p.logs_dir, Path::new(r"C:\sb\LocalAppData\AnyFont\logs"));
        assert_eq!(p.content_fonts_dir, Path::new(r"C:\sb\Documents\AnyFont\Content\Fonts"));
    }

    #[test]
    fn real_layout_names() {
        let p = AppPaths::real();
        assert!(p.settings_dir.ends_with("AnyFont"));
        assert!(p.content_fonts_dir.ends_with(r"AnyFont\Content\Fonts"));
        let d = FontDirs::real();
        assert!(d.all_users.ends_with("Fonts"));
        assert!(d.current_user.ends_with(r"Microsoft\Windows\Fonts"));
    }
}
