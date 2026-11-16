//! End-to-end checks of the font properties panel (Glyphs, Font info, Variable font tabs) and of the Options
//! window (UI size, Reset to Default, the four themes, the window border, the font matching mode, OK).
//! Expected values come from `anyfont_core` (glyph enumeration, `font_info`, the face's axes).
//!
//! The tests of this binary run one at a time (`env_lock`): the theme is process-wide.
mod common;

use anyfont_core::glyphs::{GlyphCategory, GlyphFilter};
use anyfont_core::{Catalog, FontDatabase};
use common::*;
use egui::accesskit::{Role, Toggled};
use egui::Modifiers;
use egui_kittest::kittest::{by, NodeT, Queryable};
use std::collections::BTreeSet;
use std::path::PathBuf;

const SIZE: (f32, f32) = (1600.0, 1000.0);

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

fn has_text(h: &H<'_>, t: &str) -> bool {
    painted(h).iter().any(|x| x == t)
}

fn list_rect(h: &H<'_>) -> egui::Rect {
    h.get_by_role_and_label(Role::Unknown, "Font list").rect()
}

fn row<'t>(h: &'t H<'_>, label: &'t str) -> egui_kittest::Node<'t> {
    let lr = list_rect(h);
    h.query_all(by().role(Role::ListBoxOption).label(label)).find(|n| lr.contains(n.rect().center())).unwrap_or_else(|| panic!("row {label} not shown: {:?}", h.state().row_labels()))
}

fn select(h: &mut H<'_>, label: &str) {
    row(h, label).click();
    settle(h);
}

/// A window with the folder `name` of `files` open and the properties panel shown.
struct Setup<'a> {
    _env: Env,
    h: H<'a>,
    db: PathBuf,
    dir: PathBuf,
}

fn setup<'a>(name: &str, files: &[&str]) -> Setup<'a> {
    let env = Env::new();
    let dir = env.fixture_folder(name, files);
    let cfg = env.config();
    let db = cfg.data_dir.join("data").join("fonts.db");
    let mut h = harness(cfg, SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Show font properties").click();
    settle(&mut h);
    assert!(h.state().settings().core.show_font_properties);
    Setup { _env: env, h, db, dir }
}

fn tab(h: &mut H<'_>, name: &str) {
    h.get_by_role_and_label(Role::Button, name).click();
    settle(h);
    assert_eq!(h.get_by_role_and_label(Role::Button, name).accesskit_node().toggled(), Some(Toggled::True), "{name} tab selected");
}

fn catalog(db: &std::path::Path) -> Catalog {
    Catalog::load(&FontDatabase::open(db).expect("db")).expect("catalog")
}

/// The face of the catalog by its list label.
fn face<'c>(cat: &'c Catalog, label: &str) -> &'c anyfont_core::model::FontRecord {
    cat.fonts().iter().find(|f| f.label(0x0409, false) == label).unwrap_or_else(|| panic!("{label} not in the catalog"))
}

/// Waits for the glyph count ("{n} glyphs") of the Glyphs tab and returns it.
fn glyph_count(h: &mut H<'_>) -> usize {
    let mut n = 0;
    wait_until(h, 20, "the glyph count", |h| {
        if let Some(t) = painted(h).into_iter().find(|t| t.ends_with(" glyphs") && t.split(' ').next().map(|x| x.parse::<usize>().is_ok()).unwrap_or(false)) {
            n = t.split(' ').next().unwrap().parse().unwrap();
            true
        } else {
            false
        }
    });
    n
}

fn right_panel_x(h: &H<'_>) -> f32 {
    list_rect(h).right()
}

// --------------------------------------------------------------------------------------- Glyphs tab

