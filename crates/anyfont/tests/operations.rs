//! End-to-end checks of the main operations, driven through the window like a user (menus, toolbar,
//! Libraries, Filters, Search), on a sandbox font system.
mod common;

use common::*;
use egui::accesskit::Role;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

type H<'a> = Harness<'a, anyfont::AnyFontApp>;

fn click_menu(h: &mut H<'_>, menu: &str, item: &str) {
    h.get_by_role_and_label(Role::Button, menu).click();
    settle(h);
    menu_item(h, item).click();
    settle(h);
}

/// A menu item (plain or check item) by its label.
fn menu_item<'t>(h: &'t H<'_>, item: &'t str) -> egui_kittest::Node<'t> {
    h.get_by(move |n| n.label().as_deref() == Some(item) && matches!(n.role(), Role::MenuItem | Role::MenuItemCheckBox))
}

/// Waits for a font operation (install, uninstall, delete) to finish.
fn finish_operation(h: &mut H<'_>) {
    for _ in 0..200 {
        settle(h);
        if !h.state().operation_running() {
            break;
        }
    }
    assert!(!h.state().operation_running(), "the operation did not finish");
    settle(h);
}

#[test]
fn add_folder_opens_it_with_its_fonts() {
    let env = Env::new();
    let dir = env.font_folder("My Folder", &["georgia.ttf", "verdana.ttf"]);
    let mut h = harness(env.config(), (1280.0, 800.0));
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    let app = h.state();
    assert!(app.library_names().iter().any(|n| n == "My Folder"), "{:?}", app.library_names());
    assert_eq!(app.status_text(), "My Folder: 2 fonts");
    assert_eq!(app.font_count(), 2);
    // adding it again is refused with a message
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    assert_eq!(h.state().pending_dialogs(), 1);
    h.get_by_role_and_label(Role::Button, "OK").click();
    settle(&mut h);
    assert_eq!(h.state().pending_dialogs(), 0);
    // the folder's row in Libraries
    h.get_by_role_and_label(Role::ListBoxOption, "My Folder");
    // My fonts lists only the installed fonts (the folder's fonts are not installed)
    h.get_by_role_and_label(Role::ListBoxOption, "My fonts").click();
    settle(&mut h);
    assert!(h.state().status_text().starts_with("My fonts: "));
}

#[test]
fn add_collection_rename_and_add_fonts() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    // toolbar ▸ Add collection opens the inline editor on "Collection 1"
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(&mut h);
    let editor = h.get_by_role_and_label(Role::TextInput, "Name:");
    assert_eq!(editor.value().as_deref(), Some("Collection 1"));
    editor.type_text("Sans");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().library_names().iter().any(|n| n == "Sans"), "{:?}", h.state().library_names());

    // Font ▸ Add to collection ▸ Sans, for the selected font (My fonts' first font)
    h.get_by_role_and_label(Role::ListBoxOption, "My fonts").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Font").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::MenuItem, "Add to collection").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::MenuItem, "Sans").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::ListBoxOption, "Sans").click();
    settle(&mut h);
    assert_eq!(h.state().status_text(), "Sans: 1 fonts");

    // Libraries ▸ Remove (toolbar), confirmed
    h.get_by_role_and_label(Role::Button, "Remove").click();
    settle(&mut h);
    assert_eq!(h.state().pending_dialogs(), 1);
    h.get_by_role_and_label(Role::Button, "Yes").click();
    settle(&mut h);
    assert!(!h.state().library_names().iter().any(|n| n == "Sans"));
}

#[test]
fn filters_and_search_narrow_the_list() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    let all = h.state().font_count();
    assert!(all >= 3, "seed fonts: {all}");

    h.get_by_role_and_label(Role::CheckBox, "Not installed").click();
    settle(&mut h);
    assert!(h.state().status_text().contains("(filtered)"), "{}", h.state().status_text());
    assert_eq!(h.state().font_count(), 0);
    h.get_all_by_role_and_label(Role::Button, "Clear filters").next().unwrap().click();
    settle(&mut h);
    assert_eq!(h.state().font_count(), all);

    let search = h.get_by_role_and_label(Role::TextInput, "Search");
    search.focus();
    settle(&mut h);
    h.get_by_role_and_label(Role::TextInput, "Search").type_text("Times");
    settle(&mut h);
    assert_eq!(h.state().font_count(), 1, "{:?}", h.state().row_labels());
    h.key_press(Key::Escape);
    settle(&mut h);
    assert_eq!(h.state().font_count(), all);
}

