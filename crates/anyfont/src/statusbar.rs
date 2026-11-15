//! The status bar (UI-SPEC §1.4): "{library}: {count} fonts" and the preview size control (small A, slider,
//! big A), right-aligned to the list column.

use crate::app::AnyFontApp;
use crate::icons::{self, Look};
use crate::settings::{PREVIEW_MAX, PREVIEW_MIN};
use crate::theme::{self, t};
use crate::widgets;
use anyfont_core::catalog::LibraryRef;
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui, Role};

/// The status text, translated ("My fonts (filtered): 10 fonts").
pub fn status_text(app: &AnyFontApp) -> String {
    let name = match app.library {
        LibraryRef::MyFonts => crate::tr("My fonts"),
        LibraryRef::Collection(id) => app.catalog.collection(id).map(crate::commands::collection_caption).unwrap_or_default(),
        LibraryRef::Folder(_) => app.result.library_name.clone(),
    };
    let count = app.result.count.to_string();
    if app.result.filtered {
        crate::fmt("{} (filtered): {} fonts", &[&name, &count])
    } else {
        crate::fmt("{}: {} fonts", &[&name, &count])
    }
}

pub fn draw(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect) {
    let text = if !app.db_ready && app.result.rows.is_empty() { crate::tr("Loading fonts...") } else { status_text(app) };
    // captured: the caption sits 2 px and the size control 3 px below the band centre
    widgets::data_text(ui, ui.painter(), pos2(rect.left(), rect.center().y + 2.0), Align2::LEFT_CENTER, &text, theme::body(theme::UI_SIZE), t().text);
    let enabled = app.settings.core.render_name_in_font;
    let tip = "Change the font size in the font list.";
    let (changed, v) = size_control(ui, rect.translate(vec2(0.0, 3.0)), "preview-size", app.settings.core.preview_size, PREVIEW_MIN, PREVIEW_MAX, enabled, tip);
    if changed {
        app.settings.core.preview_size = v;
        app.layout = crate::fontlist::Layout::build(&app.result.rows, v);
    }
}

/// Small A / slider / big A, right-aligned in `rect` (shared with the glyph panel). Returns (changed, value).
#[allow(clippy::too_many_arguments)]
pub fn size_control(ui: &mut Ui, rect: Rect, id: &str, value: f32, min: f32, max: f32, enabled: bool, tip: &str) -> (bool, f32) {
    let cy = rect.center().y;
    let small = Rect::from_center_size(pos2(rect.right() - 151.0, cy), vec2(16.0, 16.0));
    let big = Rect::from_center_size(pos2(rect.right() - 12.0, cy), vec2(16.0, 16.0));
    let track = Rect::from_min_max(pos2(rect.right() - 122.0, cy - 6.0), pos2(rect.right() - 42.0, cy + 6.0));
    let mut v = value;
    let look = if enabled { Look::Normal } else { Look::Disabled };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let rs = ui.interact(small, Id::unique((id, "smaller")), sense);
    icons::paint(ui, "preview-smaller", small, 16.0, look);
    widgets::a11y(&rs, Role::Button, enabled, tip);
    if widgets::tooltip(rs, tip, None).clicked() {
        v = (v - 1.0).max(min);
    }
    let rb = ui.interact(big, Id::unique((id, "larger")), sense);
    icons::paint(ui, "preview-larger", big, 16.0, look);
    widgets::a11y(&rb, Role::Button, enabled, tip);
    if widgets::tooltip(rb, tip, None).clicked() {
        v = (v + 1.0).min(max);
    }
    if enabled {
        let resp = widgets::slider(ui, track, Id::unique((id, "slider")), &mut v, min..=max, tip);
        widgets::tooltip(resp, tip, None);
    } else {
        let mut dummy = v;
        let r = widgets::slider(ui, track, Id::unique((id, "slider-disabled")), &mut dummy, min..=max, tip);
        let _ = r;
    }
    let v = v.round();
    ((v - value).abs() > 0.01, v)
}