#[test]
fn glyphs_tab_count_filters_selection_and_size() {
    let _g = env_lock();
    let mut s = setup("Glyphs", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    tab(&mut s.h, "Glyphs");
    assert_eq!(s.h.state().settings().core.window.right_panel_tab, 0);
    select(&mut s.h, "Tuffy (Regular, Normal)");
    let cat = catalog(&s.db);
    let tuffy = face(&cat, "Tuffy (Regular, Normal)");
    let list = anyfont_core::glyphs::enumerate_file(tuffy.path().unwrap(), tuffy.face_index).expect("glyphs");
    let n = glyph_count(&mut s.h);
    assert_eq!(n, list.glyphs.len(), "the count matches the core's enumeration");
    // the glyph cells are listed (the visible ones)
    let x = right_panel_x(&s.h);
    assert!(s.h.query_all(by().role(Role::ListBoxOption)).any(|g| g.rect().left() > x), "glyph cells");

    // a Common filter: the count is the core's filtered count
    let groups = list.available_filters();
    let common: Vec<GlyphCategory> = groups.iter().find(|(g, _)| g == "Common").map(|(_, c)| c.clone()).unwrap_or_default();
    assert!(!common.is_empty(), "{groups:?}");
    let clear = |h: &H<'_>| !h.get_by_role_and_label(Role::Button, "Remove all selected glyph filters.").accesskit_node().is_disabled();
    assert!(!clear(&s.h), "nothing to clear");
    // the filter rows of a group, in display order (the area scrolls: only the top ones are visible)
    let rows_of = |h: &H<'_>, labels: &[String]| -> Vec<(String, egui::Rect)> {
        let mut v: Vec<(String, egui::Rect)> = h
            .query_all(by().role(Role::CheckBox))
            .filter(|n| n.rect().left() > x)
            .filter_map(|n| {
                let l = n.accesskit_node().label().unwrap_or_default();
                labels.iter().find(|c| **c == l).map(|c| (c.clone(), n.rect()))
            })
            .collect();
        v.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
        v
    };
    let captions: Vec<String> = common.iter().map(|c| c.caption().to_string()).collect();
    let crows = rows_of(&s.h, &captions);
    assert_eq!(crows.len(), common.len(), "one row per Common category");
    let (label, r) = crows[0].clone();
    let c = *common.iter().find(|c| c.caption() == label).unwrap();
    click_at(&mut s.h, r.center());
    let set: BTreeSet<GlyphFilter> = [GlyphFilter { script: "Common".into(), category: c }].into_iter().collect();
    let want = list.filtered(&set).count();
    assert_eq!(glyph_count(&mut s.h), want, "Common ▸ {label}");
    assert!(want < n);
    assert!(clear(&s.h));
    // the Common section collapses (its rows go away) ...
    let tri = s.h.get_by_role_and_label(Role::DisclosureTriangle, "Common").rect();
    click_at(&mut s.h, tri.center());
    assert!(rows_of(&s.h, &captions).is_empty(), "Common collapsed");
    // ... and a script filter adds (OR) its glyphs
    let scripts: Vec<&(String, Vec<GlyphCategory>)> = groups.iter().filter(|(g, _)| g != "Common").collect();
    let names: Vec<String> = scripts.iter().map(|(g, _)| g.clone()).collect();
    let srows = rows_of(&s.h, &names);
    assert_eq!(srows.len(), scripts.len(), "one row per script: {names:?}");
    let (script, r) = srows[0].clone();
    let cats = &scripts.iter().find(|(g, _)| *g == script).unwrap().1;
    click_at(&mut s.h, r.center());
    let mut set2 = set.clone();
    for cc in cats {
        set2.insert(GlyphFilter { script: script.clone(), category: *cc });
    }
    assert_eq!(glyph_count(&mut s.h), list.filtered(&set2).count(), "Common ▸ {label} OR {script}");
    let tri = s.h.get_by_role_and_label(Role::DisclosureTriangle, "Common").rect();
    click_at(&mut s.h, tri.center());
    assert_eq!(rows_of(&s.h, &captions).len(), common.len(), "Common expanded");
    // Remove all selected glyph filters
    let fr = s.h.get_by_role_and_label(Role::Button, "Remove all selected glyph filters.").rect();
    click_at(&mut s.h, fr.center());
    assert_eq!(glyph_count(&mut s.h), n);
    assert!(!clear(&s.h));

    // another face: its own count (the filters are cleared)
    select(&mut s.h, "Hack (Regular, Normal)");
    let hack = face(&cat, "Hack (Regular, Normal)");
    let hl = anyfont_core::glyphs::enumerate_file(hack.path().unwrap(), hack.face_index).unwrap();
    assert_eq!(glyph_count(&mut s.h), hl.glyphs.len());

    // two faces: the multiple selection note, no count
    row(&s.h, "Tuffy (Regular, Normal)").click_modifiers(Modifiers::COMMAND);
    settle(&mut s.h);
    assert!(has_text(&s.h, "Multiple fonts are selected. To view a glyph set, choose only one font."), "{:?}", painted(&s.h));
    assert!(!painted(&s.h).iter().any(|t| t.ends_with(" glyphs")));

    // the glyph size: small/big buttons and the slider, 24..=96
    select(&mut s.h, "Tuffy (Regular, Normal)");
    let tip = "Change the glyph size in the Glyph Preview area.";
    let size = |h: &H<'_>| h.state().settings().core.glyph_cell_size;
    let v0 = size(&s.h);
    let mut b: Vec<egui::Rect> = s.h.query_all(by().role(Role::Button).label(tip)).map(|n| n.rect()).collect();
    b.sort_by(|a, b| a.left().total_cmp(&b.left()));
    assert_eq!(b.len(), 2);
    click_at(&mut s.h, b[1].center());
    assert_eq!(size(&s.h), v0 + 1.0);
    click_at(&mut s.h, b[0].center());
    assert_eq!(size(&s.h), v0);
    let sl = s.h.get_by_role_and_label(Role::Slider, tip).rect();
    click_at(&mut s.h, egui::pos2(sl.right() - 1.0, sl.center().y));
    assert_eq!(size(&s.h), 96.0);
    click_at(&mut s.h, egui::pos2(sl.left() + 1.0, sl.center().y));
    assert_eq!(size(&s.h), 24.0);
    click_at(&mut s.h, b[0].center());
    assert_eq!(size(&s.h), 24.0, "not below 24");
}

