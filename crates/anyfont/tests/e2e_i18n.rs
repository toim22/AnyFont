//! Every UI language (`anyfont::i18n::Lang::ALL`): the window starts in it, every menu and the main dialogs
//! are opened (Options, PANOSE exceptions, About, System Info, Legal Notices, Updates, Report a Bug, Submit
//! Ideas and Feedback, Get More, the Remove confirmation) and what each one paints is kept (text shapes of the
//! frame, with their clip rectangles, plus the controls of the accessibility tree). Three checks per language:
//! - `untranslated`: no painted text is an English key of `docs/strings-en.tsv` that the language translates
//!   to something else;
//! - `overflow`: no caption is cut by its clip rectangle or wider than the control it belongs to;
//! - `a11y`: no control shows a translation while its accessibility label (what a screen reader says) is
//!   still the English key.
//!
//! The language is process-wide, so the window walks run one at a time (`env_lock`); each language is walked
//! once per process and its three checks share the result. `ANYFONT_E2E_SHOTS=1` also saves a screenshot of
//! each place to `target/ui-compare/`.
mod common;

use anyfont::i18n::Lang;
use common::*;
use egui::accesskit::Role;
use egui::Key;
use egui_kittest::kittest::{NodeT, Queryable};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

/// `&File` → the accessibility label of a control with this caption in the current language
/// (`widgets::a11y`: the translation without its `&`).
fn lbl(key: &str) -> String {
    strip_amp(&anyfont::i18n::t(key))
}

/// The caption a menu item shows for `key` (`widgets::caption_parts`).
fn menu_lbl(key: &str) -> String {
    strip_amp(&anyfont::i18n::caption(key))
}

fn strip_amp(s: &str) -> String {
    s.replace("&&", "\u{1}").replace('&', "").replace('\u{1}', "&")
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// English key (without `&`) → translation (without `&`) of a language file.
fn translations(code: &str) -> HashMap<String, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/i18n").join(format!("{code}.tsv"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut m = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('\t') {
            if !v.trim().is_empty() {
                m.insert(unescape(&strip_amp(k)), unescape(&strip_amp(v)));
            }
        }
    }
    m
}

fn english_keys() -> BTreeSet<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/strings-en.tsv");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| unescape(&strip_amp(l)))
        .collect()
}

/// A control of the accessibility tree.
struct Control {
    role: Role,
    label: String,
    rect: egui::Rect,
}

/// A text painted in the last frame: the galley's text, its rectangle and the clip rectangle.
struct Painted {
    text: String,
    rect: egui::Rect,
    clip: egui::Rect,
}

#[derive(Default)]
struct Place {
    name: String,
    controls: Vec<Control>,
    texts: Vec<Painted>,
}

fn painted(shape: &egui::epaint::Shape, clip: egui::Rect, out: &mut Vec<Painted>) {
    match shape {
        egui::epaint::Shape::Text(t) => {
            let text = t.galley.text().to_string();
            if !text.trim().is_empty() {
                out.push(Painted { text, rect: t.galley.rect.translate(t.pos.to_vec2()), clip });
            }
        }
        egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| painted(s, clip, out)),
        _ => {}
    }
}

/// What the window shows now: the controls and every painted text.
fn collect(h: &mut H<'_>, out: &mut Vec<Place>, place: &str) {
    let mut p = Place { name: place.to_string(), ..Default::default() };
    for n in h.query_all(egui_kittest::kittest::by().predicate(|_| true)) {
        let a = n.accesskit_node();
        if a.bounding_box().is_none() {
            continue;
        }
        if matches!(a.role(), Role::Button | Role::MenuItem | Role::MenuItemCheckBox | Role::CheckBox | Role::RadioButton | Role::ComboBox) {
            p.controls.push(Control { role: a.role(), label: a.label().unwrap_or_default(), rect: n.rect() });
        }
    }
    for cs in &h.output().shapes {
        painted(&cs.shape, cs.clip_rect, &mut p.texts);
    }
    out.push(p);
    if std::env::var_os("ANYFONT_E2E_SHOTS").is_some() {
        let name = format!("i18n-{}-{}", anyfont::i18n::lang().code(), place.replace([' ', '&', '.'], "_"));
        screenshot(h, &name);
    }
}

/// A few frames: enough for a click, a key or a hover to take effect (menus and dialogs open synchronously).
fn quick(h: &mut H<'_>) {
    h.run_steps(5);
}

fn click_button(h: &mut H<'_>, key: &str) -> bool {
    let l = lbl(key);
    let found = h.query_by(|n| n.role() == Role::Button && n.label().as_deref() == Some(l.as_str())).map(|n| n.click()).is_some();
    quick(h);
    found
}

