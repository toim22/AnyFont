//! End-to-end checks of the Libraries panel: Add Folder (menu, toolbar, queued folder picks) and its refusals,
//! Add Collection and the inline name editor, Rename (F2, context menu), Remove with the confirmation and its
//! "Do not show this message again", Favorites, the library context menu (Install / Uninstall a whole library),
//! the section headers, dragging fonts onto a collection, and File ▸ Export / Import.
//! Every test runs the real window on a sandbox font system; file dialogs are answered with `queue_pick`.
mod common;

use anyfont_sys::FontSystem;
use common::*;
use egui::accesskit::{Role, Toggled};
use egui::Key;
use egui_kittest::kittest::{by, NodeT, Queryable};
use std::path::PathBuf;

const SIZE: (f32, f32) = (1280.0, 1000.0);

fn names(h: &H<'_>) -> Vec<String> {
    h.state().library_names()
}

fn has_lib(h: &H<'_>, name: &str) -> bool {
    names(h).iter().any(|n| n == name)
}

/// The label of the open message box (the first label of a dialog that is not a static UI text).
fn has_label(h: &H<'_>, text: &str) -> bool {
    h.query_all(by().label(text)).next().is_some()
}

/// Every text the last frame painted.
fn painted(h: &H<'_>) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for cs in &h.output().shapes {
        walk(&cs.shape, &mut v);
    }
    v
}

fn ok(h: &mut H<'_>) {
    h.get_by_role_and_label(Role::Button, "OK").click();
    settle(h);
}

fn lib_row<'t>(h: &'t H<'_>, name: &'t str) -> egui_kittest::Node<'t> {
    h.get_by_role_and_label(Role::ListBoxOption, name)
}

fn open_lib(h: &mut H<'_>, name: &str) {
    lib_row(h, name).click();
    settle(h);
}

fn is_open(h: &H<'_>, name: &str) -> bool {
    lib_row(h, name).accesskit_node().toggled() == Some(Toggled::True)
}

fn editor_value(h: &H<'_>) -> Option<String> {
    h.query_by_role_and_label(Role::TextInput, "Name:").map(|n| n.value().unwrap_or_default())
}

/// Replaces the editor's text (it starts with its text selected) and presses Enter.
fn type_name(h: &mut H<'_>, text: &str) {
    h.get_by_role_and_label(Role::TextInput, "Name:").type_text(text);
    settle(h);
    h.key_press(Key::Enter);
    settle(h);
}

/// Toolbar ▸ Add collection, named `name`.
fn new_collection(h: &mut H<'_>, name: &str) {
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(h);
    type_name(h, name);
    assert!(has_lib(h, name), "{:?}", names(h));
}

/// Selects a face of the list (by label, first match in the list).
fn select_face(h: &mut H<'_>, label: &str) {
    let lr = h.get_by_role_and_label(Role::Unknown, "Font list").rect();
    h.query_all(by().role(Role::ListBoxOption).label(label)).find(|n| lr.contains(n.rect().center())).unwrap_or_else(|| panic!("{label} not listed: {:?}", h.state().row_labels())).click();
    settle(h);
}

fn lib_menu(h: &mut H<'_>, name: &str) -> Vec<(String, bool, bool)> {
    lib_row(h, name).click_secondary();
    settle(h);
    open_menu_items(h)
}

fn enabled(items: &[(String, bool, bool)], label: &str) -> Option<bool> {
    items.iter().find(|i| i.0 == label).map(|i| i.1)
}

// --------------------------------------------------------------------------------------- Add Folder

