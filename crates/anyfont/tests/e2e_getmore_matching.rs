//! File ▸ Get More... (the font catalog, search, filters, Download into the sandbox's content folder, then
//! Install) and PANOSE font matching (Options ▸ Font Matching modes, the Exceptions dialog, the Font
//! Matching Results dialog after an import with missing fonts, the Type 1 message after Add Folder).
//!
//! The catalog is a local JSON file or a local HTTP server (`ANYFONT_CATALOG_URL`); the Google Fonts file
//! list and font downloads are redirected to the local server by `anyfont::testing::redirect_prefix`, and
//! with interception on every other internet address is unreachable (no test reaches the internet).
mod common;
mod svc;

use anyfont::settings::AppSettings;
use anyfont_core::db::FontDatabase;
use anyfont_core::panose::PanoseMode;
use anyfont_sys::FontSystem as _;
use common::http::{Response, Server};
use common::*;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{by, NodeT, Queryable};
use std::path::{Path, PathBuf};
use svc::*;

const SIZE: (f32, f32) = (1400.0, 1000.0);

// --- Get More ---

const CATALOG: &str = r#")]}'
{"familyMetadataList":[
 {"family":"Tuffy","category":"Sans Serif","designers":["Thatcher Ulrich"],"fonts":{"400":{}},"popularity":5,"defaultSort":3,"dateAdded":"2020-01-01","size":40000},
 {"family":"Hack","category":"Monospace","designers":["Source Foundry"],"fonts":{"400":{}},"popularity":3,"defaultSort":2,"dateAdded":"2019-01-01","size":300000},
 {"family":"Zeta Display","category":"Display","designers":["Nobody"],"fonts":{"400":{},"700":{}},"popularity":9,"defaultSort":9,"dateAdded":"2024-01-01"}
]}"#;

/// Removes the network redirections when the test ends.
struct Redirects;
impl Drop for Redirects {
    fn drop(&mut self) {
        anyfont::testing::clear_redirects();
    }
}

/// A local "Google Fonts": the file list of Tuffy and its font file (the core fixture).
fn google(srv: &Server) -> Redirects {
    let manifest = serde_json::json!({
        "zipName": "Tuffy.zip",
        "manifest": {
            "files": [ { "filename": "OFL.txt", "contents": "Test licence of Tuffy." } ],
            "fileRefs": [ { "filename": "Tuffy-Regular.ttf", "url": "https://fonts.gstatic.com/s/tuffy/v1/Tuffy-Regular.ttf" } ]
        }
    });
    srv.route("GET", "/download/list", Response::new(200, "application/json", format!(")]}}'\n{manifest}")));
    srv.route("GET", "/gstatic/s/tuffy/v1/Tuffy-Regular.ttf", Response::bytes(std::fs::read(fixture("Tuffy.ttf").unwrap()).unwrap()));
    anyfont::testing::redirect_prefix("https://fonts.google.com/download/list", &srv.url("/download/list"));
    anyfont::testing::redirect_prefix("https://fonts.gstatic.com/", &srv.url("/gstatic/"));
    Redirects
}

/// The enabled Download buttons of the list (the toolbar's Download is disabled with nothing selected).
fn download_buttons(h: &H<'_>) -> usize {
    buttons(h, "Download").into_iter().filter(|e| *e).count()
}

fn open_get_more(h: &mut H<'_>) {
    click_menu(h, "File", "Get More...");
    wait_for(h, 20, "the catalog", |h| button_enabled(h, "Search") && (download_buttons(h) > 0 || has_text(h, "Retry") || has_text(h, "No fonts found.")));
}

fn search(h: &mut H<'_>, text: &str) {
    let n = || by().role(Role::TextInput).label("Search");
    h.query_all(n()).last().expect("Search box").focus();
    settle(h);
    h.query_all(n()).last().unwrap().type_text(text);
    settle(h);
    click_button(h, "Search");
}

fn checkbox<'t>(h: &'t H<'_>, label: &'t str) -> egui_kittest::Node<'t> {
    h.query_all(by().role(Role::CheckBox).label(label)).last().unwrap_or_else(|| panic!("checkbox {label}"))
}

