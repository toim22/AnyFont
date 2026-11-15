//! Renders the main states of the window to `target/ui-compare/` for a side-by-side check against the
//! captures of the original (UI-SPEC). The test also checks that every state can be reached.
mod common;

use anyfont::theme::Scheme;
use common::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

fn open_menu(h: &mut egui_kittest::Harness<'_, anyfont::AnyFontApp>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    settle(h);
}

#[test]
fn main_window_states() {
    let env = Env::new();
    let mut h = harness(env.config(), (1920.0, 1080.0));
    screenshot(&mut h, "main-1920x1080");
    for m in ["File", "Edit", "Font", "View", "Help"] {
        open_menu(&mut h, m);
        screenshot(&mut h, &format!("menu-{}", m.to_lowercase()));
        h.key_press(egui::Key::Escape);
        settle(&mut h);
    }
    // Font ▸ Add to collection submenu
    open_menu(&mut h, "Font");
    h.get_by_role_and_label(Role::MenuItem, "Add to collection").hover();
    settle(&mut h);
    screenshot(&mut h, "menu-font-add-to-collection");
    h.key_press(egui::Key::Escape);
    h.key_press(egui::Key::Escape);
    settle(&mut h);

    // the font properties panel, its three tabs
    open_menu(&mut h, "View");
    h.get_by_role_and_label(Role::MenuItem, "Show font properties").click();
    settle(&mut h);
    screenshot(&mut h, "properties-glyphs");
    h.get_by_label("Font info").click();
    settle(&mut h);
    screenshot(&mut h, "properties-font-info");
    h.get_by_label("Variable font").click();
    settle(&mut h);
    screenshot(&mut h, "properties-variable");

    // Options
    h.get_by_role_and_label(Role::Button, "Options").click();
    settle(&mut h);
    screenshot(&mut h, "options");
}

#[test]
fn restored_800x600_and_other_themes() {
    let env = Env::new();
    let mut h = harness(env.config(), (800.0, 600.0));
    screenshot(&mut h, "main-800x600");
    for s in [Scheme::Light, Scheme::Medium, Scheme::Black] {
        let env = Env::new();
        let mut cfg = env.config();
        cfg.scheme = s;
        let mut h = harness(cfg, (1280.0, 800.0));
        screenshot(&mut h, &format!("main-{}", format!("{s:?}").to_lowercase()));
    }
}
