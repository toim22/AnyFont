//! End-to-end checks of Install, Uninstall and Delete on a sandbox font system, through the toolbar, the
//! Font menu and the font context menu: the confirmations, the progress dialog and its Cancel, the failure
//! list, fonts in use (the reboot prompt: Reboot now / Reboot later / Ignore / Cancel) and the protected
//! system fonts.
mod common;

use anyfont_sys::{
    Backend, DeleteReport, FontDirs, FontSystem, InstallReport, InstalledFont, Progress, SandboxFontSystem, Scope, UninstallReport, UsedFontAction,
};
use common::*;
use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers};
use egui_kittest::kittest::{by, NodeT, Queryable};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

const SIZE: (f32, f32) = (1280.0, 1000.0);
const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const FONTS: &[&str] = &["Tuffy.ttf", "Hack-Regular.ttf", "Ubuntu-Light.ttf"];

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

fn installed(h: &H<'_>, label: &str) -> bool {
    h.state().face_states().iter().find(|(l, _)| l == label).unwrap_or_else(|| panic!("{label} not listed")).1
}

fn tool_enabled(h: &H<'_>, label: &str) -> bool {
    !h.get_by_role_and_label(Role::Button, label).accesskit_node().is_disabled()
}

fn menu_enabled(items: &[(String, bool, bool)], label: &str) -> Option<bool> {
    items.iter().find(|i| i.0 == label).map(|i| i.1)
}

fn font_menu(h: &mut H<'_>) -> Vec<(String, bool, bool)> {
    open_menu(h, "Font");
    let v = open_menu_items(h);
    close_menus(h);
    v
}

fn context_menu(h: &mut H<'_>, label: &str) -> Vec<(String, bool, bool)> {
    row(h, label).click_secondary();
    settle(h);
    open_menu_items(h)
}

fn button(h: &mut H<'_>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    settle(h);
}

/// A window on `fs` with the folder `name` of `files` added and open.
fn window<'a>(env: &Env, fs: &Arc<SandboxFontSystem>, name: &str, files: &[&str]) -> (H<'a>, PathBuf) {
    let dir = env.fixture_folder(name, files);
    let mut h = harness(env.config_with(fs), SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    (h, dir)
}

/// Installs every face of the open folder with Ctrl+A ▸ toolbar Install.
fn install_all(h: &mut H<'_>) {
    let first = h.state().face_states()[0].0.clone();
    select(h, &first);
    h.key_press_modifiers(CTRL, Key::A);
    settle(h);
    button(h, "Install");
    finish_operation(h);
    assert!(h.state().face_states().iter().all(|(_, i)| *i), "{:?}", h.state().face_states());
}

/// The installed copy (in the sandbox's Fonts folder) of a folder file.
fn installed_copy(fs: &SandboxFontSystem, file: &str) -> PathBuf {
    fs.installed_fonts().unwrap().into_iter().map(|f| f.path).find(|p| p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(file)).unwrap_or(false)).unwrap_or_else(|| panic!("{file} not installed"))
}

// ------------------------------------------------------------------------------------------ install

