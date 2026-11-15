//! The main window starts on a sandbox: title bar, menus, toolbar, Libraries, Filters and the font list.
mod common;

use common::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

#[test]
fn main_window_shows_the_installed_fonts() {
    let env = Env::new();
    let mut h = harness(env.config(), (1280.0, 800.0));
    let app = h.state();
    assert!(app.font_count() > 0, "the sandbox seed fonts are listed");
    assert!(app.status_text().starts_with("My fonts: "), "{}", app.status_text());
    for label in ["File", "Edit", "Font", "View", "Help"] {
        h.get_by_role_and_label(Role::Button, label);
    }
    for label in ["Minimize", "Close"] {
        h.get_by_label(label);
    }
    h.get_by_role_and_label(Role::ListBoxOption, "My fonts");
    screenshot(&mut h, "anyfont-1280x800-dark");
}