#[test]
fn add_folder_from_the_menu_and_the_toolbar_with_its_refusals() {
    let env = Env::new();
    // a folder with a subfolder: the folder is added recursively
    let dir = env.fixture_folder("Library", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    let sub = dir.join("Sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::copy(common::fixture("Ubuntu-Light.ttf").unwrap(), sub.join("Ubuntu-Light.ttf")).unwrap();
    let other = env.fixture_folder("Other", &["colr_1_variable.ttf"]);
    let mut h = harness(env.config(), SIZE);

    // cancelled dialog: nothing happens
    anyfont::testing::queue_pick(None);
    click_menu(&mut h, "File", "Add Folder...");
    assert!(!has_lib(&h, "Library"));
    assert_eq!(h.state().pending_dialogs(), 0);

    anyfont::testing::queue_pick(Some(dir.clone()));
    click_menu(&mut h, "File", "Add Folder...");
    assert!(has_lib(&h, "Library"), "{:?}", names(&h));
    assert!(is_open(&h, "Library"), "the new folder is opened");
    assert_eq!(h.state().status_text(), "Library: 3 fonts", "the subfolder's font counts: {:?}", h.state().row_labels());
    assert!(h.state().row_labels().iter().any(|l| l == "Ubuntu Light"));

    // the same folder again
    anyfont::testing::queue_pick(Some(dir.clone()));
    click_menu(&mut h, "File", "Add Folder...");
    assert_eq!(h.state().pending_dialogs(), 1);
    assert!(has_label(&h, "This folder has already been added."));
    ok(&mut h);
    assert_eq!(h.state().pending_dialogs(), 0);
    // its subfolder
    anyfont::testing::queue_pick(Some(sub.clone()));
    h.get_by_role_and_label(Role::Button, "Add folder").click();
    settle(&mut h);
    let msg = format!("This folder has already been added as a subfolder of {}.", dir.display());
    assert!(has_label(&h, &msg), "expected {msg:?}");
    ok(&mut h);
    // a folder that does not exist
    anyfont::testing::queue_pick(Some(env.path("Nowhere")));
    h.get_by_role_and_label(Role::Button, "Add folder").click();
    settle(&mut h);
    assert!(has_label(&h, "Folder not found. Please choose another location."));
    ok(&mut h);
    assert_eq!(names(&h), vec!["Library".to_string(), "Favorites".to_string()]);

    // toolbar ▸ Add folder, another folder
    anyfont::testing::queue_pick(Some(other.clone()));
    h.get_by_role_and_label(Role::Button, "Add folder").click();
    settle(&mut h);
    assert!(is_open(&h, "Other"));
    assert_eq!(h.state().status_text(), "Other: 1 fonts");
    // with interception on and nothing queued, the dialog counts as cancelled
    h.get_by_role_and_label(Role::Button, "Add folder").click();
    settle(&mut h);
    assert_eq!(names(&h).len(), 3);
    // the folder row's tooltip is its path; My fonts' is its description (hover shows them)
    lib_row(&h, "Library").hover();
    for _ in 0..6 {
        h.step();
    }
    let path = dir.display().to_string();
    let painted = painted(&h);
    assert!(painted.iter().any(|t| *t == path), "the folder's path in its tooltip: {painted:?}");
}

// ----------------------------------------------------------------------------- collections, editor

#[test]
fn add_collection_and_the_inline_name_editor() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    // toolbar ▸ Add collection: "Collection 1", Enter keeps it
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(&mut h);
    assert_eq!(editor_value(&h).as_deref(), Some("Collection 1"));
    assert!(is_open(&h, "Collection 1"), "the new collection is opened");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(editor_value(&h).is_none(), "the editor closes");
    assert!(has_lib(&h, "Collection 1"));
    // an empty collection shows how to fill it
    assert_eq!(h.state().status_text(), "Collection 1: 0 fonts");

    // File ▸ Add Collection: "Collection 2", typed over
    click_menu(&mut h, "File", "Add Collection");
    assert_eq!(editor_value(&h).as_deref(), Some("Collection 2"));
    type_name(&mut h, "Serif");
    assert!(has_lib(&h, "Serif") && !has_lib(&h, "Collection 2"), "{:?}", names(&h));

    // Esc on a new collection removes it
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(&mut h);
    let n = names(&h).len();
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(editor_value(&h).is_none());
    assert_eq!(names(&h).len(), n - 1, "{:?}", names(&h));
    assert!(is_open(&h, "My fonts"), "My fonts is opened again");

    // an empty name is refused: the editor stays open
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(&mut h);
    h.key_press(Key::Backspace);
    settle(&mut h);
    assert_eq!(editor_value(&h).as_deref(), Some(""));
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(editor_value(&h).as_deref(), Some(""), "empty name refused");
    // a duplicate name is refused too (case-insensitive or not, "Serif" exists)
    h.get_by_role_and_label(Role::TextInput, "Name:").type_text("Serif");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(editor_value(&h).as_deref(), Some("Serif"), "duplicate name refused");
    assert_eq!(names(&h).iter().filter(|n| *n == "Serif").count(), 1);
    // Esc: the new collection goes away
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(editor_value(&h).is_none());
    assert_eq!(names(&h).iter().filter(|n| n.starts_with("Collection ")).count(), 1, "{:?}", names(&h));
}

#[test]
fn rename_with_f2_and_the_context_menu() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    new_collection(&mut h, "Draft");
    // F2 with the Libraries panel focused (a click on the row)
    open_lib(&mut h, "Draft");
    h.key_press(Key::F2);
    settle(&mut h);
    assert_eq!(editor_value(&h).as_deref(), Some("Draft"));
    type_name(&mut h, "Final");
    assert!(has_lib(&h, "Final") && !has_lib(&h, "Draft"), "{:?}", names(&h));
    assert!(is_open(&h, "Final"));
    // Esc while renaming keeps the old name (not a new collection)
    h.key_press(Key::F2);
    settle(&mut h);
    h.get_by_role_and_label(Role::TextInput, "Name:").type_text("Nope");
    settle(&mut h);
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(has_lib(&h, "Final") && !has_lib(&h, "Nope"));
    // context menu ▸ Rename
    let items = lib_menu(&mut h, "Final");
    assert_eq!(items.iter().map(|i| i.0.as_str()).collect::<Vec<_>>(), vec!["Install", "Uninstall", "Remove Folder or Collection", "Rename"]);
    menu_item(&h, "Rename").click();
    settle(&mut h);
    assert_eq!(editor_value(&h).as_deref(), Some("Final"));
    type_name(&mut h, "Mono");
    assert!(has_lib(&h, "Mono"), "{:?}", names(&h));
    // Favorites cannot be renamed: no Rename item, F2 does nothing
    let items = lib_menu(&mut h, "Favorites");
    close_menus(&mut h);
    assert!(items.iter().all(|i| i.0 != "Rename"), "{items:?}");
    open_lib(&mut h, "Favorites");
    h.key_press(Key::F2);
    settle(&mut h);
    assert!(editor_value(&h).is_none());
    // My fonts' menu: nothing to remove or rename
    let items = lib_menu(&mut h, "My fonts");
    close_menus(&mut h);
    assert_eq!(enabled(&items, "Remove Folder or Collection"), Some(false), "{items:?}");
    assert!(items.iter().all(|i| i.0 != "Rename"));
}

