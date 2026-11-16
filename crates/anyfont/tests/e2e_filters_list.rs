//! End-to-end checks of the Filters panel and the font list: every filter option against the core's
//! predicate, OR inside a group and AND across groups, Clear filters, the empty states, Search, the sample
//! text combo, the preview size control, the View toggles, the selection (mouse, keyboard, type-ahead) and
//! the edit box context menu. Every test runs the real window on a sandbox and a folder of fixture fonts.
//!
//! The tests of this binary run one at a time (`env_lock`): one of them switches the global UI language.
mod common;

use anyfont_core::catalog::LibraryRef;
use anyfont_core::filters::{FilterGroup, FilterOption};
use anyfont_core::{Catalog, FontDatabase};
use common::*;
use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers};
use egui_kittest::kittest::{by, NodeT, Queryable};
use std::path::{Path, PathBuf};

/// The fixture fonts of the filter tests (fonts of known properties; the `%WINDIR%` ones are skipped when
/// absent, the expectations come from the core so the set may vary).
const FIXTURES: &[&str] = &[
    "Hack-Regular.ttf",
    "TTC.ttc",
    "colr_1_variable.ttf",
    "Tuffy.ttf",
    "Ubuntu-Light.ttf",
    "NotoEmoji-Regular.ttf",
    "georgia.ttf",
    "georgiab.ttf",
    "verdana.ttf",
    "consola.ttf",
    "bahnschrift.ttf",
    "comic.ttf",
    "segoesc.ttf",
    "symbol.ttf",
    "wingding.ttf",
    "msgothic.ttc",
    "malgun.ttf",
    "simsun.ttc",
];

/// A small folder for the list tests: Comic Sans MS, Consolas, Georgia (2 styles), Hack, Tuffy, Verdana.
const SMALL: &[&str] = &["georgia.ttf", "georgiab.ttf", "verdana.ttf", "consola.ttf", "comic.ttf", "Tuffy.ttf", "Hack-Regular.ttf"];

const EN: u16 = 0x0409;

/// Ctrl as egui-winit reports it on Windows (ctrl and command).
const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

// ---------------------------------------------------------------------------------------------- helpers

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

fn painted_contains(h: &H<'_>, text: &str) -> bool {
    painted(h).iter().any(|t| t == text)
}

fn list_rect(h: &H<'_>) -> egui::Rect {
    h.get_by_role_and_label(Role::Unknown, "Font list").rect()
}

/// The visible rows of the font list: (label, selected).
fn list_rows(h: &H<'_>) -> Vec<(String, bool)> {
    let lr = list_rect(h);
    h.query_all(by().role(Role::ListBoxOption))
        .filter(|n| lr.contains(n.rect().center()))
        .map(|n| {
            let a = n.accesskit_node();
            (a.label().unwrap_or_default(), a.toggled() == Some(Toggled::True))
        })
        .collect()
}

fn selected_rows(h: &H<'_>) -> Vec<String> {
    list_rows(h).into_iter().filter(|(_, s)| *s).map(|(l, _)| l).collect()
}

/// A visible row of the font list by its label.
fn row<'t>(h: &'t H<'_>, label: &'t str) -> egui_kittest::Node<'t> {
    let lr = list_rect(h);
    h.query_all(by().role(Role::ListBoxOption).label(label))
        .find(|n| lr.contains(n.rect().center()))
        .unwrap_or_else(|| panic!("row {label:?} not shown; rows: {:?}", list_rows(h)))
}

fn checked(n: &egui_kittest::Node<'_>) -> bool {
    n.accesskit_node().toggled() == Some(Toggled::True)
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The face labels the list shows (sorted).
fn app_faces(h: &H<'_>) -> Vec<String> {
    sorted(h.state().face_states().into_iter().map(|(l, _)| l).collect())
}

/// A window on a sandbox with the folder `name` of `files` added and open.
struct Setup<'a> {
    _env: Env,
    h: H<'a>,
    db: PathBuf,
    dir: PathBuf,
}

fn setup<'a>(name: &str, files: &[&str], size: (f32, f32), tweak: impl FnOnce(&mut anyfont::TestConfig)) -> Setup<'a> {
    let env = Env::new();
    let dir = env.fixture_folder(name, files);
    let mut cfg = env.config();
    tweak(&mut cfg);
    let db = cfg.data_dir.join("data").join("fonts.db");
    let mut h = harness(cfg, size);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    Setup { _env: env, h, db, dir }
}

/// The catalog as the core reads it from the window's database, and the folder's library.
fn core(db: &Path, dir: &Path) -> (Catalog, LibraryRef) {
    let db = FontDatabase::open(db).expect("open the window's database");
    let cat = Catalog::load(&db).expect("catalog");
    let id = cat.folders().iter().find(|f| f.path == dir || f.path.ends_with(dir.file_name().unwrap())).expect("folder in the catalog").id;
    (cat, LibraryRef::Folder(id))
}

fn expected(cat: &Catalog, lib: &LibraryRef, pred: impl Fn(&anyfont_core::model::FontRecord) -> bool) -> Vec<String> {
    sorted(cat.visible_fonts(lib, false, false).into_iter().filter(|f| pred(f)).map(|f| f.label(EN, false)).collect())
}

fn click_filter(h: &mut H<'_>, caption: &str) {
    h.get_by_role_and_label(Role::CheckBox, caption).click();
    settle(h);
}

fn clear_filters_enabled(h: &H<'_>) -> bool {
    let lr = list_rect(h);
    h.query_all(by().role(Role::Button).label("Clear filters")).filter(|n| !lr.contains(n.rect().center())).any(|n| !n.accesskit_node().is_disabled())
}

// ------------------------------------------------------------------------------------------- Filters