// ------------------------------------------------------------------------------------ Font info tab

#[test]
fn font_info_tab_shows_the_core_values() {
    let _g = env_lock();
    let mut s = setup("Info", &["Tuffy.ttf", "TTC.ttc", "colr_1_variable.ttf"]);
    // a collection to show in the "Collection:" row
    s.h.get_by_role_and_label(Role::Button, "Add collection").click();
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::TextInput, "Name:").type_text("Shelf");
    settle(&mut s.h);
    s.h.key_press(egui::Key::Enter);
    settle(&mut s.h);
    s.h.get_by_role_and_label(Role::ListBoxOption, "Info").click();
    settle(&mut s.h);
    select(&mut s.h, "Tuffy (Regular, Normal)");
    open_menu(&mut s.h, "Font");
    menu_item(&s.h, "Add to collection").click();
    settle(&mut s.h);
    menu_item(&s.h, "Shelf").click();
    settle(&mut s.h);
    tab(&mut s.h, "Font info");
    assert_eq!(s.h.state().settings().core.window.right_panel_tab, 1);
    let cat = catalog(&s.db);
    for label in ["Tuffy (Regular, Normal)", "COLRv1 Variable Test Glyphs (Regular, Normal)"] {
        select(&mut s.h, label);
        let f = face(&cat, label);
        let rows = anyfont_core::catalog::font_info(f, cat.collections());
        let texts = painted(&s.h);
        for (cap, value) in &rows {
            assert!(texts.iter().any(|t| *t == format!("{cap}:")), "{label}: {cap}: label missing in {texts:?}");
            if value.is_empty() || *cap == "Vendor URL" {
                continue;
            }
            if *cap == "File Path" {
                // lower case, maybe ellipsized at the end
                let lower = value.to_lowercase();
                let shown = texts.iter().find(|t| lower.starts_with(t.trim_end_matches("...")) && t.len() > 8).unwrap_or_else(|| panic!("{label}: path {lower} not in {texts:?}"));
                assert_eq!(*shown, shown.to_lowercase(), "the path is shown in lower case");
                continue;
            }
            assert!(texts.iter().any(|t| t == value), "{label}: {cap} = {value:?} not in {texts:?}");
        }
        let fam = rows.iter().find(|r| r.0 == "Family").unwrap().1.clone();
        assert!(!fam.is_empty());
    }
    // Tuffy's Collection row names the collection
    select(&mut s.h, "Tuffy (Regular, Normal)");
    assert!(has_text(&s.h, "Shelf"), "{:?}", painted(&s.h));
    // technology / collection file values of the TTC
    let ttc = cat
        .fonts()
        .iter()
        .find(|f| f.path().map(|p| p.starts_with(&s.dir) && p.extension().map(|e| e == "ttc").unwrap_or(false)).unwrap_or(false))
        .expect("TTC face");
    let lr = list_rect(&s.h);
    let ttc_label = ttc.label(0x0409, false);
    s.h.query_all(by().role(Role::ListBoxOption).label(&ttc_label)).find(|n| lr.contains(n.rect().center())).unwrap().click();
    settle(&mut s.h);
    let tech = ttc.technology.caption();
    assert!(has_text(&s.h, tech), "{tech} in {:?}", painted(&s.h));
}