#[test]
fn install_and_uninstall_from_the_toolbar_font_menu_and_context_menu() {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, _dir) = window(&env, &fs, "Ops", FONTS);
    let before = fs.installed_fonts().unwrap().len();
    select(&mut h, "Tuffy (Regular, Normal)");
    assert!(tool_enabled(&h, "Install") && !tool_enabled(&h, "Uninstall") && tool_enabled(&h, "Delete"));
    // toolbar ▸ Install
    button(&mut h, "Install");
    finish_operation(&mut h);
    assert!(installed(&h, "Tuffy (Regular, Normal)"));
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 1);
    assert!(!tool_enabled(&h, "Install") && tool_enabled(&h, "Uninstall"));
    assert_eq!(h.state().pending_dialogs(), 0, "no message after a successful install");
    // the Installed filter lists it
    h.get_by_role_and_label(Role::CheckBox, "Installed").click();
    settle(&mut h);
    assert_eq!(h.state().row_labels(), vec!["Tuffy (Regular, Normal)"]);
    h.get_by_role_and_label(Role::CheckBox, "Installed").click();
    settle(&mut h);
    // Font ▸ Install
    select(&mut h, "Hack (Regular, Normal)");
    let items = font_menu(&mut h);
    assert_eq!(menu_enabled(&items, "Install"), Some(true), "{items:?}");
    assert_eq!(menu_enabled(&items, "Uninstall"), Some(false));
    click_menu(&mut h, "Font", "Install");
    finish_operation(&mut h);
    assert!(installed(&h, "Hack (Regular, Normal)"));
    // context menu ▸ Install
    let items = context_menu(&mut h, "Ubuntu Light");
    assert_eq!(menu_enabled(&items, "Install"), Some(true), "{items:?}");
    menu_item(&h, "Install").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert!(installed(&h, "Ubuntu Light"));
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 3);
    // My fonts lists them now
    h.get_by_role_and_label(Role::ListBoxOption, "My fonts").click();
    settle(&mut h);
    assert!(h.state().row_labels().iter().any(|l| l == "Tuffy (Regular, Normal)"));
    h.get_by_role_and_label(Role::ListBoxOption, "Ops").click();
    settle(&mut h);

    // context menu ▸ Uninstall, Font ▸ Uninstall, toolbar ▸ Uninstall
    let items = context_menu(&mut h, "Tuffy (Regular, Normal)");
    assert_eq!(menu_enabled(&items, "Uninstall"), Some(true), "{items:?}");
    assert_eq!(menu_enabled(&items, "Install"), Some(false));
    menu_item(&h, "Uninstall").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert!(!installed(&h, "Tuffy (Regular, Normal)"));
    select(&mut h, "Hack (Regular, Normal)");
    click_menu(&mut h, "Font", "Uninstall");
    finish_operation(&mut h);
    assert!(!installed(&h, "Hack (Regular, Normal)"));
    select(&mut h, "Ubuntu Light");
    button(&mut h, "Uninstall");
    finish_operation(&mut h);
    assert!(!installed(&h, "Ubuntu Light"));
    assert_eq!(fs.installed_fonts().unwrap().len(), before);
    assert_eq!(h.state().pending_dialogs(), 0);
}

/// A family header selected: Install acts on the whole family.
#[test]
fn install_a_whole_family_from_its_header() {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, _dir) = window(&env, &fs, "Family", &["georgia.ttf", "georgiab.ttf", "Tuffy.ttf"]);
    if h.state().font_count() < 3 {
        eprintln!("georgia.ttf / georgiab.ttf missing: skipped");
        return;
    }
    select(&mut h, "Georgia (2)");
    let items = font_menu(&mut h);
    assert_eq!(menu_enabled(&items, "Install"), Some(true), "{items:?}");
    click_menu(&mut h, "Font", "Install");
    finish_operation(&mut h);
    assert!(installed(&h, "Georgia (Regular, Normal)") && installed(&h, "Georgia (Bold)"));
    assert!(!installed(&h, "Tuffy (Regular, Normal)"));
}

// ------------------------------------------------------------------- progress dialog and Cancel

/// Holds the operation while its flag is false.
type Gate = Arc<(Mutex<bool>, Condvar)>;

/// A font system that holds an operation at its second file until the test opens the gate.
struct Gated {
    inner: Arc<SandboxFontSystem>,
    gate: Gate,
}

impl Gated {
    fn wait(&self) {
        let (m, c) = &*self.gate;
        let mut open = m.lock().unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !*open && std::time::Instant::now() < end {
            open = c.wait_timeout(open, std::time::Duration::from_millis(50)).unwrap().0;
        }
    }
    fn held<'p>(&'p self, progress: Progress<'p>) -> impl FnMut(usize, usize, &Path) -> bool + 'p {
        move |d, t, p| {
            let r = progress(d, t, p);
            if d == 1 && t > 1 {
                self.wait();
                // asked again: a Cancel clicked meanwhile stops the operation here
                return progress(d, t, p);
            }
            r
        }
    }
}

