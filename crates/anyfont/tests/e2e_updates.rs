//! Product updates: Help ▸ Check for Updates... and the title-bar Updates button (new version, up to date,
//! offline, Check Now, Install Now, the "Allow product updates" combo box) and the message the check at
//! start shows (OK, Later). The release feed (`ANYFONT_UPDATE_FEED`) is a local JSON file or a local HTTP
//! server; the temporary folder (where setups are downloaded and old ones removed) is the test's own; the
//! downloaded "setup" is a few dummy bytes and is never run (the window only hands its path to `main`).
mod common;
mod svc;

use anyfont::testing;
use anyfont::updater::{Status, Updater, UpdaterRequest};
use anyfont_core::settings::UpdateMode;
use common::http::{Response, Server};
use common::*;
use egui::accesskit::Role;
use egui::ViewportCommand;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use svc::*;

const SIZE: (f32, f32) = (1200.0, 900.0);
const NOTE: &str = "The test fix of this release.";

fn setup_bytes() -> Vec<u8> {
    b"MZ dummy setup for the AnyFont end-to-end tests, never run. ".repeat(2000)
}

fn feed(version: &str, page: &str, asset_url: &str, size: u64) -> serde_json::Value {
    serde_json::json!({
        "tag_name": format!("v{version}"),
        "body": format!("# AnyFont {version}\n\n## Fixes\n\n- {NOTE}\n"),
        "html_url": page,
        "assets": [
            { "name": "AnyFont-checksums.txt", "browser_download_url": "http://127.0.0.1:9/never", "size": 10 },
            { "name": format!("AnyFont-{version}-setup.exe"), "browser_download_url": asset_url, "size": size }
        ]
    })
}

/// A release feed served on this computer: (server, release page URL).
fn serve(version: &str) -> (Server, String) {
    let srv = Server::start();
    let setup = setup_bytes();
    let page = srv.url(&format!("/{}/releases/tag/v{version}", unique("rel")));
    let name = format!("/dl/AnyFont-{version}-setup.exe");
    srv.route("GET", "/latest", Response::json(200, &feed(version, &page, &srv.url(&name), setup.len() as u64)));
    srv.route("GET", &name, Response::bytes(setup));
    (srv, page)
}

/// Runs single frames until the window asked to exit and install, checking that the same frame closes the
/// window. Returns the setup handed to `main`.
fn wait_exit_and_install(h: &mut H<'_>, out: &parking_lot::Mutex<Option<PathBuf>>) -> PathBuf {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        h.step();
        if let Some(p) = out.lock().clone() {
            let cmds = viewport_commands(h);
            assert!(cmds.iter().any(|c| matches!(c, ViewportCommand::Close)), "the window closes to install: {cmds:?}");
            return p;
        }
        assert!(Instant::now() < end, "timed out waiting for Install Now; shown: {:?}", all_texts(h));
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn updates_dir(tmp: &Path) -> PathBuf {
    tmp.join("AnyFont Updates")
}

#[test]
fn newer_version_is_shown_downloaded_and_installed_after_exit() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let (srv, page) = serve("99.0.0");
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", &srv.url("/latest"));
    let fs = env.sandbox();
    let start = Start::new(&env, &fs);
    let out = start.updater_out.clone();
    let mut h = harness_start(start, SIZE);
    assert!(srv.requests_to("/latest").is_empty(), "no check at start when updates are off");
    click_menu(&mut h, "Help", "Check for Updates...");
    wait_text(&mut h, 20, "AnyFont 99.0.0 is available.");
    assert!(has_text(&h, &format!("You are using AnyFont {}.", env!("CARGO_PKG_VERSION"))));
    assert!(has_text(&h, NOTE), "release notes shown: {:?}", all_texts(&h));
    let req = &srv.requests_to("/latest")[0];
    assert_eq!(req.header("user-agent"), Some(format!("AnyFont/{}", env!("CARGO_PKG_VERSION")).as_str()));
    // Release Notes opens the release page (recorded, no browser)
    click_button(&mut h, "Release Notes");
    assert!(testing::urls().contains(&page), "{:?}", testing::urls());
    // Install Now: download, then close the window and hand the setup to main (which would run it)
    assert!(button_enabled(&h, "Install Now"));
    h.get_by_role_and_label(Role::Button, "Install Now").click();
    let setup = wait_exit_and_install(&mut h, &out);
    assert_eq!(setup, updates_dir(&tmp).join("AnyFont-99.0.0-setup.exe"));
    assert_eq!(std::fs::read(&setup).unwrap(), setup_bytes(), "the whole setup was downloaded");
    assert!(!setup.with_extension("part").exists(), "no partial file left");
    assert_eq!(srv.requests_to("/dl/AnyFont-99.0.0-setup.exe").len(), 1);
    assert!(srv.requests().iter().all(|r| r.path != "/never"), "only the setup asset is downloaded");
}