/// Every option of every group shown for the fixture folder lists exactly the faces the core's
/// `FilterOption::matches` accepts; unchecking it restores the whole folder.
#[test]
fn every_filter_option_lists_the_matching_fonts() {
    let _g = env_lock();
    let mut s = setup("Fixtures", FIXTURES, (1280.0, 1500.0), |_| {});
    let (cat, lib) = core(&s.db, &s.dir);
    let all = expected(&cat, &lib, |_| true);
    assert_eq!(app_faces(&s.h), all, "the folder lists every visible face");
    assert!(!clear_filters_enabled(&s.h), "Clear filters is disabled without a filter");
    let groups = cat.available_filters(&lib);
    assert!(groups.len() >= 8, "groups: {:?}", groups.iter().map(|g| g.group.caption()).collect::<Vec<_>>());
    let mut tested = 0;
    for g in &groups {
        let cap = g.group.caption();
        let header = s.h.get_by_role_and_label(Role::DisclosureTriangle, cap);
        let expanded = checked(&header);
        assert_eq!(expanded, g.group == FilterGroup::FontStatus, "{cap}: only Font Status starts expanded");
        if !expanded {
            header.click();
            settle(&mut s.h);
            assert!(checked(&s.h.get_by_role_and_label(Role::DisclosureTriangle, cap)), "{cap} expands");
        }
        for o in &g.options {
            let want = expected(&cat, &lib, |f| o.matches(f));
            click_filter(&mut s.h, o.caption());
            assert!(checked(&s.h.get_by_role_and_label(Role::CheckBox, o.caption())), "{cap} ▸ {} checked", o.caption());
            assert_eq!(app_faces(&s.h), want, "{cap} ▸ {}", o.caption());
            assert_eq!(s.h.state().status_text(), format!("Fixtures (filtered): {} fonts", want.len()));
            assert!(clear_filters_enabled(&s.h), "Clear filters is enabled with a filter");
            if want.is_empty() {
                assert!(painted_contains(&s.h, "No fonts found. Remove or change filters or explore the font packs available in Get More."), "{:?}", painted(&s.h));
            }
            click_filter(&mut s.h, o.caption());
            assert_eq!(app_faces(&s.h), all, "{cap} ▸ {} unchecked", o.caption());
            tested += 1;
        }
        if g.group != FilterGroup::FontStatus {
            s.h.get_by_role_and_label(Role::DisclosureTriangle, cap).click();
            settle(&mut s.h);
            assert!(s.h.query_by_role_and_label(Role::CheckBox, g.options[0].caption()).is_none(), "{cap} collapses");
        }
    }
    assert_eq!(s.h.state().status_text(), format!("Fixtures: {} fonts", all.len()));
    assert!(tested >= 40, "options tested: {tested}");
}

/// OR inside a group, AND across groups; both Clear filters buttons; the empty state.
#[test]
fn filters_combine_or_within_and_across_groups() {
    let _g = env_lock();
    let mut s = setup("Fixtures", FIXTURES, (1280.0, 1500.0), |_| {});
    let (cat, lib) = core(&s.db, &s.dir);
    let opt = |cap: &str| FilterOption::all().into_iter().find(|o| o.caption() == cap).unwrap();
    let (mono, serif, bold) = (opt("Monospaced"), opt("Serif"), opt("Bold"));

    s.h.get_by_role_and_label(Role::DisclosureTriangle, "Style").click();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::DisclosureTriangle, "Weight").click();
    settle(&mut s.h);
    click_filter(&mut s.h, "Monospaced");
    click_filter(&mut s.h, "Serif");
    let or = expected(&cat, &lib, |f| mono.matches(f) || serif.matches(f));
    assert!(or.len() > expected(&cat, &lib, |f| mono.matches(f)).len(), "Serif adds Georgia");
    assert_eq!(app_faces(&s.h), or, "Monospaced OR Serif");
    click_filter(&mut s.h, "Bold");
    let and = expected(&cat, &lib, |f| (mono.matches(f) || serif.matches(f)) && bold.matches(f));
    assert_eq!(app_faces(&s.h), and, "(Monospaced OR Serif) AND Bold");
    assert_eq!(and, vec!["Georgia (Bold)".to_string()]);

    // the panel's Clear filters button
    s.h.get_all_by_role_and_label(Role::Button, "Clear filters").next().unwrap().click();
    settle(&mut s.h);
    assert_eq!(app_faces(&s.h), expected(&cat, &lib, |_| true));
    assert!(!checked(&s.h.get_by_role_and_label(Role::CheckBox, "Serif")));
    assert!(!clear_filters_enabled(&s.h));
    assert!(!s.h.state().status_text().contains("(filtered)"));

    // nothing matches: the empty state and its own Clear filters button
    click_filter(&mut s.h, "Installed");
    assert_eq!(s.h.state().font_count(), 0);
    assert!(painted_contains(&s.h, "No fonts found. Remove or change filters or explore the font packs available in Get More."));
    let lr = list_rect(&s.h);
    let n = s.h.query_all(by().role(Role::Button).label("Clear filters")).count();
    assert_eq!(n, 2, "the panel's and the empty state's Clear filters");
    s.h.query_all(by().role(Role::Button).label("Clear filters")).find(|n| lr.contains(n.rect().center())).expect("empty-state button").click();
    settle(&mut s.h);
    assert_eq!(app_faces(&s.h), expected(&cat, &lib, |_| true));
    assert!(!checked(&s.h.get_by_role_and_label(Role::CheckBox, "Installed")));
    // a single row is selected again
    assert_eq!(selected_rows(&s.h).len(), 1, "{:?}", list_rows(&s.h));
    // collapsing a group is remembered in the settings (default expanded state XOR the stored key)
    assert!(s.h.state().settings().is_collapsed("filter:Style"));
    s.h.get_by_role_and_label(Role::DisclosureTriangle, "Font Status").click();
    settle(&mut s.h);
    assert!(s.h.state().settings().is_collapsed("filter:Font Status"));
    assert!(s.h.query_by_role_and_label(Role::CheckBox, "Installed").is_none());
}

/// Opening another library clears the filters.
#[test]
fn opening_another_library_clears_the_filters() {
    let _g = env_lock();
    let mut s = setup("Small", SMALL, (1280.0, 1100.0), |_| {});
    click_filter(&mut s.h, "Not installed");
    assert!(s.h.state().status_text().contains("(filtered)"));
    s.h.get_by_role_and_label(Role::ListBoxOption, "My fonts").click();
    settle(&mut s.h);
    assert!(!s.h.state().status_text().contains("(filtered)"), "{}", s.h.state().status_text());
    assert!(!checked(&s.h.get_by_role_and_label(Role::CheckBox, "Not installed")));
}

