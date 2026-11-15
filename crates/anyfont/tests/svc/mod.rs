//! Helpers of the services end-to-end tests (`e2e_services`, `e2e_updates`, `e2e_bugreport`,
//! `e2e_getmore_matching`): texts and buttons of the services dialogs found through the accessibility
//! tree, and the temporary folder of the process pointed at the test's own folder.
#![allow(dead_code)]

use egui_kittest::Harness;
use egui::accesskit::Role;
use egui_kittest::kittest::{by, NodeT, Queryable};
use std::ffi::OsString;
use std::path::Path;

/// True when a node whose label (or a label's text) contains `text` is shown.
pub fn has_text<S>(h: &Harness<'_, S>, text: &str) -> bool {
    h.query_all(by().label_contains(text).include_labels()).next().is_some()
}

/// Every label of the tree containing `text` (for failure messages).
pub fn texts_with<S>(h: &Harness<'_, S>, text: &str) -> Vec<String> {
    h.query_all(by().label_contains(text).include_labels())
        .map(|n| {
            let a = n.accesskit_node();
            a.label().or_else(|| a.value()).unwrap_or_default()
        })
        .collect()
}

/// All the labels and texts of the tree (for failure messages).
pub fn all_texts<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all(by().predicate(|n| n.label().is_some() || (n.role() == Role::Label && n.value().is_some())).include_labels())
        .map(|n| {
            let a = n.accesskit_node();
            a.label().or_else(|| a.value()).unwrap_or_default()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Waits until a text is shown.
pub fn wait_text<S>(h: &mut Harness<'_, S>, secs: u64, text: &str) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        h.run_steps(2);
        if has_text(h, text) {
            return;
        }
        if std::time::Instant::now() > end {
            panic!("timed out waiting for the text {text:?}; shown: {:?}", all_texts(h));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The buttons with exactly this label: (enabled) in tree order.
pub fn buttons<S>(h: &Harness<'_, S>, label: &str) -> Vec<bool> {
    h.query_all(by().role(Role::Button).label(label)).map(|n| !n.accesskit_node().is_disabled()).collect()
}

/// True when an enabled button with this label is shown.
pub fn button_enabled<S>(h: &Harness<'_, S>, label: &str) -> bool {
    buttons(h, label).into_iter().any(|e| e)
}

/// Clicks the last enabled button with this label (the topmost dialog's, when several dialogs show one).
pub fn click_button<S>(h: &mut Harness<'_, S>, label: &str) {
    {
        let n = h
            .query_all(by().role(Role::Button).label(label))
            .filter(|n| !n.accesskit_node().is_disabled())
            .last()
            .unwrap_or_else(|| panic!("no enabled button {label:?}; buttons: {:?}", button_labels(h)));
        n.click();
    }
    pump(h);
}

/// Clicks the last node (any role) with exactly this label.
pub fn click_label<S>(h: &mut Harness<'_, S>, label: &str) {
    {
        let n = h.query_all(by().label(label)).last().unwrap_or_else(|| panic!("no node {label:?}; shown: {:?}", all_texts(h)));
        n.click();
    }
    pump(h);
}

/// Points the temporary folder of this process (`TMP`, `TEMP`: `std::env::temp_dir`) at `dir` until
/// dropped. Hold `env_lock` for as long as it lives.
pub struct TempDirGuard {
    old: Vec<(&'static str, Option<OsString>)>,
}

impl TempDirGuard {
    pub fn set(dir: &Path) -> TempDirGuard {
        std::fs::create_dir_all(dir).unwrap();
        let old = ["TMP", "TEMP"].into_iter().map(|k| (k, std::env::var_os(k))).collect();
        for k in ["TMP", "TEMP"] {
            std::env::set_var(k, dir);
        }
        assert!(same(&std::env::temp_dir(), dir), "temp_dir follows TMP: {}", std::env::temp_dir().display());
        TempDirGuard { old }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        for (k, v) in &self.old {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

fn same(a: &Path, b: &Path) -> bool {
    let n = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_ascii_lowercase();
    n(a) == n(b)
}

/// A unique token for URLs and names (tests of one binary share the recorded requests).
pub fn unique(tag: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    format!("{tag}-{}-{nanos}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst))
}

/// The labels of every button shown.
pub fn button_labels<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all(by().role(Role::Button)).filter_map(|n| n.accesskit_node().label()).collect()
}

/// Runs a few frames (like `common::settle`, for any harness).
pub fn pump<S>(h: &mut Harness<'_, S>) {
    for _ in 0..4 {
        h.run_steps(4);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

/// Runs frames until `cond` holds (or panics after `secs` seconds).
pub fn wait_for<S>(h: &mut Harness<'_, S>, secs: u64, what: &str, mut cond: impl FnMut(&Harness<'_, S>) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        h.run_steps(2);
        if cond(h) {
            return;
        }
        if std::time::Instant::now() > end {
            panic!("timed out waiting for: {what}; shown: {:?}", all_texts(h));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Opens the combo box with this accessible name and picks `item`.
pub fn choose<S>(h: &mut Harness<'_, S>, combo: &str, item: &str) {
    {
        let n = h
            .query_all(by().role(Role::ComboBox).label(combo))
            .last()
            .unwrap_or_else(|| panic!("no combo box {combo:?}; shown: {:?}", all_texts(h)));
        n.click();
    }
    pump(h);
    {
        let n = h
            .query_all(by().label(item))
            .filter(|n| n.accesskit_node().role() != Role::ComboBox)
            .last()
            .unwrap_or_else(|| panic!("no item {item:?} in {combo:?}; shown: {:?}", all_texts(h)));
        n.click();
    }
    pump(h);
}

/// The value (shown choice) of the combo box with this accessible name.
pub fn combo_value<S>(h: &Harness<'_, S>, combo: &str) -> String {
    h.query_all(by().role(Role::ComboBox).label(combo)).last().and_then(|n| n.accesskit_node().value()).unwrap_or_default()
}
