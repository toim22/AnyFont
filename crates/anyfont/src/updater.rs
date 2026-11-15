//! Product updates (owned by the services UI developer; ported from AnyVector's `updater.rs`). AnyFont looks
//! for a newer release of `toim22/AnyFont` on GitHub, downloads its setup and installs it after it closes.
//!
//! - Every start looks for a new version in the background ([`Updater::on_startup`], the start is not
//!   delayed) unless the mode is "Never check for updates", and shows a message only when one is found:
//!   the new version and its release notes, **OK** (install now: download with progress, close AnyFont, run
//!   the setup silently, which starts AnyFont again) or **Later** (asked again at the next start).
//! - "Keep the product up to date" downloads the setup by itself while that message is shown, so OK installs
//!   at once; "Check for updates and let me choose" downloads only after OK.
//! - Help ▸ Check for Updates... and the title-bar Updates button ([`Updater::open`]) show the version,
//!   check now, show the notes and install (UI-SPEC §7.2.15).
//! - The release comes from the GitHub API (`releases/latest`); when the repository is private the GitHub
//!   CLI (`gh`, signed in) reads and downloads it instead.
//! - `ANYFONT_UPDATE_FEED` = a URL or a local JSON file standing in for the `releases/latest` answer, so the
//!   "update available" path can be tested offline. An asset URL that is a local path is copied.
//!
//! Contract: `docs/RELEASING.md` (asset `AnyFont-<version>-setup.exe`, silent switches, `/RELAUNCH=1`).

use crate::bugreport::REPO;
use crate::help::dlg;
use anyfont_core::settings::UpdateMode;
use egui::{vec2, Color32, RichText};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// What the main window must do after `Updater::ui`.
pub enum UpdaterRequest {
    /// Close AnyFont (asking nothing – font manager has no documents), then run this setup silently with
    /// `/RELAUNCH=1`; `main` calls [`run_setup_after_exit`] after the event loop ends.
    ExitAndInstall(PathBuf),
}

/// The switches of a silent update (`docs/RELEASING.md`).
pub const SETUP_ARGS: [&str; 5] = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/RELAUNCH=1"];

/// The setup asset name used when the release does not say.
pub const FALLBACK_ASSET: &str = "AnyFont-setup.exe";

// ------------------------------------------------------------------------------------------------
// update modes
// ------------------------------------------------------------------------------------------------

/// The three choices of "Allow product updates:" (UI-SPEC §7.2.15), in the original's order and words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Keep the product up to date (recommended): downloads by itself, asks before installing.
    KeepUpToDate,
    /// Check for updates and let me choose when to install them.
    Choose,
    /// Never check for updates.
    Never,
}

impl Policy {
    pub const ALL: [Policy; 3] = [Policy::KeepUpToDate, Policy::Choose, Policy::Never];

    pub fn caption(self) -> &'static str {
        match self {
            Policy::KeepUpToDate => "Keep the product up to date (recommended)",
            Policy::Choose => "Check for updates and let me choose when to install them",
            Policy::Never => "Never check for updates",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Policy::KeepUpToDate => "Automatically downloads product updates, but prompts you before installing them.",
            Policy::Choose => "AnyFont tells you when a new version is available; you choose when to download and install it.",
            Policy::Never => "AnyFont never connects to the Internet to look for updates. Help ▸ Check for Updates still checks when you ask.",
        }
    }

    /// The names an `anyfont_core::settings::UpdateMode` variant may have for this choice. The mode is
    /// matched by its serde name, so this module follows the core enum as it grows (today it has
    /// `Notify` and `Never`; a `KeepUpToDate` variant is picked up without a code change here).
    fn variant_names(self) -> &'static [&'static str] {
        match self {
            Policy::KeepUpToDate => &["KeepUpToDate", "Keep", "Automatic", "Auto", "Download"],
            Policy::Choose => &["Notify", "Choose", "CheckAndChoose", "Check", "Ask"],
            Policy::Never => &["Never", "Off", "Disabled"],
        }
    }

    /// The core mode of this choice, if the core enum has one.
    pub fn to_mode(self) -> Option<UpdateMode> {
        self.variant_names().iter().find_map(|n| serde_json::from_value::<UpdateMode>(serde_json::Value::String((*n).into())).ok())
    }

    /// The choice a core mode stands for.
    pub fn of(mode: UpdateMode) -> Policy {
        let name = serde_json::to_value(mode).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        Policy::ALL.into_iter().find(|p| p.variant_names().iter().any(|n| n.eq_ignore_ascii_case(&name))).unwrap_or(Policy::Choose)
    }

    /// The choices the core settings can store, in dialog order.
    pub fn available() -> Vec<Policy> {
        Policy::ALL.into_iter().filter(|p| p.to_mode().is_some()).collect()
    }
}

