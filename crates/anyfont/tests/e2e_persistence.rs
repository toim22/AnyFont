//! What survives a restart: two windows built one after the other through the real start-up path
//! (`harness_start`, `AnyFontApp::new` as `main.rs` does) on the same data folder and sandbox. The first one is
//! changed through its user interface and closed with `eframe::App::on_exit` (what eframe calls when the
//! window closes); the second one must come back the same: settings (theme, scale, group by family, preview
//! size, sample text and its history, font properties, filter list, left panel width, last library), window
//! placement, collections and folders. Also: a change is written to `settings.json` about 800 ms later even
//! without closing the window.
mod common;

use common::*;
use egui::accesskit::Role;
use egui::{pos2, Event, Key, Modifiers, PointerButton, Pos2, ViewportCommand};
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};

const SIZE: (f32, f32) = (1280.0, 800.0);
const COLLECTION: &str = "Collection 1";

/// `harness_start` without its first frames run in bulk, and with 1 ms of simulated time per frame (the
/// splash stays at least 1.2 s): the caller sees every frame's viewport commands, the splash's end included.
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

fn open_library(h: &mut H<'_>, name: &str) {
    h.get_by_role_and_label(Role::ListBoxOption, name).click();
    settle(h);
}

fn settings_file(start: &Start) -> std::path::PathBuf {
    start.paths().settings_file
}

/// Presses and drags the primary button from `from` to `to` (raw pointer events, one frame each).
fn drag(h: &mut H<'_>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for i in 1..=4 {
        let p = from + (to - from) * (i as f32 / 4.0);
        h.event(Event::PointerMoved(p));
        h.step();
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    settle(h);
}

/// Clicks a point (raw pointer events).
fn click_at(h: &mut H<'_>, p: Pos2) {
    h.event(Event::PointerMoved(p));
    h.step();
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    settle(h);
}