// -------------------------------------------------------------------------------------------- Search

#[test]
fn search_filters_live_flat_list_suggestions_and_escape() {
    let _g = env_lock();
    let mut s = setup("Small", SMALL, (1280.0, 1100.0), |_| {});
    let all = s.h.state().font_count();
    assert_eq!(all, 7);
    assert!(s.h.state().row_labels().contains(&"Georgia".to_string()), "grouped: a Georgia header");

    s.h.get_by_role_and_label(Role::TextInput, "Search").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("verd");
    settle(&mut s.h);
    // live (the names, localized names and styles are searched)
    assert_eq!(s.h.state().row_labels(), vec!["Verdana (Regular, Normal)"]);
    for _ in 0..4 {
        s.h.key_press(Key::Backspace);
        settle(&mut s.h);
    }
    assert_eq!(s.h.state().font_count(), all);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("geo");
    settle(&mut s.h);
    // flat list (no family header), relevance order: Regular before Bold
    assert_eq!(s.h.state().row_labels(), vec!["Georgia (Regular, Normal)", "Georgia (Bold)"]);
    assert_eq!(s.h.state().status_text(), "Small (filtered): 2 fonts");
    // a suggestion is not shown for the exact family name only; "geo" ≠ "georgia": one suggestion
    assert!(s.h.query_by_role_and_label(Role::Button, "Georgia").is_some(), "suggestion Georgia");
    // Esc clears the box
    s.h.key_press(Key::Escape);
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().unwrap_or_default(), "");
    assert_eq!(s.h.state().font_count(), all);
    assert!(!s.h.state().status_text().contains("(filtered)"));

    // suggestions: "co" → Comic Sans MS and Consolas; picking one searches for it
    s.h.get_by_role_and_label(Role::TextInput, "Search").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("co");
    settle(&mut s.h);
    let lr = list_rect(&s.h);
    let sugg: Vec<String> = s
        .h
        .query_all(by().role(Role::Button))
        .filter(|n| n.rect().top() < lr.top() + 200.0 && ["Comic Sans MS", "Consolas"].contains(&n.accesskit_node().label().unwrap_or_default().as_str()))
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(sorted(sugg), vec!["Comic Sans MS", "Consolas"]);
    s.h.get_by_role_and_label(Role::Button, "Consolas").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().as_deref(), Some("Consolas"));
    assert_eq!(s.h.state().row_labels(), vec!["Consolas (Regular, Normal)"]);
    assert_eq!(selected_rows(&s.h), vec!["Consolas (Regular, Normal)"]);

    // no match: the search empty state
    s.h.get_by_role_and_label(Role::TextInput, "Search").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("zzz");
    settle(&mut s.h);
    assert_eq!(s.h.state().font_count(), 0);
    assert!(painted_contains(&s.h, "No fonts found. Clear the Search box, type a different keyword, or explore the font packs available in Get More."), "{:?}", painted(&s.h));
    s.h.key_press(Key::Escape);
    settle(&mut s.h);
    assert_eq!(s.h.state().font_count(), all);
}

/// Search combined with a filter: both apply (AND).
#[test]
fn search_and_filters_combine() {
    let _g = env_lock();
    let mut s = setup("Small", SMALL, (1280.0, 1100.0), |_| {});
    s.h.get_by_role_and_label(Role::DisclosureTriangle, "Weight").click();
    settle(&mut s.h);
    click_filter(&mut s.h, "Bold");
    assert_eq!(s.h.state().row_labels(), vec!["Georgia (Bold)"]);
    s.h.get_by_role_and_label(Role::TextInput, "Search").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("verdana");
    settle(&mut s.h);
    assert_eq!(s.h.state().font_count(), 0);
    // with a filter checked, the empty state is the filters' one
    assert!(painted_contains(&s.h, "No fonts found. Remove or change filters or explore the font packs available in Get More."));
}

// ----------------------------------------------------------------------------------- sample text

#[test]
fn sample_text_combo_presets_free_text_and_history() {
    let _g = env_lock();
    let mut s = setup("Small", SMALL, (1280.0, 1100.0), |_| {});
    let sample = |h: &H<'_>| h.get_by_role_and_label(Role::TextInput, "Enter sample text").value().unwrap_or_default();
    assert_eq!(sample(&s.h), "");
    let open = |h: &mut H<'_>| {
        h.get_by_role_and_label(Role::ComboBox, "Enter sample text").click();
        settle(h);
    };
    open(&mut s.h);
    for p in ["Font Name", "abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "1234567890", "The quick brown fox jumps over the lazy dog."] {
        assert!(s.h.query_by_role_and_label(Role::Button, p).is_some(), "preset {p}");
    }
    s.h.get_by_role_and_label(Role::Button, "1234567890").click();
    settle(&mut s.h);
    assert_eq!(sample(&s.h), "1234567890");
    assert!(s.h.query_by_role_and_label(Role::Button, "abcdefghijklmnopqrstuvwxyz").is_none(), "the drop-down closes");
    // the previews show the sample (painted text when previews are not rendered in the font)
    open(&mut s.h);
    s.h.get_by_role_and_label(Role::Button, "The quick brown fox jumps over the lazy dog.").click();
    settle(&mut s.h);
    assert_eq!(sample(&s.h), "The quick brown fox jumps over the lazy dog.");
    // "Font Name" empties the box (previews show the family)
    open(&mut s.h);
    s.h.get_by_role_and_label(Role::Button, "Font Name").click();
    settle(&mut s.h);
    assert_eq!(sample(&s.h), "");
    // Esc closes the drop-down without a choice
    open(&mut s.h);
    s.h.key_press(Key::Escape);
    settle(&mut s.h);
    assert!(s.h.query_by_role_and_label(Role::Button, "1234567890").is_none());

    // free text, remembered in the history when the box loses the focus
    s.h.get_by_role_and_label(Role::TextInput, "Enter sample text").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Enter sample text").type_text("Hello AnyFont");
    settle(&mut s.h);
    assert_eq!(sample(&s.h), "Hello AnyFont");
    row(&s.h, "Verdana (Regular, Normal)").click();
    settle(&mut s.h);
    assert_eq!(s.h.state().settings().core.sample_history.first().map(String::as_str), Some("Hello AnyFont"));
    open(&mut s.h);
    assert!(s.h.query_by_role_and_label(Role::Button, "Hello AnyFont").is_some(), "the history in the drop-down");
    s.h.key_press(Key::Escape);
    settle(&mut s.h);
    // a preset is not added to the history
    open(&mut s.h);
    s.h.get_by_role_and_label(Role::Button, "1234567890").click();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Enter sample text").focus();
    settle(&mut s.h);
    row(&s.h, "Verdana (Regular, Normal)").click();
    settle(&mut s.h);
    assert_eq!(s.h.state().settings().core.sample_history, vec!["Hello AnyFont".to_string()]);
}