// -------------------------------------------------------------------------------- Variable font tab

#[test]
fn variable_font_tab_axes_and_values() {
    let _g = env_lock();
    let mut s = setup("Var", &["colr_1_variable.ttf", "Tuffy.ttf"]);
    tab(&mut s.h, "Variable font");
    assert_eq!(s.h.state().settings().core.window.right_panel_tab, 2);
    let cat = catalog(&s.db);
    let x = right_panel_x(&s.h);

    // a non-variable face: the note
    select(&mut s.h, "Tuffy (Regular, Normal)");
    assert!(has_text(&s.h, anyfont_core::catalog::NO_VARIABLE_FONT_TEXT), "{:?}", painted(&s.h));
    assert!(has_text(&s.h, "Choose one variable font to see and adjust its properties."));

    // the COLRv1 variable test font: one slider + one spin box per visible axis, at the axis default
    let label = "COLRv1 Variable Test Glyphs (Regular, Normal)";
    select(&mut s.h, label);
    let f = face(&cat, label);
    assert!(f.is_variable && !f.axes.is_empty());
    let visible: Vec<_> = f.axes.iter().filter(|a| !a.hidden).collect();
    let sliders: Vec<(String, Option<f64>)> = s
        .h
        .query_all(by().role(Role::Slider))
        .filter(|n| n.rect().left() > x)
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.accesskit_node().numeric_value()))
        .collect();
    assert_eq!(sliders.len(), visible.len(), "{sliders:?}");
    for a in &visible {
        let name = if a.name.is_empty() { a.tag.clone() } else { a.name.clone() };
        let sl = sliders.iter().find(|s| s.0 == name).unwrap_or_else(|| panic!("axis {name} in {sliders:?}"));
        assert!((sl.1.unwrap() as f32 - a.value).abs() < 0.01, "{name}: {:?} vs default {}", sl.1, a.value);
        assert!(s.h.query_all(by().role(Role::SpinButton).label(&name)).any(|n| n.rect().left() > x), "spin box {name}");
        assert!(has_text(&s.h, &name), "axis caption {name}");
    }
    // change the first axis with its slider: the maximum at the right end
    let a = visible[0];
    let name = if a.name.is_empty() { a.tag.clone() } else { a.name.clone() };
    let r = s.h.query_all(by().role(Role::Slider).label(&name)).find(|n| n.rect().left() > x).unwrap().rect();
    let p = egui::pos2(r.right() - 1.0, r.center().y);
    s.h.hover_at(p);
    s.h.step();
    s.h.drag_at(p);
    s.h.step();
    s.h.drop_at(p);
    settle(&mut s.h);
    let v = s.h.query_all(by().role(Role::Slider).label(&name)).find(|n| n.rect().left() > x).unwrap().accesskit_node().numeric_value().unwrap();
    assert!((v as f32 - a.max).abs() < 0.01, "{name}: {v} vs max {}", a.max);
    // the value is kept for this face while another one is shown
    select(&mut s.h, "Tuffy (Regular, Normal)");
    select(&mut s.h, label);
    let v2 = s.h.query_all(by().role(Role::Slider).label(&name)).find(|n| n.rect().left() > x).unwrap().accesskit_node().numeric_value().unwrap();
    assert_eq!(v, v2);

    // two faces: the note again
    row(&s.h, "Tuffy (Regular, Normal)").click_modifiers(Modifiers::COMMAND);
    settle(&mut s.h);
    assert!(has_text(&s.h, anyfont_core::catalog::NO_VARIABLE_FONT_TEXT));
}

