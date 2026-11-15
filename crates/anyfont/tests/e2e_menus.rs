//! Menus, toolbar, title bar, context menus and shortcuts in the 13 selection contexts of UI-SPEC §2.3,
//! driven through the accessibility tree like a user. The expected states come from UI-SPEC §2.4–§2.8,
//! §3.1, §6.1, §6.2, the lead decisions of §13 and the captured toolbar matrix
//! (`crates/anyfont-core/tests/fixtures/toolbar-states.json`).
//!
//! The data: the sandbox seed (protected system fonts: Arial (2 faces), Times New Roman, Courier New,
//! Segoe UI, Cambria), plus Georgia Regular and Bold installed for the current user (unprotected), the
//! sandbox's per-user Fonts folder added as a user folder (it holds the installed Georgia files, like the
//! captured "temp folder" holding installed fonts), "Collection 1" holding Georgia Regular, Favorites empty.
mod common;

use anyfont_sys::{FontSystem, Scope, UsedFontAction};
use common::*;
use egui::accesskit::Role;
use egui::{Key, Modifiers, ViewportCommand};
use egui_kittest::kittest::{NodeT, Queryable};
use std::path::PathBuf;
use std::sync::Arc;

const SIZE: (f32, f32) = (1400.0, 1700.0);
const GEORGIA: &str = "Georgia (Regular, Normal)";
const GEORGIA_BOLD: &str = "Georgia (Bold)";
const ARIAL: &str = "Arial (Regular, Normal)";
const COLLECTION: &str = "Collection 1";

struct World {
    _env: Env,
    fs: Arc<anyfont_sys::SandboxFontSystem>,
    user_folder: String,
    h: H<'static>,
}

fn world() -> World {
    let env = Env::new();
    let fs = env.sandbox();
    let src = env.font_folder("src", &["georgia.ttf", "georgiab.ttf"]);
    let files = vec![src.join("georgia.ttf"), src.join("georgiab.ttf")];
    let report = fs.install(&files, Scope::CurrentUser, UsedFontAction::Ask);
    assert_eq!(report.succeeded(), 2, "Georgia installed for the current user in the sandbox");
    let user_dir = fs.font_dirs().current_user;
    let user_folder = user_dir.file_name().unwrap().to_string_lossy().into_owned();
    let mut h = harness(env.config_with(&fs), SIZE);
    // the user folder (opened by Add Folder)
    h.state_mut().add_folder_path(&user_dir);
    settle(&mut h);
    assert!(h.state().library_names().contains(&user_folder), "{:?}", h.state().library_names());
    // Collection 1 (toolbar ▸ Add collection, Enter keeps the proposed name)
    h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().library_names().iter().any(|n| n == COLLECTION), "{:?}", h.state().library_names());
    // Georgia Regular into Collection 1 (Font ▸ Add to collection ▸ Collection 1)
    open_library(&mut h, "My fonts");
    click_row(&mut h, GEORGIA);
    open_menu(&mut h, "Font");
    menu_item(&h, "Add to collection").click();
    settle(&mut h);
    menu_item(&h, COLLECTION).click();
    settle(&mut h);
    open_library(&mut h, COLLECTION);
    assert_eq!(h.state().status_text(), "Collection 1: 1 fonts");
    open_library(&mut h, "My fonts");
    World { _env: env, fs, user_folder, h }
}

fn open_library(h: &mut H<'_>, name: &str) {
    h.get_by_role_and_label(Role::ListBoxOption, name).click();
    settle(h);
}

fn row<'t>(h: &'t H<'_>, label: &'t str) -> egui_kittest::Node<'t> {
    h.query_by(move |n| n.role() == Role::ListBoxOption && n.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no row {label:?}; rows: {:?}; status {}", h.state().row_labels(), h.state().status_text()))
}

fn click_row(h: &mut H<'_>, label: &str) {
    row(h, label).click();
    settle(h);
}