fn click_item(h: &mut H<'_>, menu: &str, item: &str) {
    open_menu(h, &lbl(menu));
    let (a, b) = (lbl(item), menu_lbl(item));
    match query_menu_item(h, &a).or_else(|| query_menu_item(h, &b)) {
        Some(n) => n.click(),
        None => panic!("no menu item {a:?} / {b:?} in {:?}", open_menu_items(h)),
    }
    quick(h);
}

fn escape(h: &mut H<'_>, n: usize) {
    for _ in 0..n {
        h.key_press(Key::Escape);
        quick(h);
    }
}

/// Opens everything in `lang` once per process and keeps what was shown (the three checks of a language
/// share one walk through the window).
fn walk(lang: Lang) -> Arc<Vec<Place>> {
    static WALKS: Mutex<Vec<(Lang, Arc<Vec<Place>>)>> = Mutex::new(Vec::new());
    let _g = env_lock();
    if let Some((_, w)) = WALKS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(l, _)| *l == lang) {
        return w.clone();
    }
    let seen = Arc::new(walk_now(lang));
    WALKS.lock().unwrap_or_else(|e| e.into_inner()).push((lang, seen.clone()));
    seen
}

fn walk_now(lang: Lang) -> Vec<Place> {
    let env = Env::new();
    let mut cfg = env.config();
    cfg.language = lang.code().to_string();
    let mut h = harness(cfg, (1280.0, 800.0));
    assert_eq!(anyfont::i18n::lang(), lang, "the window runs in {}", lang.code());
    let mut seen: Vec<Place> = Vec::new();
    collect(&mut h, &mut seen, "main window");
    // every menu of the menu bar, Font's submenus too
    for m in ["&File", "&Edit", "Font", "View", "Help"] {
        open_menu(&mut h, &lbl(m));
        if m == "Font" {
            for sub in ["Add to collection", "Open font file location"] {
                let (a, b) = (lbl(sub), menu_lbl(sub));
                if let Some(n) = query_menu_item(&h, &a).or_else(|| query_menu_item(&h, &b)) {
                    n.hover();
                    quick(&mut h);
                }
            }
        }
        collect(&mut h, &mut seen, &format!("menu {m}"));
        close_menus(&mut h);
    }
    // Options and the PANOSE exceptions
    assert!(click_button(&mut h, "Options"), "title bar Options");
    collect(&mut h, &mut seen, "Options");
    if click_button(&mut h, "&Exceptions...") {
        collect(&mut h, &mut seen, "PANOSE Font Matching Exceptions");
        escape(&mut h, 1);
    }
    escape(&mut h, 1);
    if h.query_by(|n| n.role() == Role::Button && n.label().as_deref() == Some(lbl("&Reset to Default").as_str())).is_some() {
        click_button(&mut h, "OK");
    }
    // About ▸ System Info, Legal Notices
    click_item(&mut h, "Help", "&About AnyFont...");
    collect(&mut h, &mut seen, "About");
    if click_button(&mut h, "System Info...") {
        collect(&mut h, &mut seen, "System Info");
        escape(&mut h, 1);
    }
    if click_button(&mut h, "Legal Notices...") {
        collect(&mut h, &mut seen, "Legal Notices");
        escape(&mut h, 1);
    }
    escape(&mut h, 1);
    for (item, place) in [
        ("Check for &Updates...", "Updates"),
        ("Report a &Bug...", "Report a Bug"),
        ("Submit Ideas and Feedback...", "Submit Ideas and Feedback"),
        ("Get More...", "Get More"),
    ] {
        let menu = if item == "Get More..." { "&File" } else { "Help" };
        click_item(&mut h, menu, item);
        collect(&mut h, &mut seen, place);
        escape(&mut h, 1);
    }
    // the Remove confirmation of a new collection
    click_item(&mut h, "&File", "Add Collection");
    h.key_press(Key::Enter);
    quick(&mut h);
    click_item(&mut h, "&File", "Remove Folder or Collection");
    collect(&mut h, &mut seen, "Remove confirmation");
    escape(&mut h, 1);
    seen
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// An English caption painted where the language has a translation.
    Untranslated,
    /// A caption cut by its clip rectangle or wider than its control.
    Overflow,
    /// The accessibility label (what a screen reader says) is the English key while the control shows the
    /// translation.
    A11y,
}