#[test]
fn variable_font_tab_named_instance_values() {
    let _g = env_lock();
    // Bahnschrift (a copy of the Windows font): weight and width axes, a named instance's coordinates
    let mut s = setup("VarB", &["bahnschrift.ttf"]);
    tab(&mut s.h, "Variable font");
    let cat = catalog(&s.db);
    let x = right_panel_x(&s.h);
    // a face with a named instance whose row is visible (the list has 16 Bahnschrift rows)
    let lr = list_rect(&s.h);
    let shown: Vec<String> = s.h.query_all(by().role(Role::ListBoxOption)).filter(|n| lr.contains(n.rect().center())).filter_map(|n| n.accesskit_node().label()).collect();
    let found = cat.fonts().iter().find(|f| {
        f.path().map(|p| p.starts_with(&s.dir)).unwrap_or(false) && f.instance_index.is_some() && f.axes.iter().any(|a| !a.hidden) && shown.contains(&f.label(0x0409, false))
    });
    if let Some(b) = found {
        let blabel = b.label(0x0409, false);
        select(&mut s.h, &blabel);
        let sliders: Vec<(String, f64)> = s
            .h
            .query_all(by().role(Role::Slider))
            .filter(|n| n.rect().left() > x)
            .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.accesskit_node().numeric_value().unwrap_or_default()))
            .collect();
        assert_eq!(sliders.len(), b.axes.iter().filter(|a| !a.hidden).count(), "{sliders:?}");
        let inst = b.instance_index.and_then(|i| b.instances.get(i as usize)).expect("a named instance");
        for (a, c) in b.axes.iter().zip(&inst.coordinates) {
            let name = if a.name.is_empty() { a.tag.clone() } else { a.name.clone() };
            let v = sliders.iter().find(|s| s.0 == name).unwrap_or_else(|| panic!("{name} in {sliders:?}")).1;
            assert!((v as f32 - c).abs() < 0.01, "{blabel} {name}: {v} vs {c}");
        }
    } else {
        panic!("no Bahnschrift face with a named instance is shown: {shown:?}");
    }
}

#[test]
fn glyph_filter_names_keep_the_ampersand() {
    let _g = env_lock();
    let mut s = setup("Amp", &["Tuffy.ttf"]);
    tab(&mut s.h, "Glyphs");
    select(&mut s.h, "Tuffy (Regular, Normal)");
    let cat = catalog(&s.db);
    let tuffy = face(&cat, "Tuffy (Regular, Normal)");
    let list = anyfont_core::glyphs::enumerate_file(tuffy.path().unwrap(), tuffy.face_index).expect("glyphs");
    let common = list.available_filters().into_iter().find(|(g, _)| g == "Common").map(|(_, c)| c).unwrap_or_default();
    assert!(common.contains(&GlyphCategory::PunctuationAndSeparators), "{common:?}");
    let x = right_panel_x(&s.h);
    assert!(has_text(&s.h, "Punctuation & Separators"), "the caption is painted with its &");
    let names: Vec<String> = s.h.query_all(by().role(Role::CheckBox)).filter(|n| n.rect().left() > x).filter_map(|n| n.accesskit_node().label()).collect();
    assert!(names.iter().any(|n| n == "Punctuation & Separators"), "accessible names: {names:?}");
}

// ------------------------------------------------------------------------------------------ Options

fn open_options(h: &mut H<'_>) {
    h.get_by_role_and_label(Role::Button, "Options").click();
    settle(h);
    assert!(h.query_by_role_and_label(Role::Button, "Reset to Default").is_some(), "the Options window");
}

/// The two combo boxes of the Options window (theme, window border), top first.
fn option_combos(h: &H<'_>) -> Vec<egui::Rect> {
    let mut v: Vec<egui::Rect> = h.query_all(by().role(Role::ComboBox)).filter(|n| n.accesskit_node().label().as_deref() != Some("Enter sample text")).map(|n| n.rect()).collect();
    v.sort_by(|a, b| a.top().total_cmp(&b.top()));
    v
}

fn click_at(h: &mut H<'_>, p: egui::Pos2) {
    h.hover_at(p);
    h.step();
    h.drag_at(p);
    h.step();
    h.drop_at(p);
    settle(h);
}

