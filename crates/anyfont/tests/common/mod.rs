//! Shared set-up of the end-to-end tests: every test runs the real window (`AnyFontApp`) on a sandbox font
//! system and a data folder inside a temporary directory, in en-US, with the scan on the UI thread. Nothing
//! touches the fonts of this PC or the user's AnyFont profile (`anyfont-sys` is built with
//! `forbid-real-writes` for the tests), nothing opens a browser, Explorer or a file dialog
//! (`anyfont::testing` records those requests), and every network address the app knows is redirected to
//! this computer (`safe_env`).
#![allow(dead_code)]

pub mod http;

use anyfont::app::StartOptions;
use anyfont::settings::{AppSettings, Paths};
use anyfont::{AnyFontApp, TestConfig};
use anyfont_sys::{FontSystem, SandboxConfig, SandboxFontSystem};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard, Once};

pub type H<'a> = Harness<'a, AnyFontApp>;

/// A local address nothing listens on: "the internet is down".
pub const DEAD_URL: &str = "http://127.0.0.1:9/offline";

/// Points every network address of the app at this computer (unless a test set one), and turns on the
/// interception of browser / Explorer / printer requests. Called by every harness constructor.
pub fn safe_env() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for var in ["ANYFONT_UPDATE_FEED", "ANYFONT_ISSUE_RELAY", "ANYFONT_CATALOG_URL", "ANYFONT_HELP_URL"] {
            if std::env::var_os(var).is_none() {
                std::env::set_var(var, DEAD_URL);
            }
        }
        std::env::remove_var("ANYFONT_SANDBOX_OPEN_EXPLORER");
    });
    anyfont::testing::enable();
}

/// Serialises the tests of one test binary that change process-wide state (environment variables,
/// the global language). Poisoning by a failed test is ignored.
pub fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sets an environment variable for the rest of the current test (restored to the safe default on drop).
pub struct EnvVar(&'static str);
impl EnvVar {
    pub fn set(name: &'static str, value: &str) -> EnvVar {
        safe_env();
        std::env::set_var(name, value);
        EnvVar(name)
    }
}
impl Drop for EnvVar {
    fn drop(&mut self) {
        std::env::set_var(self.0, DEAD_URL);
    }
}

pub struct Env {
    pub dir: tempfile::TempDir,
}

impl Env {
    pub fn new() -> Env {
        safe_env();
        Env { dir: tempfile::Builder::new().prefix("anyfont-e2e-").tempdir().expect("temp dir") }
    }
    pub fn root(&self) -> PathBuf {
        self.dir.path().join("sandbox")
    }
    pub fn config(&self) -> TestConfig {
        TestConfig::new(&self.root())
    }
    /// Opens the sandbox font system of this environment (default seed: protected copies of a few
    /// `%WINDIR%\Fonts` files). Keep the `Arc` to use the sandbox's test controls, and pass it to the window
    /// with [`Env::config_with`].
    pub fn sandbox(&self) -> Arc<SandboxFontSystem> {
        Arc::new(SandboxFontSystem::open(self.root(), SandboxConfig::default()).expect("sandbox"))
    }
    /// A sandbox seeded with these files (names in `%WINDIR%\Fonts` or full paths).
    pub fn sandbox_seeded(&self, seed: &[&str]) -> Arc<SandboxFontSystem> {
        let cfg = SandboxConfig { seed: seed.iter().map(PathBuf::from).collect() };
        Arc::new(SandboxFontSystem::open(self.root(), cfg).expect("sandbox"))
    }
    /// The test config using an already opened sandbox.
    pub fn config_with(&self, fs: &Arc<SandboxFontSystem>) -> TestConfig {
        let mut c = self.config();
        let sys: Arc<dyn FontSystem> = fs.clone();
        c.system = Some(sys);
        c
    }
    /// A folder of font files copied from `%WINDIR%\Fonts` (read only there; missing files are skipped).
    pub fn font_folder(&self, name: &str, files: &[&str]) -> PathBuf {
        let dir = self.dir.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            let src = windows_fonts().join(f);
            if src.exists() {
                std::fs::copy(&src, dir.join(f)).unwrap();
            }
        }
        dir
    }
    /// A folder of fixture fonts: names of `crates/anyfont-core/tests/fixtures` files (`Hack-Regular.ttf`,
    /// `TTC.ttc`, `colr_1_variable.ttf`, `Tuffy.ttf`), of `epaint_default_fonts` (`Ubuntu-Light.ttf`,
    /// `NotoEmoji-Regular.ttf`, `emoji-icon-font.ttf`) or of `%WINDIR%\Fonts`. Panics if a fixture is missing
    /// (the `%WINDIR%` ones are skipped when absent).
    pub fn fixture_folder(&self, name: &str, files: &[&str]) -> PathBuf {
        let dir = self.dir.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            let src = fixture(f);
            if let Some(src) = src {
                std::fs::copy(&src, dir.join(Path::new(f).file_name().unwrap())).unwrap();
            }
        }
        dir
    }
    /// A path inside the temporary folder (not created).
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