// --------------------------------------------------------------------------------- preview size

#[test]
fn preview_size_buttons_and_slider_stay_within_8_and_86() {
    let _g = env_lock();
    let mut s = setup("Small", SMALL, (1280.0, 1100.0), |_| {});
    let size = |h: &H<'_>| h.state().settings().core.preview_size;
    let tip = "Change the font size in the font list.";
    let buttons = |h: &H<'_>| {
        let mut b: Vec<egui::Rect> = h.query_all(by().role(Role::Button).label(tip)).map(|n| n.rect()).collect();
        b.sort_by(|a, b| a.left().total_cmp(&b.left()));
        b
    };
    let v0 = size(&s.h);
    assert!((8.0..=86.0).contains(&v0));
    let b = buttons(&s.h);
    assert_eq!(b.len(), 2);
    let click_at = |h: &mut H<'_>, p: egui::Pos2| {
        h.hover_at(p);
        h.step();
        h.drag_at(p);
        h.step();
        h.drop_at(p);
        settle(h);
    };
    click_at(&mut s.h, b[1].center());
    assert_eq!(size(&s.h), v0 + 1.0, "big A");
    click_at(&mut s.h, b[0].center());
    click_at(&mut s.h, b[0].center());
    assert_eq!(size(&s.h), v0 - 1.0, "small A");
    // the slider: its left end is 8, its right end 86
    let sl = s.h.get_by_role_and_label(Role::Slider, tip).rect();
    click_at(&mut s.h, egui::pos2(sl.right() - 1.0, sl.center().y));
    assert_eq!(size(&s.h), 86.0);
    assert_eq!(s.h.get_by_role_and_label(Role::Slider, tip).accesskit_node().numeric_value(), Some(86.0));
    click_at(&mut s.h, b[1].center());
    assert_eq!(size(&s.h), 86.0, "big A stops at 86");
    click_at(&mut s.h, egui::pos2(sl.left() + 1.0, sl.center().y));
    assert_eq!(size(&s.h), 8.0);
    click_at(&mut s.h, b[0].center());
    assert_eq!(size(&s.h), 8.0, "small A stops at 8");
    // the rows follow the size: more rows fit at 8 than at 86
    let small_rows = list_rows(&s.h).len();
    click_at(&mut s.h, egui::pos2(sl.right() - 1.0, sl.center().y));
    assert!(list_rows(&s.h).len() <= small_rows);

    // with "Use font to display font name" off, the control is disabled
    click_menu(&mut s.h, "View", "Use font to display font name");
    assert!(!s.h.state().settings().core.render_name_in_font);
    assert!(s.h.query_all(by().role(Role::Button).label(tip)).all(|n| n.accesskit_node().is_disabled()));
    let before = size(&s.h);
    let b0 = buttons(&s.h)[0].center();
    click_at(&mut s.h, b0);
    assert_eq!(size(&s.h), before, "disabled");
}

// ----------------------------------------------------------------------------------- View toggles

#[test]
fn view_toggles_group_recent_render_and_filter_list() {
    let _g = env_lock();
    let mut s = setup("Small", SMALL, (1280.0, 1100.0), |_| {});
    let view = |h: &mut H<'_>| -> Vec<(String, bool, bool)> {
        open_menu(h, "View");
        let v = open_menu_items(h);
        close_menus(h);
        v
    };
    let state_of = |items: &[(String, bool, bool)], l: &str| items.iter().find(|i| i.0 == l).map(|i| i.2);
    let v = view(&mut s.h);
    assert_eq!(state_of(&v, "Group by family"), Some(true));
    assert_eq!(state_of(&v, "Show recently used fonts"), Some(false));
    assert_eq!(state_of(&v, "Use font to display font name"), Some(true));
    assert_eq!(state_of(&v, "Show Latin names"), Some(false));
    assert_eq!(state_of(&v, "Hide filter list"), Some(true));

    // Group by family: off → no family header, the faces in a flat list
    let grouped = s.h.state().row_labels();
    assert!(grouped.contains(&"Georgia".to_string()));
    assert!(row(&s.h, "Georgia (2)").accesskit_node().role() == Role::ListBoxOption);
    click_menu(&mut s.h, "View", "Group by family");
    assert!(!s.h.state().settings().core.group_by_family);
    let flat = s.h.state().row_labels();
    assert!(!flat.contains(&"Georgia".to_string()), "{flat:?}");
    assert_eq!(flat.len(), 7);
    assert!(list_rows(&s.h).iter().all(|(l, _)| l != "Georgia (2)"));
    click_menu(&mut s.h, "View", "Group by family");
    assert_eq!(s.h.state().row_labels(), grouped);

    // Use font to display font name: off, then on again
    click_menu(&mut s.h, "View", "Use font to display font name");
    assert!(!s.h.state().settings().core.render_name_in_font);
    // the previews are painted in the UI font: the family names appear as painted text
    assert!(painted_contains(&s.h, "Verdana"), "{:?}", painted(&s.h));
    click_menu(&mut s.h, "View", "Use font to display font name");
    assert!(s.h.state().settings().core.render_name_in_font);

    // Hide filter list / Show filter list
    click_menu(&mut s.h, "View", "Hide filter list");
    assert!(!s.h.state().settings().show_filter_list);
    assert!(s.h.query_by_role_and_label(Role::CheckBox, "Installed").is_none());
    assert!(s.h.query_by_role_and_label(Role::DisclosureTriangle, "Font Status").is_none());
    let v = view(&mut s.h);
    assert_eq!(state_of(&v, "Show filter list"), Some(false));
    click_menu(&mut s.h, "View", "Show filter list");
    assert!(s.h.state().settings().show_filter_list);
    s.h.get_by_role_and_label(Role::CheckBox, "Installed");
}