// ------------------------------------------------------------------------------------------ Remove

#[test]
fn remove_with_confirmation_and_do_not_show_again() {
    let env = Env::new();
    let dir = env.fixture_folder("Folder A", &["Tuffy.ttf"]);
    let mut h = harness(env.config(), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    new_collection(&mut h, "One");
    new_collection(&mut h, "Two");

    // toolbar ▸ Remove on the open collection: No keeps it
    open_lib(&mut h, "One");
    h.get_by_role_and_label(Role::Button, "Remove").click();
    settle(&mut h);
    assert!(has_label(&h, "Are you sure you want to remove this collection?"));
    h.get_by_role_and_label(Role::Button, "No").click();
    settle(&mut h);
    assert!(has_lib(&h, "One"));
    assert_eq!(h.state().pending_dialogs(), 0);
    // Yes removes it; My fonts is opened
    h.get_by_role_and_label(Role::Button, "Remove").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Yes").click();
    settle(&mut h);
    assert!(!has_lib(&h, "One"));
    assert!(is_open(&h, "My fonts"));

    // the folder, from the context menu: the folder text; "Do not show this message again" + Yes
    let items = lib_menu(&mut h, "Folder A");
    assert_eq!(enabled(&items, "Remove Folder or Collection"), Some(true));
    menu_item(&h, "Remove Folder or Collection").click();
    settle(&mut h);
    assert!(has_label(&h, "Are you sure you want to remove this folder?"));
    h.get_by_role_and_label(Role::CheckBox, "Do not show this message again").click();
    settle(&mut h);
    assert_eq!(h.get_by_role_and_label(Role::CheckBox, "Do not show this message again").accesskit_node().toggled(), Some(Toggled::True));
    h.get_by_role_and_label(Role::Button, "Yes").click();
    settle(&mut h);
    assert!(!has_lib(&h, "Folder A"));
    assert!(dir.join("Tuffy.ttf").exists(), "the files are not touched");
    assert!(h.state().settings().skip_remove_confirmation);

    // next time: no question (File ▸ Remove Folder or Collection)
    open_lib(&mut h, "Two");
    click_menu(&mut h, "File", "Remove Folder or Collection");
    assert_eq!(h.state().pending_dialogs(), 0);
    assert!(!has_lib(&h, "Two"));
    // File ▸ Remove Folder or Collection is disabled on My fonts
    open_menu(&mut h, "File");
    let items = open_menu_items(&h);
    close_menus(&mut h);
    assert_eq!(enabled(&items, "Remove Folder or Collection"), Some(false));
}

/// "Do not show this message again" is saved with the settings: a new window does not ask either.
#[test]
fn do_not_show_again_survives_a_restart() {
    let env = Env::new();
    let fs = env.sandbox();
    {
        let mut h = harness_start(Start::new(&env, &fs), SIZE);
        new_collection(&mut h, "Keep");
        new_collection(&mut h, "Gone");
        h.get_by_role_and_label(Role::Button, "Remove").click();
        settle(&mut h);
        h.get_by_role_and_label(Role::CheckBox, "Do not show this message again").click();
        settle(&mut h);
        h.get_by_role_and_label(Role::Button, "Yes").click();
        settle(&mut h);
        assert!(!has_lib(&h, "Gone"));
        // closing the window saves the settings
        eframe::App::on_exit(h.state_mut());
    }
    let mut h = harness_start(Start::new(&env, &fs), SIZE);
    assert!(h.state().settings().skip_remove_confirmation);
    open_lib(&mut h, "Keep");
    h.get_by_role_and_label(Role::Button, "Remove").click();
    settle(&mut h);
    assert_eq!(h.state().pending_dialogs(), 0);
    assert!(!has_lib(&h, "Keep"));
}

// ---------------------------------------------------------------------------------------- Favorites

#[test]
fn favorites_add_remove_and_remove_favorites_itself() {
    let env = Env::new();
    let dir = env.fixture_folder("Faces", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    let mut h = harness(env.config(), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    select_face(&mut h, "Tuffy (Regular, Normal)");
    // Font ▸ Add to collection ▸ Favorites
    open_menu(&mut h, "Font");
    menu_item(&h, "Add to collection").click();
    settle(&mut h);
    menu_item(&h, "Favorites").click();
    settle(&mut h);
    // outside a collection: Remove from collection ▸ Favorites
    open_menu(&mut h, "Font");
    assert_eq!(enabled(&open_menu_items(&h), "Remove from collection"), Some(true));
    menu_item(&h, "Remove from collection").click();
    settle(&mut h);
    assert!(query_menu_item(&h, "Favorites").is_some(), "{:?}", open_menu_items(&h));
    close_menus(&mut h);
    open_lib(&mut h, "Favorites");
    assert_eq!(h.state().status_text(), "Favorites: 1 fonts");
    assert_eq!(h.state().row_labels(), vec!["Tuffy (Regular, Normal)"]);
    // in the collection: Font ▸ Remove from Favorites (the context menu of the row too)
    select_face(&mut h, "Tuffy (Regular, Normal)");
    let lr = h.get_by_role_and_label(Role::Unknown, "Font list").rect();
    h.query_all(by().role(Role::ListBoxOption).label("Tuffy (Regular, Normal)")).find(|n| lr.contains(n.rect().center())).unwrap().click_secondary();
    settle(&mut h);
    let items = open_menu_items(&h);
    assert_eq!(enabled(&items, "Remove from Favorites"), Some(true), "{items:?}");
    assert!(items.iter().all(|i| i.0 != "Remove from collection"));
    // "Add to collection" lists the other collections only (none here)
    menu_item(&h, "Remove from Favorites").click();
    settle(&mut h);
    assert_eq!(h.state().status_text(), "Favorites: 0 fonts");
    assert!(painted(&h).iter().any(|t| t == "To populate this collection, drag fonts from an added folder or another collection."), "the empty collection's hint");
}

/// UI-SPEC §13 lead decision 2: "Favorites can be removed, as in the original; it is not re-created
/// automatically" (and §2.4 #4: Remove Folder or Collection is enabled for Favorites).
#[test]
#[ignore = "BUG-306: Remove Folder or Collection on Favorites only empties it; Favorites stays in Libraries"]
fn removing_favorites_removes_it_for_good() {
    let env = Env::new();
    let dir = env.fixture_folder("Faces", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    let fs = env.sandbox();
    {
        let mut h = harness(env.config_with(&fs), SIZE);
        h.state_mut().add_folder_path(&dir);
        settle(&mut h);
        select_face(&mut h, "Hack (Regular, Normal)");
        open_menu(&mut h, "Font");
        menu_item(&h, "Add to collection").click();
        settle(&mut h);
        menu_item(&h, "Favorites").click();
        settle(&mut h);
        let items = lib_menu(&mut h, "Favorites");
        assert_eq!(enabled(&items, "Remove Folder or Collection"), Some(true), "{items:?}");
        menu_item(&h, "Remove Folder or Collection").click();
        settle(&mut h);
        assert!(has_label(&h, "Are you sure you want to remove this collection?"));
        h.get_by_role_and_label(Role::Button, "Yes").click();
        settle(&mut h);
        assert!(!has_lib(&h, "Favorites"), "Favorites is removed: {:?}", names(&h));
    }
    // and it does not come back after a restart
    let h = harness(env.config_with(&fs), SIZE);
    assert!(!has_lib(&h, "Favorites"), "Favorites is not re-created: {:?}", names(&h));
}

// ---------------------------------------------------------------------- library menu: install all

#[test]
fn library_menu_installs_and_uninstalls_a_whole_folder_and_collection() {
    let env = Env::new();
    let dir = env.fixture_folder("Pack", &["Tuffy.ttf", "Hack-Regular.ttf", "Ubuntu-Light.ttf"]);
    let fs = env.sandbox();
    let mut h = harness(env.config_with(&fs), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    let before = fs.installed_fonts().unwrap().len();
    let items = lib_menu(&mut h, "Pack");
    assert_eq!(enabled(&items, "Install"), Some(true), "{items:?}");
    assert_eq!(enabled(&items, "Uninstall"), Some(false));
    menu_item(&h, "Install").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert!(h.state().face_states().iter().all(|(_, i)| *i), "{:?}", h.state().face_states());
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 3);
    let items = lib_menu(&mut h, "Pack");
    assert_eq!(enabled(&items, "Install"), Some(false), "{items:?}");
    assert_eq!(enabled(&items, "Uninstall"), Some(true));
    menu_item(&h, "Uninstall").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert!(h.state().face_states().iter().all(|(_, i)| !*i), "{:?}", h.state().face_states());
    assert_eq!(fs.installed_fonts().unwrap().len(), before);

    // a collection's menu acts on its fonts only
    new_collection(&mut h, "Pick");
    open_lib(&mut h, "Pack");
    select_face(&mut h, "Hack (Regular, Normal)");
    open_menu(&mut h, "Font");
    menu_item(&h, "Add to collection").click();
    settle(&mut h);
    menu_item(&h, "Pick").click();
    settle(&mut h);
    let items = lib_menu(&mut h, "Pick");
    assert_eq!(enabled(&items, "Install"), Some(true), "{items:?}");
    menu_item(&h, "Install").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 1);
    assert_eq!(h.state().face_states(), vec![("Hack (Regular, Normal)".to_string(), true)]);
    // My fonts: Uninstall skips the protected seed fonts (and uninstalls Hack)
    let items = lib_menu(&mut h, "My fonts");
    assert_eq!(enabled(&items, "Uninstall"), Some(true), "{items:?}");
    menu_item(&h, "Uninstall").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), before, "only Hack was uninstalled");
    let items = lib_menu(&mut h, "My fonts");
    close_menus(&mut h);
    assert_eq!(enabled(&items, "Uninstall"), Some(false), "only protected fonts are left: {items:?}");
}

// -------------------------------------------------------------------------------- sections, keys

#[test]
fn section_headers_collapse_and_expand() {
    let env = Env::new();
    let dir = env.fixture_folder("Folder B", &["Tuffy.ttf"]);
    let mut h = harness(env.config(), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    new_collection(&mut h, "Coll");
    let tri = |h: &H<'_>, n: &str| h.get_by_role_and_label(Role::DisclosureTriangle, n).accesskit_node().toggled();
    assert_eq!(tri(&h, "Folders"), Some(Toggled::True));
    h.get_by_role_and_label(Role::DisclosureTriangle, "Folders").click();
    settle(&mut h);
    assert_eq!(tri(&h, "Folders"), Some(Toggled::False));
    assert!(h.query_by_role_and_label(Role::ListBoxOption, "My fonts").is_none());
    assert!(h.query_by_role_and_label(Role::ListBoxOption, "Folder B").is_none());
    assert!(h.query_by_role_and_label(Role::ListBoxOption, "Coll").is_some());
    assert!(h.state().settings().is_collapsed("Folders"));
    h.get_by_role_and_label(Role::DisclosureTriangle, "My Collections").click();
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::ListBoxOption, "Coll").is_none());
    assert!(h.query_by_role_and_label(Role::ListBoxOption, "Favorites").is_none());
    h.get_by_role_and_label(Role::DisclosureTriangle, "Folders").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::DisclosureTriangle, "My Collections").click();
    settle(&mut h);
    assert!(!h.state().settings().is_collapsed("Folders") && !h.state().settings().is_collapsed("My Collections"));
    lib_row(&h, "My fonts");

    // the keyboard: ↓/↑ open the next/previous library, ← collapses the section of the open one, → expands
    open_lib(&mut h, "My fonts");
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    assert!(is_open(&h, "Folder B"));
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    assert!(is_open(&h, "Coll"), "↓ crosses into My Collections");
    h.key_press(Key::ArrowUp);
    settle(&mut h);
    assert!(is_open(&h, "Folder B"));
    h.key_press(Key::ArrowLeft);
    settle(&mut h);
    assert!(h.state().settings().is_collapsed("Folders"));
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    assert!(!h.state().settings().is_collapsed("Folders"));
    // Shift+F10 opens the library menu
    h.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    settle(&mut h);
    assert!(query_menu_item(&h, "Remove Folder or Collection").is_some(), "{:?}", open_menu_items(&h));
    close_menus(&mut h);
}