fn checked(h: &H<'_>, label: &str) -> bool {
    checkbox(h, label).accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn get_more_search_filters_download_and_install() {
    let _lock = env_lock();
    let env = Env::new();
    let catalog = env.path("catalog.json");
    std::fs::write(&catalog, CATALOG).unwrap();
    let _c = EnvVar::set("ANYFONT_CATALOG_URL", &catalog.to_string_lossy());
    let srv = Server::start();
    let _r = google(&srv);
    let fs = env.sandbox_seeded(&[]);
    let mut h = harness(env.config_with(&fs), SIZE);
    assert!(h.state().face_states().is_empty(), "an empty sandbox: {:?}", h.state().face_states());
    open_get_more(&mut h);
    assert_eq!(download_buttons(&h), 3, "the three families of the catalog");
    assert!(checkbox(&h, "Free").accesskit_node().is_disabled(), "Free is always on, disabled");
    assert!(checked(&h, "Free"));
    // category filter: Monospace = Hack only
    checkbox(&h, "Monospace").click();
    settle(&mut h);
    assert_eq!(download_buttons(&h), 1);
    checkbox(&h, "Display").click();
    settle(&mut h);
    assert_eq!(download_buttons(&h), 2, "Monospace or Display");
    checkbox(&h, "Monospace").click();
    settle(&mut h);
    checkbox(&h, "Display").click();
    settle(&mut h);
    assert_eq!(download_buttons(&h), 3, "no category = all");
    // sort
    choose(&mut h, "Sort by:", "Name");
    assert_eq!(combo_value(&h, "Sort by:"), "Name");
    // search by name, then by designer
    search(&mut h, "zzz");
    assert_eq!(download_buttons(&h), 0);
    assert!(has_text(&h, "No fonts found."));
    search_clear(&mut h);
    assert_eq!(download_buttons(&h), 3, "an empty search shows all");
    search(&mut h, "thatcher");
    assert_eq!(download_buttons(&h), 1, "Tuffy by its designer");
    // Download
    click_button(&mut h, "Download");
    wait_for(&mut h, 20, "the download to finish", |h| has_text(h, "Downloading Tuffy... 100%") && button_enabled(h, "Close"));
    let content = h.state().font_system().content_folder();
    let file = content.join("Tuffy").join("Tuffy-Regular.ttf");
    assert_eq!(std::fs::read(&file).ok(), std::fs::read(fixture("Tuffy.ttf").unwrap()).ok(), "{} downloaded", file.display());
    assert_eq!(std::fs::read_to_string(content.join("Tuffy").join("OFL.txt")).unwrap(), "Test licence of Tuffy.");
    let list = srv.requests_to("/download/list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].path, "/download/list?family=Tuffy");
    assert!(list[0].header("user-agent").is_some_and(|u| u.starts_with("AnyFont/")));
    click_button(&mut h, "Close");
    assert!(buttons(&h, "Downloaded").iter().all(|e| !e) && buttons(&h, "Downloaded").len() == 1, "the row says Downloaded: {:?}", button_labels(&h));
    // My Library: only the downloaded family
    search_clear(&mut h);
    checkbox(&h, "My Library").click();
    settle(&mut h);
    assert_eq!(buttons(&h, "Downloaded").len(), 1);
    assert_eq!(download_buttons(&h), 0);
    // close Get More: the font is in My fonts, not installed
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    assert!(buttons(&h, "Search").is_empty(), "Get More closed");
    wait_until(&mut h, 20, "the downloaded font in My fonts", |h| h.state().face_states().iter().any(|(l, _)| l.starts_with("Tuffy")));
    let (label, installed) = h.state().face_states().into_iter().find(|(l, _)| l.starts_with("Tuffy")).unwrap();
    assert!(!installed, "downloaded fonts are not installed");
    // Install it (in the sandbox)
    h.get_by(|n| n.role() == Role::ListBoxOption && n.label().as_deref() == Some(label.as_str())).click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Install").click();
    finish_operation(&mut h);
    if h.state().pending_dialogs() > 0 {
        // "Install for me" when the sandbox asks (never the case with elevation allowed)
        if button_enabled(&h, "Install for me") {
            click_button(&mut h, "Install for me");
            finish_operation(&mut h);
        }
    }
    let now = h.state().face_states().into_iter().find(|(l, _)| *l == label).unwrap();
    assert!(now.1, "installed: {:?}", h.state().face_states());
    assert!(fs.installed_fonts().unwrap().iter().any(|f| f.path.file_name().is_some_and(|n| n.to_string_lossy().contains("Tuffy"))), "in the sandbox's fonts");
}

fn search_clear(h: &mut H<'_>) {
    h.query_all(by().role(Role::TextInput).label("Search")).last().expect("Search box").focus();
    settle(h);
    for _ in 0..12 {
        h.key_press(egui::Key::Backspace);
    }
    settle(h);
}

#[test]
fn get_more_offline_then_retry() {
    let _lock = env_lock();
    let env = Env::new();
    let _c = EnvVar::set("ANYFONT_CATALOG_URL", DEAD_URL);
    let mut h = harness(env.config(), SIZE);
    open_get_more(&mut h);
    assert!(has_text(&h, "Remote font catalog cannot connect to the internet."), "{:?}", all_texts(&h));
    assert_eq!(download_buttons(&h), 0);
    // a broken catalog
    let srv = Server::start();
    srv.route("GET", "/catalog", Response::new(200, "application/json", "{\"nothing\":1}"));
    std::env::set_var("ANYFONT_CATALOG_URL", srv.url("/catalog"));
    click_button(&mut h, "Retry");
    wait_text(&mut h, 20, "An error occurred while downloading the remote font catalog.");
    // the connection is back
    srv.route("GET", "/catalog", Response::new(200, "application/json", CATALOG));
    click_button(&mut h, "Retry");
    wait_for(&mut h, 20, "the catalog", |h| download_buttons(h) == 3);
    assert!(!has_text(&h, "cannot connect"));
}

#[test]
fn get_more_download_failure_is_reported_and_nothing_is_left() {
    let _lock = env_lock();
    let env = Env::new();
    let catalog = env.path("catalog.json");
    std::fs::write(&catalog, CATALOG).unwrap();
    let _c = EnvVar::set("ANYFONT_CATALOG_URL", &catalog.to_string_lossy());
    let srv = Server::start();
    let _r = google(&srv);
    // the font file is missing on the server
    srv.route("GET", "/gstatic/s/tuffy/v1/Tuffy-Regular.ttf", Response::new(404, "text/plain", "gone"));
    let fs = env.sandbox_seeded(&[]);
    let mut h = harness(env.config_with(&fs), SIZE);
    open_get_more(&mut h);
    search(&mut h, "tuffy");
    click_button(&mut h, "Download");
    wait_text(&mut h, 20, "Unable to download Tuffy.");
    assert!(has_text(&h, "HTTP 404"), "{:?}", all_texts(&h));
    click_button(&mut h, "Close");
    let dir = h.state().font_system().content_folder().join("Tuffy");
    assert!(!dir.exists(), "the family folder is removed again");
    assert_eq!(download_buttons(&h), 1, "can be tried again");
}

// --- PANOSE font matching ---

fn db_path(env: &Env) -> PathBuf {
    env.root().join("anyfont-data").join("data").join("fonts.db")
}

fn settings_path(env: &Env) -> PathBuf {
    env.root().join("anyfont-data").join("settings").join("settings.json")
}

fn open_options(h: &mut H<'_>) {
    h.get_by_role_and_label(Role::Button, "Options").click();
    settle(h);
    assert!(h.query_by_role_and_label(Role::RadioButton, "Never show").is_some(), "Options open: {:?}", labels_of(h, Role::RadioButton));
}

fn radio_on(h: &H<'_>, label: &str) -> bool {
    h.get_by_role_and_label(Role::RadioButton, label).accesskit_node().toggled() == Some(Toggled::True)
}

fn click_radio(h: &mut H<'_>, label: &str) {
    h.query_all(by().role(Role::RadioButton).label(label)).last().unwrap_or_else(|| panic!("radio {label}")).click();
    settle(h);
}

#[test]
fn font_matching_modes_are_saved() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    open_options(&mut h);
    for (label, mode) in [("Text and styles", PanoseMode::TextAndStyles), ("Never show", PanoseMode::NeverShow), ("Text", PanoseMode::Text)] {
        click_radio(&mut h, label);
        assert!(radio_on(&h, label), "{label} chosen");
        assert_eq!(h.state().settings().core.panose.mode, mode);
    }
    click_radio(&mut h, "Never show");
    click_button(&mut h, "OK");
    assert!(h.query_by_role_and_label(Role::RadioButton, "Never show").is_none(), "Options closed");
    let saved = AppSettings::load(&settings_path(&env));
    assert_eq!(saved.core.panose.mode, PanoseMode::NeverShow, "saved by OK");
    open_options(&mut h);
    assert!(radio_on(&h, "Never show"), "shown again");
}

#[test]
fn exceptions_add_and_remove() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    open_options(&mut h);
    click_button(&mut h, "Exceptions...");
    assert!(button_enabled(&h, "OK") && !button_enabled(&h, "Add Exception") && !button_enabled(&h, "Remove Exception"), "{:?}", button_labels(&h));
    let add = |h: &mut H<'_>, missing: &str, sub: &str| {
        h.get_by_role_and_label(Role::TextInput, "Missing Font:").focus();
        settle(h);
        h.get_by_role_and_label(Role::TextInput, "Missing Font:").type_text(missing);
        settle(h);
        choose(h, "Substituted Font:", sub);
        assert!(button_enabled(h, "Add Exception"));
        click_button(h, "Add Exception");
    };
    add(&mut h, "Helvetica Test", "Arial");
    add(&mut h, "Futura Test", "Times New Roman");
    // the row just added is selected: Remove Exception takes it out again
    assert!(button_enabled(&h, "Remove Exception"));
    click_button(&mut h, "Remove Exception");
    assert!(!button_enabled(&h, "Remove Exception"));
    // OK saves the list in the database (Cancel would not)
    click_button(&mut h, "OK");
    assert!(buttons(&h, "Add Exception").is_empty(), "Exceptions closed");
    let db = FontDatabase::open(&db_path(&env)).unwrap();
    let list: Vec<(String, String)> = db.panose_exceptions().unwrap().into_iter().map(|e| (e.missing, e.substitute)).collect();
    assert_eq!(list, vec![("Helvetica Test".to_string(), "Arial".to_string())]);
    // Cancel discards a change
    click_button(&mut h, "Exceptions...");
    add(&mut h, "Garamond Test", "Arial");
    click_button(&mut h, "Cancel");
    let list = FontDatabase::open(&db_path(&env)).unwrap().panose_exceptions().unwrap();
    assert_eq!(list.len(), 1, "Cancel keeps the saved list");
}