/// A double-click on a face (both clicks in one frame: a double click for egui) and the database's MRU.
fn double_click(h: &mut H<'_>, p: egui::Pos2) {
    h.hover_at(p);
    h.step();
    // Both clicks in one frame: queued events run one frame each, and a harness frame is 0.25 s, so four
    // queued events put 0.5 s between the two releases, more than egui's double-click delay (0.3 s).
    let button = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    h.input_mut().events.extend([button(true), button(false), button(true), button(false)]);
    h.step();
    settle(h);
}

/// Show recently used fonts: a double-clicked face is listed again at the top.
#[test]
fn recently_used_fonts_after_a_double_click() {
    let _g = env_lock();
    let mut s = small();
    click_menu(&mut s.h, "View", "Show recently used fonts");
    assert!(s.h.state().settings().core.show_recent);
    let n = s.h.state().face_states().len();
    let p = row(&s.h, "Verdana (Regular, Normal)").rect().center();
    double_click(&mut s.h, p);
    let (cat, _) = core(&s.db, &s.dir);
    assert_eq!(cat.mru().len(), 1, "the double-click is recorded in the database");
    // the list shows it once the catalog is reloaded (View ▸ Show recently used fonts off and on)
    click_menu(&mut s.h, "View", "Show recently used fonts");
    click_menu(&mut s.h, "View", "Show recently used fonts");
    let faces = s.h.state().face_states();
    assert_eq!(faces.len(), n + 1, "the recent face is listed twice: {faces:?}");
    assert_eq!(faces[0].0, "Verdana (Regular, Normal)");
    click_menu(&mut s.h, "View", "Show recently used fonts");
    assert_eq!(s.h.state().face_states().len(), n);
}

/// The double-click itself works in the harness (it selects the row like a click).
#[test]
fn double_click_selects_the_row() {
    let _g = env_lock();
    let mut s = small();
    let p = row(&s.h, "Verdana (Regular, Normal)").rect().center();
    double_click(&mut s.h, p);
    assert_eq!(selected_rows(&s.h), vec!["Verdana (Regular, Normal)"]);
}