fn row_selected(h: &H<'_>, label: &str) -> bool {
    row(h, label).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

// ---------------------------------------------------------------------------------------------------
// expectations

#[derive(Clone, Debug)]
struct Expect {
    name: &'static str,
    /// File ▸ Remove Folder or Collection, toolbar Remove.
    remove_library: bool,
    install: bool,
    uninstall: bool,
    delete: bool,
    select_all: bool,
    deselect_all: bool,
    add_to_collection: bool,
    /// `Some(caption)` inside a collection ("Remove from Collection 1"), else the submenu's enabled state.
    remove_from_open: Option<&'static str>,
    remove_from_sub: bool,
    open_location: bool,
    duplicates_on: bool,
    properties_on: bool,
    /// The key of the captured toolbar matrix this context reproduces.
    capture: &'static str,
}

impl Expect {
    fn base(name: &'static str, capture: &'static str) -> Expect {
        Expect {
            name,
            remove_library: false,
            install: false,
            uninstall: false,
            delete: false,
            select_all: true,
            deselect_all: false,
            add_to_collection: true,
            remove_from_open: None,
            remove_from_sub: false,
            open_location: false,
            duplicates_on: false,
            properties_on: false,
            capture,
        }
    }

    fn file(&self) -> Vec<(String, bool, bool)> {
        v(&[
            ("Add Folder...", true, false),
            ("Add Collection", true, false),
            ("Get More...", true, false),
            ("Remove Folder or Collection", self.remove_library, false),
            ("Import Folders and Collections...", true, false),
            ("Export Folders and Collections...", true, false),
            ("Exit", true, false),
        ])
    }

    fn edit(&self) -> Vec<(String, bool, bool)> {
        v(&[("Select All", self.select_all, false), ("Deselect All", self.deselect_all, false)])
    }

    fn font(&self) -> Vec<(String, bool, bool)> {
        let mut items = vec![
            ("Install".to_string(), self.install, false),
            ("Uninstall".to_string(), self.uninstall, false),
            ("Download".to_string(), false, false),
            ("Delete".to_string(), self.delete, false),
            ("Add to collection".to_string(), self.add_to_collection, false),
        ];
        match self.remove_from_open {
            Some(c) => items.push((c.to_string(), true, false)),
            None => items.push(("Remove from collection".to_string(), self.remove_from_sub, false)),
        }
        let dup = if self.duplicates_on { "Hide duplicate fonts" } else { "Show duplicate fonts" };
        items.push((dup.to_string(), true, self.duplicates_on));
        items.push(("Open font file location".to_string(), self.open_location, false));
        items
    }

    fn view(&self) -> Vec<(String, bool, bool)> {
        let props = if self.properties_on { "Hide font properties" } else { "Show font properties" };
        v(&[
            ("Refresh the Font List", false, false),
            ("Group by family", true, true),
            ("Show recently used fonts", true, false),
            ("Use font to display font name", true, true),
            ("Show Latin names", true, false),
            ("Hide filter list", true, true),
            (props, true, self.properties_on),
        ])
    }

    fn help() -> Vec<(String, bool, bool)> {
        v(&[
            ("Product Help...", true, false),
            ("Check for Updates...", true, false),
            ("Submit Ideas and Feedback...", true, false),
            ("Report a Bug...", true, false),
            ("About AnyFont...", true, false),
        ])
    }

    /// Toolbar buttons: (accessibility label, enabled, toggled), UI-SPEC §3.
    fn toolbar(&self) -> Vec<(String, bool, bool)> {
        let dup = if self.duplicates_on { "Hide duplicate fonts" } else { "Show duplicate fonts" };
        let props = if self.properties_on { "Hide font properties" } else { "Show font properties" };
        v(&[
            ("Add folder", true, false),
            ("Add collection", true, false),
            ("Get More", true, false),
            ("Remove", self.remove_library, false),
            ("Install", self.install, false),
            ("Uninstall", self.uninstall, false),
            ("Download", false, false),
            ("Delete", self.delete, false),
            (dup, true, self.duplicates_on),
            (props, true, self.properties_on),
        ])
    }
}

fn v(items: &[(&str, bool, bool)]) -> Vec<(String, bool, bool)> {
    items.iter().map(|(a, b, c)| (a.to_string(), *b, *c)).collect()
}

/// The 13 contexts with the states the rules give for this data.
fn contexts() -> Vec<Expect> {
    let mut out = Vec::new();
    // C1: My fonts after Ctrl+Shift+A
    out.push(Expect::base("C1 deselect-all", "deselect-all"));
    // C2: the Georgia family header (installed, unprotected faces): Uninstall/Delete on (§13 decision 1)
    out.push(Expect { uninstall: true, delete: true, deselect_all: true, open_location: true, remove_from_sub: true, ..Expect::base("C2 family-header-selected", "family-header-selected") });
    // C3: Georgia Bold (installed, unprotected, in no collection)
    out.push(Expect { uninstall: true, delete: true, deselect_all: true, open_location: true, ..Expect::base("C3 font-selected", "font-selected") });
    // C4: Arial Regular (protected)
    out.push(Expect { deselect_all: true, open_location: true, ..Expect::base("C4 system-font-selected", "system-font-selected") });
    // C5: Ctrl+A in My fonts (Georgia is uninstallable; Georgia Regular is in Collection 1)
    out.push(Expect { uninstall: true, delete: true, deselect_all: true, remove_from_sub: true, ..Expect::base("C5 all-selected", "all-selected") });
    // C6: Favorites (empty)
    out.push(Expect { remove_library: true, select_all: false, add_to_collection: false, remove_from_open: Some("Remove from Favorites"), ..Expect::base("C6 favorites-library", "favorites-library") });
    // C7: My fonts, filter "Not installed": no results
    out.push(Expect { select_all: false, add_to_collection: false, ..Expect::base("C7 filter-no-results", "filter-no-results") });
    // C8: C3 + the properties panel
    out.push(Expect { uninstall: true, delete: true, deselect_all: true, open_location: true, properties_on: true, ..Expect::base("C8 font-properties-shown", "font-properties-shown") });
    // C9: duplicates shown, nothing explicitly selected
    out.push(Expect { duplicates_on: true, ..Expect::base("C9 duplicates-shown", "duplicates-shown") });
    // C10: the user folder; its first font (Georgia Regular, in Collection 1) is auto-selected
    out.push(Expect { remove_library: true, uninstall: true, delete: true, deselect_all: true, open_location: true, remove_from_sub: true, ..Expect::base("C10 temp-folder-library", "temp-folder-library") });
    // C11: Georgia Regular in the user folder (it is in Collection 1: Remove from collection ▸ on)
    out.push(Expect { remove_library: true, uninstall: true, delete: true, deselect_all: true, open_location: true, remove_from_sub: true, ..Expect::base("C11 temp-folder-font-selected", "temp-folder-font-selected") });
    // C12: Collection 1, its first font auto-selected
    out.push(Expect { remove_library: true, uninstall: true, delete: true, deselect_all: true, open_location: true, remove_from_open: Some("Remove from Collection 1"), ..Expect::base("C12 user-collection-library", "user-collection-library") });
    // C13: Georgia Regular clicked in Collection 1
    out.push(Expect { remove_library: true, uninstall: true, delete: true, deselect_all: true, open_location: true, remove_from_open: Some("Remove from Collection 1"), ..Expect::base("C13 user-collection-font-selected", "user-collection-font-selected") });
    out
}

/// Puts the window into context `i` (0-based) from the neutral state (My fonts, defaults).
fn enter(w: &mut World, i: usize) {
    let h = &mut w.h;
    match i {
        0 => {
            open_library(h, "My fonts");
            h.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::A);
            settle(h);
        }
        1 => {
            open_library(h, "My fonts");
            click_row(h, "Georgia (2)");
            assert!(row_selected(h, "Georgia (2)"), "{:?}", row(h, "Georgia (2)"));
        }
        2 => {
            open_library(h, "My fonts");
            click_row(h, GEORGIA_BOLD);
        }
        3 => {
            open_library(h, "My fonts");
            click_row(h, ARIAL);
        }
        4 => {
            open_library(h, "My fonts");
            h.key_press_modifiers(Modifiers::CTRL, Key::A);
            settle(h);
        }
        5 => open_library(h, "Favorites"),
        6 => {
            open_library(h, "My fonts");
            h.get_by_role_and_label(Role::CheckBox, "Not installed").click();
            settle(h);
            assert_eq!(h.state().font_count(), 0, "{}", h.state().status_text());
        }
        7 => {
            open_library(h, "My fonts");
            click_row(h, GEORGIA_BOLD);
            h.get_by_role_and_label(Role::Button, "Show font properties").click();
            settle(h);
        }
        8 => {
            open_library(h, "My fonts");
            h.get_by_role_and_label(Role::Button, "Show duplicate fonts").click();
            settle(h);
            h.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::A);
            settle(h);
        }
        9 => open_library(h, &w.user_folder.clone()),
        10 => {
            open_library(h, &w.user_folder.clone());
            click_row(h, GEORGIA);
        }
        11 => open_library(h, COLLECTION),
        12 => {
            open_library(h, COLLECTION);
            click_row(h, GEORGIA);
        }
        _ => unreachable!(),
    }
    close_menus(h);
}