/// An exchange file whose "Imported" collection holds a face that is not on this computer.
fn import_file(env: &Env) -> PathBuf {
    let xml = r#"<?xml version='1.0'?><Export format="AnyFont" version="1"><UserGroups><group name="Imported"><font DBFamily="Helvetica Neue Test" DBSubFamily="Bold" DBVersion="1.0" DBTech="TrueType" DBWeight="700" DBWidth="5" DBItalic="0" DBManufacturer=""/></group></UserGroups></Export>"#;
    let p = env.path("collections.xml");
    std::fs::write(&p, xml).unwrap();
    p
}

fn set_mode(h: &mut H<'_>, label: &str) {
    open_options(h);
    click_radio(h, label);
    click_button(h, "OK");
}

#[test]
fn import_with_missing_fonts_links_the_chosen_substitute() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    set_mode(&mut h, "Text");
    let file = import_file(&env);
    h.state_mut().import_path(&file);
    settle(&mut h);
    wait_for(&mut h, 20, "the matching to finish", |h| button_enabled(h, "OK") && has_text(h, "Substitute font with:"));
    click_radio(&mut h, "Substitute font with:");
    choose(&mut h, "Substitute font with:", "Arial");
    click_button(&mut h, "OK");
    h.get_by_role_and_label(Role::ListBoxOption, "Imported").click();
    settle(&mut h);
    assert_eq!(h.state().status_text(), "Imported: 1 fonts", "the substitute takes the missing font's place");
}

