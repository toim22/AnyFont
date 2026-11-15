//! Elevated install helper.
//!
//! All-users operations need administrator rights. Like the original (which hands the request to
//! a callee process started with `ShellExecuteEx` verb `runas`, hidden, and waits for it -
//! `CrlInterprocess` 0x18000A2F0), AnyFont re-launches its own executable elevated:
//!
//! ```text
//! anyfont.exe /InstallHelper:"%LOCALAPPDATA%\AnyFont\helper\request-<id>.json"
//! ```
//!
//! The request file holds one [`HelperRequest`]; the helper runs it with the normal engine
//! and writes the [`HelperResponse`] next to it as `request-<id>.json.result.json`, then exits.
//! The application's `main` must call [`helper_main`] **before** creating any window:
//!
//! ```ignore
//! let cl = anyfont_sys::CommandLine::from_env();
//! if let Some(req) = cl.install_helper() {
//!     std::process::exit(anyfont_sys::helper::helper_main(&req));
//! }
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{DeleteReport, FontOpError, FontSystem, InstallReport, Scope, UninstallReport, UsedFontAction};

const MAGIC: &str = "AnyFont.InstallHelper";
const VERSION: u32 = 1;
const MAX_REQUEST_BYTES: u64 = 4 * 1024 * 1024;

/// Work for the elevated helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperRequest {
    Install { files: Vec<PathBuf>, scope: Scope, used: UsedFontAction },
    Uninstall { paths: Vec<PathBuf>, used: UsedFontAction },
    Delete { paths: Vec<PathBuf>, used: UsedFontAction },
}

/// What the helper did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperResponse {
    Install(InstallReport),
    Uninstall(UninstallReport),
    Delete(DeleteReport),
    /// The request was rejected or could not be read.
    Error(String),
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    magic: String,
    version: u32,
    body: T,
}

/// `<request>.result.json`
pub fn response_path(request: &Path) -> PathBuf {
    let mut s = request.as_os_str().to_os_string();
    s.push(".result.json");
    PathBuf::from(s)
}

/// Entry point of `anyfont.exe /InstallHelper:<file>`. Returns the process exit code
/// (0 = response written).
pub fn helper_main(request_file: &Path) -> i32 {
    let fs = crate::system_for_helper();
    let response = match read_request(request_file) {
        Ok(req) => execute(fs.as_ref(), req),
        Err(e) => {
            log::error!("install helper: rejected {}: {e}", request_file.display());
            HelperResponse::Error(e)
        }
    };
    match write_envelope(&response_path(request_file), &response) {
        Ok(()) => 0,
        Err(e) => {
            log::error!("install helper: cannot write the response: {e}");
            2
        }
    }
}

/// Validate and parse a request file.
pub fn read_request(path: &Path) -> Result<HelperRequest, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !(name.starts_with("request-") && name.ends_with(".json")) || !path.is_absolute() {
        return Err("unexpected request file name".into());
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_REQUEST_BYTES {
        return Err("request file is not a regular file of acceptable size".into());
    }
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let env: Envelope<HelperRequest> = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if env.magic != MAGIC || env.version != VERSION {
        return Err("unknown request format".into());
    }
    let req = env.body;
    let (paths, ok_scope) = match &req {
        HelperRequest::Install { files, scope, .. } => (files, *scope == Scope::AllUsers),
        HelperRequest::Uninstall { paths, .. } | HelperRequest::Delete { paths, .. } => (paths, true),
    };
    if !ok_scope {
        return Err("only all-users installs need the helper".into());
    }
    if paths.is_empty() || paths.len() > 100_000 {
        return Err("bad number of files".into());
    }
    if let Some(p) = paths.iter().find(|p| !p.is_absolute() || p.to_string_lossy().contains('\0')) {
        return Err(format!("not an absolute path: {}", p.display()));
    }
    Ok(req)
}

/// Run a request against a font system (the helper uses the non-elevating backend).
pub fn execute(fs: &dyn FontSystem, req: HelperRequest) -> HelperResponse {
    match req {
        HelperRequest::Install { files, scope, used } => HelperResponse::Install(fs.install(&files, scope, used)),
        HelperRequest::Uninstall { paths, used } => HelperResponse::Uninstall(fs.uninstall(&paths, used)),
        HelperRequest::Delete { paths, used } => HelperResponse::Delete(fs.delete(&paths, used)),
    }
}