#[test]
fn options_size_reset_and_ok() {
    let _g = env_lock();
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    let scale = |h: &H<'_>| h.state().settings().core.ui_scale_percent;
    assert_eq!(scale(&h), 100.0);
    open_options(&mut h);
    let sl = h.get_by_role_and_label(Role::Slider, "Size").rect();
    // a click on the slider snaps to 25 %: right end 225, left end 75, middle 150
    click_at(&mut h, egui::pos2(sl.right() - 1.0, sl.center().y));
    assert_eq!(scale(&h), 225.0);
    // (the window grew with the zoom: find the controls again)
    let sl = h.get_by_role_and_label(Role::Slider, "Size").rect();
    click_at(&mut h, egui::pos2(sl.left() + 1.0, sl.center().y));
    assert_eq!(scale(&h), 75.0);
    let sl = h.get_by_role_and_label(Role::Slider, "Size").rect();
    click_at(&mut h, egui::pos2(sl.center().x, sl.center().y));
    assert_eq!(scale(&h), 150.0);
    assert_eq!(h.get_by_role_and_label(Role::Slider, "Size").accesskit_node().numeric_value(), Some(150.0));
    // the window really zooms
    assert!((h.ctx.zoom_factor() - 1.5).abs() < 0.01, "zoom {}", h.ctx.zoom_factor());
    // the spin box: a typed value inside 75..=225 is kept, outside it is clamped
    h.get_by_role_and_label(Role::SpinButton, "Size").set_value_accesskit(130.0);
    settle(&mut h);
    assert_eq!(scale(&h), 130.0);
    h.get_by_role_and_label(Role::SpinButton, "Size").set_value_accesskit(500.0);
    settle(&mut h);
    assert_eq!(scale(&h), 225.0);
    h.get_by_role_and_label(Role::SpinButton, "Size").set_value_accesskit(10.0);
    settle(&mut h);
    assert_eq!(scale(&h), 75.0);
    // Reset to Default
    h.get_by_role_and_label(Role::Button, "Reset to Default").click();
    settle(&mut h);
    assert_eq!(scale(&h), 100.0);
    // (in test mode a scale of 100 is not re-applied to the context's zoom, so it is not checked here)
    // the font matching mode
    for (radio, mode) in [("Text and styles", anyfont_core::PanoseMode::TextAndStyles), ("Never show", anyfont_core::PanoseMode::NeverShow), ("Text", anyfont_core::PanoseMode::Text)] {
        h.get_by_label(radio).click();
        settle(&mut h);
        assert_eq!(h.state().settings().core.panose.mode, mode, "{radio}");
    }
    // OK closes the window and saves the settings
    let sl = h.get_by_role_and_label(Role::Slider, "Size").rect();
    click_at(&mut h, egui::pos2(sl.center().x, sl.center().y));
    let saved = h.state().settings().core.ui_scale_percent;
    let ok = h.query_all(by().role(Role::Button).label("OK")).next().expect("OK");
    ok.click();
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::Button, "Reset to Default").is_none(), "closed");
    let file = env.config().data_dir.join("settings").join("settings.json");
    let on_disk = anyfont::settings::AppSettings::load(&file);
    assert_eq!(on_disk.core.ui_scale_percent, saved, "saved on OK ({})", file.display());
}

#[test]
fn options_themes_and_window_border() {
    let _g = env_lock();
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    assert_eq!(anyfont::theme::scheme(), anyfont::theme::Scheme::Dark);
    open_options(&mut h);
    for s in [anyfont::theme::Scheme::Light, anyfont::theme::Scheme::Medium, anyfont::theme::Scheme::Black, anyfont::theme::Scheme::Dark] {
        let combo = option_combos(&h)[0];
        click_at(&mut h, combo.center());
        let item = h.query_all(by().label(s.caption())).find(|n| n.rect().top() > combo.top() && (n.rect().center().x - combo.center().x).abs() < 120.0).unwrap_or_else(|| panic!("theme {} in the drop-down", s.caption())).rect();
        click_at(&mut h, item.center());
        assert_eq!(h.state().settings().scheme, s, "{}", s.caption());
        assert_eq!(anyfont::theme::scheme(), s, "applied: {}", s.caption());
        assert_eq!(h.ctx.global_style().visuals.dark_mode, s != anyfont::theme::Scheme::Light, "{}", s.caption());
    }
    // Window border: the swatches by their colour; "default" is none
    for (label, want) in [("#017DBC", Some([0x01, 0x7D, 0xBC])), ("#C42B1C", Some([0xC4, 0x2B, 0x1C])), ("default", None)] {
        let combo = option_combos(&h)[1];
        click_at(&mut h, combo.center());
        let n = h.query_all(by().role(Role::ListBoxOption).label(label)).next().unwrap_or_else(|| panic!("{label} swatch"));
        n.click();
        settle(&mut h);
        assert_eq!(h.state().settings().core.window_border_color, want, "{label}");
    }
    let combo = option_combos(&h)[1];
    click_at(&mut h, combo.center());
    let swatches: Vec<String> = h.query_all(by().role(Role::ListBoxOption)).filter_map(|n| n.accesskit_node().label()).filter(|l| l.starts_with('#') || l == "default").collect();
    assert_eq!(swatches, vec!["default", "#000000", "#525252", "#017DBC", "#1D4470", "#66BF83", "#C42B1C", "#F2C21B", "#FFFFFF"]);
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    // Enter = OK
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert!(h.query_by_role_and_label(Role::Button, "Reset to Default").is_none(), "Enter closes the Options");
}
