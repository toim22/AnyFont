//! `anyfont.exe /SelfTest:<script.json>`: a scripted run of the font system. With the real backend it
//! installs, uninstalls and deletes fonts of the Windows it runs on, so it runs only on a disposable machine: a
//! GitHub-hosted Actions runner (`tests/real-system/run-real.ps1`, `.github/workflows/e2e-real.yml`) or Windows
//! Sandbox, and the script must say `"i_am_a_disposable_machine": true` ([`allowed`]). With the sandbox backend
//! (`ANYFONT_SANDBOX_ROOT`) it runs anywhere (`crates/anyfont/tests/e2e_selftest.rs`).
//!
//! The operations go through the program's own layers: `ops::Running` (the worker the Install / Uninstall /
//! Delete commands use), the elevated helper protocol (`/InstallHelper:`), and the core's scan and catalog
//! (what My fonts lists). Each step is checked against the registry (`reg.exe query`), the fonts folders and
//! the catalog, and the results are written as JSON:
//!
//! ```json
//! { "passed": 12, "failed": 0, "steps": [ { "op": "install", "ok": true, "detail": "...", "ms": 31 }, ... ] }
//! ```
//!
//! Script:
//!
//! ```json
//! {
//!   "i_am_a_disposable_machine": true,
//!   "results": "C:\\results\\selftest.json",
//!   "fixtures": "C:\\host\\fixtures",
//!   "work": "C:\\selftest-work",
//!   "steps": [
//!     { "op": "environment" },
//!     { "op": "list_installed", "min": 20, "protected": ["arial.ttf"] },
//!     { "op": "install", "font": "Tuffy.ttf", "scope": "CurrentUser" },
//!     { "op": "check_installed", "font": "Tuffy.ttf", "scope": "CurrentUser", "family": "Tuffy" },
//!     { "op": "uninstall", "font": "Tuffy.ttf", "scope": "CurrentUser" },
//!     { "op": "check_not_installed", "font": "Tuffy.ttf", "scope": "CurrentUser" },
//!     { "op": "helper_install", "font": "Hack-Regular.ttf", "scope": "AllUsers" },
//!     { "op": "delete_copy", "font": "Tuffy.ttf" },
//!     { "op": "protected_refused", "file": "C:\\Windows\\Fonts\\arial.ttf" }
//!   ]
//! }
//! ```

use crate::ops::{OpKind, Running};
use anyfont_core::catalog::{Catalog, FontQuery, LibraryRef};
use anyfont_core::db::FontDatabase;
use anyfont_core::model::InstalledFont as CoreInstalled;
use anyfont_core::scan::ScanRequest;
use anyfont_sys::{FontOpError, FontSystem, Scope, UsedFontAction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct Script {
    #[serde(default)]
    i_am_a_disposable_machine: bool,
    results: PathBuf,
    fixtures: PathBuf,
    work: PathBuf,
    /// A PowerShell script printing the processes that hold a file open (`tests/real-system/holders.ps1`).
    #[serde(default)]
    holders_script: Option<PathBuf>,
    steps: Vec<Value>,
}

#[derive(Serialize)]
struct StepResult {
    op: String,
    args: Value,
    ok: bool,
    detail: String,
    ms: u128,
}

#[derive(Serialize)]
struct Results {
    passed: usize,
    failed: usize,
    elevated: bool,
    user: String,
    steps: Vec<StepResult>,
}

struct Runner {
    fs: Arc<dyn FontSystem>,
    fixtures: PathBuf,
    work: PathBuf,
    /// Files deleted to the Recycle Bin (checked by the PowerShell side).
    deleted: Vec<PathBuf>,
    holders_script: Option<PathBuf>,
    /// Font files whose removal was deferred to the next restart ("Reboot later").
    pending: Vec<PathBuf>,
}

/// Runs the script and returns the process exit code (0 = every step passed, 1 = a step failed, 2 = the
/// script could not run).
pub fn run(script: &Path) -> i32 {
    let text = match std::fs::read_to_string(script) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("self-test: cannot read {}: {e}", script.display());
            return 2;
        }
    };
    let s: Script = match serde_json::from_str(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("self-test: bad script {}: {e}", script.display());
            return 2;
        }
    };
    let user = std::env::var("USERNAME").unwrap_or_default();
    if let Err(why) = allowed(s.i_am_a_disposable_machine, &user) {
        eprintln!("self-test: refused: {why}");
        return 2;
    }
    // only now: a refused run leaves no trace (not even a log line in %LOCALAPPDATA%)
    let _ = anyfont_sys::logging::init();
    let _ = std::fs::create_dir_all(&s.work);
    let fs: Arc<dyn FontSystem> = Arc::from(anyfont_sys::system());
    let mut r = Runner { fs, fixtures: s.fixtures.clone(), work: s.work.clone(), deleted: Vec::new(), holders_script: s.holders_script.clone(), pending: Vec::new() };
    let mut out = Results { passed: 0, failed: 0, elevated: anyfont_sys::is_elevated(), user, steps: Vec::new() };
    for step in &s.steps {
        let op = step.get("op").and_then(Value::as_str).unwrap_or("").to_string();
        let t0 = Instant::now();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.step(&op, step))).unwrap_or_else(|p| {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            Err(format!("panicked: {msg}"))
        });
        let (ok, detail) = match res {
            Ok(d) => (true, d),
            Err(d) => (false, d),
        };
        log::info!("self-test {op}: {} {detail}", if ok { "ok" } else { "FAILED" });
        println!("{} {op}: {detail}", if ok { "PASS" } else { "FAIL" });
        if ok {
            out.passed += 1;
        } else {
            out.failed += 1;
        }
        out.steps.push(StepResult { op, args: step.clone(), ok, detail, ms: t0.elapsed().as_millis() });
        let _ = write_results(&s.results, &out, &r.deleted, &r.pending);
    }
    if write_results(&s.results, &out, &r.deleted, &r.pending).is_err() {
        return 2;
    }
    if out.failed == 0 {
        0
    } else {
        1
    }
}

