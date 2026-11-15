//! Protected-system-font check on the real system.

use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows::Win32::Security::{
    GetAce, IsWellKnownSid, WinBuiltinAdministratorsSid, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
};
use windows::Win32::System::ApplicationInstallationAndServicing::SfcIsFileProtected;

use super::wide;

/// Protected = Windows Resource Protection covers the file, or the dossier's DACL rule holds, or
/// it is a critical shell font in the all-users fonts folder.
pub(crate) fn is_protected(path: &Path, all_users_dir: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let w = wide(path);
    if unsafe { SfcIsFileProtected(HANDLE::default(), PCWSTR(w.as_ptr())) }.as_bool() {
        return true;
    }
    if dacl_protects(path) == Some(true) {
        return true;
    }
    crate::util::is_directly_in(all_users_dir, path) && crate::protect::is_critical_font(path)
}

/// Dossier `CdrFnt` 0x1802028E0: `GetNamedSecurityInfoW(DACL)`, walk the ACEs and look at the
/// ones for `BUILTIN\Administrators`. `None` when the security information is unavailable.
pub(crate) fn dacl_protects(path: &Path) -> Option<bool> {
    let w = wide(path);
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    let r = unsafe {
        GetNamedSecurityInfoW(PCWSTR(w.as_ptr()), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION, None, None, Some(&mut dacl), None, &mut sd)
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    let mut aces = Vec::new();
    let result = if dacl.is_null() {
        // NULL DACL: everyone has full access.
        Some(false)
    } else {
        let count = unsafe { (*dacl).AceCount } as u32;
        for i in 0..count {
            let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
            if unsafe { GetAce(dacl, i, &mut ace) }.is_err() || ace.is_null() {
                continue;
            }
            let header = unsafe { &*(ace as *const ACE_HEADER) };
            if header.AceType > 1 {
                continue;
            }
            // ACCESS_ALLOWED_ACE / ACCESS_DENIED_ACE: header (4), mask (4), SID.
            let mask = unsafe { *((ace as *const u8).add(4) as *const u32) };
            let sid = PSID(unsafe { (ace as *mut u8).add(8) } as *mut _);
            let admins = unsafe { IsWellKnownSid(sid, WinBuiltinAdministratorsSid) }.as_bool();
            aces.push((header.AceType, admins, mask));
        }
        Some(crate::protect::acl_rule_protects(&aces))
    };
    if !sd.0.is_null() {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(sd.0)));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_font_is_protected_and_temp_file_is_not() {
        let fonts = crate::paths::windows_dir().join("Fonts");
        let arial = fonts.join("arial.ttf");
        if arial.is_file() {
            assert_eq!(dacl_protects(&arial), Some(true));
            assert!(is_protected(&arial, &fonts));
        }
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.ttf");
        std::fs::write(&p, b"x").unwrap();
        assert!(!is_protected(&p, &fonts));
    }
}