/// "Allow product updates:" – the combo box and the note of the chosen mode, for the Updates dialog and for
/// the shell's Options dialog. Returns true when the mode changed.
pub fn mode_ui(ui: &mut egui::Ui, mode: &mut UpdateMode) -> bool {
    let mut changed = false;
    let cur = Policy::of(*mode);
    ui.horizontal(|ui| {
        ui.label("Allow product updates:");
        egui::ComboBox::from_id_salt("anyfont_update_mode").selected_text(cur.caption()).width(330.0).show_ui(ui, |ui| {
            for p in Policy::available() {
                if ui.selectable_label(p == cur, p.caption()).clicked() && p != cur {
                    if let Some(m) = p.to_mode() {
                        *mode = m;
                        changed = true;
                    }
                }
            }
        })
        .response
        .accessible_name("Allow product updates:");
    });
    dlg::note(ui, Policy::of(*mode).note());
    changed
}

// ------------------------------------------------------------------------------------------------
// releases
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// "0.2.0"
    pub version: String,
    /// "v0.2.0"
    pub tag: String,
    /// Release notes (GitHub Markdown).
    pub notes: String,
    /// The release page.
    pub page: String,
    /// The setup file and its download address (or a local path, for tests).
    pub asset_name: String,
    pub asset_url: String,
    /// The setup's size in bytes (0 when unknown): a setup already downloaded is used again.
    pub asset_size: u64,
    /// Read through the GitHub CLI (private repository): download with it too.
    pub via_gh: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(Release, f32),
    Ready(Release, PathBuf),
    Failed(String),
}

enum Msg {
    Checked(Result<Option<Release>, String>),
    Progress(f32),
    Downloaded(Result<PathBuf, String>),
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn user_agent() -> String {
    format!("AnyFont/{}", current_version())
}

/// "v1.2.3" → [1, 2, 3]
pub fn parse_version(s: &str) -> [u64; 3] {
    let mut out = [0u64; 3];
    let core = s.trim().trim_start_matches(['v', 'V']);
    let core = core.split(['-', '+']).next().unwrap_or("");
    for (i, p) in core.split('.').take(3).enumerate() {
        out[i] = p.trim().parse().unwrap_or(0);
    }
    out
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    parse_version(remote) > parse_version(local)
}

/// The release described by GitHub's JSON, if it carries a setup for Windows (`*-setup.exe`).
pub fn release_from_json(v: &serde_json::Value, via_gh: bool) -> Option<Release> {
    let tag = v["tag_name"].as_str()?.to_string();
    let asset = v["assets"].as_array()?.iter().find(|a| a["name"].as_str().is_some_and(|n| n.to_ascii_lowercase().ends_with("-setup.exe")))?;
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        notes: v["body"].as_str().unwrap_or_default().to_string(),
        page: v["html_url"].as_str().map(str::to_string).unwrap_or_else(|| format!("https://github.com/{REPO}/releases")),
        asset_name: asset["name"].as_str().unwrap_or(FALLBACK_ASSET).to_string(),
        asset_url: asset["browser_download_url"].as_str().unwrap_or_default().to_string(),
        asset_size: asset["size"].as_u64().unwrap_or(0),
        tag,
        via_gh,
    })
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(timeout))).build().into()
}

const NO_INTERNET: &str = "An Internet connection is required. Please connect to the Internet and try again.";
const NO_SETUP: &str = "The latest release has no setup for Windows.";

