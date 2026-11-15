//! The toolbar (UI-SPEC §3): ten 24×24 icon buttons on a 32 px pitch with 1 px separators, tooltips with a
//! bold header and a body line, the toggle look of Show duplicate fonts / Show font properties.

use crate::app::AnyFontApp;
use crate::commands::Cmd;
use crate::theme::t;
use crate::widgets;
use egui::{pos2, vec2, Id, Rect, Stroke, Ui};

enum B {
    Button { x: f32, icon: &'static str, header: &'static str, body: Option<&'static str>, enabled: bool, toggled: bool, cmd: Cmd },
    Sep(f32),
}

pub fn draw(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, t().toolbar_bg);
    let e = app.eligibility();
    let dup = app.settings.core.show_duplicates;
    let props = app.settings.core.show_font_properties;
    let items = [
        B::Button { x: 4.0, icon: "add-folder", header: "Add folder", body: Some("Browse to a folder that contains fonts to add it to your Libraries."), enabled: true, toggled: false, cmd: Cmd::AddFolder },
        B::Button { x: 36.0, icon: "add-collection", header: "Add collection", body: Some("Add a collection for easy access to fonts that reside in different folders."), enabled: true, toggled: false, cmd: Cmd::AddCollection },
        B::Button { x: 68.0, icon: "get-more", header: "Get More", body: None, enabled: true, toggled: false, cmd: Cmd::GetMore },
        B::Sep(100.0),
        B::Button { x: 109.0, icon: "remove", header: "Remove", body: Some("Remove a folder or collection from your Libraries."), enabled: e.remove_folder_or_collection, toggled: false, cmd: Cmd::RemoveLibrary(app.library) },
        B::Sep(141.0),
        B::Button { x: 150.0, icon: "install", header: "Install", body: Some("Install fonts and font families."), enabled: e.menu_install, toggled: false, cmd: Cmd::Install },
        B::Button { x: 182.0, icon: "uninstall", header: "Uninstall", body: Some("Uninstall fonts and font families."), enabled: e.menu_uninstall, toggled: false, cmd: Cmd::Uninstall },
        B::Button { x: 214.0, icon: "download", header: "Download", body: Some("Download fonts to the AnyFont Content folder."), enabled: e.menu_download, toggled: false, cmd: Cmd::Download },
        B::Sep(246.0),
        B::Button { x: 255.0, icon: "delete", header: "Delete", body: Some("Delete font files from your system."), enabled: e.menu_delete, toggled: false, cmd: Cmd::Delete },
        B::Sep(287.0),
        B::Button {
            x: 296.0,
            icon: "show-duplicates",
            header: if dup { "Hide duplicate fonts" } else { "Show duplicate fonts" },
            body: Some(if dup { "Hide duplicate font versions." } else { "Show duplicate font versions." }),
            enabled: true,
            toggled: dup,
            cmd: Cmd::ToggleDuplicates,
        },
        B::Sep(328.0),
        B::Button {
            x: 337.0,
            icon: "font-properties",
            header: if props { "Hide font properties" } else { "Show font properties" },
            body: Some("Show font details or glyph sets."),
            enabled: true,
            toggled: props,
            cmd: Cmd::ToggleProperties,
        },
    ];
    let mut run = None;
    for it in items {
        match it {
            B::Sep(x) => {
                let x = rect.left() + x + 0.5;
                ui.painter().vline(x, (rect.top() + 6.0)..=(rect.top() + 27.0), Stroke::new(1.0, t().line_strong));
            }
            B::Button { x, icon, header, body, enabled, toggled, cmd } => {
                let r = Rect::from_min_size(pos2(rect.left() + x, rect.top() + 4.0), vec2(24.0, 24.0));
                if r.right() > rect.right() {
                    continue;
                }
                let resp = widgets::icon_button(ui, r, Id::unique(("toolbar", icon)), icon, header, enabled, toggled);
                let resp = widgets::tooltip(resp, header, body);
                if resp.clicked() && enabled {
                    run = Some(cmd);
                }
            }
        }
    }
    if let Some(c) = run {
        let ctx = ui.ctx().clone();
        app.run(&ctx, c);
    }
}