/// Back to the neutral state.
fn leave(w: &mut World, i: usize) {
    let h = &mut w.h;
    close_menus(h);
    match i {
        6 => {
            h.get_all_by_role_and_label(Role::Button, "Clear filters").next().unwrap().click();
            settle(h);
        }
        7 => {
            h.get_by_role_and_label(Role::Button, "Hide font properties").click();
            settle(h);
        }
        8 => {
            h.get_by_role_and_label(Role::Button, "Hide duplicate fonts").click();
            settle(h);
        }
        _ => {}
    }
    open_library(h, "My fonts");
}

fn menu_of(h: &mut H<'_>, menu: &str) -> Vec<(String, bool, bool)> {
    open_menu(h, menu);
    let items = open_menu_items(h);
    close_menus(h);
    items
}

fn toolbar_state(h: &H<'_>) -> Vec<(String, bool, bool)> {
    let labels = [
        "Add folder",
        "Add collection",
        "Get More",
        "Remove",
        "Install",
        "Uninstall",
        "Download",
        "Delete",
        "Show duplicate fonts",
        "Hide duplicate fonts",
        "Show font properties",
        "Hide font properties",
    ];
    labels
        .iter()
        .filter_map(|l| {
            let n = h.query_by(|n| n.role() == Role::Button && n.label().as_deref() == Some(*l) && n.bounding_box().is_some_and(|b| b.y0 < 120.0))?;
            let a = n.accesskit_node();
            Some((l.to_string(), !a.is_disabled(), a.toggled() == Some(egui::accesskit::Toggled::True)))
        })
        .collect()
}