/// The test feed (`ANYFONT_UPDATE_FEED`): a URL or a local JSON file.
pub fn feed_override() -> Option<String> {
    std::env::var("ANYFONT_UPDATE_FEED").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// A local path given as `file:///C:/x` or `C:\x`.
fn local_path(s: &str) -> PathBuf {
    PathBuf::from(s.strip_prefix("file:///").or_else(|| s.strip_prefix("file://")).unwrap_or(s))
}

/// GET `url` as JSON: `Ok(Some)` on 200, `Ok(None)` when the server answered something else, `Err` when unreachable.
fn get_json(url: &str) -> Result<Option<serde_json::Value>, ()> {
    match agent(20).get(url).header("Accept", "application/vnd.github+json").header("User-Agent", &user_agent()).call() {
        Ok(mut r) if r.status().as_u16() == 200 => r.body_mut().read_json().map(Some).map_err(|_| ()),
        Ok(_) => Ok(None),
        Err(_) => Err(()),
    }
}

/// The latest release: `Ok(None)` when it is not newer than this version.
fn check() -> Result<Option<Release>, String> {
    let release = if let Some(feed) = feed_override() {
        let v: serde_json::Value = if is_url(&feed) {
            get_json(&feed).map_err(|_| NO_INTERNET.to_string())?.ok_or_else(|| format!("No published release was found at github.com/{REPO}."))?
        } else {
            let text = std::fs::read_to_string(local_path(&feed)).map_err(|e| format!("{}: {e}", feed))?;
            serde_json::from_str(&text).map_err(|e| format!("Invalid answer from the update server: {e}"))?
        };
        release_from_json(&v, false).ok_or(NO_SETUP)?
    } else {
        let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
        let web = get_json(&url);
        let reached = matches!(web, Ok(None));
        match web {
            Ok(Some(v)) => release_from_json(&v, false).ok_or(NO_SETUP)?,
            // not public (or no network): the GitHub CLI, when it is signed in, can read a private repository
            _ => match gh_json(&["api", &format!("repos/{REPO}/releases/latest")]) {
                Some(v) => release_from_json(&v, true).ok_or(NO_SETUP)?,
                None if reached => return Err(format!("No published release was found at github.com/{REPO}.")),
                None => return Err(NO_INTERNET.into()),
            },
        }
    };
    Ok(is_newer(&release.version, current_version()).then_some(release))
}

fn gh_json(args: &[&str]) -> Option<serde_json::Value> {
    let out = crate::bugreport::gh(args)?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

/// Where setups are downloaded.
pub fn download_dir() -> PathBuf {
    std::env::temp_dir().join("AnyFont Updates")
}

fn copy_with_progress(mut reader: impl std::io::Read, part: &Path, total: Option<u64>, tx: &Sender<Msg>, read_err: &str) -> Result<u64, String> {
    let mut file = std::fs::File::create(part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|_| read_err.to_string())?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if let Some(t) = total.filter(|t| *t > 0) {
            let _ = tx.send(Msg::Progress((done as f32 / t as f32).min(1.0)));
        }
    }
    Ok(done)
}

fn download(r: &Release, tx: &Sender<Msg>) -> Result<PathBuf, String> {
    let dir = download_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = Path::new(&r.asset_name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| FALLBACK_ASSET.into());
    let path = dir.join(&name);
    let part = path.with_extension("part");
    if r.via_gh {
        let out = crate::bugreport::gh(&["release", "download", &r.tag, "--repo", REPO, "--pattern", &r.asset_name, "--dir", &dir.to_string_lossy(), "--clobber"]).ok_or(NO_INTERNET)?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
    } else if is_url(&r.asset_url) {
        let mut resp = agent(1800).get(&r.asset_url).header("User-Agent", &user_agent()).call().map_err(|_| NO_INTERNET.to_string())?;
        if resp.status().as_u16() != 200 {
            return Err(format!("The update server answered {}.", resp.status().as_u16()));
        }
        let total: Option<u64> = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        let reader = resp.body_mut().with_config().limit(1 << 31).reader();
        let done = copy_with_progress(reader, &part, total, tx, NO_INTERNET)?;
        if total.is_some_and(|t| t != done) {
            return Err("The download was interrupted. Please try again.".into());
        }
        let _ = std::fs::remove_file(&path);
        std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    } else {
        // a local setup (ANYFONT_UPDATE_FEED tests)
        let src = local_path(&r.asset_url);
        let total = std::fs::metadata(&src).map(|m| m.len()).ok();
        let file = std::fs::File::open(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        copy_with_progress(file, &part, total, tx, "The download was interrupted. Please try again.")?;
        let _ = std::fs::remove_file(&path);
        std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    }
    match std::fs::metadata(&path) {
        Ok(m) if m.len() > 0 => Ok(path),
        _ => Err("The downloaded setup is empty.".into()),
    }
}

/// Runs the downloaded setup after the window closed: silently, closing a running AnyFont if needed and
/// starting it again afterwards (`/RELAUNCH=1`).
pub fn run_setup_after_exit(setup: &std::path::Path) {
    if let Err(e) = std::process::Command::new(setup).args(SETUP_ARGS).spawn() {
        log::error!("could not start the setup {}: {e}", setup.display());
    }
}

// ------------------------------------------------------------------------------------------------
// state
// ------------------------------------------------------------------------------------------------

pub struct Updater {
    pub status: Status,
    rx: Option<Receiver<Msg>>,
    /// Help ▸ Check for Updates... is open.
    dialog: bool,
    /// The "new version" message (from the check at start) is open.
    prompt: bool,
    /// The check was started by the program, not by the user: failures stay quiet.
    quiet: bool,
    /// Install as soon as the download finishes.
    install_when_ready: bool,
    /// The mode passed to `on_startup` and changed in the dialog.
    mode: UpdateMode,
    mode_changed: Option<UpdateMode>,
    /// Set by `ui` when the user chose to install: returned once.
    request: Option<PathBuf>,
    ctx: Option<egui::Context>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater {
    pub fn new() -> Self {
        Updater {
            status: Status::Idle,
            rx: None,
            dialog: false,
            prompt: false,
            quiet: false,
            install_when_ready: false,
            mode: UpdateMode::default(),
            mode_changed: None,
            request: None,
            ctx: None,
        }
    }

    /// Called once when the main window is shown: checks in the background according to the update mode.
    pub fn on_startup(&mut self, ctx: &egui::Context, mode: anyfont_core::settings::UpdateMode) {
        self.ctx = Some(ctx.clone());
        self.mode = mode;
        if cfg!(test) || Policy::of(mode) == Policy::Never {
            return;
        }
        self.start_check(true);
    }

    /// Help ▸ Check for Updates... and the title-bar Updates button.
    pub fn open(&mut self) {
        self.dialog = true;
        self.prompt = false;
        if matches!(self.status, Status::Idle | Status::UpToDate | Status::Failed(_)) {
            self.start_check(false);
        }
    }

    /// True while a newer version is known (the title-bar Updates button may show a dot).
    pub fn update_available(&self) -> bool {
        matches!(self.status, Status::Available(_) | Status::Downloading(..) | Status::Ready(..))
    }

    /// The newer version, when one is known ("0.2.0").
    pub fn available_version(&self) -> Option<&str> {
        match &self.status {
            Status::Available(r) | Status::Downloading(r, _) | Status::Ready(r, _) => Some(&r.version),
            _ => None,
        }
    }

    /// True while the Updates dialog or the start-up message is open.
    pub fn is_open(&self) -> bool {
        self.dialog || self.prompt
    }

    /// The mode the user chose in the Updates dialog since the last call: the shell stores it in its
    /// settings (`Settings::update_mode`) and saves them.
    pub fn take_mode_change(&mut self) -> Option<UpdateMode> {
        self.mode_changed.take()
    }

    /// The mode in use (the one given to `on_startup`, or the last one chosen in the dialog).
    pub fn mode(&self) -> UpdateMode {
        self.mode
    }

    /// Tells the updater about a mode changed elsewhere (Options).
    pub fn set_mode(&mut self, mode: UpdateMode) {
        self.mode = mode;
    }

    fn repaint(&self) {
        if let Some(c) = &self.ctx {
            c.request_repaint();
        }
    }

    /// Starts a check in the background. `quiet`: started by the program, so a failure is not shown.
    pub fn start_check(&mut self, quiet: bool) {
        if self.rx.is_some() {
            return;
        }
        self.quiet = quiet;
        self.status = Status::Checking;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Checked(check()));
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
    }

    fn start_download(&mut self, r: Release) {
        if self.rx.is_some() {
            return;
        }
        // the setup downloaded at an earlier start (the user chose Later) is used again
        let path = download_dir().join(&r.asset_name);
        if r.asset_size > 0 && std::fs::metadata(&path).is_ok_and(|m| m.len() == r.asset_size) {
            self.status = Status::Ready(r, path.clone());
            if std::mem::take(&mut self.install_when_ready) {
                self.install(path);
            }
            return;
        }
        self.status = Status::Downloading(r.clone(), 0.0);
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let res = download(&r, &tx);
            let _ = tx.send(Msg::Downloaded(res));
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
    }

    fn install(&mut self, path: PathBuf) {
        self.request = Some(path);
        self.prompt = false;
        self.dialog = false;
    }

    /// Install Now (or OK in the message): download first when needed, then close and install.
    fn install_now(&mut self) {
        match self.status.clone() {
            Status::Ready(_, path) => self.install(path),
            Status::Available(r) => {
                self.install_when_ready = true;
                self.quiet = false;
                self.start_download(r);
            }
            Status::Downloading(..) => {
                self.install_when_ready = true;
                self.quiet = false;
            }
            _ => {}
        }
    }

    /// Background results.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        let mut msgs = vec![];
        while let Ok(m) = rx.try_recv() {
            msgs.push(m);
        }
        ctx.request_repaint_after(Duration::from_millis(200));
        for m in msgs {
            match m {
                Msg::Progress(p) => {
                    if let Status::Downloading(_, q) = &mut self.status {
                        *q = p;
                    }
                }
                Msg::Checked(res) => {
                    self.rx = None;
                    match res {
                        Ok(None) => {
                            self.status = Status::UpToDate;
                            // the setup of the version now running is no longer needed
                            let _ = std::fs::remove_dir_all(download_dir());
                        }
                        Ok(Some(r)) => {
                            self.status = Status::Available(r.clone());
                            if self.quiet {
                                // the message at start; keeping up to date downloads meanwhile, so OK installs at once
                                self.prompt = !self.dialog;
                                if Policy::of(self.mode) == Policy::KeepUpToDate {
                                    self.start_download(r);
                                }
                            }
                        }
                        Err(e) => self.status = if self.quiet { Status::Idle } else { Status::Failed(e) },
                    }
                }
                Msg::Downloaded(res) => {
                    self.rx = None;
                    let Status::Downloading(r, _) = std::mem::take(&mut self.status) else { continue };
                    match res {
                        Ok(path) => {
                            self.status = Status::Ready(r, path.clone());
                            if std::mem::take(&mut self.install_when_ready) {
                                self.install(path);
                            }
                        }
                        Err(e) => {
                            self.install_when_ready = false;
                            self.status = if self.quiet && !self.dialog && !self.prompt { Status::Available(r) } else { Status::Failed(e) };
                        }
                    }
                }
            }
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) -> Option<UpdaterRequest> {
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }
        self.poll(ctx);
        if self.dialog {
            self.dialog_ui(ctx);
        } else if self.prompt {
            self.prompt_ui(ctx);
        }
        if self.request.is_some() {
            self.repaint();
        }
        self.request.take().map(UpdaterRequest::ExitAndInstall)
    }

    /// Help ▸ Check for Updates...: UI-SPEC §7.2.15.
    fn dialog_ui(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let ((), dismissed) = dlg::window(ctx, "updates", "Updates", vec2(540.0, 420.0), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at(ui, top, |ui| {
                ui.add_space(6.0);
                dlg::heading(ui, &format!("You are using AnyFont {}.", current_version()));
                ui.add_space(2.0);
                // the mode setting keeps the bottom of the area
                let room = 78.0;
                self.status_ui(ui, room);
                ui.add_space((ui.available_height() - room).max(0.0));
                dlg::separator(ui);
                let mut m = self.mode;
                if mode_ui(ui, &mut m) {
                    self.mode = m;
                    self.mode_changed = Some(m);
                }
            });
            dlg::at_left(ui, bar, |ui| {
                let page = match &self.status {
                    Status::Available(r) | Status::Downloading(r, _) | Status::Ready(r, _) => r.page.clone(),
                    _ => format!("https://github.com/{REPO}/releases"),
                };
                if dlg::button(ui, "Release Notes", true) {
                    dlg::open_url(&page);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dlg::button(ui, "Close", true) {
                        close = true;
                    }
                    // also while the setup downloads: it installs when the download ends
                    let (can_install, enabled) = match self.status {
                        Status::Available(_) | Status::Ready(..) => (true, self.rx.is_none()),
                        Status::Downloading(..) => (true, !self.install_when_ready),
                        _ => (false, false),
                    };
                    if can_install && dlg::default_button(ui, "Install Now", enabled) {
                        self.install_now();
                    }
                    let busy = self.rx.is_some();
                    if dlg::button(ui, "Check Now", !busy) {
                        self.start_check(false);
                    }
                });
            });
        });
        if close || dismissed {
            self.dialog = false;
        }
    }

    fn status_ui(&mut self, ui: &mut egui::Ui, room: f32) {
        match self.status.clone() {
            Status::Idle => dlg::note(ui, "Click Check Now to look for a new version."),
            Status::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking for updates...");
                });
            }
            Status::UpToDate => {
                ui.label("AnyFont is up to date.");
            }
            Status::Available(r) => {
                ui.label(RichText::new(format!("AnyFont {} is available.", r.version)).strong());
                release_notes(ui, &r, room);
            }
            Status::Downloading(r, p) => {
                ui.label(RichText::new(format!("AnyFont {} is available.", r.version)).strong());
                ui.label(format!("Downloading AnyFont {}...", r.version));
                ui.add(egui::ProgressBar::new(p).desired_width(ui.available_width()).show_percentage());
                if self.install_when_ready {
                    dlg::note(ui, "AnyFont closes to install it as soon as the download finishes.");
                }
                release_notes(ui, &r, room + 50.0);
            }
            Status::Ready(r, _) => {
                ui.label(RichText::new(format!("AnyFont {} is ready to install.", r.version)).strong());
                dlg::note(ui, "AnyFont closes to install it and starts again when it is done.");
                release_notes(ui, &r, room + 20.0);
            }
            Status::Failed(e) => dlg::error(ui, &format!("Error: {e}")),
        }
    }

    /// The message shown when the check at start finds a new version: OK installs now, Later asks again at
    /// the next start.
    fn prompt_ui(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let release = match &self.status {
            Status::Available(r) | Status::Downloading(r, _) | Status::Ready(r, _) => Some(r.clone()),
            _ => None,
        };
        let installing = self.install_when_ready;
        let ((), dismissed) = dlg::window(ctx, "update_prompt", "Updates", vec2(500.0, 380.0), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at(ui, top, |ui| {
                ui.add_space(6.0);
                match (&release, &self.status) {
                    (_, Status::Failed(e)) => dlg::error(ui, &format!("Error: {e}")),
                    (Some(r), Status::Downloading(_, p)) if installing => {
                        ui.label(format!("Downloading AnyFont {}...", r.version));
                        ui.add(egui::ProgressBar::new(*p).desired_width(ui.available_width()).show_percentage());
                        dlg::note(ui, "AnyFont closes to install it and starts again when it is done.");
                    }
                    (Some(r), _) => {
                        ui.label(RichText::new("There is a new version of AnyFont available.").strong().size(14.0));
                        ui.label(format!("New version: {}. Your version: {}.", r.version, current_version()));
                        ui.add_space(4.0);
                        release_notes(ui, r, 50.0);
                        ui.add_space(4.0);
                        ui.label("Click OK to install the new version.");
                        dlg::note(ui, "AnyFont closes to install it and starts again when it is done.");
                    }
                    (None, _) => close = true,
                }
            });
            dlg::at_left(ui, bar, |ui| {
                if let Some(r) = &release {
                    if dlg::button(ui, "Release Notes", true) {
                        dlg::open_url(&r.page);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let failed = matches!(self.status, Status::Failed(_));
                    if dlg::button(ui, if failed { "Close" } else { "Later" }, true) {
                        close = true;
                    }
                    if !failed && (dlg::default_button(ui, "OK", !installing) || (!installing && dlg::enter_pressed(ui))) {
                        self.install_now();
                    }
                });
            });
        });
        if close || dismissed {
            self.prompt = false;
            // the download may go on; the update is offered again at the next start
            self.install_when_ready = false;
        }
    }
}