#[test]
fn import_with_missing_fonts_saves_a_permanent_substitution_and_exception() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    set_mode(&mut h, "Text");
    let file = import_file(&env);
    h.state_mut().import_path(&file);
    settle(&mut h);
    wait_for(&mut h, 20, "the Font Matching Results dialog", |h| button_enabled(h, "OK") && has_text(h, "Substitute font with:"));
    for r in ["Use the PANOSE suggested match", "Substitute font with:", "Permanent - keep these fonts", "Temporary - use original fonts"] {
        assert!(h.query_all(by().role(Role::RadioButton).label(r)).next().is_some(), "{r}: {:?}", labels_of(&h, Role::RadioButton));
    }
    click_radio(&mut h, "Substitute font with:");
    choose(&mut h, "Substitute font with:", "Arial");
    assert_eq!(combo_value(&h, "Substitute font with:"), "Arial");
    checkbox_click(&mut h, "Save this exception for future font matching");
    click_radio(&mut h, "Permanent - keep these fonts");
    click_button(&mut h, "OK");
    assert!(!has_text(&h, "Substitute font with:"), "OK closes the dialog");
    let db = FontDatabase::open(&db_path(&env)).unwrap();
    let subs: Vec<(String, String)> = db.substitutions().unwrap().into_iter().map(|s| (s.missing, s.substitute)).collect();
    assert_eq!(subs, vec![("Helvetica Neue Test".to_string(), "Arial".to_string())], "a permanent substitution");
    let exc: Vec<(String, String)> = db.panose_exceptions().unwrap().into_iter().map(|e| (e.missing, e.substitute)).collect();
    assert_eq!(exc, vec![("Helvetica Neue Test".to_string(), "Arial".to_string())], "the saved exception");
    assert!(h.state().library_names().contains(&"Imported".to_string()), "{:?}", h.state().library_names());
}