/// Show/Hide duplicate fonts. An identical copy of a file is the same face (one row, two paths); a changed
/// copy of the same face is a duplicate: hidden by default, listed by the Duplicates filter and by
/// Show duplicate fonts, and "Open font file location" lists every file.
#[test]
fn duplicate_fonts_hidden_shown_and_located() {
    let _g = env_lock();
    let env = Env::new();
    let dir = env.fixture_folder("Dups", &["georgia.ttf", "verdana.ttf"]);
    if !dir.join("georgia.ttf").exists() || !dir.join("verdana.ttf").exists() {
        eprintln!("georgia.ttf / verdana.ttf missing: skipped");
        return;
    }
    // an identical copy, and a copy with 4 bytes appended (another file of the same face)
    std::fs::create_dir_all(dir.join("copy")).unwrap();
    std::fs::copy(dir.join("georgia.ttf"), dir.join("copy").join("georgia.ttf")).unwrap();
    std::fs::create_dir_all(dir.join("alt")).unwrap();
    let mut bytes = std::fs::read(dir.join("georgia.ttf")).unwrap();
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    std::fs::write(dir.join("alt").join("georgia.ttf"), bytes).unwrap();
    let fs = env.sandbox();
    let mut h = harness(env.config_with(&fs), (1280.0, 1100.0));
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    assert_eq!(h.state().status_text(), "Dups: 2 fonts", "the duplicate is hidden: {:?}", h.state().row_labels());
    assert!(!h.state().settings().core.show_duplicates);
    // the Duplicates filter lists the shown face of the duplicate set
    click_filter(&mut h, "Duplicates");
    assert_eq!(h.state().row_labels(), vec!["Georgia (Regular, Normal)"]);
    click_filter(&mut h, "Duplicates");

    // toolbar ▸ Show duplicate fonts: both faces listed
    h.get_by_role_and_label(Role::Button, "Show duplicate fonts").click();
    settle(&mut h);
    assert!(h.state().settings().core.show_duplicates);
    assert_eq!(h.state().font_count(), 3, "{:?}", h.state().row_labels());
    let georgia = h.state().face_states().iter().filter(|(l, _)| l == "Georgia (Regular, Normal)").count();
    assert_eq!(georgia, 2);
    click_filter(&mut h, "Duplicates");
    assert_eq!(h.state().font_count(), 2, "{:?}", h.state().row_labels());
    click_filter(&mut h, "Duplicates");
    // "Open font file location" of each Georgia row: the identical copies are one face with two files, the
    // changed copy is the other face; three files in all (lower case)
    let lr = list_rect(&h);
    let g: Vec<egui::Pos2> = h.query_all(by().role(Role::ListBoxOption).label("Georgia (Regular, Normal)")).filter(|n| lr.contains(n.rect().center())).map(|n| n.rect().center()).collect();
    assert_eq!(g.len(), 2);
    let mut files: Vec<String> = Vec::new();
    let mut per_row = Vec::new();
    for p in &g {
        h.query_all(by().role(Role::ListBoxOption).label("Georgia (Regular, Normal)")).find(|n| n.rect().center() == *p).unwrap().click();
        settle(&mut h);
        assert_eq!(selected_rows(&h), vec!["Georgia (Regular, Normal)"]);
        open_menu(&mut h, "Font");
        let items = open_menu_items(&h);
        assert!(items.iter().any(|i| i.0 == "Hide duplicate fonts" && i.2), "{items:?}");
        menu_item(&h, "Open font file location").click();
        settle(&mut h);
        let all_items = open_menu_items(&h);
        let these: Vec<String> = all_items.iter().map(|i| i.0.clone()).filter(|l| l.ends_with("georgia.ttf")).collect();
        per_row.push(these.len());
        files.extend(these);
        close_menus(&mut h);
    }
    per_row.sort();
    assert_eq!(per_row, vec![1, 2], "{files:?}");
    assert!(files.iter().all(|f| *f == f.to_lowercase()));
    assert!(files.iter().any(|f| f.contains(r"\copy\")), "{files:?}");
    assert!(files.iter().any(|f| f.contains(r"\alt\")), "{files:?}");
    // with two faces selected the submenu is disabled (one face or one family only)
    h.query_all(by().role(Role::ListBoxOption).label("Georgia (Regular, Normal)")).find(|n| n.rect().center() == g[0]).unwrap().click_modifiers(Modifiers::COMMAND);
    settle(&mut h);
    open_menu(&mut h, "Font");
    let items = open_menu_items(&h);
    assert!(items.iter().any(|i| i.0 == "Open font file location" && !i.1), "{items:?}");
    close_menus(&mut h);
    h.query_all(by().role(Role::ListBoxOption).label("Georgia (Regular, Normal)")).find(|n| n.rect().center() == g[0]).unwrap().click();
    settle(&mut h);
    open_menu(&mut h, "Font");
    menu_item(&h, "Open font file location").click();
    settle(&mut h);
    let files: Vec<String> = open_menu_items(&h).into_iter().map(|i| i.0).filter(|l| l.ends_with("georgia.ttf")).collect();
    // clicking one asks the font system (the sandbox records it; no Explorer)
    menu_item(&h, &files[0].clone()).click();
    settle(&mut h);
    assert_eq!(fs.opened_locations().len(), 1);
    // Font ▸ Hide duplicate fonts
    click_menu(&mut h, "Font", "Hide duplicate fonts");
    assert!(!h.state().settings().core.show_duplicates);
    assert_eq!(h.state().font_count(), 2);
}

/// Show Latin names in a Japanese UI: the Japanese fonts are listed under their Japanese names, and under
/// the Latin ones with the toggle on.
#[test]
fn show_latin_names_in_japanese() {
    let _g = env_lock();
    if common::fixture("msgothic.ttc").is_none() {
        eprintln!("msgothic.ttc missing: skipped");
        return;
    }
    let mut s = setup("Jp", &["msgothic.ttc", "verdana.ttf"], (1280.0, 1100.0), |c| c.language = "ja-JP".into());
    let (cat, lib) = core(&s.db, &s.dir);
    let ja = 0x0411u16;
    let labels = |latin: bool| sorted(cat.visible_fonts(&lib, false, false).into_iter().map(|f| f.label(ja, latin)).collect());
    let native = labels(false);
    let latin = labels(true);
    assert_ne!(native, latin, "MS Gothic has Japanese names");
    assert_eq!(app_faces(&s.h), native);
    let view = anyfont::tr("View");
    let item = anyfont::tr("Show Latin names");
    assert_ne!(view, "View", "the UI is in Japanese");
    click_menu(&mut s.h, &view, &item);
    assert!(s.h.state().settings().core.show_latin_names);
    assert_eq!(app_faces(&s.h), latin);
    assert!(app_faces(&s.h).iter().any(|l| l.starts_with("MS Gothic")), "{:?}", app_faces(&s.h));
    click_menu(&mut s.h, &view, &item);
    assert_eq!(app_faces(&s.h), native);
    // the next test builds its window in en-US again (`i18n::init`)
}

// -------------------------------------------------------------------------------------- selection

fn small<'a>() -> Setup<'a> {
    setup("Small", SMALL, (1280.0, 1100.0), |_| {})
}

#[test]
fn selection_click_ctrl_shift_and_select_all() {
    let _g = env_lock();
    let mut s = small();
    let rows: Vec<String> = list_rows(&s.h).into_iter().map(|r| r.0).collect();
    assert_eq!(
        rows,
        vec!["Comic Sans MS (Regular, Normal)", "Consolas (Regular, Normal)", "Georgia (2)", "Georgia (Regular, Normal)", "Georgia (Bold)", "Hack (Regular, Normal)", "Tuffy (Regular, Normal)", "Verdana (Regular, Normal)"]
    );
    // opening the folder selects its first font
    assert_eq!(selected_rows(&s.h), vec!["Comic Sans MS (Regular, Normal)"]);
    row(&s.h, "Hack (Regular, Normal)").click();
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), vec!["Hack (Regular, Normal)"]);
    row(&s.h, "Consolas (Regular, Normal)").click_modifiers(Modifiers::COMMAND);
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), vec!["Consolas (Regular, Normal)", "Hack (Regular, Normal)"]);
    // Ctrl+click again deselects
    row(&s.h, "Hack (Regular, Normal)").click_modifiers(Modifiers::COMMAND);
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), vec!["Consolas (Regular, Normal)"]);
    // Shift+click: the range from the anchor (the last clicked row, Hack: a Ctrl+click sets it too)
    row(&s.h, "Tuffy (Regular, Normal)").click_modifiers(Modifiers::SHIFT);
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), rows[5..7].to_vec());
    row(&s.h, "Consolas (Regular, Normal)").click();
    settle(&mut s.h);
    row(&s.h, "Tuffy (Regular, Normal)").click_modifiers(Modifiers::SHIFT);
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), rows[1..7].to_vec());
    // Ctrl+Shift+click adds a range to the selection
    row(&s.h, "Verdana (Regular, Normal)").click_modifiers(Modifiers::COMMAND);
    settle(&mut s.h);
    row(&s.h, "Comic Sans MS (Regular, Normal)").click();
    settle(&mut s.h);
    row(&s.h, "Consolas (Regular, Normal)").click_modifiers(Modifiers::COMMAND);
    settle(&mut s.h);
    row(&s.h, "Georgia (2)").click_modifiers(Modifiers::COMMAND | Modifiers::SHIFT);
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), rows[0..3].to_vec());
    // Ctrl+A selects every row, Ctrl+Shift+A none
    s.h.key_press_modifiers(CTRL, Key::A);
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), rows);
    open_menu(&mut s.h, "Edit");
    let items = open_menu_items(&s.h);
    close_menus(&mut s.h);
    assert!(items.iter().any(|i| i.0 == "Deselect All" && i.1), "{items:?}");
    s.h.key_press_modifiers(CTRL | Modifiers::SHIFT, Key::A);
    settle(&mut s.h);
    assert!(selected_rows(&s.h).is_empty());
    open_menu(&mut s.h, "Edit");
    let items = open_menu_items(&s.h);
    close_menus(&mut s.h);
    assert!(items.iter().any(|i| i.0 == "Deselect All" && !i.1), "{items:?}");
    assert!(items.iter().any(|i| i.0 == "Select All" && i.1));
    // the current font (Consolas, the last clicked face) keeps its highlight: its row is painted like a
    // selected row, the others not
    let bg = |h: &mut H<'_>, label: &str| {
        let r = row(h, label).rect();
        let img = h.render().expect("render");
        let ppp = img.width() as f32 / 1280.0;
        let p = img.get_pixel(((r.left() + r.width() * 0.75) * ppp) as u32, ((r.top() + 3.0) * ppp) as u32);
        p.0
    };
    let current = bg(&mut s.h, "Consolas (Regular, Normal)");
    let other = bg(&mut s.h, "Hack (Regular, Normal)");
    assert_ne!(current, other, "the current font keeps its highlight");
    // ... and only Add to collection acts on it (the menu's rules for "no selection")
    open_menu(&mut s.h, "Font");
    let items = open_menu_items(&s.h);
    close_menus(&mut s.h);
    for (l, en) in [("Install", false), ("Delete", false), ("Add to collection", true), ("Open font file location", false)] {
        assert_eq!(items.iter().find(|i| i.0 == l).map(|i| i.1), Some(en), "{l}: {items:?}");
    }
    // a click on another face moves the current font (and its highlight)
    row(&s.h, "Hack (Regular, Normal)").click();
    settle(&mut s.h);
    assert_eq!(bg(&mut s.h, "Hack (Regular, Normal)"), current, "a selected row is painted like the current one");
    assert_eq!(bg(&mut s.h, "Consolas (Regular, Normal)"), other);
    // Edit ▸ Select All
    click_menu(&mut s.h, "Edit", "Select All");
    assert_eq!(selected_rows(&s.h), rows);
}