// ------------------------------------------------------------------------------------------------
// release notes
// ------------------------------------------------------------------------------------------------

/// The release notes, scrolling, in what is left above `room` (at most 200 points).
fn release_notes(ui: &mut egui::Ui, r: &Release, room: f32) {
    let h = (ui.available_height() - room).clamp(36.0, 200.0);
    egui::ScrollArea::vertical().id_salt("release_notes").max_height(h).auto_shrink([false, true]).show(ui, |ui| {
        if r.notes.trim().is_empty() {
            dlg::note(ui, "This release has no notes.");
        } else {
            notes_ui(ui, &r.notes);
        }
    });
}

/// A piece of the release notes, as GitHub shows them: a heading, a list item or a paragraph.
#[derive(Debug, PartialEq)]
pub enum NoteBlock {
    /// `## Heading` (level 2 and below; the `# AnyFont x.y.z` title is left out, the window says it already)
    Heading(Vec<(String, bool)>),
    /// `- item`, nested two spaces per level
    Item(usize, Vec<(String, bool)>),
    Para(Vec<(String, bool)>),
}

/// The text with `**bold**` as (text, bold) runs, the backticks of `code` dropped and `[text](url)` as text.
fn note_runs(s: &str) -> Vec<(String, bool)> {
    let mut s = s.replace('`', "");
    // [text](url) → text
    while let Some(a) = s.find('[') {
        let Some(b) = s[a..].find("](").map(|i| a + i) else { break };
        let Some(c) = s[b..].find(')').map(|i| b + i) else { break };
        let text = s[a + 1..b].to_string();
        s.replace_range(a..=c, &text);
    }
    s.split("**").enumerate().filter(|(_, t)| !t.is_empty()).map(|(i, t)| (t.to_string(), i % 2 == 1)).collect()
}