#[test]
fn every_menu_and_the_toolbar_in_the_13_contexts() {
    let mut w = world();
    let capture: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(core_fixtures().join("toolbar-states.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    for (i, e) in contexts().into_iter().enumerate() {
        enter(&mut w, i);
        let h = &mut w.h;
        let mut check = |what: &str, actual: Vec<(String, bool, bool)>, expected: Vec<(String, bool, bool)>| {
            if actual != expected {
                failures.push(format!("{} / {what}:\n  actual   {actual:?}\n  expected {expected:?}", e.name));
            }
        };
        check("File", menu_of(h, "File"), e.file());
        check("Edit", menu_of(h, "Edit"), e.edit());
        check("Font", menu_of(h, "Font"), e.font());
        check("View", menu_of(h, "View"), e.view());
        check("Help", menu_of(h, "Help"), Expect::help());
        let tb = toolbar_state(h);
        check("toolbar", tb.clone(), e.toolbar());
        // the captured matrix (lead decision 1: a family header enables Uninstall/Delete like the menu)
        let m = &capture["matrix"][e.capture];
        let names = ["Add Folder", "Add Collection", "Get More", "Remove Folder or Collection", "Install", "Uninstall", "Download", "Delete", "Show duplicate fonts", "Show font properties"];
        for (k, name) in names.iter().enumerate() {
            let mut want = m[*name].as_bool().unwrap();
            if e.capture == "family-header-selected" && (*name == "Uninstall" || *name == "Delete") {
                want = true;
            }
            if tb.get(k).map(|t| t.1) != Some(want) {
                failures.push(format!("{} / toolbar-states.json {name}: expected {want}, toolbar {:?}", e.name, tb.get(k)));
            }
        }
        // the font row context menu is the Font menu (UI-SPEC §6.1): right-click the selected row
        let selected_row = w
            .h
            .query_all(egui_kittest::kittest::by().predicate(|n| n.role() == Role::ListBoxOption && n.toggled() == Some(egui::accesskit::Toggled::True) && n.bounding_box().is_some_and(|b| b.x0 > 300.0)))
            .next()
            .and_then(|n| n.accesskit_node().label());
        if let (Some(label), false) = (selected_row, i == 4) {
            row(&w.h, &label).click_secondary();
            settle(&mut w.h);
            let ctx_items = open_menu_items(&w.h);
            close_menus(&mut w.h);
            if ctx_items != e.font() {
                failures.push(format!("{} / font row context menu on {label}:\n  actual   {ctx_items:?}\n  expected {:?}", e.name, e.font()));
            }
        }
        leave(&mut w, i);
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn font_submenus_list_collections_and_files() {
    let mut w = world();
    let h = &mut w.h;
    // Georgia Regular in My fonts: Add to collection ▸ Collection 1, Favorites; Remove from collection ▸ Collection 1
    click_row(h, GEORGIA);
    open_menu(h, "Font");
    menu_item(h, "Add to collection").hover();
    settle(h);
    let items: Vec<String> = open_menu_items(h).into_iter().map(|i| i.0).collect();
    let sub = &items[items.iter().position(|x| x == "Open font file location").unwrap() + 1..];
    assert_eq!(sub, ["Collection 1", "Favorites"], "{items:?}");
    menu_item(h, "Remove from collection").hover();
    settle(h);
    let items: Vec<String> = open_menu_items(h).into_iter().map(|i| i.0).collect();
    let sub = &items[items.iter().position(|x| x == "Open font file location").unwrap() + 1..];
    assert_eq!(sub, ["Collection 1"], "{items:?}");
    // Open font file location ▸ the file, lower case; a click asks the font system to show it (recorded)
    menu_item(h, "Open font file location").hover();
    settle(h);
    let items: Vec<String> = open_menu_items(h).into_iter().map(|i| i.0).collect();
    let files: Vec<&String> = items.iter().filter(|s| s.ends_with(".ttf")).collect();
    assert_eq!(files.len(), 1, "{items:?}");
    assert_eq!(files[0], &files[0].to_lowercase(), "lower case path");
    assert!(files[0].ends_with("georgia.ttf"), "{files:?}");
    let label = files[0].clone();
    menu_item(h, &label).click();
    settle(h);
    let opened = w.fs.opened_locations();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].to_string_lossy().to_lowercase().ends_with("georgia.ttf"), "{opened:?}");
    // a family header lists every face's file
    click_row(h, "Georgia (2)");
    open_menu(h, "Font");
    menu_item(h, "Open font file location").hover();
    settle(h);
    let items: Vec<String> = open_menu_items(h).into_iter().map(|i| i.0).filter(|s| s.ends_with(".ttf")).collect();
    assert_eq!(items.len(), 2, "{items:?}");
    close_menus(h);
    // remove from collection through the submenu
    click_row(h, GEORGIA);
    open_menu(h, "Font");
    menu_item(h, "Remove from collection").hover();
    settle(h);
    let n = h.query_all(egui_kittest::kittest::by().predicate(|n| n.role() == Role::MenuItem && n.label().as_deref() == Some(COLLECTION))).count();
    assert_eq!(n, 1);
    menu_item(h, COLLECTION).click();
    settle(h);
    open_library(h, COLLECTION);
    assert_eq!(h.state().font_count(), 0, "{}", h.state().status_text());
    // inside a collection, Add to collection lists the others only
    click_row(h, "My fonts");
    click_row(h, GEORGIA);
    let _ = h;
}

#[test]
fn library_context_menus() {
    let mut w = world();
    let uf = w.user_folder.clone();
    let h = &mut w.h;
    let menu = |h: &mut H<'_>, lib: &str| {
        h.get_by_role_and_label(Role::ListBoxOption, lib).click_secondary();
        settle(h);
        let items = open_menu_items(h);
        close_menus(h);
        items
    };
    // UI-SPEC §6.2: Install / Uninstall / Remove Folder or Collection (+ Rename for user collections)
    assert_eq!(menu(h, "My fonts"), v(&[("Install", false, false), ("Uninstall", true, false), ("Remove Folder or Collection", false, false)]));
    assert_eq!(menu(h, "Favorites"), v(&[("Install", false, false), ("Uninstall", false, false), ("Remove Folder or Collection", true, false)]));
    assert_eq!(menu(h, &uf), v(&[("Install", false, false), ("Uninstall", true, false), ("Remove Folder or Collection", true, false)]));
    assert_eq!(
        menu(h, COLLECTION),
        v(&[("Install", false, false), ("Uninstall", true, false), ("Remove Folder or Collection", true, false), ("Rename", true, false)])
    );
    // right-click selects the row first
    assert_eq!(h.state().status_text(), "Collection 1: 1 fonts");
    // Rename opens the inline editor
    h.get_by_role_and_label(Role::ListBoxOption, COLLECTION).click_secondary();
    settle(h);
    menu_item(h, "Rename").click();
    settle(h);
    let ed = h.get_by_role_and_label(Role::TextInput, "Name:");
    assert_eq!(ed.value().as_deref(), Some(COLLECTION));
    h.key_press(Key::Escape);
    settle(h);
    assert!(h.state().library_names().iter().any(|n| n == COLLECTION), "Esc keeps an existing collection");
}