/// A family header click selects the header: the commands act on the whole family.
#[test]
fn family_header_click_selects_the_whole_family() {
    let _g = env_lock();
    let mut s = small();
    row(&s.h, "Georgia (2)").click();
    settle(&mut s.h);
    assert_eq!(selected_rows(&s.h), vec!["Georgia (2)"]);
    open_menu(&mut s.h, "Font");
    menu_item(&s.h, "Open font file location").click();
    settle(&mut s.h);
    let files: Vec<String> = sorted(open_menu_items(&s.h).into_iter().map(|i| i.0).filter(|l| l.ends_with(".ttf")).collect());
    close_menus(&mut s.h);
    assert_eq!(files.len(), 2, "{files:?}");
    assert!(files[0].ends_with("georgia.ttf") && files[1].ends_with("georgiab.ttf"), "{files:?}");
}

#[test]
fn selection_with_the_keyboard() {
    let _g = env_lock();
    let mut s = small();
    let rows: Vec<String> = list_rows(&s.h).into_iter().map(|r| r.0).collect();
    let sel = |h: &H<'_>| selected_rows(h);
    let key = |h: &mut H<'_>, m: Modifiers, k: Key| {
        h.key_press_modifiers(m, k);
        settle(h);
    };
    // the list gets the keyboard by a click on a row
    row(&s.h, "Consolas (Regular, Normal)").click();
    settle(&mut s.h);
    key(&mut s.h, Modifiers::NONE, Key::ArrowDown);
    assert_eq!(sel(&s.h), vec![rows[2].clone()], "↓ to the Georgia header");
    key(&mut s.h, Modifiers::NONE, Key::ArrowDown);
    assert_eq!(sel(&s.h), vec![rows[3].clone()]);
    key(&mut s.h, Modifiers::NONE, Key::ArrowUp);
    key(&mut s.h, Modifiers::NONE, Key::ArrowUp);
    assert_eq!(sel(&s.h), vec![rows[1].clone()]);
    key(&mut s.h, Modifiers::NONE, Key::End);
    assert_eq!(sel(&s.h), vec![rows[7].clone()], "End");
    key(&mut s.h, Modifiers::NONE, Key::Home);
    assert_eq!(sel(&s.h), vec![rows[0].clone()], "Home");
    // the page holds every row of this short list: PgDn goes to the last row, PgUp to the first
    key(&mut s.h, Modifiers::NONE, Key::PageDown);
    assert_eq!(sel(&s.h), vec![rows[7].clone()], "PgDn");
    key(&mut s.h, Modifiers::NONE, Key::PageUp);
    assert_eq!(sel(&s.h), vec![rows[0].clone()], "PgUp");
    // Shift extends from the anchor
    key(&mut s.h, Modifiers::SHIFT, Key::ArrowDown);
    key(&mut s.h, Modifiers::SHIFT, Key::ArrowDown);
    assert_eq!(sel(&s.h), rows[0..3].to_vec(), "Shift+↓↓");
    key(&mut s.h, Modifiers::SHIFT, Key::End);
    assert_eq!(sel(&s.h), rows, "Shift+End");
    key(&mut s.h, Modifiers::SHIFT, Key::Home);
    assert_eq!(sel(&s.h), vec![rows[0].clone()], "Shift+Home");
    key(&mut s.h, Modifiers::SHIFT, Key::PageDown);
    assert_eq!(sel(&s.h), rows, "Shift+PgDn");
    // Ctrl moves the keyboard row only; Ctrl+Space toggles it
    key(&mut s.h, Modifiers::NONE, Key::Home);
    key(&mut s.h, CTRL, Key::ArrowDown);
    key(&mut s.h, CTRL, Key::ArrowDown);
    assert_eq!(sel(&s.h), vec![rows[0].clone()], "Ctrl+↓ keeps the selection");
    key(&mut s.h, Modifiers::COMMAND, Key::Space);
    assert_eq!(sel(&s.h), vec![rows[0].clone(), rows[2].clone()], "Ctrl+Space adds the keyboard row");
    key(&mut s.h, Modifiers::COMMAND, Key::End);
    key(&mut s.h, Modifiers::COMMAND, Key::Space);
    assert_eq!(sel(&s.h), vec![rows[0].clone(), rows[2].clone(), rows[7].clone()], "Ctrl+End, Ctrl+Space");
    key(&mut s.h, Modifiers::COMMAND, Key::Home);
    key(&mut s.h, Modifiers::COMMAND, Key::Space);
    assert_eq!(sel(&s.h), vec![rows[2].clone(), rows[7].clone()], "Ctrl+Space removes it again");
}