fn problems(lang: Lang, kind: Kind) -> Vec<String> {
    let seen = walk(lang);
    let keys = english_keys();
    let tr = if lang == Lang::EnUs { HashMap::new() } else { translations(lang.code()) };
    // the user interface strings of this language (what a translated caption looks like)
    let ui_strings: BTreeSet<String> = if lang == Lang::EnUs { keys.clone() } else { tr.values().cloned().collect() };
    let is_ui = |t: &str| ui_strings.contains(t.trim()) || keys.contains(t.trim());
    let mut out: Vec<(Kind, String)> = Vec::new();
    let mut reported = BTreeSet::new();
    for place in seen.iter() {
        for c in &place.controls {
            let l = strip_amp(&c.label);
            if let Some(v) = tr.get(&l) {
                let shown = place.texts.iter().any(|p| p.text.trim() == v.trim() && c.rect.contains(p.rect.center()));
                if v.trim() != l && keys.contains(&l) && shown && reported.insert(format!("a{l}")) {
                    out.push((Kind::A11y, format!("{:?} in {} shows {v:?} but its accessibility label is {l:?}", c.role, place.name)));
                }
            }
        }
        for p in &place.texts {
            let t = p.text.trim();
            if let Some(v) = tr.get(t) {
                if v.trim() != t && keys.contains(t) && reported.insert(format!("u{t}")) {
                    out.push((Kind::Untranslated, format!("untranslated in {}: {t:?} (expected {v:?})", place.name)));
                }
            }
            if !is_ui(t) || t.contains('\n') {
                continue;
            }
            // a caption cut by its clip rectangle
            if (p.rect.right() > p.clip.right() + 1.0 || p.rect.left() < p.clip.left() - 1.0) && p.rect.intersects(p.clip) && reported.insert(format!("c{t}")) {
                out.push((Kind::Overflow, format!("cut off in {}: {t:?} ({:.0} px wide, clip {:.0}..{:.0}, text {:.0}..{:.0})", place.name, p.rect.width(), p.clip.left(), p.clip.right(), p.rect.left(), p.rect.right())));
            }
            // a caption wider than the control it sits in
            // (the control whose caption it is: its accessibility label is the text, or the English key of it)
            let owns = |c: &&Control| {
                let l = strip_amp(&c.label);
                (l == t || tr.get(&l).is_some_and(|v| v.trim() == t))
                    && c.rect.contains(p.rect.center())
                    && p.rect.top() >= c.rect.top() - 2.0
                    && p.rect.bottom() <= c.rect.bottom() + 2.0
            };
            // (a caption is drawn from inside its control: when two controls with the same caption overlap – a
            // menu item over the toolbar button of the same command – the one holding the text's start owns it)
            let mut owners: Vec<&Control> = place.controls.iter().filter(owns).collect();
            owners.sort_by(|a, b| a.rect.area().total_cmp(&b.rect.area()));
            let start = |c: &&&Control| c.rect.left() <= p.rect.left() + 1.0;
            if let Some(c) = owners.iter().find(start).or(owners.first()) {
                let pad = if matches!(c.role, Role::MenuItem | Role::MenuItemCheckBox) { 36.0 } else { 0.0 };
                if p.rect.width() + pad > c.rect.width() + 1.0 && reported.insert(format!("w{t}")) {
                    out.push((Kind::Overflow, format!("wider than its {:?} in {}: {t:?} needs {:.0} px, the control has {:.0} px", c.role, place.name, p.rect.width() + pad, c.rect.width())));
                }
            }
        }
    }
    out.into_iter().filter(|(k, _)| *k == kind).map(|(_, m)| m).collect()
}

fn check(lang: Lang, kind: Kind) {
    let p = problems(lang, kind);
    assert!(p.is_empty(), "{} {kind:?}: {} problems:\n{}", lang.code(), p.len(), p.join("\n"));
}

/// One test per language and check; `[...]` holds the attributes of the untranslated, overflow and
/// accessibility tests of that language (`#[ignore = "BUG-..."]` for the known bugs).
macro_rules! lang_tests {
    ($($name:ident => $lang:expr, [$(#[$u:meta])*], [$(#[$o:meta])*], [$(#[$a:meta])*];)*) => {
        mod untranslated {
            use super::*;
            $( $(#[$u])* #[test] fn $name() { check($lang, Kind::Untranslated) } )*
        }
        mod overflow {
            use super::*;
            $( $(#[$o])* #[test] fn $name() { check($lang, Kind::Overflow) } )*
        }
        mod a11y {
            use super::*;
            $( $(#[$a])* #[test] fn $name() { check($lang, Kind::A11y) } )*
        }
    };
}

lang_tests! {
    en_us => Lang::EnUs, [], [], [];
    pt_br => Lang::PtBr, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    zh_cn => Lang::ZhCn, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    zh_tw => Lang::ZhTw, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    cs_cz => Lang::CsCz, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    nl_nl => Lang::NlNl, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    fr_fr => Lang::FrFr, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    de_de => Lang::DeDe, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    it_it => Lang::ItIt, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    ja_jp => Lang::JaJp, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    pl_pl => Lang::PlPl, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    ru_ru => Lang::RuRu, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    es_es => Lang::EsEs, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    sv_se => Lang::SvSe, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
    tr_tr => Lang::TrTr, [#[ignore = "BUG-201: captions with an & access key are shown untranslated"]], [#[ignore = "BUG-202: Updates dialog: the update choice and its note are cut off"]], [#[ignore = "BUG-203: egui-native buttons expose the English key to screen readers"]];
}

#[test]
fn every_language_is_covered() {
    assert_eq!(Lang::ALL.len(), 15);
}