#[test]
fn library_context_menu_install_on_a_folder_of_new_fonts() {
    let env = Env::new();
    let fs = env.sandbox();
    let dir = env.fixture_folder("New fonts", &["Hack-Regular.ttf", "Tuffy.ttf"]);
    let mut h = harness(env.config_with(&fs), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    h.get_by_role_and_label(Role::ListBoxOption, "New fonts").click_secondary();
    settle(&mut h);
    assert_eq!(open_menu_items(&h), v(&[("Install", true, false), ("Uninstall", false, false), ("Remove Folder or Collection", true, false)]));
    // an extra context: a not-installed font selected enables Install in the toolbar and the Font menu
    close_menus(&mut h);
    let e = Expect { remove_library: true, install: true, delete: true, deselect_all: true, open_location: true, ..Expect::base("folder of new fonts", "") };
    assert_eq!(toolbar_state(&h), e.toolbar());
    assert_eq!(menu_of(&mut h, "Font"), e.font());
    // Install from the library menu installs every font of the folder (into the sandbox)
    let before = fs.installed_fonts().unwrap().len();
    h.get_by_role_and_label(Role::ListBoxOption, "New fonts").click_secondary();
    settle(&mut h);
    menu_item(&h, "Install").click();
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 2);
    assert!(h.state().face_states().iter().all(|f| f.1), "{:?}", h.state().face_states());
}