#[test]
fn title_bar_updates_button_up_to_date_removes_old_setups() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    // a local JSON feed with an older version
    let feed_file = env.path("feed.json");
    std::fs::write(&feed_file, serde_json::to_vec(&feed("0.0.1", "http://127.0.0.1:9/rel", "http://127.0.0.1:9/old.exe", 5)).unwrap()).unwrap();
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", &feed_file.to_string_lossy());
    // the setup of an earlier update, left in the download folder
    std::fs::create_dir_all(updates_dir(&tmp)).unwrap();
    std::fs::write(updates_dir(&tmp).join("AnyFont-0.0.9-setup.exe"), b"old").unwrap();
    let mut h = harness(env.config(), SIZE);
    h.get_by_role_and_label(Role::Button, "Updates").click();
    settle(&mut h);
    wait_text(&mut h, 20, "AnyFont is up to date.");
    assert!(buttons(&h, "Install Now").is_empty(), "nothing to install");
    assert!(!updates_dir(&tmp).exists(), "the setups of earlier versions are removed");
    click_button(&mut h, "Close");
    assert!(!has_text(&h, "AnyFont is up to date."), "Close closes the dialog");
}

#[test]
fn offline_shows_the_internet_error_and_check_now_retries() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", DEAD_URL);
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Check for Updates...");
    wait_text(&mut h, 30, "Error: An Internet connection is required. Please connect to the Internet and try again.");
    assert!(buttons(&h, "Install Now").is_empty());
    // the connection is back: Check Now finds the new version
    let (srv, _page) = serve("98.1.0");
    std::env::set_var("ANYFONT_UPDATE_FEED", srv.url("/latest"));
    click_button(&mut h, "Check Now");
    wait_text(&mut h, 20, "AnyFont 98.1.0 is available.");
    assert!(!has_text(&h, "Internet connection is required"));
}

#[test]
fn local_feed_installs_a_local_setup() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let local_setup = env.path("AnyFont-97.0.0-setup.exe");
    std::fs::write(&local_setup, setup_bytes()).unwrap();
    let feed_file = env.path("feed.json");
    let v = feed("97.0.0", "http://127.0.0.1:9/rel97", &local_setup.to_string_lossy(), setup_bytes().len() as u64);
    std::fs::write(&feed_file, serde_json::to_vec(&v).unwrap()).unwrap();
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", &feed_file.to_string_lossy());
    let fs = env.sandbox();
    let start = Start::new(&env, &fs);
    let out = start.updater_out.clone();
    let mut h = harness_start(start, SIZE);
    h.get_by_role_and_label(Role::Button, "Updates").click();
    settle(&mut h);
    wait_text(&mut h, 20, "AnyFont 97.0.0 is available.");
    h.get_by_role_and_label(Role::Button, "Install Now").click();
    let setup = wait_exit_and_install(&mut h, &out);
    assert_eq!(setup, updates_dir(&tmp).join("AnyFont-97.0.0-setup.exe"));
    assert_eq!(std::fs::read(&setup).unwrap(), setup_bytes());
    assert!(local_setup.exists(), "the source setup is copied, not moved");
}

#[test]
fn update_mode_combo_changes_the_settings() {
    let _lock = env_lock();
    let env = Env::new();
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", DEAD_URL);
    // the real start-up: the updater gets the saved mode (the test window does not pass it on)
    let fs = env.sandbox();
    let start = Start::new(&env, &fs);
    let settings_file = start.paths().settings_file;
    let mut h = harness_start(start, SIZE);
    click_menu(&mut h, "Help", "Check for Updates...");
    const COMBO: &str = "Allow product updates:";
    assert_eq!(combo_value(&h, COMBO), "Never check for updates", "the tests start with updates off");
    let cases = [
        ("Keep the product up to date (recommended)", UpdateMode::KeepUpToDate, "Automatically downloads product updates"),
        ("Check for updates and let me choose when to install them", UpdateMode::Notify, "you choose when to download and install it"),
        ("Never check for updates", UpdateMode::Never, "never connects to the Internet"),
    ];
    for (caption, mode, note) in cases {
        choose(&mut h, COMBO, caption);
        assert_eq!(h.state().settings().core.update_mode, mode, "{caption}");
        assert_eq!(combo_value(&h, COMBO), caption);
        assert!(has_text(&h, note), "note of {caption}: {:?}", all_texts(&h));
    }
    choose(&mut h, COMBO, "Keep the product up to date (recommended)");
    click_button(&mut h, "Close");
    assert_eq!(h.state().settings().core.update_mode, UpdateMode::KeepUpToDate, "the choice stays after Close");
    // and is saved (after a short delay)
    wait_until(&mut h, 10, "the settings to be saved", |_| {
        settings_file.exists() && anyfont::settings::AppSettings::load(&settings_file).core.update_mode == UpdateMode::KeepUpToDate
    });
}