/// Where the self-test may run. With the sandbox backend (`ANYFONT_SANDBOX_ROOT`) it changes nothing of
/// Windows and runs anywhere. With the real backend it installs and uninstalls fonts of this Windows, so it runs
/// only on a machine that is thrown away afterwards: a GitHub-hosted Actions runner
/// (`.github/workflows/e2e-real.yml`) or Windows Sandbox. The script must say so too.
fn allowed(disposable_flag: bool, user: &str) -> Result<(), String> {
    if !disposable_flag {
        return Err("the script does not say \"i_am_a_disposable_machine\": true".into());
    }
    if sandbox_backend() {
        return Ok(());
    }
    let github_hosted = std::env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true")
        && std::env::var("RUNNER_ENVIRONMENT").is_ok_and(|v| v == "github-hosted");
    if github_hosted || user.eq_ignore_ascii_case("WDAGUtilityAccount") {
        return Ok(());
    }
    Err("it installs and uninstalls the fonts of this Windows: with the real backend it runs only on a GitHub-hosted \
         Actions runner or in Windows Sandbox (set ANYFONT_SANDBOX_ROOT to try it on the sandbox backend)"
        .into())
}

fn sandbox_backend() -> bool {
    std::env::var_os(anyfont_sys::SANDBOX_ENV).is_some_and(|v| !v.is_empty())
}

fn write_results(path: &Path, out: &Results, deleted: &[PathBuf], pending: &[PathBuf]) -> std::io::Result<()> {
    let mut v = serde_json::to_value(out).unwrap_or(Value::Null);
    v["deleted"] = serde_json::to_value(deleted).unwrap_or(Value::Null);
    v["pending_reboot"] = serde_json::to_value(pending).unwrap_or(Value::Null);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&v).unwrap_or_default())
}

fn arg<'a>(step: &'a Value, name: &str) -> Result<&'a str, String> {
    step.get(name).and_then(Value::as_str).ok_or_else(|| format!("missing \"{name}\""))
}

fn scope_of(step: &Value) -> Result<Scope, String> {
    match step.get("scope").and_then(Value::as_str).unwrap_or("CurrentUser") {
        "CurrentUser" => Ok(Scope::CurrentUser),
        "AllUsers" => Ok(Scope::AllUsers),
        other => Err(format!("unknown scope {other}")),
    }
}