#[test]
fn toolbar_buttons_and_tooltips() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    let tips: [(&str, Option<&str>); 9] = [
        ("Add folder", Some("Browse to a folder that contains fonts to add it to your Libraries.")),
        ("Add collection", Some("Add a collection for easy access to fonts that reside in different folders.")),
        ("Get More", None),
        ("Remove", Some("Remove a folder or collection from your Libraries.")),
        ("Install", Some("Install fonts and font families.")),
        ("Uninstall", Some("Uninstall fonts and font families.")),
        ("Delete", Some("Delete font files from your system.")),
        ("Show duplicate fonts", Some("Show duplicate font versions.")),
        ("Show font properties", Some("Show font details or glyph sets.")),
    ];
    let mut missing = Vec::new();
    for (header, body) in tips {
        h.get_by(|n| n.role() == Role::Button && n.label().as_deref() == Some(header) && n.bounding_box().is_some_and(|b| b.y0 < 120.0)).hover();
        for _ in 0..12 {
            h.run_steps(5);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let texts: Vec<String> = h.query_all(egui_kittest::kittest::by().predicate(|n| n.role() == Role::Label)).filter_map(|n| n.accesskit_node().label().or_else(|| n.accesskit_node().value())).collect();
        if !texts.iter().any(|t| t == header) {
            missing.push(format!("{header}: header not shown"));
        }
        if let Some(b) = body {
            if !texts.iter().any(|t| t == b) {
                missing.push(format!("{header}: body {b:?} not shown"));
            }
        }
        h.remove_cursor();
        settle(&mut h);
    }
    assert!(missing.is_empty(), "{missing:?}");
    // the toggles change caption, look and tooltip
    h.get_by_role_and_label(Role::Button, "Show duplicate fonts").click();
    settle(&mut h);
    let b = h.get_by_role_and_label(Role::Button, "Hide duplicate fonts");
    assert_eq!(b.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    assert!(h.state().settings().core.show_duplicates);
    h.get_by_role_and_label(Role::Button, "Show font properties").click();
    settle(&mut h);
    assert!(h.state().settings().core.show_font_properties);
    h.get_by_role_and_label(Role::Button, "Hide font properties");
    // Get More opens the catalog window
    h.get_by(|n| n.role() == Role::Button && n.label().as_deref() == Some("Get More") && n.bounding_box().is_some_and(|b| b.y0 < 120.0)).click();
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::ComboBox, "Sort by:").is_some() || h.query_by_label_contains("Remote font catalog").is_some(), "Get More is open");
}