/// Type-ahead: letters typed within a second form a prefix; the next matching row is selected. (The
/// harness clock advances 0.25 s a frame: `settle` lets more than a second pass.)
#[test]
fn type_ahead_selects_by_name() {
    let _g = env_lock();
    let mut s = small();
    row(&s.h, "Comic Sans MS (Regular, Normal)").click();
    settle(&mut s.h);
    let typed = |h: &mut H<'_>, t: &str| {
        h.event(egui::Event::Text(t.into()));
        h.run_steps(2);
    };
    typed(&mut s.h, "v");
    assert_eq!(selected_rows(&s.h), vec!["Verdana (Regular, Normal)"]);
    settle(&mut s.h);
    // one letter again: the next row starting with it, wrapping round
    typed(&mut s.h, "c");
    assert_eq!(selected_rows(&s.h), vec!["Comic Sans MS (Regular, Normal)"], "after Verdana, wrapping to the first C");
    settle(&mut s.h);
    typed(&mut s.h, "c");
    assert_eq!(selected_rows(&s.h), vec!["Consolas (Regular, Normal)"], "the next C");
    settle(&mut s.h);
    typed(&mut s.h, "g");
    assert_eq!(selected_rows(&s.h), vec!["Georgia (2)"], "the family header first");
    typed(&mut s.h, "eorgia (b");
    assert_eq!(selected_rows(&s.h), vec!["Georgia (Bold)"], "a longer prefix typed within a second");
    settle(&mut s.h);
    // no match: the selection stays
    typed(&mut s.h, "x");
    assert_eq!(selected_rows(&s.h), vec!["Georgia (Bold)"]);
}

// ---------------------------------------------------------------------------- edit box context menu

#[test]
fn edit_box_context_menu_enable_states_and_commands() {
    let _g = env_lock();
    let mut s = small();
    let items = |h: &H<'_>| -> Vec<(String, bool)> { open_menu_items(h).into_iter().map(|i| (i.0, i.1)).collect() };
    // empty Search box: only Paste is enabled
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    assert_eq!(
        items(&s.h),
        vec![("Undo".into(), false), ("Cut".into(), false), ("Copy".into(), false), ("Paste".into(), true), ("Delete".into(), false), ("Select All".into(), false)]
    );
    // Paste in a test run reads nothing (the clipboard of the computer running the tests is not read)
    menu_item(&s.h, "Paste").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().unwrap_or_default(), "");
    assert_eq!(s.h.state().font_count(), 7);

    s.h.get_by_role_and_label(Role::TextInput, "Search").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("georgia");
    settle(&mut s.h);
    assert_eq!(s.h.state().font_count(), 2);
    // text, no selection: Undo and Select All enabled
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    let it = items(&s.h);
    assert_eq!(it.iter().find(|i| i.0 == "Undo").map(|i| i.1), Some(true), "{it:?}");
    assert_eq!(it.iter().find(|i| i.0 == "Select All").map(|i| i.1), Some(true));
    assert_eq!(it.iter().find(|i| i.0 == "Cut").map(|i| i.1), Some(false));
    assert_eq!(it.iter().find(|i| i.0 == "Copy").map(|i| i.1), Some(false));
    assert_eq!(it.iter().find(|i| i.0 == "Delete").map(|i| i.1), Some(false));
    // Undo: the text before the last change
    menu_item(&s.h, "Undo").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().unwrap_or_default(), "");
    assert_eq!(s.h.state().font_count(), 7);
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    menu_item(&s.h, "Undo").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().as_deref(), Some("georgia"), "Undo of the Undo");
    assert_eq!(s.h.state().font_count(), 2);

    // the sample text box has the same menu
    s.h.get_by_role_and_label(Role::TextInput, "Enter sample text").click_secondary();
    settle(&mut s.h);
    let it = items(&s.h);
    assert_eq!(it.len(), 6, "{it:?}");
    assert_eq!(it.iter().find(|i| i.0 == "Paste").map(|i| i.1), Some(true));
    close_menus(&mut s.h);
}

/// Cut, Copy and Delete of the edit box menu act on the selected text: right-clicking the selection opens
/// the menu with them enabled.
#[test]
fn edit_box_menu_cut_copy_delete_with_a_selection() {
    let _g = env_lock();
    let mut s = small();
    let items = |h: &H<'_>| -> Vec<(String, bool)> { open_menu_items(h).into_iter().map(|i| (i.0, i.1)).collect() };
    s.h.get_by_role_and_label(Role::TextInput, "Search").focus();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").type_text("georgia");
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    menu_item(&s.h, "Select All").click();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    let it = items(&s.h);
    for l in ["Cut", "Copy", "Delete", "Select All", "Paste"] {
        assert_eq!(it.iter().find(|i| i.0 == l).map(|i| i.1), Some(true), "{l}: {it:?}");
    }
    menu_item(&s.h, "Delete").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().unwrap_or_default(), "");
    assert_eq!(s.h.state().font_count(), 7, "Delete clears the search");
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    menu_item(&s.h, "Undo").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().as_deref(), Some("georgia"));
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    menu_item(&s.h, "Select All").click();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Search").click_secondary();
    settle(&mut s.h);
    menu_item(&s.h, "Cut").click();
    settle(&mut s.h);
    assert_eq!(s.h.get_by_role_and_label(Role::TextInput, "Search").value().unwrap_or_default(), "");
    assert_eq!(s.h.state().font_count(), 7);
}