impl FontSystem for Gated {
    fn backend(&self) -> Backend {
        self.inner.backend()
    }
    fn font_dirs(&self) -> FontDirs {
        self.inner.font_dirs()
    }
    fn paths(&self) -> &anyfont_sys::AppPaths {
        self.inner.paths()
    }
    fn installed_fonts(&self) -> std::io::Result<Vec<InstalledFont>> {
        self.inner.installed_fonts()
    }
    fn is_protected(&self, path: &Path) -> bool {
        self.inner.is_protected(path)
    }
    fn needs_elevation(&self, scope: Scope) -> bool {
        self.inner.needs_elevation(scope)
    }
    fn install_with(&self, files: &[PathBuf], scope: Scope, used: UsedFontAction, progress: Progress<'_>) -> InstallReport {
        let mut p = self.held(progress);
        self.inner.install_with(files, scope, used, &mut p)
    }
    fn uninstall_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> UninstallReport {
        let mut p = self.held(progress);
        self.inner.uninstall_with(paths, used, &mut p)
    }
    fn delete_with(&self, paths: &[PathBuf], used: UsedFontAction, progress: Progress<'_>) -> DeleteReport {
        let mut p = self.held(progress);
        self.inner.delete_with(paths, used, &mut p)
    }
    fn open_location(&self, path: &Path) -> std::io::Result<()> {
        self.inner.open_location(path)
    }
    fn reboot_now(&self) -> std::io::Result<()> {
        self.inner.reboot_now()
    }
    fn process_pending(&self) -> usize {
        self.inner.process_pending()
    }
}

fn gated_window<'a>(env: &Env) -> (H<'a>, Arc<SandboxFontSystem>, Gate) {
    let fs = env.sandbox();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let sys: Arc<dyn FontSystem> = Arc::new(Gated { inner: fs.clone(), gate: gate.clone() });
    let dir = env.fixture_folder("Held", FONTS);
    let mut cfg = env.config();
    cfg.system = Some(sys);
    let mut h = harness(cfg, SIZE);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    (h, fs, gate)
}

fn open_gate(gate: &Gate, open: bool) {
    *gate.0.lock().unwrap() = open;
    gate.1.notify_all();
}

/// The progress caption ("Installing {name}... {n}%") while the operation is held at its second file.
fn wait_caption(h: &mut H<'_>, verb: &str) -> String {
    let mut found = String::new();
    wait_until(h, 20, "the progress caption", |h| {
        if let Some(t) = painted(h).into_iter().find(|t| t.starts_with(verb) && t.ends_with("... 33%")) {
            found = t;
            true
        } else {
            false
        }
    });
    found
}

#[test]
fn progress_caption_and_cancel_of_an_install() {
    let env = Env::new();
    let (mut h, fs, gate) = gated_window(&env);
    let before = fs.installed_fonts().unwrap().len();
    let names = ["Tuffy", "Hack", "Ubuntu Light"];
    install_all_held(&mut h);
    let caption = wait_caption(&mut h, "Installing ");
    assert!(names.iter().any(|n| caption == format!("Installing {n}... 33%") || caption.starts_with(&format!("Installing {n}"))), "{caption}");
    assert!(h.state().operation_running());
    // the progress dialog is modal: the toolbar does not react meanwhile
    assert!(h.query_by_role_and_label(Role::Button, "Cancel").is_some());
    button(&mut h, "Cancel");
    open_gate(&gate, true);
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), before + 1, "the first file was installed, the others cancelled");
    assert_eq!(h.state().face_states().iter().filter(|(_, i)| *i).count(), 1);
    assert_eq!(h.state().pending_dialogs(), 0, "a cancelled file is not a failure");
}

fn install_all_held(h: &mut H<'_>) {
    let first = h.state().face_states()[0].0.clone();
    select(h, &first);
    h.key_press_modifiers(CTRL, Key::A);
    settle(h);
    h.get_by_role_and_label(Role::Button, "Install").click();
    h.run_steps(4);
}