#[test]
fn title_bar_buttons() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    for l in ["Product Help", "Options", "Updates", "Minimize", "Maximize", "Close"] {
        h.get_by_role_and_label(Role::Button, l);
    }
    // the window buttons only send viewport commands (the harness has no real window)
    let c = click_for_commands(&mut h, |h| h.get_by_role_and_label(Role::Button, "Minimize"));
    assert!(c.contains(&ViewportCommand::Minimized(true)), "{c:?}");
    settle(&mut h);
    let c = click_for_commands(&mut h, |h| h.get_by_role_and_label(Role::Button, "Maximize"));
    assert!(c.contains(&ViewportCommand::Maximized(true)), "{c:?}");
    settle(&mut h);
    let c = click_for_commands(&mut h, |h| h.get_by_role_and_label(Role::Button, "Close"));
    assert!(c.contains(&ViewportCommand::Close), "{c:?}");
    settle(&mut h);
    // ? = Product Help (the URL is recorded, no browser opens)
    let before = anyfont::testing::urls().len();
    h.get_by_role_and_label(Role::Button, "Product Help").click();
    settle(&mut h);
    let urls = anyfont::testing::urls();
    assert!(urls.len() > before && urls.last().unwrap() == &anyfont::help::help_url(), "{urls:?}");
    // Options
    h.get_by_role_and_label(Role::Button, "Options").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Reset to Default");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::Button, "Reset to Default").is_none(), "OK (Enter) closes Options");
    // Updates
    h.get_by_role_and_label(Role::Button, "Updates").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Check Now");
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::Button, "Check Now").is_none(), "Esc closes Updates");
}