/// Write a request file in `dir`; returns its path.
pub fn write_request(dir: &Path, req: &HelperRequest) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("request-{}.json", crate::util::unique_id()));
    write_envelope(&path, req)?;
    Ok(path)
}

/// Read the response written by the helper.
pub fn read_response(request: &Path) -> Result<HelperResponse, String> {
    let data = std::fs::read(response_path(request)).map_err(|e| e.to_string())?;
    let env: Envelope<HelperResponse> = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if env.magic != MAGIC {
        return Err("unknown response format".into());
    }
    Ok(env.body)
}

fn write_envelope<T: Serialize>(path: &Path, body: &T) -> std::io::Result<()> {
    let env = Envelope { magic: MAGIC.to_string(), version: VERSION, body };
    let data = serde_json::to_vec_pretty(&env).map_err(std::io::Error::other)?;
    crate::util::write_atomic(path, &data)
}

/// Remove request/response files older than a day (left behind by a crash or a cancelled prompt).
pub(crate) fn clean_stale(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|age| age.as_secs() > 24 * 3600)
            .unwrap_or(false);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Run a request in an elevated copy of this executable and wait for it.
#[cfg(windows)]
pub(crate) fn run_elevated(req: &HelperRequest, helper_dir: &Path) -> Result<HelperResponse, FontOpError> {
    clean_stale(helper_dir);
    let request = write_request(helper_dir, req).map_err(|e| crate::engine::e_to_op(&e))?;
    let exe = std::env::current_exe().map_err(|e| FontOpError::Io(e.to_string()))?;
    let arg = format!("/{}:{}", crate::cmdline::INSTALL_HELPER_SWITCH, crate::cmdline::quote_arg(&request.to_string_lossy()));
    log::info!("starting the elevated helper for {} file(s)", req_len(req));
    let launched = crate::win::shell::run_elevated_and_wait(&exe, &arg);
    let result = match launched {
        Ok(code) => read_response(&request).map_err(|e| FontOpError::Io(format!("helper exit code {code}: {e}"))),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_file(&request);
    let _ = std::fs::remove_file(response_path(&request));
    result
}

#[cfg(windows)]
fn req_len(req: &HelperRequest) -> usize {
    match req {
        HelperRequest::Install { files, .. } => files.len(),
        HelperRequest::Uninstall { paths, .. } | HelperRequest::Delete { paths, .. } => paths.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trip_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let req = HelperRequest::Install { files: vec![dir.path().join("a.ttf")], scope: Scope::AllUsers, used: UsedFontAction::Ask };
        let path = write_request(dir.path(), &req).unwrap();
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("request-"));
        assert_eq!(read_request(&path).unwrap(), req);

        // Per-user installs never go through the helper.
        let bad = HelperRequest::Install { files: vec![dir.path().join("a.ttf")], scope: Scope::CurrentUser, used: UsedFontAction::Ask };
        let p2 = write_request(dir.path(), &bad).unwrap();
        assert!(read_request(&p2).is_err());

        // Relative paths are rejected.
        let rel = HelperRequest::Delete { paths: vec![PathBuf::from("a.ttf")], used: UsedFontAction::Ignore };
        let p3 = write_request(dir.path(), &rel).unwrap();
        assert!(read_request(&p3).is_err());

        // Wrong file name / garbage.
        let p4 = dir.path().join("evil.json");
        std::fs::write(&p4, b"{}").unwrap();
        assert!(read_request(&p4).is_err());
        let p5 = dir.path().join("request-x.json");
        std::fs::write(&p5, b"{\"magic\":\"x\",\"version\":1,\"body\":null}").unwrap();
        assert!(read_request(&p5).is_err());
    }

    #[test]
    fn response_path_appends_suffix() {
        assert_eq!(response_path(Path::new(r"C:\h\request-1.json")), PathBuf::from(r"C:\h\request-1.json.result.json"));
    }
}