fn checkbox_click(h: &mut H<'_>, label: &str) {
    h.query_all(by().role(Role::CheckBox).label(label)).last().unwrap_or_else(|| panic!("checkbox {label}")).click();
    settle(h);
}

#[test]
fn never_show_imports_without_the_results_dialog() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    set_mode(&mut h, "Never show");
    let file = import_file(&env);
    h.state_mut().import_path(&file);
    settle(&mut h);
    settle(&mut h);
    assert!(!has_text(&h, "Substitute font with:"), "no Font Matching Results dialog");
    assert!(buttons(&h, "Stop").is_empty());
    assert!(h.state().library_names().contains(&"Imported".to_string()), "the collection is imported");
}

#[test]
fn results_cancel_saves_nothing() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    set_mode(&mut h, "Text and styles");
    let file = import_file(&env);
    h.state_mut().import_path(&file);
    settle(&mut h);
    wait_for(&mut h, 20, "the Font Matching Results dialog", |h| button_enabled(h, "Cancel") && has_text(h, "Substitute font with:"));
    click_radio(&mut h, "Permanent - keep these fonts");
    click_button(&mut h, "Cancel");
    assert!(!has_text(&h, "Substitute font with:"));
    let db = FontDatabase::open(&db_path(&env)).unwrap();
    assert!(db.substitutions().unwrap().is_empty());
    assert!(db.panose_exceptions().unwrap().is_empty());
}

// --- Type 1 ---

/// A tiny Type 1 font (`.pfb`) with three glyphs, as in the core's tests.
fn build_pfb(font_name: &str, family: &str, weight: &str) -> Vec<u8> {
    let clear = format!(
        "%!PS-AdobeFont-1.0: {font_name} 001.000\n11 dict begin\n/FontInfo 10 dict dup begin\n\
         /version (001.000) readonly def\n/Notice (Test notice) readonly def\n\
         /FullName ({family} {weight}) readonly def\n/FamilyName ({family}) readonly def\n\
         /Weight ({weight}) readonly def\n/ItalicAngle 0 def\n/isFixedPitch false def\nend readonly def\n\
         /FontName /{font_name} def\n/Encoding StandardEncoding def\ncurrentdict end\ncurrentfile eexec\n"
    );
    let mut private = b"dup /Private 8 dict dup begin\n/RD{string currentfile exch readstring pop}executeonly def\n\
                         end\n2 index /CharStrings 3 dict dup begin\n"
        .to_vec();
    for (name, len) in [(".notdef", 4usize), ("A", 6), ("Aacute", 5)] {
        private.extend_from_slice(format!("/{name} {len} RD ").as_bytes());
        private.extend(std::iter::repeat_n(0xAAu8, len));
        private.extend_from_slice(b" ND\n");
    }
    private.extend_from_slice(b"end\nend\nreadonly put\nput\ndup /FontName get exch definefont pop\nmark currentfile closefile\n");
    let enc = anyfont_core::type1::eexec_encrypt(&private);
    let trailer = format!("{}cleartomark\n", "0".repeat(64));
    let mut out = Vec::new();
    for (kind, seg) in [(1u8, clear.as_bytes()), (2u8, enc.as_slice()), (1u8, trailer.as_bytes())] {
        out.push(0x80);
        out.push(kind);
        out.extend_from_slice(&(seg.len() as u32).to_le_bytes());
        out.extend_from_slice(seg);
    }
    out.extend_from_slice(&[0x80, 3]);
    out
}