pub fn windows_fonts() -> PathBuf {
    std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows")).join("Fonts")
}

/// The core crate's fixtures folder.
pub fn core_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../anyfont-core/tests/fixtures")
}

/// The `fonts` folder of the `epaint_default_fonts` crate in the cargo registry (the newest version).
pub fn epaint_fonts() -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))?;
    let mut best: Option<PathBuf> = None;
    for idx in std::fs::read_dir(home.join("registry").join("src")).ok()?.flatten() {
        for e in std::fs::read_dir(idx.path()).ok()?.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("epaint_default_fonts-") && e.path().join("fonts").is_dir() {
                if best.as_ref().map(|b| b.to_string_lossy() < e.path().to_string_lossy()).unwrap_or(true) {
                    best = Some(e.path());
                }
            }
        }
    }
    best.map(|b| b.join("fonts"))
}

/// Finds a fixture font by name (see [`Env::fixture_folder`]). `None` only for absent `%WINDIR%` fonts.
pub fn fixture(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    let core = core_fixtures().join(name);
    if core.exists() {
        return Some(core);
    }
    if let Some(e) = epaint_fonts() {
        if e.join(name).exists() {
            return Some(e.join(name));
        }
    }
    let w = windows_fonts().join(name);
    if w.exists() {
        return Some(w);
    }
    if ["Hack-Regular.ttf", "TTC.ttc", "colr_1_variable.ttf", "Tuffy.ttf", "Ubuntu-Light.ttf", "NotoEmoji-Regular.ttf", "emoji-icon-font.ttf"].contains(&name) {
        panic!("fixture font {name} not found");
    }
    None
}

pub fn harness<'a>(cfg: TestConfig, size: (f32, f32)) -> H<'a> {
    safe_env();
    let mut h = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_max_steps(64)
        .wgpu()
        .build_eframe(move |cc| AnyFontApp::new_for_test(cc, cfg));
    settle(&mut h);
    h
}

/// What a test passes to the real start-up path (`AnyFontApp::new`, as `main.rs` does), on a sandbox.
pub struct Start {
    pub fs: Arc<SandboxFontSystem>,
    pub data_dir: PathBuf,
    pub settings: Option<AppSettings>,
    pub files: Vec<PathBuf>,
    pub lang: Option<String>,
    pub splash: bool,
    pub incoming: Option<std::sync::mpsc::Receiver<anyfont_sys::single_instance::Message>>,
    pub updater_out: Arc<Mutex<Option<PathBuf>>>,
}

impl Start {
    /// `/NoSplash`, en-US, update checks off, the settings saved in `env`'s data folder (if any).
    pub fn new(env: &Env, fs: &Arc<SandboxFontSystem>) -> Start {
        Start {
            fs: fs.clone(),
            data_dir: env.root().join("anyfont-data"),
            settings: None,
            files: Vec::new(),
            lang: Some("en-US".into()),
            splash: false,
            incoming: None,
            updater_out: Arc::new(Mutex::new(None)),
        }
    }
    pub fn paths(&self) -> Paths {
        Paths::in_dir(&self.data_dir, self.fs.paths().content_fonts_dir.clone())
    }
}