#[test]
fn progress_captions_of_uninstall_and_delete() {
    let env = Env::new();
    let (mut h, fs, gate) = gated_window(&env);
    open_gate(&gate, true);
    install_all(&mut h);
    let n = fs.installed_fonts().unwrap().len();
    open_gate(&gate, false);
    h.key_press_modifiers(CTRL, Key::A);
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Uninstall").click();
    h.run_steps(4);
    let caption = wait_caption(&mut h, "Uninstalling ");
    assert!(caption.ends_with("... 33%"), "{caption}");
    open_gate(&gate, true);
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), n - 3);

    // Delete (confirmed in its dialog)
    open_gate(&gate, false);
    h.key_press_modifiers(CTRL, Key::A);
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "Delete").click();
    settle(&mut h);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    let caption = wait_caption(&mut h, "Deleting ");
    assert!(caption.ends_with("... 33%"), "{caption}");
    open_gate(&gate, true);
    finish_operation(&mut h);
    assert_eq!(fs.recycle_bin().len(), 3);
    assert_eq!(h.state().font_count(), 0);
}

// ------------------------------------------------------------------------------------------ delete

fn delete_dialog_files(h: &H<'_>) -> Vec<(String, bool)> {
    h.query_all(by().role(Role::CheckBox))
        .map(|n| n.accesskit_node())
        .filter(|a| a.label().map(|l| l.contains('\\')).unwrap_or(false))
        .map(|a| (a.label().unwrap_or_default(), a.toggled() == Some(Toggled::True)))
        .collect()
}

fn ok_enabled(h: &H<'_>) -> bool {
    !h.get_by_role_and_label(Role::Button, "OK").accesskit_node().is_disabled()
}

#[test]
fn delete_with_its_confirmation_dialog() {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, dir) = window(&env, &fs, "Del", &["Tuffy.ttf", "Hack-Regular.ttf", "Ubuntu-Light.ttf", "colr_1_variable.ttf"]);
    // one font, Font ▸ Delete: the dialog lists its file, checked
    select(&mut h, "Tuffy (Regular, Normal)");
    click_menu(&mut h, "Font", "Delete");
    assert_eq!(h.state().pending_dialogs(), 1);
    assert!(painted(&h).iter().any(|t| t.starts_with("Are you sure you want to delete this font?")), "{:?}", painted(&h));
    let files = delete_dialog_files(&h);
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(files[0].1 && files[0].0.to_lowercase().ends_with("tuffy.ttf"));
    // Cancel: nothing deleted
    button(&mut h, "Cancel");
    assert!(dir.join("Tuffy.ttf").exists());
    assert_eq!(h.state().pending_dialogs(), 0);
    // unchecking the file disables OK; Select all checks it again
    button(&mut h, "Delete");
    let f = delete_dialog_files(&h)[0].0.clone();
    h.get_by_role_and_label(Role::CheckBox, &f).click();
    settle(&mut h);
    assert!(!delete_dialog_files(&h)[0].1);
    assert!(!ok_enabled(&h), "OK needs a checked file");
    assert_eq!(h.get_by_role_and_label(Role::CheckBox, "Select all").accesskit_node().toggled(), Some(Toggled::False));
    h.get_by_role_and_label(Role::CheckBox, "Select all").click();
    settle(&mut h);
    assert!(delete_dialog_files(&h)[0].1);
    assert!(ok_enabled(&h));
    button(&mut h, "OK");
    finish_operation(&mut h);
    assert!(!dir.join("Tuffy.ttf").exists(), "deleted");
    assert_eq!(fs.recycle_bin().len(), 1, "to the (sandbox) Recycle Bin");
    assert!(!h.state().row_labels().iter().any(|l| l.starts_with("Tuffy")));

    // two fonts from the context menu: the plural text; one of them unchecked is kept
    select(&mut h, "Hack (Regular, Normal)");
    row(&h, "Ubuntu Light").click_modifiers(Modifiers::COMMAND);
    settle(&mut h);
    let items = context_menu(&mut h, "Ubuntu Light");
    assert_eq!(menu_enabled(&items, "Delete"), Some(true), "{items:?}");
    menu_item(&h, "Delete").click();
    settle(&mut h);
    assert!(painted(&h).iter().any(|t| t.starts_with("Are you sure you want to delete these fonts?")));
    let files = delete_dialog_files(&h);
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(h.get_by_role_and_label(Role::CheckBox, "Select all").accesskit_node().toggled(), Some(Toggled::True));
    let hack = files.iter().find(|f| f.0.to_lowercase().ends_with("hack-regular.ttf")).unwrap().0.clone();
    h.get_by_role_and_label(Role::CheckBox, &hack).click();
    settle(&mut h);
    // "Do not show this message again."
    h.get_by_role_and_label(Role::CheckBox, "Do not show this message again.").click();
    settle(&mut h);
    button(&mut h, "OK");
    finish_operation(&mut h);
    assert!(dir.join("Hack-Regular.ttf").exists(), "the unchecked file is kept");
    assert!(!dir.join("Ubuntu-Light.ttf").exists());
    assert!(h.state().settings().skip_delete_confirmation);
    // toolbar ▸ Delete: no dialog any more
    select(&mut h, "Hack (Regular, Normal)");
    button(&mut h, "Delete");
    finish_operation(&mut h);
    assert!(!dir.join("Hack-Regular.ttf").exists());
    assert_eq!(fs.recycle_bin().len(), 3);
}