// ------------------------------------------------------------------------------------ drag & drop

#[test]
fn dragging_fonts_onto_a_collection_adds_them() {
    let env = Env::new();
    let dir = env.fixture_folder("Drag", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    let mut h = harness(env.config(), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    new_collection(&mut h, "Target");
    open_lib(&mut h, "Drag");
    let lr = h.get_by_role_and_label(Role::Unknown, "Font list").rect();
    let from = h.query_all(by().role(Role::ListBoxOption).label("Tuffy (Regular, Normal)")).find(|n| lr.contains(n.rect().center())).unwrap().rect().center();
    let to = lib_row(&h, "Target").rect().center();
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    for k in 1..=6 {
        let t = k as f32 / 6.0;
        h.hover_at(from + (to - from) * t);
        h.step();
    }
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    settle(&mut h);
    assert!(is_open(&h, "Drag"), "dropping does not change the open library");
    open_lib(&mut h, "Target");
    assert_eq!(h.state().row_labels(), vec!["Tuffy (Regular, Normal)"], "the dragged font is in the collection");
    // dropping somewhere else adds nothing
    open_lib(&mut h, "Drag");
    let from = h.query_all(by().role(Role::ListBoxOption).label("Hack (Regular, Normal)")).find(|n| lr.contains(n.rect().center())).unwrap().rect().center();
    let nowhere = from + egui::vec2(0.0, 200.0);
    h.drag_at(from);
    h.step();
    for k in 1..=4 {
        h.hover_at(from + (nowhere - from) * (k as f32 / 4.0));
        h.step();
    }
    h.drop_at(nowhere);
    settle(&mut h);
    open_lib(&mut h, "Target");
    assert_eq!(h.state().font_count(), 1);
}

// --------------------------------------------------------------------------------- Import / Export

#[test]
fn export_and_import_folders_and_collections() {
    let env = Env::new();
    let keep = env.fixture_folder("Kept", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    let lost = env.fixture_folder("Lost", &["Ubuntu-Light.ttf"]);
    let xml: PathBuf = env.path("export.xml");
    {
        let mut h = harness(env.config(), SIZE);
        h.state_mut().add_folder_path(&keep);
        settle(&mut h);
        h.state_mut().add_folder_path(&lost);
        settle(&mut h);
        new_collection(&mut h, "Exported");
        open_lib(&mut h, "Kept");
        select_face(&mut h, "Tuffy (Regular, Normal)");
        open_menu(&mut h, "Font");
        menu_item(&h, "Add to collection").click();
        settle(&mut h);
        menu_item(&h, "Exported").click();
        settle(&mut h);
        // a cancelled dialog writes nothing
        anyfont::testing::queue_pick(None);
        click_menu(&mut h, "File", "Export Folders and Collections...");
        assert!(!xml.exists());
        anyfont::testing::queue_pick(Some(xml.clone()));
        click_menu(&mut h, "File", "Export Folders and Collections...");
        assert!(xml.exists(), "exported");
        assert_eq!(h.state().pending_dialogs(), 0);
    }
    let text = std::fs::read_to_string(&xml).unwrap();
    assert!(text.contains("Exported"), "{text}");
    assert!(text.contains("Kept") && text.contains("Lost"), "{text}");

    // a fresh profile imports it; the "Lost" folder is gone meanwhile
    std::fs::remove_dir_all(&lost).unwrap();
    let env2 = Env::new();
    let mut h = harness(env2.config(), SIZE);
    anyfont::testing::queue_pick(None);
    click_menu(&mut h, "File", "Import Folders and Collections...");
    assert!(!has_lib(&h, "Exported"));
    anyfont::testing::queue_pick(Some(xml.clone()));
    click_menu(&mut h, "File", "Import Folders and Collections...");
    assert!(has_label(&h, "We can't find the following:"), "the missing folder is reported");
    let lost_path = lost.display().to_string();
    assert!(painted(&h).iter().any(|t| *t == lost_path), "the missing folder's path is listed: {:?}", painted(&h));
    assert!(!painted(&h).iter().any(|t| *t == keep.display().to_string()), "only the missing folder");
    // the PANOSE results window opened over it (BUG-304): Cancel it first
    if h.query_by_role_and_label(Role::Button, "Cancel").is_some() {
        h.get_by_role_and_label(Role::Button, "Cancel").click();
        settle(&mut h);
    }
    ok(&mut h);
    assert_eq!(h.state().pending_dialogs(), 0);
    assert!(has_lib(&h, "Kept"), "{:?}", names(&h));
    assert!(has_lib(&h, "Exported"), "{:?}", names(&h));
    // (the collection's fonts: see BUG-304, `import_does_not_report_fonts_of_the_imported_folders_as_missing`)
    open_lib(&mut h, "Kept");
    assert_eq!(h.state().font_count(), 2);
}

/// File ▸ Export writes AnyFont's own (hidden) content folder with the user's folders; imported into another
/// profile it shows up as a user folder named "Fonts".
#[test]
#[ignore = "BUG-303: Export includes the hidden content folder, which Import then adds as a user folder"]
fn export_leaves_out_the_content_folder() {
    let env = Env::new();
    let keep = env.fixture_folder("Mine", &["Tuffy.ttf"]);
    let xml = env.path("export.xml");
    let fs = env.sandbox();
    let content = anyfont_sys::FontSystem::paths(fs.as_ref()).content_fonts_dir.clone();
    {
        let mut h = harness(env.config_with(&fs), SIZE);
        h.state_mut().add_folder_path(&keep);
        settle(&mut h);
        anyfont::testing::queue_pick(Some(xml.clone()));
        click_menu(&mut h, "File", "Export Folders and Collections...");
    }
    let text = std::fs::read_to_string(&xml).unwrap();
    assert!(text.contains(&keep.display().to_string()));
    let env2 = Env::new();
    let mut h = harness(env2.config(), SIZE);
    anyfont::testing::queue_pick(Some(xml.clone()));
    click_menu(&mut h, "File", "Import Folders and Collections...");
    assert_eq!(names(&h).iter().filter(|n| *n != "Favorites").cloned().collect::<Vec<_>>(), vec!["Mine".to_string()], "only the user's folder is imported");
    assert!(!text.contains(&content.display().to_string()), "the content folder is not exported:
{text}");
}

/// Import of folders and collections into a fresh profile: the collection's fonts are in the imported
/// folders, so nothing is "missing", no PANOSE results window opens, and the collection keeps its fonts.
#[test]
#[ignore = "BUG-304: Import resolves collection fonts before scanning the imported folders (PANOSE results, empty collection)"]
fn import_does_not_report_fonts_of_the_imported_folders_as_missing() {
    let env = Env::new();
    let keep = env.fixture_folder("Mine", &["Tuffy.ttf"]);
    let xml = env.path("export.xml");
    {
        let mut h = harness(env.config(), SIZE);
        h.state_mut().add_folder_path(&keep);
        settle(&mut h);
        new_collection(&mut h, "Picked");
        open_lib(&mut h, "Mine");
        select_face(&mut h, "Tuffy (Regular, Normal)");
        open_menu(&mut h, "Font");
        menu_item(&h, "Add to collection").click();
        settle(&mut h);
        menu_item(&h, "Picked").click();
        settle(&mut h);
        anyfont::testing::queue_pick(Some(xml.clone()));
        click_menu(&mut h, "File", "Export Folders and Collections...");
    }
    let env2 = Env::new();
    let mut h = harness(env2.config(), SIZE);
    anyfont::testing::queue_pick(Some(xml.clone()));
    click_menu(&mut h, "File", "Import Folders and Collections...");
    assert!(!painted(&h).iter().any(|t| t == "PANOSE Font Matching Results"), "no missing fonts: {:?}", painted(&h));
    open_lib(&mut h, "Picked");
    assert_eq!(h.state().row_labels(), vec!["Tuffy (Regular, Normal)"]);
}