// --- the message of the check at start (the app's harnesses start with updates off, so these tests drive
// the updater on its own, the way the window does: `on_startup`, then `ui` every frame) ---

struct U {
    u: Updater,
    out: Option<PathBuf>,
}

fn updater_harness(mode: UpdateMode) -> Harness<'static, U> {
    let mut h = Harness::builder().with_size(egui::vec2(900.0, 700.0)).with_max_steps(64).build_ui_state(
        |ui, s: &mut U| {
            if let Some(UpdaterRequest::ExitAndInstall(p)) = s.u.ui(ui.ctx()) {
                s.out = Some(p);
            }
        },
        U { u: Updater::new(), out: None },
    );
    let ctx = h.ctx.clone();
    h.state_mut().u.on_startup(&ctx, mode);
    h
}

const PROMPT: &str = "There is a new version of AnyFont available.";

#[test]
fn startup_message_later_is_not_shown_again() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let (srv, page) = serve("99.1.0");
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", &srv.url("/latest"));
    let mut h = updater_harness(UpdateMode::Notify);
    wait_text(&mut h, 20, PROMPT);
    assert!(has_text(&h, &format!("New version: 99.1.0. Your version: {}.", env!("CARGO_PKG_VERSION"))));
    assert!(has_text(&h, NOTE));
    for b in ["Release Notes", "Later", "OK"] {
        assert!(button_enabled(&h, b), "{b}: {:?}", button_labels(&h));
    }
    click_button(&mut h, "Release Notes");
    assert!(testing::urls().contains(&page));
    click_button(&mut h, "Later");
    assert!(!has_text(&h, PROMPT), "Later closes the message");
    assert!(!h.state().u.is_open());
    for _ in 0..10 {
        pump(&mut h);
    }
    assert!(!has_text(&h, PROMPT), "not shown again in this session");
    assert_eq!(srv.requests_to("/latest").len(), 1, "one check per start");
    assert!(srv.requests_to("/dl/AnyFont-99.1.0-setup.exe").is_empty(), "\"let me choose\" does not download by itself");
    assert!(h.state().out.is_none());
    // Check for Updates still knows the version, without checking again
    h.state_mut().u.open();
    pump(&mut h);
    assert!(has_text(&h, "AnyFont 99.1.0 is available."), "{:?}", all_texts(&h));
    assert_eq!(srv.requests_to("/latest").len(), 1);
}

#[test]
fn startup_message_keep_up_to_date_downloads_and_ok_installs() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let (srv, _page) = serve("99.2.0");
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", &srv.url("/latest"));
    let mut h = updater_harness(UpdateMode::KeepUpToDate);
    wait_text(&mut h, 20, PROMPT);
    // keeping up to date downloads while the message is shown
    wait_for(&mut h, 20, "the setup to be downloaded", |h| matches!(h.state().u.status, Status::Ready(..)));
    assert_eq!(srv.requests_to("/dl/AnyFont-99.2.0-setup.exe").len(), 1);
    assert!(h.state().out.is_none(), "nothing installs before OK");
    click_button(&mut h, "OK");
    let setup = h.state().out.clone().expect("OK asks to exit and install");
    assert_eq!(setup, updates_dir(&tmp).join("AnyFont-99.2.0-setup.exe"));
    assert_eq!(std::fs::read(&setup).unwrap(), setup_bytes());
    assert!(!has_text(&h, PROMPT));
}

#[test]
fn startup_check_is_quiet_when_offline_or_off() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let _f = EnvVar::set("ANYFONT_UPDATE_FEED", DEAD_URL);
    let mut h = updater_harness(UpdateMode::Notify);
    wait_for(&mut h, 30, "the quiet check to end", |h| !matches!(h.state().u.status, Status::Checking));
    pump(&mut h);
    assert!(matches!(h.state().u.status, Status::Idle), "a failed check at start stays quiet");
    assert!(!h.state().u.is_open());
    assert!(!has_text(&h, "Error:"));
    // "Never check for updates": no check at all
    let (srv, _page) = serve("99.3.0");
    std::env::set_var("ANYFONT_UPDATE_FEED", srv.url("/latest"));
    let mut h = updater_harness(UpdateMode::Never);
    for _ in 0..5 {
        pump(&mut h);
    }
    assert!(srv.requests().is_empty(), "never checks");
    assert!(!has_text(&h, PROMPT));
}