/// Types `text` into the sample text box and presses Enter (the box then loses focus: the text goes into the
/// history).
fn enter_sample(h: &mut H<'_>, text: &str) {
    h.get_by_role_and_label(Role::TextInput, "Enter sample text").click();
    settle(h);
    h.event(Event::Key { key: Key::A, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    h.event(Event::Key { key: Key::A, pressed: false, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    h.step();
    h.event(Event::Text(text.to_string()));
    h.step();
    h.key_press(Key::Enter);
    settle(h);
}

/// The preview size slider of the status bar (the lowest slider of the window).
fn preview_slider_rect(h: &H<'_>) -> egui::Rect {
    h.query_all(egui_kittest::kittest::by().role(Role::Slider))
        .map(|n| n.rect())
        .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
        .expect("the preview size slider")
}

/// Everything the first window changes, through its user interface.
fn change_everything(h: &mut H<'_>, folder: &std::path::Path) {
    // a folder and a collection with a font in it
    h.state_mut().add_folder_path(folder);
    settle(h);
    wait_until(h, 20, "the folder's scan", |h| !h.state().operation_running());
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(h);
    h.key_press(Key::Enter);
    settle(h);
    assert!(h.state().library_names().iter().any(|n| n == COLLECTION), "{:?}", h.state().library_names());
    open_library(h, "My fonts");
    let first = h.state().row_labels().into_iter().find(|l| l.contains("(Regular, Normal)")).expect("a face row");
    h.get_by_role_and_label(Role::ListBoxOption, &first).click();
    settle(h);
    open_menu(h, "Font");
    menu_item(h, "Add to collection").click();
    settle(h);
    menu_item(h, COLLECTION).click();
    settle(h);
    // sample text and its history
    enter_sample(h, "Persisted sample one");
    enter_sample(h, "Persisted sample two");
    // View toggles: group by family off, filter list hidden, font properties shown
    click_menu(h, "View", "Group by family");
    settle(h);
    click_menu(h, "View", "Hide filter list");
    settle(h);
    h.get_by_role_and_label(Role::Button, "Show font properties").click();
    settle(h);
    // the preview size: a click near the right end of the status bar slider
    let r = preview_slider_rect(h);
    click_at(h, pos2(r.left() + r.width() * 0.8, r.center().y));
    // the left panel 80 px wider (its splitter is its right edge)
    let lw = h.state().settings().core.window.left_panel_width;
    drag(h, pos2(lw + 0.5, 450.0), pos2(lw + 80.5, 450.0));
    // the last library: the collection
    open_library(h, COLLECTION);
    // Options: theme Light, scale 150 % (the middle tick of the slider), OK
    h.get_by_role_and_label(Role::Button, "Options").click();
    settle(h);
    let theme = h
        .query_by(|n| n.role() == Role::ComboBox && n.value().as_deref() == Some("Dark"))
        .unwrap_or_else(|| panic!("the Theme combo box; combo boxes: {:?}", h.query_all(egui_kittest::kittest::by().role(Role::ComboBox)).map(|n| (n.accesskit_node().label(), n.accesskit_node().value())).collect::<Vec<_>>()));
    theme.click();
    settle(h);
    h.get_by_label("Light").click();
    settle(h);
    let size = h.get_by_role_and_label(Role::Slider, "Size").rect();
    click_at(h, size.center());
    h.get_by_role_and_label(Role::Button, "OK").click();
    settle(h);
}

#[test]
fn settings_library_and_window_survive_a_restart() {
    // (the colour scheme is process-wide)
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let folder = env.font_folder("persisted", &["georgia.ttf", "georgiab.ttf", "verdana.ttf"]);
    let start = Start::new(&env, &fs);
    let file = settings_file(&start);
    assert!(!file.exists(), "a fresh data folder");

    // the first window
    let mut h = harness_start(start, SIZE);
    wait_until(&mut h, 30, "the first scan", |h| !h.state().operation_running() && h.state().font_count() > 0);
    let before = h.state().settings().clone();
    change_everything(&mut h, &folder);
    let s = h.state().settings().clone();
    let folder_name = folder.file_name().unwrap().to_string_lossy().into_owned();
    let libraries = h.state().library_names();
    let status = h.state().status_text();
    // what the user interface did to the settings
    assert_eq!(s.scheme, anyfont::theme::Scheme::Light, "theme");
    assert_eq!(s.core.ui_scale_percent, 150.0, "scale");
    assert!(!s.core.group_by_family, "group by family off");
    assert!(!s.show_filter_list, "filter list hidden");
    assert!(s.core.show_font_properties, "font properties shown");
    assert!(s.core.preview_size > before.core.preview_size, "preview size {} > {}", s.core.preview_size, before.core.preview_size);
    assert!((s.core.window.left_panel_width - before.core.window.left_panel_width - 80.0).abs() <= 2.0, "left panel {} (was {})", s.core.window.left_panel_width, before.core.window.left_panel_width);
    assert_eq!(s.core.sample_text, "Persisted sample two");
    for t in ["Persisted sample one", "Persisted sample two"] {
        assert!(s.core.sample_history.iter().any(|x| x == t), "history {:?}", s.core.sample_history);
    }
    assert_eq!(s.core.last_library, anyfont_core::settings::LastLibrary::Collection(COLLECTION.into()));
    assert!(libraries.contains(&folder_name) && libraries.iter().any(|n| n == COLLECTION), "{libraries:?}");
    assert_eq!(status, "Collection 1: 1 fonts");

    // the window moved and resized by the user (what eframe reports of the native window)
    {
        let v = h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default();
        v.outer_rect = Some(egui::Rect::from_min_size(pos2(90.0, 60.0), egui::vec2(1116.0, 719.0)));
        v.inner_rect = Some(egui::Rect::from_min_size(pos2(98.0, 91.0), egui::vec2(1100.0, 680.0)));
        v.maximized = Some(false);
        v.minimized = Some(false);
    }
    settle(&mut h);
    let w = &h.state().settings().core.window;
    assert_eq!((w.position, w.size, w.maximized), (Some([90.0, 60.0]), [1100.0, 680.0], false), "window placement tracked");

    // close: eframe calls on_exit, the window goes
    eframe::App::on_exit(h.state_mut());
    let saved = h.state().settings().clone();
    drop(h);
    let json = std::fs::read_to_string(&file).expect("settings.json written on exit");
    assert!(json.contains("Persisted sample two"), "{json}");

    // the second window, same data folder
    let mut start = Start::new(&env, &fs);
    start.splash = true;
    let mut h = start_frame_by_frame(start, SIZE);
    // the splash turns into the main window with the saved placement (restore_window)
    let mut cmds: Vec<ViewportCommand> = Vec::new();
    let end = Instant::now() + Duration::from_secs(30);
    while !cmds.iter().any(|c| matches!(c, ViewportCommand::InnerSize(_))) {
        assert!(Instant::now() < end, "the splash never ended");
        h.step();
        cmds.extend(viewport_commands(&h));
        std::thread::sleep(Duration::from_millis(5));
    }
    let w = &saved.core.window;
    assert!(cmds.contains(&ViewportCommand::Resizable(true)), "{cmds:?}");
    assert!(cmds.contains(&ViewportCommand::InnerSize(egui::vec2(w.size[0].max(800.0), w.size[1].max(600.0)))), "saved size {:?}: {cmds:?}", w.size);
    assert_eq!(cmds.contains(&ViewportCommand::Maximized(true)), w.maximized, "saved maximized {}: {cmds:?}", w.maximized);
    assert!(cmds.contains(&ViewportCommand::OuterPosition(pos2(90.0, 60.0))), "saved position: {cmds:?}");
    assert!(cmds.contains(&ViewportCommand::InnerSize(egui::vec2(1100.0, 680.0))), "saved size: {cmds:?}");
    wait_until(&mut h, 30, "the scan of the second start", |h| !h.state().operation_running() && h.state().font_count() > 0);
    settle(&mut h);
    let s2 = h.state().settings().clone();
    assert_eq!(s2.scheme, saved.scheme);
    assert_eq!(s2.core.ui_scale_percent, saved.core.ui_scale_percent);
    assert_eq!(s2.core.group_by_family, saved.core.group_by_family);
    assert_eq!(s2.core.preview_size, saved.core.preview_size);
    assert_eq!(s2.core.sample_text, saved.core.sample_text);
    assert_eq!(s2.core.sample_history, saved.core.sample_history);
    assert_eq!(s2.core.show_font_properties, saved.core.show_font_properties);
    assert_eq!(s2.show_filter_list, saved.show_filter_list);
    assert_eq!(s2.core.window.left_panel_width, saved.core.window.left_panel_width);
    assert_eq!(s2.core.last_library, saved.core.last_library);
    // ...and the window shows them
    assert_eq!(anyfont::theme::scheme(), anyfont::theme::Scheme::Light);
    assert_eq!(h.state().status_text(), "Collection 1: 1 fonts", "the last library is open again");
    let libraries2 = h.state().library_names();
    assert!(libraries2.contains(&folder_name) && libraries2.iter().any(|n| n == COLLECTION), "{libraries2:?}");
    let sample = h.get_by_role_and_label(Role::TextInput, "Enter sample text");
    assert_eq!(sample.accesskit_node().value().as_deref(), Some("Persisted sample two"));
    assert!(h.query_by(|n| n.role() == Role::CheckBox && n.label().as_deref() == Some("Installed")).is_none(), "the filter list stays hidden");
    assert_eq!(h.get_by_role_and_label(Role::Button, "Hide font properties").accesskit_node().toggled().is_some(), true);
    open_menu(&mut h, "View");
    let view = open_menu_items(&h);
    close_menus(&mut h);
    assert!(view.iter().any(|(l, _, c)| l == "Group by family" && !*c), "{view:?}");
    assert!(view.iter().any(|(l, _, _)| l == "Show filter list"), "{view:?}");
    // the sample history in the drop-down of the sample box
    h.get_by_role_and_label(Role::ComboBox, "Enter sample text").click();
    settle(&mut h);
    let painted: Vec<String> = texts_painted(&h);
    for t in ["Persisted sample one", "Persisted sample two"] {
        assert!(painted.iter().any(|x| x == t), "{t:?} in the drop-down: {painted:?}");
    }
    h.key_press(Key::Escape);
    settle(&mut h);
    eframe::App::on_exit(h.state_mut());
}

/// Every text painted in the last frame.
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

#[test]
fn a_change_is_saved_about_800_ms_later_without_closing() {
    let _g = env_lock();
    let env = Env::new();
    let fs = env.sandbox();
    let start = Start::new(&env, &fs);
    let file = settings_file(&start);
    let mut h = harness_start(start, SIZE);
    wait_until(&mut h, 30, "the first scan", |h| !h.state().operation_running());
    settle(&mut h);
    // whatever the start-up changed has been saved: nothing pending now
    let end = Instant::now() + Duration::from_millis(1200);
    while Instant::now() < end {
        h.run_steps(2);
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(!before.contains("\"group_by_family\": false"), "{before}");
    // View ▸ Group by family off
    open_menu(&mut h, "View");
    let item = menu_item(&h, "Group by family");
    let t0 = Instant::now();
    item.click();
    h.run_steps(3);
    assert!(!h.state().settings().core.group_by_family);
    let now = std::fs::read_to_string(&file).unwrap_or_default();
    if t0.elapsed() < Duration::from_millis(700) {
        assert_eq!(now, before, "not written at once (the save waits ~800 ms)");
    }
    // ...but written soon, without on_exit
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        h.run_steps(2);
        std::thread::sleep(Duration::from_millis(20));
        let s = anyfont::settings::AppSettings::load(&file);
        if !s.core.group_by_family {
            break;
        }
        assert!(Instant::now() < end, "settings.json not written 10 s after the change");
    }
    assert!(t0.elapsed() >= Duration::from_millis(790), "written after {:?}, before the 800 ms delay", t0.elapsed());
}