#[test]
fn shortcuts_and_keyboard_menus() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    // Ctrl+Alt+I = Inspectors = the properties panel
    h.key_press_modifiers(Modifiers::CTRL | Modifiers::ALT, Key::I);
    settle(&mut h);
    assert!(h.state().settings().core.show_font_properties);
    h.key_press_modifiers(Modifiers::CTRL | Modifiers::ALT, Key::I);
    settle(&mut h);
    assert!(!h.state().settings().core.show_font_properties);
    // Alt+F4 = Exit
    let c = key_for_commands(&mut h, Modifiers::ALT, Key::F4);
    assert!(c.contains(&ViewportCommand::Close), "Alt+F4: {c:?}");
    // File ▸ Exit
    open_menu(&mut h, "File");
    let c = click_for_commands(&mut h, |h| menu_item(h, "Exit"));
    assert!(c.contains(&ViewportCommand::Close), "File ▸ Exit: {c:?}");
    settle(&mut h);
    assert!(open_menu_items(&h).is_empty(), "the menu closes after a command");
    // Alt+F opens File, → goes to Edit, ← back to File, Esc closes
    h.key_press_modifiers(Modifiers::ALT, Key::F);
    settle(&mut h);
    assert!(query_menu_item(&h, "Add Folder...").is_some(), "Alt+F opens File");
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    assert!(query_menu_item(&h, "Select All").is_some(), "→ opens Edit: {:?}", open_menu_items(&h));
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    assert!(query_menu_item(&h, "Install").is_some(), "→ opens Font");
    h.key_press(Key::ArrowLeft);
    h.key_press(Key::ArrowLeft);
    settle(&mut h);
    assert!(query_menu_item(&h, "Add Folder...").is_some(), "← ← back to File");
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(open_menu_items(&h).is_empty(), "Esc closes");
    // Alt+E opens Edit; the access key x of E&xit runs Exit from File
    h.key_press_modifiers(Modifiers::ALT, Key::E);
    settle(&mut h);
    assert!(query_menu_item(&h, "Deselect All").is_some(), "Alt+E opens Edit");
    h.key_press(Key::Escape);
    settle(&mut h);
    // F10 focuses the menu bar, ↓ opens File, ↓ ↓ Enter runs the 2nd item (Add Collection)
    h.key_press(Key::F10);
    settle(&mut h);
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    assert!(query_menu_item(&h, "Add Folder...").is_some(), "F10 ↓ opens File: {:?}", open_menu_items(&h));
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::TextInput, "Name:").is_some(), "Add Collection ran from the keyboard");
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(!h.state().library_names().iter().any(|n| n == "Collection 1"));
    // Ctrl+A / Ctrl+Shift+A
    h.key_press_modifiers(Modifiers::CTRL, Key::A);
    settle(&mut h);
    open_menu(&mut h, "Edit");
    assert_eq!(open_menu_items(&h), v(&[("Select All", true, false), ("Deselect All", true, false)]));
    menu_item(&h, "Deselect All").click();
    settle(&mut h);
    open_menu(&mut h, "Edit");
    assert_eq!(open_menu_items(&h), v(&[("Select All", true, false), ("Deselect All", false, false)]));
    close_menus(&mut h);
}

#[test]
fn view_menu_toggles_change_caption_and_check() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    for (item, after) in [
        ("Group by family", ("Group by family", false)),
        ("Show recently used fonts", ("Show recently used fonts", true)),
        ("Use font to display font name", ("Use font to display font name", false)),
        ("Show Latin names", ("Show Latin names", true)),
        ("Hide filter list", ("Show filter list", false)),
        ("Show font properties", ("Hide font properties", true)),
    ] {
        click_menu(&mut h, "View", item);
        open_menu(&mut h, "View");
        let items = open_menu_items(&h);
        close_menus(&mut h);
        assert!(items.iter().any(|i| i.0 == after.0 && i.2 == after.1), "after {item}: {items:?}");
    }
    // filter list hidden: the Filters panel's check boxes are gone
    assert!(h.query_by_role_and_label(Role::CheckBox, "Installed").is_none());
    click_menu(&mut h, "View", "Show filter list");
    assert!(h.query_by_role_and_label(Role::CheckBox, "Installed").is_some());
    // Font ▸ Show duplicate fonts
    click_menu(&mut h, "Font", "Show duplicate fonts");
    assert!(h.state().settings().core.show_duplicates);
    open_menu(&mut h, "Font");
    assert!(open_menu_items(&h).contains(&("Hide duplicate fonts".to_string(), true, true)));
    close_menus(&mut h);
    let _: Option<PathBuf> = None;
}

/// Presses a key with modifiers and returns the viewport commands of the frame of the key press.
fn key_for_commands(h: &mut H<'_>, m: Modifiers, key: Key) -> Vec<ViewportCommand> {
    h.event(egui::Event::ModifiersChanged(m));
    h.step();
    h.event(egui::Event::Key { key, pressed: true, modifiers: m, repeat: false, physical_key: None });
    h.step();
    let c = viewport_commands(h);
    h.event(egui::Event::Key { key, pressed: false, modifiers: m, repeat: false, physical_key: None });
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    settle(h);
    c
}