fn reg_key(scope: Scope) -> &'static str {
    match scope {
        Scope::AllUsers => r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
        _ => r"HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts",
    }
}

/// Whether `PendingFileRenameOperations` (MoveFileEx's delete-at-restart list) names `path`.
fn pending_rename_has(path: &Path) -> bool {
    let mut cmd = std::process::Command::new("reg.exe");
    cmd.args(["query", r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager", "/v", "PendingFileRenameOperations"]);
    crate::help::dlg::hide_console(&mut cmd);
    let Ok(out) = cmd.output() else { return false };
    String::from_utf8_lossy(&out.stdout).to_ascii_lowercase().contains(&path.to_string_lossy().to_ascii_lowercase())
}

/// Whether AnyFont's own delete-at-next-start list names `path` (used when MoveFileEx is refused).
fn pending_list_has(path: &Path) -> bool {
    let file = anyfont_sys::paths::app_paths().data_dir.join("pending-deletes.json");
    std::fs::read_to_string(file).map(|t| t.to_ascii_lowercase().contains(&path.to_string_lossy().to_ascii_lowercase().replace('\\', "\\\\"))).unwrap_or(false)
}

/// `reg.exe query <key> /v <value>`: the value's data, or `None` when it does not exist.
fn reg_value(scope: Scope, value: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("reg.exe");
    cmd.args(["query", reg_key(scope), "/v", value]);
    crate::help::dlg::hide_console(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.lines().find(|l| l.contains("REG_SZ")).map(|l| l.split("REG_SZ").nth(1).unwrap_or("").trim().to_string())
}

impl Runner {
    fn step(&mut self, op: &str, step: &Value) -> Result<String, String> {
        match op {
            "environment" => Ok(format!(
                "user {}, elevated {}, backend {:?}, all users {}, current user {}",
                std::env::var("USERNAME").unwrap_or_default(),
                anyfont_sys::is_elevated(),
                self.fs.backend(),
                self.fs.font_dirs().all_users.display(),
                self.fs.font_dirs().current_user.display()
            )),
            "list_installed" => self.list_installed(step),
            "install" => {
                let file = self.fixture(arg(step, "font")?)?;
                let scope = scope_of(step)?;
                self.op(OpKind::Install, scope, vec![file], UsedFontAction::Ask).map(|o| format!("{o} (needs elevation: {})", self.fs.needs_elevation(scope)))
            }
            "uninstall" | "helper_uninstall" => self.uninstall(op, step),
            "check_installed" => self.check_installed(step),
            "check_not_installed" => self.check_not_installed(step),
            "helper_install" => self.helper(op, step, UsedFontAction::Ask),
            "delete_copy" => self.delete_copy(step),
            "protected_refused" => self.protected_refused(step),
            other => Err(format!("unknown op {other}")),
        }
    }

    fn fixture(&self, name: &str) -> Result<PathBuf, String> {
        let p = self.fixtures.join(name);
        if p.is_file() {
            Ok(p)
        } else {
            Err(format!("fixture {} not found", p.display()))
        }
    }

    /// Where an installed font file lives for a scope.
    fn installed_path(&self, name: &str, scope: Scope) -> PathBuf {
        let d = self.fs.font_dirs();
        match scope {
            Scope::AllUsers => d.all_users.join(name),
            _ => d.current_user.join(name),
        }
    }

    /// Runs one operation through `ops::Running`, as the Install / Uninstall / Delete commands do.
    fn op(&self, kind: OpKind, scope: Scope, files: Vec<PathBuf>, used: UsedFontAction) -> Result<String, String> {
        let ctx = egui::Context::default();
        let mut run = Running::start(self.fs.clone(), kind, scope, files.clone(), used, HashMap::new(), ctx);
        let end = Instant::now() + Duration::from_secs(120);
        while !run.poll() {
            if Instant::now() > end {
                return Err("timed out after 120 s".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let o = run.outcome.clone().unwrap_or_default();
        let text = format!("{kind:?} {} file(s): {} ok, failures {:?}, in use {:?}", files.len(), o.succeeded, o.failures, o.in_use);
        if o.succeeded == files.len() && o.failures.is_empty() && o.in_use.is_empty() {
            Ok(text)
        } else {
            Err(text)
        }
    }

    fn list_installed(&self, step: &Value) -> Result<String, String> {
        let list = self.fs.installed_fonts().map_err(|e| e.to_string())?;
        let min = step.get("min").and_then(Value::as_u64).unwrap_or(1) as usize;
        let protected = list.iter().filter(|f| f.protected).count();
        if list.len() < min {
            return Err(format!("{} installed fonts, expected at least {min}", list.len()));
        }
        for name in step.get("protected").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            let hit = list.iter().find(|f| f.path.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(name)).unwrap_or(false));
            match hit {
                Some(f) if f.protected => {}
                Some(_) => return Err(format!("{name} is installed but not protected")),
                None => return Err(format!("{name} is not listed")),
            }
        }
        // the catalog lists them in My fonts
        let n = self.my_fonts()?.len();
        if n == 0 {
            return Err("My fonts is empty".into());
        }
        Ok(format!("{} registrations ({protected} protected); My fonts lists {n} faces", list.len()))
    }

    /// The faces My fonts lists after a scan of the installed fonts (the app's own query), as (family, files).
    fn my_fonts(&self) -> Result<Vec<(String, Vec<PathBuf>, bool)>, String> {
        let db_path = self.work.join(format!("fonts-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db_path);
        let installed: Vec<CoreInstalled> = self.fs.installed_fonts().map_err(|e| e.to_string())?.into_iter().map(|f| CoreInstalled::new(f.path, f.protected)).collect();
        let mut db = FontDatabase::open(&db_path).map_err(|e| e.to_string())?;
        let req = ScanRequest { db_path: db_path.clone(), installed, force: true, threads: 4 };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        anyfont_core::scan::scan(&mut db, &req, &mut |_| {}, &cancel).map_err(|e| e.to_string())?;
        let catalog = Catalog::load(&db).map_err(|e| e.to_string())?;
        let q = FontQuery { library: LibraryRef::MyFonts, group_by_family: false, ..FontQuery::default() };
        let res = catalog.query(&q);
        Ok(res.font_ids.iter().filter_map(|id| catalog.font(*id)).map(|f| (f.family.clone(), f.paths.clone(), f.installed)).collect())
    }

    fn check_installed(&self, step: &Value) -> Result<String, String> {
        let name = arg(step, "font")?;
        let scope = scope_of(step)?;
        let path = self.installed_path(name, scope);
        if !path.is_file() {
            return Err(format!("{} does not exist", path.display()));
        }
        let regs = self.fs.registrations(&path);
        let Some(reg) = regs.iter().find(|r| r.scope == scope) else {
            return Err(format!("no {scope:?} registration of {} (registrations: {regs:?})", path.display()));
        };
        let data = if sandbox_backend() {
            // the sandbox's registry is a file of its own; its value resolves to the installed file
            let listed = self.fs.installed_fonts().map_err(|e| e.to_string())?;
            let f = listed
                .iter()
                .find(|f| f.value_name == reg.value_name && f.scope == scope)
                .ok_or_else(|| format!("the sandbox lists no value \"{}\"", reg.value_name))?;
            if anyfont_core::db::normalize_path(&f.path) != anyfont_core::db::normalize_path(&path) {
                return Err(format!("sandbox value \"{}\" points to {}, expected {}", reg.value_name, f.path.display(), path.display()));
            }
            format!("(sandbox) {}", f.path.display())
        } else {
            let data = reg_value(scope, &reg.value_name).ok_or_else(|| format!("reg.exe finds no value \"{}\" under {}", reg.value_name, reg_key(scope)))?;
            let expect_data = match scope {
                Scope::AllUsers => name.to_string(),
                _ => path.display().to_string(),
            };
            if !data.eq_ignore_ascii_case(&expect_data) {
                return Err(format!("registry value \"{}\" = \"{data}\", expected \"{expect_data}\"", reg.value_name));
            }
            data
        };
        let family = step.get("family").and_then(Value::as_str);
        let listed = self.my_fonts()?;
        let hit = listed.iter().find(|(_, paths, _)| paths.iter().any(|p| anyfont_core::db::normalize_path(p) == anyfont_core::db::normalize_path(&path)));
        match (hit, family) {
            (None, _) => Err(format!("My fonts does not list {}", path.display())),
            (Some((f, _, installed)), Some(want)) if f != want || !installed => Err(format!("My fonts lists it as \"{f}\" (installed {installed}), expected \"{want}\" installed")),
            (Some((f, _, _)), _) => Ok(format!("{} registered as \"{}\" = \"{data}\"; My fonts lists \"{f}\"", path.display(), reg.value_name)),
        }
    }

    fn check_not_installed(&self, step: &Value) -> Result<String, String> {
        let name = arg(step, "font")?;
        let scope = scope_of(step)?;
        let path = self.installed_path(name, scope);
        let regs = self.fs.registrations(&path);
        if !regs.is_empty() {
            return Err(format!("still registered: {regs:?}"));
        }
        let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(v) = self.fs.installed_fonts().map_err(|e| e.to_string())?.iter().find(|f| f.scope == scope && f.path.file_stem().map(|s| s.to_string_lossy() == stem).unwrap_or(false)) {
            return Err(format!("a registration is left: {v:?}"));
        }
        if path.exists() {
            if self.pending.iter().any(|p| p == &path) {
                return Ok(format!("no registration left; {} is removed at the next restart", path.display()));
            }
            return Err(format!("{} is still on disk", path.display()));
        }
        Ok(format!("{} gone, no registration left", path.display()))
    }

    /// Uninstall as the app does: the first pass ("Ask"); when Windows reports the file in use, the step says
    /// who holds it, waits like a user would (1 s, 3 s) and asks again, and finally answers the prompt with
    /// "Reboot later": the registration must be gone and the file queued for deletion at the next restart.
    /// The step fails when the font stays in use because of AnyFont itself.
    fn uninstall(&mut self, op: &str, step: &Value) -> Result<String, String> {
        let name = arg(step, "font")?;
        let scope = scope_of(step)?;
        let path = self.installed_path(name, scope);
        let helper = op == "helper_uninstall";
        let pass = |used: UsedFontAction| -> Result<String, String> {
            if helper {
                self.helper(op, step, used)
            } else {
                self.op(OpKind::Uninstall, Scope::CurrentUser, vec![path.clone()], used)
            }
        };
        let first = match pass(UsedFontAction::Ask) {
            Ok(t) => return Ok(t),
            Err(e) => e,
        };
        if !(first.contains("InUse") || first.contains("in use [\"")) {
            return Err(first);
        }
        let holders = self.holders(&path);
        let t0 = Instant::now();
        for wait in [1000, 3000] {
            std::thread::sleep(Duration::from_millis(wait));
            if let Ok(t) = pass(UsedFontAction::Ask) {
                return Ok(format!("in use right after the install (held by: {holders}); {t} after {} ms", t0.elapsed().as_millis()));
            }
        }
        let holders_later = self.holders(&path);
        let later = pass(UsedFontAction::RebootLater).map_err(|e| format!("in use (held by: {holders_later}); Reboot later failed: {e}"))?;
        let regs = self.fs.registrations(&path);
        if !regs.is_empty() {
            return Err(format!("Reboot later left the registration: {regs:?}"));
        }
        let queued = pending_rename_has(&path) || pending_list_has(&path);
        if path.exists() && !queued {
            return Err(format!("Reboot later: {} is neither deleted nor queued for deletion at restart", path.display()));
        }
        self.pending.push(path.clone());
        let text = format!(
            "in use right after the install and still 4 s later (held by: {holders}; then: {holders_later}); Reboot later: {later}, registration removed, file {}",
            if path.exists() { "queued for deletion at restart" } else { "deleted" }
        );
        if holders.to_ascii_lowercase().contains("anyfont") || holders_later.to_ascii_lowercase().contains("anyfont") {
            return Err(format!("held by AnyFont itself: {text}"));
        }
        Ok(text)
    }

    /// The processes holding `path` open, from `holders_script` (Restart Manager).
    fn holders(&self, path: &Path) -> String {
        let Some(script) = &self.holders_script else { return "unknown (no holders_script)".into() };
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(script).arg(path);
        crate::help::dlg::hide_console(&mut cmd);
        match cmd.output() {
            Ok(o) => {
                let t = String::from_utf8_lossy(&o.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("; ");
                if t.is_empty() {
                    format!("? ({})", String::from_utf8_lossy(&o.stderr).trim())
                } else {
                    t
                }
            }
            Err(e) => format!("? ({e})"),
        }
    }

    /// Runs the elevated helper protocol explicitly: the request file and a child `anyfont.exe
    /// /InstallHelper:<file>` (without the UAC prompt: Windows Sandbox's user is an administrator already).
    fn helper(&self, op: &str, step: &Value, used: UsedFontAction) -> Result<String, String> {
        use anyfont_sys::helper::{read_response, write_request, HelperRequest, HelperResponse};
        let name = arg(step, "font")?;
        let scope = scope_of(step)?;
        let req = if op == "helper_install" {
            HelperRequest::Install { files: vec![self.fixture(name)?], scope, used }
        } else {
            HelperRequest::Uninstall { paths: vec![self.installed_path(name, scope)], used }
        };
        let dir = self.work.join("helper");
        let _ = std::fs::create_dir_all(&dir);
        let file = write_request(&dir, &req).map_err(|e| e.to_string())?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = std::process::Command::new(&exe)
            .arg(format!("/InstallHelper:{}", file.display()))
            .status()
            .map_err(|e| format!("cannot start the helper: {e}"))?;
        if !status.success() {
            return Err(format!("helper exit status {status}"));
        }
        let resp = read_response(&file)?;
        let (ok, total, failures) = match &resp {
            HelperResponse::Install(r) => (r.succeeded(), r.results.len(), r.failed().map(|f| format!("{:?}", f.result)).collect::<Vec<_>>()),
            HelperResponse::Uninstall(r) => (r.succeeded(), r.results.len(), r.failed().map(|f| format!("{:?}", f.result)).collect::<Vec<_>>()),
            HelperResponse::Delete(r) => (r.succeeded(), r.results.len(), r.failed().map(|f| format!("{:?}", f.result)).collect::<Vec<_>>()),
            HelperResponse::Error(e) => return Err(format!("helper error: {e}")),
        };
        if ok == total && failures.is_empty() {
            Ok(format!("helper {op}: {ok}/{total} ok"))
        } else {
            Err(format!("helper {op}: {ok}/{total} ok, failures {failures:?}"))
        }
    }

    /// Copies a fixture into the work folder and deletes the copy through the Delete command's worker.
    fn delete_copy(&mut self, step: &Value) -> Result<String, String> {
        let name = arg(step, "font")?;
        let dir = self.work.join("to-delete");
        let _ = std::fs::create_dir_all(&dir);
        let copy = dir.join(name);
        std::fs::copy(self.fixture(name)?, &copy).map_err(|e| e.to_string())?;
        let text = self.op(OpKind::Delete, Scope::CurrentUser, vec![copy.clone()], UsedFontAction::Ask)?;
        if copy.exists() {
            return Err(format!("{} still exists after Delete", copy.display()));
        }
        self.deleted.push(copy);
        Ok(text)
    }

    fn protected_refused(&self, step: &Value) -> Result<String, String> {
        // "font": a file name in the all-users fonts folder of the backend; "file": a full path
        let file = match step.get("font").and_then(Value::as_str) {
            Some(name) => self.fs.font_dirs().all_users.join(name),
            None => PathBuf::from(arg(step, "file")?),
        };
        if !file.is_file() {
            return Err(format!("{} does not exist", file.display()));
        }
        if !self.fs.is_protected(&file) {
            return Err(format!("{} is not reported as protected", file.display()));
        }
        let u = self.fs.uninstall(std::slice::from_ref(&file), UsedFontAction::Ask);
        let d = self.fs.delete(std::slice::from_ref(&file), UsedFontAction::Ask);
        let refused = |r: Option<&Result<_, FontOpError>>| matches!(r, Some(Err(FontOpError::Protected)));
        let ur = u.results.first().map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone));
        let dr = d.results.first().map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone));
        if !refused(ur.as_ref()) || !refused(dr.as_ref()) {
            return Err(format!("uninstall: {ur:?}, delete: {dr:?} (expected Protected for both)"));
        }
        if !file.is_file() || self.fs.registrations(&file).is_empty() {
            return Err(format!("{} was changed", file.display()));
        }
        Ok(format!("{}: uninstall and delete refused (Protected), file and registration intact", file.display()))
    }
}