/// A tiny `.pfm` (Windows metrics) for the `.pfb`.
fn build_pfm(face: &str, ps: &str, weight: u16) -> Vec<u8> {
    let mut d = vec![0u8; 147];
    d[0..2].copy_from_slice(&0x0100u16.to_le_bytes());
    d[6..20].copy_from_slice(b"Copyright test");
    d[83..85].copy_from_slice(&weight.to_le_bytes());
    let face_off = d.len() as u32;
    d.extend_from_slice(face.as_bytes());
    d.push(0);
    let ps_off = d.len() as u32;
    d.extend_from_slice(ps.as_bytes());
    d.push(0);
    d[105..109].copy_from_slice(&face_off.to_le_bytes());
    d[139..143].copy_from_slice(&ps_off.to_le_bytes());
    let len = d.len() as u32;
    d[2..6].copy_from_slice(&len.to_le_bytes());
    d
}

fn type1_folder(env: &Env, name: &str, family: &str) -> PathBuf {
    let dir = env.path(name);
    std::fs::create_dir_all(&dir).unwrap();
    let ps = format!("{}-Bold", family.replace(' ', ""));
    std::fs::write(dir.join(format!("{ps}.pfb")), build_pfb(&ps, family, "Bold")).unwrap();
    std::fs::write(dir.join(format!("{ps}.pfm")), build_pfm(family, &ps, 700)).unwrap();
    dir
}

const TYPE1_TITLE: &str = "Type 1 fonts detected";

fn add_folder(h: &mut H<'_>, dir: &Path) {
    h.state_mut().add_folder_path(dir);
    settle(h);
    settle(h);
}

#[test]
fn type1_message_after_add_folder_and_replace_fonts() {
    let env = Env::new();
    let fs = env.sandbox();
    let mut h = harness(env.config_with(&fs), SIZE);
    let dir = type1_folder(&env, "Type1", "Test Serif");
    add_folder(&mut h, &dir);
    assert!(h.state().face_states().iter().any(|(l, _)| l.starts_with("Test Serif")), "the Type 1 face is listed: {:?}", h.state().row_labels());
    assert!(has_text(&h, TYPE1_TITLE), "{:?}", all_texts(&h));
    assert!(has_text(&h, anyfont::matching::TYPE1_TEXT));
    assert!(button_enabled(&h, "Replace fonts"));
    assert!(!checked(&h, "Do not show again"));
    // Replace fonts: the results dialog with a suggested OpenType/TrueType replacement
    click_button(&mut h, "Replace fonts");
    assert!(!has_text(&h, TYPE1_TITLE));
    wait_for(&mut h, 20, "the replacements", |h| button_enabled(h, "OK"));
    assert!(has_text(&h, "AnyFont suggests an OpenType or TrueType replacement for each Type 1 font."), "{:?}", all_texts(&h));
    click_button(&mut h, "Cancel");
    assert!(!button_enabled(&h, "Replace fonts"));
}

#[test]
fn type1_message_do_not_show_again() {
    let env = Env::new();
    let fs = env.sandbox();
    let mut h = harness(env.config_with(&fs), SIZE);
    add_folder(&mut h, &type1_folder(&env, "Type1a", "Test Serif"));
    assert!(has_text(&h, TYPE1_TITLE));
    checkbox_click(&mut h, "Do not show again");
    assert!(checked(&h, "Do not show again"));
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    assert!(!has_text(&h, TYPE1_TITLE), "Esc closes the message");
    add_folder(&mut h, &type1_folder(&env, "Type1b", "Other Serif"));
    assert!(h.state().face_states().iter().any(|(l, _)| l.starts_with("Other Serif")), "{:?}", h.state().row_labels());
    assert!(!has_text(&h, TYPE1_TITLE), "not shown again");
}
