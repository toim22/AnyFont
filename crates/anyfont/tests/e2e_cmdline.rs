//! The command line, as `main.rs` handles it: the arguments are parsed with `anyfont_sys::CommandLine`, the
//! file arguments (`forwardable_files`), `/Lang:` and `/NoSplash` go into the start options of the real
//! start-up path (`harness_start`), unknown switches are ignored; and a second instance forwards its files to
//! the first one through the single-instance pipe (with a test-only instance name, never "AnyFont").
//!
//! The UI language is process-wide: every window of this binary is built under `env_lock`.
mod common;

use anyfont_sys::single_instance::{self, Instance, Message};
use anyfont_sys::{CommandLine, FontSystem};
use common::*;
use egui::accesskit::Role;
use egui::ViewportCommand;
use egui_kittest::kittest::{NodeT, Queryable};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SIZE: (f32, f32) = (1280.0, 800.0);

/// A `Start` filled from a command line the way `main.rs` fills its `StartOptions`.
fn start_from(env: &Env, fs: &std::sync::Arc<anyfont_sys::SandboxFontSystem>, args: &[&str]) -> Start {
    let cl = CommandLine::parse(args.iter().copied());
    let mut start = Start::new(env, fs);
    start.files = cl.forwardable_files();
    start.lang = cl.lang().map(str::to_string);
    start.splash = !cl.no_splash();
    start
}

/// `harness_start` without its first frames run in bulk, and with 1 ms of simulated time per frame (the
/// splash stays at least 1.2 s): the caller sees every frame, the splash's end included.
fn start_frame_by_frame<'a>(start: Start, size: (f32, f32)) -> H<'a> {
    safe_env();
    let paths = start.paths();
    let mut settings = start.settings.unwrap_or_else(|| anyfont::settings::AppSettings::load(&paths.settings_file));
    settings.core.update_mode = anyfont_core::settings::UpdateMode::Never;
    let fs: std::sync::Arc<dyn anyfont_sys::FontSystem> = start.fs.clone();
    let opts = anyfont::app::StartOptions {
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
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_max_steps(8)
        .with_step_dt(0.001)
        .wgpu()
        .build_eframe(move |cc| anyfont::AnyFontApp::new(cc, opts))
}

fn selected_rows(h: &H<'_>) -> Vec<String> {
    h.query_all(egui_kittest::kittest::by().role(Role::ListBoxOption))
        .filter(|n| n.accesskit_node().toggled().is_some_and(|t| t == egui::accesskit::Toggled::True))
        // (the font list, not the libraries of the left panel)
        .filter(|n| n.rect().left() > 240.0)
        .filter_map(|n| n.accesskit_node().label())
        .collect()
}

fn texts_painted(h: &H<'_>) -> Vec<String> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

fn folder_name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// Waits until the library `name` is open with `count` fonts.
fn wait_for_library(h: &mut H<'_>, name: &str, count: usize) {
    let want = format!("{name}: {count} fonts");
    wait_until(h, 30, &format!("status {want:?}"), |h| !h.state().operation_running() && h.state().status_text() == want);
    settle(h);
}

#[test]
fn the_command_line_is_parsed_like_the_original() {
    let cl = CommandLine::parse(["/NoSplash", "/Lang:pt-BR", "/Bogus", "--weird:1", "-x", "C:\\Fonts\\a.ttf", "\"C:\\My Fonts\\b.otf\"", "rel\\c.ttf", "/"]);
    assert!(cl.no_splash());
    assert_eq!(cl.lang(), Some("pt-BR"));
    let unknown: Vec<&str> = cl.unknown_switches().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(unknown, ["Bogus", "weird", "x"], "unknown switches (ignored by main.rs)");
    assert_eq!(cl.files, [PathBuf::from("C:\\Fonts\\a.ttf"), PathBuf::from("C:\\My Fonts\\b.otf"), PathBuf::from("rel\\c.ttf")]);
    let fwd = cl.forwardable_files();
    assert_eq!(fwd[0], PathBuf::from("C:\\Fonts\\a.ttf"));
    assert_eq!(fwd[2], std::env::current_dir().unwrap().join("rel\\c.ttf"), "relative files are made absolute");
    let msg = Message::from_command_line(&cl);
    assert_eq!(msg.files, fwd);
    assert!(msg.switches.contains(&"/Lang:pt-BR".to_string()) && msg.switches.contains(&"/NoSplash".to_string()), "{:?}", msg.switches);
    // case-insensitive switches, the later one wins
    let cl = CommandLine::parse(["/nosplash", "/LANG:de-DE", "/lang:fr-FR"]);
    assert!(cl.no_splash());
    assert_eq!(cl.lang(), Some("fr-FR"));
    assert!(cl.unknown_switches().is_empty());
    // no splash switch: the splash
    assert!(!CommandLine::parse(["x.ttf"]).no_splash());
}

#[test]
fn a_font_file_argument_adds_its_folder_and_selects_the_font() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let dir = env.font_folder("argfonts", &["georgia.ttf", "georgiab.ttf", "verdana.ttf"]);
    let file = dir.join("georgiab.ttf");
    let installed = fs.installed_fonts().unwrap().len();
    let start = start_from(&env, &fs, &["/NoSplash", "/UnknownSwitch:42", "-whatever", file.to_str().unwrap()]);
    assert!(!start.splash);
    let mut h = harness_start(start, SIZE);
    wait_for_library(&mut h, "argfonts", 3);
    assert!(h.state().library_names().contains(&"argfonts".to_string()), "{:?}", h.state().library_names());
    wait_until(&mut h, 10, "Georgia Bold selected", |h| selected_rows(h) == ["Georgia (Bold)"]);
    assert_eq!(fs.installed_fonts().unwrap().len(), installed, "opening a file installs nothing");
    eframe::App::on_exit(h.state_mut());
}

#[test]
fn a_folder_argument_opens_the_folder() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let dir = env.font_folder("argfolder", &["georgia.ttf", "verdana.ttf"]);
    let start = start_from(&env, &fs, &["/NoSplash", dir.to_str().unwrap()]);
    let mut h = harness_start(start, SIZE);
    wait_for_library(&mut h, &folder_name(&dir), 2);
    assert_eq!(selected_rows(&h), ["Georgia (Regular, Normal)"], "a folder: its first font is selected");
    eframe::App::on_exit(h.state_mut());
}

