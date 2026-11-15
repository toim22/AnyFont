//! Help ▸ Product Help... and the title-bar "?", About AnyFont, System Info (categories, Save) and Legal
//! Notices (Print, OK), driven like a user through the accessibility tree. The browser and the printer are
//! never reached: `anyfont::testing` records what would have been opened.
mod common;
mod svc;

use anyfont::testing::{self, External};
use common::*;
use egui::accesskit::Role;
use egui_kittest::kittest::{by, NodeT, Queryable};
use svc::*;

const SIZE: (f32, f32) = (1200.0, 900.0);

fn count(url: &str) -> usize {
    testing::urls().iter().filter(|u| *u == url).count()
}

#[test]
fn product_help_from_menu_and_title_bar() {
    let _lock = env_lock();
    let env = Env::new();
    let url = format!("http://127.0.0.1:9/{}/HELP.md", unique("help"));
    let _v = EnvVar::set("ANYFONT_HELP_URL", &url);
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Product Help...");
    assert_eq!(count(&url), 1, "Help ▸ Product Help... asks for the guide: {:?}", testing::urls());
    h.get_by_role_and_label(Role::Button, "Product Help").click();
    settle(&mut h);
    assert_eq!(count(&url), 2, "the title-bar ? asks for the guide: {:?}", testing::urls());
}

#[test]
fn product_help_through_the_real_start_up() {
    // AnyFontApp::new (not the test window): the guide goes through dlg::open_url, recorded by the seam
    let _lock = env_lock();
    let env = Env::new();
    let url = format!("http://127.0.0.1:9/{}/HELP.md", unique("help-start"));
    let _v = EnvVar::set("ANYFONT_HELP_URL", &url);
    let fs = env.sandbox();
    let mut h = harness_start(Start::new(&env, &fs), SIZE);
    h.get_by_role_and_label(Role::Button, "Product Help").click();
    settle(&mut h);
    assert_eq!(count(&url), 1, "{:?}", testing::urls());
}

#[test]
fn about_dialog_shows_version_buttons_and_closes() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "About AnyFont...");
    for b in ["System Info...", "Legal Notices...", "Close"] {
        assert!(button_enabled(&h, b), "About shows {b}: {:?}", labels_of(&h, Role::Button));
    }
    // the version and copyright lines are painted (not in the accessibility tree): check their text
    let v = anyfont::about::version_line();
    assert!(v.starts_with(&format!("Version {}", env!("CARGO_PKG_VERSION"))), "{v}");
    assert!(v.contains("Release"), "{v}");
    let c = anyfont::about::copyright_line();
    assert!(c.contains("AnyFont") && c.contains("All rights reserved"), "{c}");
    // Close
    click_button(&mut h, "Close");
    assert!(buttons(&h, "System Info...").is_empty(), "About closed");
    // Esc closes it too
    click_menu(&mut h, "Help", "About AnyFont...");
    assert!(button_enabled(&h, "Legal Notices..."));
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    assert!(buttons(&h, "Legal Notices...").is_empty(), "Esc closes About");
}

fn open_system_info(h: &mut H<'_>) {
    click_menu(h, "Help", "About AnyFont...");
    click_button(h, "System Info...");
    assert!(button_enabled(h, "Save"), "System Info open: {:?}", labels_of(h, Role::Button));
    assert!(!button_enabled(h, "System Info..."), "About's buttons wait while System Info is open");
}

fn category_combo_text(h: &H<'_>) -> String {
    let n = h.get_by(|n| n.role() == Role::ComboBox && n.label().as_deref() == Some("Choose a category:"));
    let a = n.accesskit_node();
    format!("{} {}", a.label().unwrap_or_default(), a.value().unwrap_or_default())
}