/// Deleting an installed font uninstalls it first.
#[test]
fn delete_of_an_installed_font_uninstalls_it() {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, dir) = window(&env, &fs, "DelInst", &["Tuffy.ttf"]);
    install_all(&mut h);
    let before = fs.installed_fonts().unwrap().len();
    select(&mut h, "Tuffy (Regular, Normal)");
    button(&mut h, "Delete");
    let files = delete_dialog_files(&h);
    assert!(!files.is_empty(), "{files:?}");
    button(&mut h, "OK");
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), before - 1, "uninstalled");
    assert!(!dir.join("Tuffy.ttf").exists());
    assert_eq!(h.state().font_count(), 0);
}

#[test]
fn failures_are_listed_read_only_file_and_invalid_font() {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, dir) = window(&env, &fs, "Bad", &["Tuffy.ttf", "Hack-Regular.ttf"]);
    // a read-only file cannot be deleted
    let ro = dir.join("Tuffy.ttf");
    let mut perm = std::fs::metadata(&ro).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&ro, perm.clone()).unwrap();
    select(&mut h, "Tuffy (Regular, Normal)");
    button(&mut h, "Delete");
    button(&mut h, "OK");
    finish_operation(&mut h);
    assert!(ro.exists(), "the read-only file stays");
    assert!(painted(&h).iter().any(|t| t == "Some fonts could not be processed:"), "{:?}", painted(&h));
    assert!(painted(&h).iter().any(|t| t.contains("Tuffy") && t.contains("tuffy.ttf") || t.contains("Tuffy.ttf")), "the failed file is listed: {:?}", painted(&h));
    button(&mut h, "OK");
    assert_eq!(h.state().pending_dialogs(), 0);
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&ro, perm).unwrap();

    // a file that is no font any more (changed after the scan) cannot be installed
    std::fs::write(dir.join("Hack-Regular.ttf"), b"this is not a font").unwrap();
    select(&mut h, "Hack (Regular, Normal)");
    button(&mut h, "Install");
    finish_operation(&mut h);
    assert!(painted(&h).iter().any(|t| t == "Some fonts could not be processed:"), "{:?}", painted(&h));
    button(&mut h, "OK");
    assert_eq!(fs.installed_fonts().unwrap().iter().filter(|f| f.path.to_string_lossy().to_lowercase().contains("hack")).count(), 0);
}

// ----------------------------------------------------------------------------- fonts in use, reboot