#[test]
fn the_same_folder_twice_is_not_added_twice() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let dir = env.font_folder("twice", &["georgia.ttf", "georgiab.ttf"]);
    let start = start_from(&env, &fs, &["/NoSplash", dir.join("georgia.ttf").to_str().unwrap(), dir.join("georgiab.ttf").to_str().unwrap()]);
    let mut h = harness_start(start, SIZE);
    wait_for_library(&mut h, "twice", 2);
    eframe::App::on_exit(h.state_mut());
    drop(h);
    // the next start with the folder again: the existing folder is opened
    let start = start_from(&env, &fs, &["/NoSplash", dir.to_str().unwrap()]);
    let mut h = harness_start(start, SIZE);
    wait_for_library(&mut h, "twice", 2);
    assert_eq!(h.state().library_names().iter().filter(|n| *n == "twice").count(), 1, "{:?}", h.state().library_names());
    eframe::App::on_exit(h.state_mut());
}

#[test]
fn the_splash_turns_into_the_main_window() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let start = start_from(&env, &fs, &["/Ignored:yes"]);
    assert!(start.splash, "no /NoSplash: the splash");
    let mut h = start_frame_by_frame(start, (640.0, 360.0));
    // the splash: no menu bar, the loading status
    assert!(h.query_by(|n| n.role() == Role::MenuItem || n.label().as_deref() == Some("File")).is_none(), "no menu bar under the splash");
    let painted = texts_painted(&h).join(" | ");
    assert!(painted.contains("Initializing...") || painted.contains("Loading fonts..."), "{painted}");
    assert!(painted.contains(env!("CARGO_PKG_VERSION")), "the version on the splash: {painted}");
    // (the placement the main window gets: the saved one – the defaults here; read before the main window
    // starts tracking the real window)
    let w = h.state().settings().core.window.clone();
    let mut cmds: Vec<ViewportCommand> = Vec::new();
    let mut frames = 0;
    let end = Instant::now() + Duration::from_secs(60);
    while !cmds.iter().any(|c| matches!(c, ViewportCommand::InnerSize(_))) {
        assert!(Instant::now() < end, "the splash never ended ({frames} frames)");
        h.step();
        frames += 1;
        cmds.extend(viewport_commands(&h));
        if frames % 50 == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert!(frames >= 1000, "the splash stays at least 1.2 s ({frames} frames of 1 ms)");
    // the start window becomes the main window (restore_window, from the splash)
    assert!(cmds.contains(&ViewportCommand::Resizable(true)), "{cmds:?}");
    assert!(cmds.contains(&ViewportCommand::MinInnerSize(egui::vec2(800.0, 600.0))), "{cmds:?}");
    assert!(cmds.contains(&ViewportCommand::InnerSize(egui::vec2(w.size[0].max(800.0), w.size[1].max(600.0)))), "{cmds:?}");
    assert_eq!(cmds.contains(&ViewportCommand::Maximized(true)), w.maximized, "{cmds:?}");
    h.set_size(egui::vec2(SIZE.0, SIZE.1));
    settle(&mut h);
    assert!(h.query_by_label("File").is_some(), "the menu bar after the splash");
    eframe::App::on_exit(h.state_mut());
}