#[test]
fn view_menu_toggles_grouping_and_properties() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    let grouped = h.state().row_labels().len();
    assert!(h.state().settings().core.group_by_family);
    click_menu(&mut h, "View", "Group by family");
    assert!(!h.state().settings().core.group_by_family);
    assert!(h.state().row_labels().len() < grouped, "family headers go away");
    click_menu(&mut h, "View", "Show font properties");
    assert!(h.state().settings().core.window.right_panel_visible);
    h.get_by_label("Font info");
    // Ctrl+Alt+I toggles the panel too
    h.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::ALT, Key::I);
    settle(&mut h);
    assert!(!h.state().settings().core.window.right_panel_visible);
}

#[test]
fn install_and_uninstall_in_the_sandbox() {
    let env = Env::new();
    let dir = env.font_folder("Extra", &["georgia.ttf"]); // not a protected system font
    let mut h = harness(env.config(), (1280.0, 800.0));
    let fs = h.state().font_system();
    let before = fs.installed_fonts().unwrap().len();
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    assert_eq!(h.state().status_text(), "Extra: 1 fonts");

    h.get_by_role_and_label(Role::Button, "Install").click();
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 1, "installed into the sandbox");
    assert_eq!(h.state().pending_dialogs(), 0);
    assert_eq!(h.state().face_states(), vec![("Georgia (Regular, Normal)".to_string(), true)]);

    h.get_by_role_and_label(Role::Button, "Uninstall").click();
    finish_operation(&mut h);
    // a font in use may ask for a restart: answer later
    if h.state().pending_dialogs() > 0 {
        h.key_press(Key::Escape);
        settle(&mut h);
    }
    assert_eq!(fs.installed_fonts().unwrap().len(), before, "uninstalled from the sandbox");
    assert_eq!(h.state().face_states(), vec![("Georgia (Regular, Normal)".to_string(), false)]);
}

#[test]
fn export_and_import_libraries() {
    let env = Env::new();
    let dir = env.font_folder("Shared", &["consola.ttf"]);
    let file = env.dir.path().join("libraries.xml");
    {
        let mut h = harness(env.config(), (1280.0, 800.0));
        h.state_mut().add_folder_path(&dir);
        settle(&mut h);
        h.state_mut().export_path(&file);
        settle(&mut h);
    }
    assert!(file.exists());
    // a fresh profile imports the folder
    let env2 = Env::new();
    let mut h = harness(env2.config(), (1280.0, 800.0));
    assert!(!h.state().library_names().iter().any(|n| n == "Shared"));
    h.state_mut().import_path(&file);
    settle(&mut h);
    while h.state().pending_dialogs() > 0 {
        h.get_by_role_and_label(Role::Button, "OK").click();
        settle(&mut h);
    }
    assert!(h.state().library_names().iter().any(|n| n == "Shared"), "{:?}", h.state().library_names());
}

#[test]
fn settings_stay_in_the_test_data_folder() {
    let env = Env::new();
    let cfg = env.config();
    let data = cfg.data_dir.clone();
    let mut h = harness(cfg, (1280.0, 800.0));
    h.get_by_role_and_label(Role::Button, "Options").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "OK").click();
    settle(&mut h);
    let files: Vec<_> = std::fs::read_dir(&data).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(files.iter().any(|f| f.ends_with(".json") || f.ends_with(".toml") || f.contains("settings")), "{files:?}");
}

#[test]
fn delete_asks_and_removes_the_file() {
    let env = Env::new();
    let dir = env.font_folder("Trash", &["verdana.ttf"]);
    let file = dir.join("verdana.ttf");
    let mut h = harness(env.config(), (1280.0, 800.0));
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Delete").click();
    settle(&mut h);
    assert_eq!(h.state().pending_dialogs(), 1, "the confirmation");
    // Cancel keeps the file
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    settle(&mut h);
    assert!(file.exists());
    h.get_by_role_and_label(Role::Button, "Delete").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "OK").click();
    finish_operation(&mut h);
    assert!(!file.exists(), "deleted (to the sandbox's recycle step)");
    assert_eq!(h.state().font_count(), 0);
}

#[test]
fn f2_renames_a_collection() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    click_menu(&mut h, "File", "Add Collection");
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(!h.state().library_names().iter().any(|n| n == "Collection 1"), "Esc removes the new collection");
    click_menu(&mut h, "File", "Add Collection");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().library_names().iter().any(|n| n == "Collection 1"));
    h.key_press(Key::F2);
    settle(&mut h);
    h.get_by_role_and_label(Role::TextInput, "Name:").type_text("Serif");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().library_names().iter().any(|n| n == "Serif"), "{:?}", h.state().library_names());
}
