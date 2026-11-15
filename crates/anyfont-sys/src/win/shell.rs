//! Shell operations: Explorer "open file location", Recycle Bin, reboot, elevated launch.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE, HWND, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, EWX_REBOOT, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_APPLICATION, SHTDN_REASON_MINOR_MAINTENANCE,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Shell::{
    ILCreateFromPathW, ILFree, SHFileOperationW, SHOpenFolderAndSelectItems, ShellExecuteExW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION,
    FOF_WANTNUKEWARNING, FO_DELETE, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, SHFILEOPSTRUCTW,
};
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};

use crate::FontOpError;

fn owner() -> HWND {
    HWND(crate::util::OWNER_HWND.load(Ordering::Relaxed) as *mut _)
}

/// Explorer on the containing folder with the file selected (`SHOpenFolderAndSelectItems`);
/// falls back to `explorer.exe /select,`.
pub(crate) fn open_location(path: &Path) -> io::Result<()> {
    let target: PathBuf = if path.exists() { path.to_path_buf() } else { path.parent().map(Path::to_path_buf).unwrap_or_default() };
    let w = HSTRING::from(target.as_os_str());
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let ok = unsafe {
        let pidl = ILCreateFromPathW(PCWSTR(w.as_ptr()));
        if pidl.is_null() {
            false
        } else {
            let r = SHOpenFolderAndSelectItems(pidl, None, 0).is_ok();
            ILFree(Some(pidl));
            r
        }
    };
    if com.is_ok() {
        unsafe { CoUninitialize() };
    }
    if ok {
        return Ok(());
    }
    let mut arg = std::ffi::OsString::from("/select,");
    arg.push(target.as_os_str());
    std::process::Command::new("explorer.exe").arg(arg).spawn().map(|_| ())
}

/// `SHFileOperationW(FO_DELETE)` with the original's flags 0x4050
/// (`FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING`, CrlUtils 0x1800A2660).
pub(crate) fn recycle(paths: &[PathBuf]) -> io::Result<()> {
    crate::win::forbid_in_tests!("move files to the Recycle Bin");
    if paths.is_empty() {
        return Ok(());
    }
    let mut from: Vec<u16> = Vec::new();
    for p in paths {
        from.extend(p.as_os_str().encode_wide_compat());
        from.push(0);
    }
    from.push(0);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: owner(),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        pTo: PCWSTR::null(),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_WANTNUKEWARNING.0) as u16,
        ..Default::default()
    };
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 {
        return Err(io::Error::other(format!("SHFileOperation failed (0x{r:X})")));
    }
    if op.fAnyOperationsAborted.as_bool() {
        return Err(io::Error::other("the operation was cancelled"));
    }
    Ok(())
}

trait EncodeWide {
    fn encode_wide_compat(&self) -> Vec<u16>;
}
impl EncodeWide for std::ffi::OsStr {
    fn encode_wide_compat(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

/// Enable `SeShutdownPrivilege` and restart (`ExitWindowsEx(EWX_REBOOT, 0x80040001)`, CdrFnt 0x180202780).
pub(crate) fn reboot_now() -> io::Result<()> {
    crate::win::forbid_in_tests!("restart Windows");
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).map_err(io::Error::other)?;
        let mut luid = LUID::default();
        let r = LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid).and_then(|_| {
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
            };
            AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None)
        });
        let _ = CloseHandle(token);
        r.map_err(io::Error::other)?;
        ExitWindowsEx(EWX_REBOOT, SHTDN_REASON_MAJOR_APPLICATION | SHTDN_REASON_MINOR_MAINTENANCE | SHTDN_REASON_FLAG_PLANNED)
            .map_err(io::Error::other)
    }
}

/// `ShellExecuteExW` verb `runas`, hidden window, then wait for the process (as the original's
/// `CInterprocessProtocol::LaunchProtocolCallee`). Returns the exit code.
pub(crate) fn run_elevated_and_wait(exe: &Path, params: &str) -> Result<u32, FontOpError> {
    let file = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(params);
    let verb = HSTRING::from("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        hwnd: owner(),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    let _ = SW_SHOWNORMAL;
    if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
        if (e.code().0 & 0xFFFF) as u32 == ERROR_CANCELLED.0 {
            return Err(FontOpError::ElevationCancelled);
        }
        return Err(FontOpError::Io(e.to_string()));
    }
    if info.hProcess.is_invalid() {
        return Err(FontOpError::Io("the helper process could not be started".into()));
    }
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 0u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        Ok(code)
    }
}