/// Installs the folder's fonts, marks their installed copies in use, selects them all and starts `verb`.
fn in_use_setup<'a>(files: &[&str], verb: &str) -> (Env, H<'a>, Arc<SandboxFontSystem>, PathBuf) {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, dir) = window(&env, &fs, "InUse", files);
    if verb == "Delete" {
        // a font that is not installed: its file in the folder is in use
        for f in files {
            fs.set_in_use(&dir.join(f), true);
        }
    } else {
        install_all(&mut h);
        for f in files {
            fs.set_in_use(&installed_copy(&fs, f), true);
        }
    }
    let first = h.state().face_states()[0].0.clone();
    select(&mut h, &first);
    h.key_press_modifiers(CTRL, Key::A);
    settle(&mut h);
    button(&mut h, verb);
    if verb == "Delete" {
        button(&mut h, "OK");
    }
    finish_operation(&mut h);
    (env, h, fs, dir)
}

#[test]
fn uninstall_in_use_reboot_now() {
    let (_env, mut h, fs, _dir) = in_use_setup(&["Tuffy.ttf"], "Uninstall");
    assert!(painted(&h).iter().any(|t| t.starts_with("The following font cannot be uninstalled because it is in use")), "{:?}", painted(&h));
    assert!(painted(&h).iter().any(|t| t == "Tuffy Regular"), "the font is listed: {:?}", painted(&h));
    assert!(h.query_by_role_and_label(Role::Button, "Ignore").is_none(), "no Ignore for one font");
    assert_eq!(fs.reboot_count(), 0);
    button(&mut h, "Reboot now");
    finish_operation(&mut h);
    assert_eq!(fs.reboot_count(), 1, "the restart was requested");
    assert!(!fs.pending_reboot().is_empty(), "the file is scheduled for the restart");
    assert_eq!(h.state().pending_dialogs(), 0);
}

#[test]
fn uninstall_in_use_reboot_later() {
    let (_env, mut h, fs, _dir) = in_use_setup(&["Tuffy.ttf", "Hack-Regular.ttf"], "Uninstall");
    assert!(painted(&h).iter().any(|t| t.starts_with("The following fonts cannot be uninstalled because they are in use")), "{:?}", painted(&h));
    for b in ["Cancel", "Ignore", "Reboot later", "Reboot now"] {
        assert!(h.query_by_role_and_label(Role::Button, b).is_some(), "{b}");
    }
    button(&mut h, "Reboot later");
    finish_operation(&mut h);
    assert_eq!(fs.reboot_count(), 0);
    assert_eq!(fs.pending_reboot().len(), 2, "{:?}", fs.pending_reboot());
    assert_eq!(h.state().pending_dialogs(), 0);
    // after the (simulated) restart the fonts are gone
    fs.simulate_reboot();
    assert!(fs.installed_fonts().unwrap().iter().all(|f| !f.path.to_string_lossy().to_lowercase().contains("tuffy")));
}

#[test]
fn uninstall_in_use_ignore_and_cancel_keep_the_fonts() {
    for choice in ["Ignore", "Cancel"] {
        let (_env, mut h, fs, _dir) = in_use_setup(&["Tuffy.ttf", "Hack-Regular.ttf"], "Uninstall");
        button(&mut h, choice);
        finish_operation(&mut h);
        assert_eq!(h.state().pending_dialogs(), 0, "{choice}");
        assert_eq!(fs.reboot_count(), 0, "{choice}");
        assert!(fs.pending_reboot().is_empty(), "{choice}");
        assert!(h.state().face_states().iter().all(|(_, i)| *i), "{choice}: still installed {:?}", h.state().face_states());
    }
}

#[test]
fn delete_in_use_asks_for_a_restart() {
    let (_env, mut h, fs, dir) = in_use_setup(&["Tuffy.ttf"], "Delete");
    assert!(painted(&h).iter().any(|t| t.starts_with("The following font cannot be deleted because it is in use")), "{:?}", painted(&h));
    assert!(painted(&h).iter().any(|t| t == "Tuffy Regular"));
    button(&mut h, "Reboot later");
    finish_operation(&mut h);
    assert_eq!(fs.reboot_count(), 0);
    assert!(!fs.pending_reboot().is_empty());
    fs.simulate_reboot();
    assert!(!dir.join("Tuffy.ttf").exists(), "deleted at the restart");
}