#[test]
fn system_info_categories() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    open_system_info(&mut h);
    let combo = category_combo_text(&h);
    assert!(combo.contains("Choose a category:"), "{combo}");
    for cat in anyfont::about::sysinfo::CATEGORIES {
        h.get_by(|n| n.role() == Role::ComboBox && n.label().as_deref() == Some("Choose a category:")).click();
        settle(&mut h);
        {
            let item = h
                .query_all(by().label(cat))
                .filter(|n| n.accesskit_node().role() != Role::ComboBox)
                .last()
                .unwrap_or_else(|| panic!("category {cat} in the list: {:?}", all_texts(&h)));
            item.click();
        }
        settle(&mut h);
        let shown = category_combo_text(&h);
        assert!(shown.ends_with(&format!(" {cat}")), "category {cat} chosen: {shown}");
    }
    click_button(&mut h, "Close");
    assert!(buttons(&h, "Save").is_empty(), "System Info closed");
    assert!(button_enabled(&h, "System Info..."), "back in About");
}

#[test]
fn system_info_save_writes_the_report() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    open_system_info(&mut h);
    let file = env.path("System Info.txt");
    // cancelled dialog: nothing happens
    testing::queue_pick(None);
    click_button(&mut h, "Save");
    assert!(!has_text(&h, "System info was saved"), "cancel writes nothing");
    assert!(!file.exists());
    // saved
    testing::queue_pick(Some(file.clone()));
    click_button(&mut h, "Save");
    let msg = format!("System info was saved to {}.", file.display());
    assert!(has_text(&h, &msg), "message {msg:?}; shown: {:?}", texts_with(&h, "System info"));
    let text = std::fs::read_to_string(&file).expect("report written");
    assert!(text.starts_with(&format!("AnyFont {} - System Info", env!("CARGO_PKG_VERSION"))), "{}", &text[..text.len().min(200)]);
    for cat in anyfont::about::sysinfo::CATEGORIES {
        assert!(text.contains(cat), "the report has the {cat} section");
    }
    click_button(&mut h, "OK");
    assert!(!has_text(&h, "System info was saved"), "OK closes the message");
    assert!(button_enabled(&h, "Save"), "System Info stays open");
}

#[test]
fn system_info_save_failure_is_reported() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    open_system_info(&mut h);
    // a folder cannot be written as a file
    let dir = env.path("a folder");
    std::fs::create_dir_all(&dir).unwrap();
    testing::queue_pick(Some(dir.clone()));
    click_button(&mut h, "Save");
    let msg = format!("Unable to save system info to {}.", dir.display());
    assert!(has_text(&h, &msg), "message {msg:?}; shown: {:?}", all_texts(&h));
    click_button(&mut h, "OK");
    assert!(!has_text(&h, "Unable to save system info"));
}

#[test]
fn legal_notices_print_and_ok() {
    let _lock = env_lock();
    let env = Env::new();
    let tmp = env.path("tmp");
    let _t = TempDirGuard::set(&tmp);
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "About AnyFont...");
    click_button(&mut h, "Legal Notices...");
    assert!(button_enabled(&h, "Print") && button_enabled(&h, "OK"), "{:?}", labels_of(&h, Role::Button));
    assert!(has_text(&h, "open-source"), "the notices are shown: {:?}", &all_texts(&h).iter().take(20).collect::<Vec<_>>());
    click_button(&mut h, "Print");
    let printed = tmp.join("AnyFont Legal Notices.txt");
    assert!(testing::requests().contains(&External::Print(printed.clone())), "Print sends the text file to the printer: {:?}", testing::requests());
    let text = std::fs::read_to_string(&printed).expect("the printed file");
    assert!(text.contains("AnyFont") && text.contains("\r\n"), "Windows line ends");
    assert!(button_enabled(&h, "OK"), "the dialog stays open after Print");
    click_button(&mut h, "OK");
    assert!(buttons(&h, "Print").is_empty(), "OK closes Legal Notices");
    assert!(button_enabled(&h, "System Info..."), "back in About");
}
