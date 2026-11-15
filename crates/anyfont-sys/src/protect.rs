//! Protected system fonts and in-use detection (shared by both backends).
//!
//! Dossier rule (`CdrFnt` 0x1802028E0): a font file is protected when its DACL does not grant
//! `BUILTIN\Administrators` full control (`FILE_ALL_ACCESS` = 0x1F01FF): Windows ships its fonts
//! owned by TrustedInstaller with Administrators limited to read & execute, so neither the
//! program nor an administrator can remove them. AnyFont combines that with Windows Resource
//! Protection (`SfcIsFileProtected`) and, as a last safety net, the shell's critical fonts below
//! (only when they live directly in the all-users fonts folder).

use std::path::Path;

/// Fonts the Windows shell itself needs (file names, lower case).
pub const CRITICAL_SYSTEM_FONTS: &[&str] = &[
    "marlett.ttf",
    "segoeui.ttf",
    "segoeuib.ttf",
    "segoeuii.ttf",
    "segoeuil.ttf",
    "segoeuisl.ttf",
    "segoeuiz.ttf",
    "seguisb.ttf",
    "seguisym.ttf",
    "seguiemj.ttf",
    "segmdl2.ttf",
    "segoeicons.ttf",
    "seguivar.ttf",
    "segoeuivf.ttf",
    "tahoma.ttf",
    "tahomabd.ttf",
    "micross.ttf",
    "arial.ttf",
];

/// File name is one of [`CRITICAL_SYSTEM_FONTS`].
pub fn is_critical_font(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .map(|n| CRITICAL_SYSTEM_FONTS.contains(&n.as_str()))
        .unwrap_or(false)
}

/// `SystemIO::File::IsOpenElsewhere` (`CrlUtils` 0x1800A2840): open the file for reading with no
/// sharing; `ERROR_SHARING_VIOLATION` means another process holds it. Folders are never "open".
pub fn is_open_elsewhere(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        if path.is_dir() {
            return false;
        }
        match std::fs::OpenOptions::new().read(true).share_mode(0).open(path) {
            Ok(_) => false,
            Err(e) => e.raw_os_error() == Some(32),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// Evaluate the dossier's ACL rule on a list of ACEs: `(ace_type, is_administrators, mask)`.
/// `ace_type` 0 = ACCESS_ALLOWED, 1 = ACCESS_DENIED; other types are ignored. An empty DACL
/// protects; a later matching ACE overrides an earlier one (as in the original loop).
pub fn acl_rule_protects(aces: &[(u8, bool, u32)]) -> bool {
    const FILE_ALL_ACCESS: u32 = 0x1F01FF;
    let mut protected = aces.is_empty();
    for &(ty, admins, mask) in aces {
        if !admins || ty > 1 {
            continue;
        }
        protected = if ty == 0 { mask & FILE_ALL_ACCESS != FILE_ALL_ACCESS } else { mask & FILE_ALL_ACCESS != 0 };
    }
    protected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_names() {
        assert!(is_critical_font(Path::new(r"C:\Windows\Fonts\SegoeUI.ttf")));
        assert!(is_critical_font(Path::new("marlett.ttf")));
        assert!(!is_critical_font(Path::new(r"C:\Windows\Fonts\comic.ttf")));
    }

    #[test]
    fn acl_rule() {
        // Shipped font: TrustedInstaller full, Administrators read & execute -> protected.
        assert!(acl_rule_protects(&[(0, false, 0x1F01FF), (0, true, 0x1200A9), (0, false, 0x1200A9)]));
        // User-installed font: Administrators full control -> not protected.
        assert!(!acl_rule_protects(&[(0, true, 0x1F01FF), (0, false, 0x1200A9)]));
        // Deny ACE for Administrators -> protected.
        assert!(acl_rule_protects(&[(0, true, 0x1F01FF), (1, true, 0x10000)]));
        // No Administrators ACE at all (e.g. a share) -> not protected.
        assert!(!acl_rule_protects(&[(0, false, 0x1F01FF)]));
        // Empty DACL -> protected.
        assert!(acl_rule_protects(&[]));
    }

    #[test]
    fn open_file_is_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.ttf");
        std::fs::write(&p, b"x").unwrap();
        assert!(!is_open_elsewhere(&p));
        let held = std::fs::File::open(&p).unwrap();
        assert!(is_open_elsewhere(&p));
        drop(held);
        assert!(!is_open_elsewhere(&p));
        assert!(!is_open_elsewhere(dir.path()));
    }
}