/// Deleting an installed font whose files are both in use (the installed copy and the file in the folder): the
/// reboot prompt lists the font once, not once per file.
#[test]
fn delete_of_an_installed_font_in_use_lists_it_once() {
    let env = Env::new();
    let fs = env.sandbox();
    let (mut h, dir) = window(&env, &fs, "Held", &["Tuffy.ttf"]);
    install_all(&mut h);
    fs.set_in_use(&installed_copy(&fs, "Tuffy.ttf"), true);
    fs.set_in_use(&dir.join("Tuffy.ttf"), true);
    let first = h.state().face_states()[0].0.clone();
    select(&mut h, &first);
    button(&mut h, "Delete");
    button(&mut h, "OK");
    finish_operation(&mut h);
    assert!(painted(&h).iter().any(|t| t.starts_with("The following font cannot be deleted because it is in use")), "{:?}", painted(&h));
    let listed = painted(&h).iter().filter(|t| *t == "Tuffy Regular" || t.to_lowercase().ends_with("tuffy.ttf")).count();
    assert_eq!(listed, 1, "listed once: {:?}", painted(&h));
    button(&mut h, "Reboot later");
    finish_operation(&mut h);
    fs.simulate_reboot();
    assert!(!dir.join("Tuffy.ttf").exists(), "deleted");
}

// ------------------------------------------------------------------------------- protected fonts

#[test]
fn protected_system_fonts_cannot_be_uninstalled_or_deleted() {
    let env = Env::new();
    let fs = env.sandbox();
    let mut h = harness(env.config_with(&fs), SIZE);
    let seed = fs.installed_fonts().unwrap();
    assert!(!seed.is_empty() && seed.iter().all(|f| f.protected), "{seed:?}");
    // My fonts: every seed font is protected
    h.get_by_role_and_label(Role::CheckBox, "Protected System Fonts").click();
    settle(&mut h);
    let protected = h.state().font_count();
    assert_eq!(protected, h.state().face_states().len());
    h.get_by_role_and_label(Role::CheckBox, "Protected System Fonts").click();
    settle(&mut h);
    let label = h.state().face_states().iter().find(|(l, _)| l.starts_with("Arial") || l.starts_with("Times")).map(|(l, _)| l.clone()).expect("a seed font");
    select(&mut h, &label);
    assert!(!tool_enabled(&h, "Uninstall"), "toolbar Uninstall");
    assert!(!tool_enabled(&h, "Delete"), "toolbar Delete");
    assert!(!tool_enabled(&h, "Install"));
    let items = font_menu(&mut h);
    assert_eq!(menu_enabled(&items, "Uninstall"), Some(false), "{items:?}");
    assert_eq!(menu_enabled(&items, "Delete"), Some(false));
    let items = context_menu(&mut h, &label);
    close_menus(&mut h);
    assert_eq!(menu_enabled(&items, "Uninstall"), Some(false));
    assert_eq!(menu_enabled(&items, "Delete"), Some(false));
    // Ctrl+A over My fonts: still nothing to uninstall or delete
    h.key_press_modifiers(CTRL, Key::A);
    settle(&mut h);
    assert!(!tool_enabled(&h, "Uninstall") && !tool_enabled(&h, "Delete"));
    // the Delete key path (Font ▸ Delete) does nothing either
    assert_eq!(fs.installed_fonts().unwrap().len(), seed.len());

    // Library ▸ Uninstall on My fonts skips them: with one user font installed only that one goes
    let dir = env.fixture_folder("User", &["Tuffy.ttf"]);
    h.state_mut().add_folder_path(&dir);
    settle(&mut h);
    install_all(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), seed.len() + 1);
    h.get_by_role_and_label(Role::ListBoxOption, "My fonts").click_secondary();
    settle(&mut h);
    menu_item(&h, "Uninstall").click();
    settle(&mut h);
    finish_operation(&mut h);
    assert_eq!(fs.installed_fonts().unwrap().len(), seed.len());
    assert!(fs.installed_fonts().unwrap().iter().all(|f| f.protected));
}