/// Builds the window through `AnyFontApp::new` (the start-up of `main.rs`: settings loaded from the data
/// folder, `/Lang:`, splash, files, messages from later instances). The scan runs on its thread: the
/// harness waits for it.
pub fn harness_start<'a>(start: Start, size: (f32, f32)) -> H<'a> {
    safe_env();
    let paths = start.paths();
    let mut settings = start.settings.unwrap_or_else(|| AppSettings::load(&paths.settings_file));
    settings.core.update_mode = anyfont_core::settings::UpdateMode::Never;
    let fs: Arc<dyn FontSystem> = start.fs.clone();
    let opts = StartOptions {
        fs,
        paths,
        settings,
        files: start.files,
        lang: start.lang,
        splash: start.splash,
        crash_report: None,
        incoming: start.incoming,
        updater_out: start.updater_out,
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_max_steps(64)
        .wgpu()
        .build_eframe(move |cc| AnyFontApp::new(cc, opts));
    settle(&mut h);
    h
}

/// Runs a few frames (the app keeps asking for repaints while previews load, so `run` may not settle).
pub fn settle(h: &mut H<'_>) {
    for _ in 0..4 {
        h.run_steps(4);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

/// Runs frames until `cond` holds (or panics after `secs` seconds with `what`).
pub fn wait_until(h: &mut H<'_>, secs: u64, what: &str, mut cond: impl FnMut(&H<'_>) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        h.run_steps(2);
        if cond(h) {
            return;
        }
        if std::time::Instant::now() > end {
            panic!("timed out waiting for: {what}");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Waits for a font operation (install, uninstall, delete) to finish.
pub fn finish_operation(h: &mut H<'_>) {
    wait_until(h, 30, "the font operation to finish", |h| !h.state().operation_running());
    settle(h);
}

/// Opens a menu of the menu bar.
pub fn open_menu(h: &mut H<'_>, menu: &str) {
    h.get_by_role_and_label(Role::Button, menu).click();
    settle(h);
}

/// Opens a menu of the menu bar and clicks one of its items.
pub fn click_menu(h: &mut H<'_>, menu: &str, item: &str) {
    open_menu(h, menu);
    menu_item(h, item).click();
    settle(h);
}

/// A menu item (plain or check item) by its label.
pub fn menu_item<'t>(h: &'t H<'_>, item: &'t str) -> egui_kittest::Node<'t> {
    h.get_by(move |n| n.label().as_deref() == Some(item) && matches!(n.role(), Role::MenuItem | Role::MenuItemCheckBox))
}

/// A menu item if it is shown.
pub fn query_menu_item<'t>(h: &'t H<'_>, item: &'t str) -> Option<egui_kittest::Node<'t>> {
    h.query_by(move |n| n.label().as_deref() == Some(item) && matches!(n.role(), Role::MenuItem | Role::MenuItemCheckBox))
}

/// The items of the open popup menus: (label, enabled, checked), in tree order.
pub fn open_menu_items(h: &H<'_>) -> Vec<(String, bool, bool)> {
    h.query_all(egui_kittest::kittest::by().predicate(|n| matches!(n.role(), Role::MenuItem | Role::MenuItemCheckBox)))
        .map(|n| {
            let a = n.accesskit_node();
            (a.label().unwrap_or_default(), !a.is_disabled(), a.role() == Role::MenuItemCheckBox && a.toggled() == Some(egui::accesskit::Toggled::True))
        })
        .collect()
}

/// Closes every open menu.
pub fn close_menus(h: &mut H<'_>) {
    for _ in 0..3 {
        if open_menu_items(h).is_empty() {
            break;
        }
        h.key_press(egui::Key::Escape);
        settle(h);
    }
}

/// The viewport commands the app sent in the last frame of the harness.
pub fn viewport_commands(h: &H<'_>) -> Vec<egui::ViewportCommand> {
    h.output().viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default()
}

/// Clicks a node and returns the viewport commands the click produced (the frame of the release).
pub fn click_for_commands(h: &mut H<'_>, node: impl for<'t> FnOnce(&'t H<'_>) -> egui_kittest::Node<'t>) -> Vec<egui::ViewportCommand> {
    node(h).click();
    h.step();
    viewport_commands(h)
}

/// The labels of every node of the accessibility tree with this role.
pub fn labels_of(h: &H<'_>, role: Role) -> Vec<String> {
    h.query_all(egui_kittest::kittest::by().role(role)).filter_map(|n| n.accesskit_node().label()).collect()
}

/// Renders the window to `target/ui-compare/<name>.png` (comparison screenshots, not committed).
pub fn screenshot(h: &mut H<'_>, name: &str) -> Option<PathBuf> {
    let img = match h.render() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("render failed: {e}");
            return None;
        }
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-compare");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{name}.png"));
    img.save(&path).ok()?;
    Some(path)
}