/// The release notes (GitHub Markdown, lines wrapped at a fixed width) as blocks: wrapped lines join their block.
pub fn note_blocks(md: &str) -> Vec<NoteBlock> {
    let mut out = vec![];
    let mut cur: Option<(Option<usize>, String)> = None;
    let flush = |cur: &mut Option<(Option<usize>, String)>, out: &mut Vec<NoteBlock>| {
        if let Some((depth, text)) = cur.take() {
            out.push(match depth {
                Some(d) => NoteBlock::Item(d, note_runs(&text)),
                None => NoteBlock::Para(note_runs(&text)),
            });
        }
    };
    let mut in_comment = false;
    for line in md.lines() {
        let t = line.trim();
        // HTML comments are left out (the changelog's notes to the maintainers)
        if in_comment || t.starts_with("<!--") {
            in_comment = !t.contains("-->");
            continue;
        }
        if t.is_empty() {
            flush(&mut cur, &mut out);
            continue;
        }
        if t.starts_with('#') {
            flush(&mut cur, &mut out);
            let level = t.chars().take_while(|c| *c == '#').count();
            if level >= 2 {
                out.push(NoteBlock::Heading(note_runs(t.trim_start_matches('#').trim())));
            }
            continue;
        }
        if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut cur, &mut out);
            let indent = line.len() - line.trim_start().len();
            cur = Some((Some(indent / 2), item.trim().to_string()));
            continue;
        }
        match &mut cur {
            Some((_, text)) => {
                text.push(' ');
                text.push_str(t);
            }
            None => cur = Some((None, t.to_string())),
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn notes_ui(ui: &mut egui::Ui, md: &str) {
    let text = ui.visuals().text_color();
    let strong = ui.visuals().strong_text_color();
    // a LayoutJob is not translated: the notes stay in the language they were written in
    let job = |runs: &[(String, bool)], size: f32, base: Color32| {
        let mut job = egui::text::LayoutJob::default();
        for (t, bold) in runs {
            let color = if *bold { strong } else { base };
            job.append(t, 0.0, egui::TextFormat { font_id: egui::FontId::proportional(size), color, ..Default::default() });
        }
        job
    };
    ui.spacing_mut().item_spacing.y = 3.0;
    for (i, b) in note_blocks(md).iter().enumerate() {
        match b {
            NoteBlock::Heading(runs) => {
                if i > 0 {
                    ui.add_space(6.0);
                }
                ui.add(egui::Label::new(job(runs, 13.0, strong)).wrap());
            }
            NoteBlock::Para(runs) => {
                ui.add(egui::Label::new(job(runs, 12.0, text)).wrap());
            }
            NoteBlock::Item(depth, runs) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0 + 14.0 * *depth as f32);
                    ui.label(egui::text::LayoutJob::simple_singleline(if *depth == 0 { "•".into() } else { "◦".into() }, egui::FontId::proportional(12.0), text));
                    ui.add(egui::Label::new(job(runs, 12.0, text)).wrap());
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_read_as_github_shows_them() {
        let md = "# AnyFont 0.3.0

<!-- note to the maintainers
spanning lines -->
Intro that wraps
onto a second line.

## What's `new`

- **Get More.** A [catalog](https://fonts.google.com)
  of free fonts:
  - search
- Last
";
        let b = note_blocks(md);
        let run = |t: &str, bold: bool| (t.to_string(), bold);
        assert_eq!(b, vec![
            NoteBlock::Para(vec![run("Intro that wraps onto a second line.", false)]),
            NoteBlock::Heading(vec![run("What's new", false)]),
            NoteBlock::Item(0, vec![run("Get More.", true), run(" A catalog of free fonts:", false)]),
            NoteBlock::Item(1, vec![run("search", false)]),
            NoteBlock::Item(0, vec![run("Last", false)]),
        ]);
    }

    #[test]
    fn versions_compare() {
        assert_eq!(parse_version("v1.2.3"), [1, 2, 3]);
        assert_eq!(parse_version("0.2"), [0, 2, 0]);
        assert_eq!(parse_version("V0.3.1-beta.2"), [0, 3, 1]);
        assert!(is_newer("v0.1.1", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    fn release_json(tag: &str, asset_url: &str) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "body": "## Fixes\n\n- Faster scans",
            "html_url": format!("https://github.com/toim22/AnyFont/releases/tag/{tag}"),
            "assets": [
                { "name": "AnyFont-9.0.0-win-x64.zip", "browser_download_url": "https://x/AnyFont-9.0.0-win-x64.zip" },
                { "name": "AnyFont-9.0.0-Setup.exe", "browser_download_url": asset_url, "size": 4 }
            ]
        })
    }

    #[test]
    fn release_json_needs_a_setup() {
        let r = release_from_json(&release_json("v9.0.0", "https://x/AnyFont-9.0.0-setup.exe"), false).unwrap();
        assert_eq!(r.version, "9.0.0");
        assert_eq!(r.asset_name, "AnyFont-9.0.0-Setup.exe");
        assert_eq!(r.asset_size, 4);
        assert_eq!(r.page, "https://github.com/toim22/AnyFont/releases/tag/v9.0.0");
        let none = serde_json::json!({ "tag_name": "v0.2.0", "assets": [] });
        assert!(release_from_json(&none, false).is_none());
    }

    #[test]
    fn policies_follow_the_core_modes() {
        // the core has Never and Notify today; KeepUpToDate appears when the core adds it
        assert_eq!(Policy::of(UpdateMode::Never), Policy::Never);
        assert_eq!(Policy::Never.to_mode(), Some(UpdateMode::Never));
        assert!(Policy::Choose.to_mode().is_some());
        assert_eq!(Policy::of(Policy::Choose.to_mode().unwrap()), Policy::Choose);
        assert!(Policy::available().contains(&Policy::Never));
        for p in Policy::available() {
            assert_eq!(Policy::of(p.to_mode().unwrap()), p);
        }
    }

    #[test]
    fn setup_switches_match_releasing_md() {
        let doc = include_str!("../../../docs/RELEASING.md");
        assert!(doc.contains(&SETUP_ARGS.join(" ")));
    }

    /// The offline "update available" path QA uses: a local feed and a local setup file.
    #[test]
    fn local_feed_finds_and_copies_the_setup() {
        let dir = std::env::temp_dir().join(format!("anyfont-upd-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let setup = dir.join("AnyFont-9.0.0-setup.exe");
        std::fs::write(&setup, b"MZ!!").unwrap();
        let feed = dir.join("latest.json");
        std::fs::write(&feed, release_json("v9.0.0", &setup.to_string_lossy()).to_string()).unwrap();
        // only this test sets the variable
        std::env::set_var("ANYFONT_UPDATE_FEED", &feed);
        let r = check().unwrap().expect("9.0.0 is newer");
        std::env::remove_var("ANYFONT_UPDATE_FEED");
        assert_eq!(r.version, "9.0.0");
        let (tx, rx) = channel();
        let got = download(&r, &tx).unwrap();
        assert_eq!(std::fs::read(&got).unwrap(), b"MZ!!");
        assert!(rx.try_iter().any(|m| matches!(m, Msg::Progress(p) if p >= 1.0)));
        let _ = std::fs::remove_file(got);
        let _ = std::fs::remove_dir_all(dir);
    }
}