#[test]
fn no_splash_starts_with_the_main_window() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let start = start_from(&env, &fs, &["/NoSplash"]);
    let mut h = start_frame_by_frame(start, SIZE);
    assert!(h.query_by_label("File").is_some(), "the menu bar from the first frame");
    let mut cmds = viewport_commands(&h);
    for _ in 0..300 {
        h.step();
        cmds.extend(viewport_commands(&h));
    }
    assert!(!cmds.iter().any(|c| matches!(c, ViewportCommand::InnerSize(_) | ViewportCommand::Resizable(_))), "no splash-to-main resize: {cmds:?}");
    eframe::App::on_exit(h.state_mut());
}

#[test]
fn lang_switch_sets_the_language_without_saving_it() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let start = start_from(&env, &fs, &["/NoSplash", "/Lang:pt-BR"]);
    assert_eq!(start.lang.as_deref(), Some("pt-BR"));
    let file = start.paths().settings_file;
    let mut h = harness_start(start, SIZE);
    assert_eq!(anyfont::i18n::lang(), anyfont::i18n::Lang::PtBr);
    assert!(h.query_by_label("Fonte").is_some() && h.query_by_label("Exibir").is_some(), "the menu bar in Portuguese");
    assert!(h.query_by_label("View").is_none());
    eframe::App::on_exit(h.state_mut());
    let saved = anyfont::settings::AppSettings::load(&file);
    assert_eq!(saved.core.language, None, "/Lang: is for this run only");
    drop(h);
    // the next start without /Lang: is in the language of the settings (en-US here)
    let start = start_from(&env, &fs, &["/NoSplash", "/Lang:en-US"]);
    let h = harness_start(start, SIZE);
    assert_eq!(anyfont::i18n::lang(), anyfont::i18n::Lang::EnUs);
    assert!(h.query_by_label("View").is_some());
}

/// A unique single-instance name for this test run (never "AnyFont": a real AnyFont may be running).
fn test_app_name(tag: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("AnyFont-e2e-{tag}-{}-{nanos}", std::process::id())
}

#[test]
fn a_second_instance_forwards_its_files_to_the_first() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let dir = env.font_folder("forwarded", &["georgia.ttf", "georgiab.ttf", "verdana.ttf"]);
    let name = test_app_name("fwd");

    // the first instance: primary, listening; its messages go to the window like in main.rs
    let guard = match single_instance::acquire(&name).expect("acquire") {
        Instance::Primary(g) => g,
        Instance::Secondary => panic!("a fresh name must make the first instance primary"),
    };
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    guard.listen(move |m| {
        let _ = tx.send(m);
    })
    .expect("listen");
    let mut start = start_from(&env, &fs, &["/NoSplash"]);
    start.incoming = Some(rx);
    let mut h = harness_start(start, SIZE);
    wait_until(&mut h, 30, "the first scan", |h| !h.state().operation_running() && h.state().font_count() > 0);
    settle(&mut h);
    let fonts_before = h.state().status_text();

    // the second instance: secondary, forwards its command line and would exit
    assert!(matches!(single_instance::acquire(&name).expect("acquire 2"), Instance::Secondary), "a second acquire sees the first instance");
    let cl = CommandLine::parse(["/NoSplash", "/SomethingUnknown", dir.join("verdana.ttf").to_str().unwrap()]);
    let msg = Message::from_command_line(&cl);
    let t = std::thread::spawn({
        let name = name.clone();
        move || single_instance::forward_to_primary(&name, &msg, Duration::from_secs(10))
    });
    t.join().unwrap().expect("forwarded to the primary instance");

    // the first window opens the folder and selects the font
    wait_for_library(&mut h, "forwarded", 3);
    wait_until(&mut h, 10, "Verdana selected", |h| selected_rows(h).len() == 1 && selected_rows(h)[0].starts_with("Verdana"));
    assert_ne!(h.state().status_text(), fonts_before);
    let cmds = viewport_commands(&h);
    let _ = cmds; // (Focus / Minimized(false) were sent in an earlier frame)
    eframe::App::on_exit(h.state_mut());
    drop(h);

    // the first instance gone: the name is free again
    drop(guard);
    match single_instance::acquire(&name).expect("acquire 3") {
        Instance::Primary(_) => {}
        Instance::Secondary => panic!("the name is still taken after the primary instance closed"),
    }
}

#[test]
fn a_forwarded_message_brings_the_window_forward() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    let mut start = start_from(&env, &fs, &["/NoSplash"]);
    start.incoming = Some(rx);
    let mut h = harness_start(start, SIZE);
    wait_until(&mut h, 30, "the first scan", |h| !h.state().operation_running());
    settle(&mut h);
    // a message without files (the second instance was started with no arguments)
    tx.send(Message { files: vec![], switches: vec!["/NoSplash".into()] }).unwrap();
    h.step();
    let cmds = viewport_commands(&h);
    assert!(cmds.contains(&ViewportCommand::Minimized(false)) && cmds.contains(&ViewportCommand::Focus), "{cmds:?}");
    eframe::App::on_exit(h.state_mut());
}
