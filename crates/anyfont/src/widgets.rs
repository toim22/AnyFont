//! Drawing helpers and the custom controls of the main window: text that is (UI strings) or is not (font
//! data) translated, captions with access keys, the thin overlay scroll bar, section headers, filter check
//! rows, the blue slider, toolbar-style icon buttons, dialog push buttons and check boxes, tooltips with a
//! bold header.

use crate::icons::{self, Look};
use crate::theme::{self, t};
use egui::text::LayoutJob;
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, Role};

/// Lays out text that must not be translated (font names, labels, paths).
pub fn data_galley(ui: &Ui, text: &str, font: FontId, color: Color32) -> std::sync::Arc<egui::Galley> {
    let job = LayoutJob::simple_singleline(text.to_string(), font, color);
    ui.painter().layout_job(job)
}

/// Paints untranslated text anchored at `pos`. Returns its rect.
pub fn data_text(ui: &Ui, painter: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32) -> Rect {
    let g = data_galley(ui, text, font, color);
    let rect = align.anchor_size(pos, g.size());
    painter.galley(rect.min, g, color);
    rect
}

/// Paints a UI string (translated) anchored at `pos`.
pub fn ui_text(painter: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32) -> Rect {
    painter.text(pos, align, text, font, color)
}

/// A caption with an `&` access key, translated, without the `&`, and the char index of the key.
pub fn caption_parts(caption: &str) -> (String, Option<usize>) {
    let c = crate::i18n::caption(caption);
    let mut out = String::new();
    let mut key = None;
    let mut it = c.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '&' {
            if it.peek() == Some(&'&') {
                out.push('&');
                it.next();
                continue;
            }
            key = Some(out.chars().count());
            continue;
        }
        out.push(ch);
    }
    (out, key)
}

/// Paints a caption, underlining its access key when `underline`.
pub fn caption_text(ui: &Ui, painter: &egui::Painter, pos: Pos2, align: Align2, caption: &str, font: FontId, color: Color32, underline: bool) -> Rect {
    let (text, key) = caption_parts(caption);
    let g = data_galley(ui, &text, font.clone(), color);
    let rect = align.anchor_size(pos, g.size());
    if underline {
        if let Some(k) = key {
            let prefix: String = text.chars().take(k).collect();
            let ch: String = text.chars().skip(k).take(1).collect();
            let x0 = data_galley(ui, &prefix, font.clone(), color).size().x;
            let w = data_galley(ui, &ch, font.clone(), color).size().x;
            let y = (rect.top() + font.size * 1.12).round() + 0.5;
            painter.hline(rect.left() + x0..=rect.left() + x0 + w, y, Stroke::new(1.0, color));
        }
    }
    painter.galley(rect.min, g, color);
    rect
}

/// The width of a caption.
pub fn caption_width(ui: &Ui, caption: &str, font: FontId) -> f32 {
    let (text, _) = caption_parts(caption);
    data_galley(ui, &text, font, Color32::WHITE).size().x
}

/// Sets the accessibility info of a custom control.
pub fn a11y(resp: &Response, kind: Role, enabled: bool, label: &str) {
    let label = crate::i18n::t(label).replace('&', "");
    resp.widget_info(|| WidgetInfo::labeled(kind, enabled, label.clone()));
}

/// Same for a check box / toggle.
pub fn a11y_checked(resp: &Response, kind: Role, enabled: bool, label: &str, checked: bool) {
    let label = crate::i18n::t(label).replace('&', "");
    resp.widget_info(|| WidgetInfo::selected(kind, enabled, checked, label.clone()));
}

/// A tooltip with a bold header line and an optional body line (UI-SPEC §3.2).
pub fn tooltip(resp: Response, header: &str, body: Option<&str>) -> Response {
    let header = header.to_string();
    let body = body.map(str::to_string);
    resp.on_hover_ui(move |ui| {
        ui.set_max_width(360.0);
        ui.label(egui::RichText::new(header.as_str()).font(theme::bold(theme::UI_SIZE)).color(t().text));
        if let Some(b) = &body {
            ui.label(egui::RichText::new(b.as_str()).color(t().text));
        }
    })
}

/// Width of the list/panel scroll bar track (UI-SPEC §1.3).
pub const SCROLLBAR_W: f32 = 17.0;

/// The overlay-style scroll bar: a dark track with a thin thumb that widens while hovered or dragged.
/// Handles the mouse wheel over `area`. Returns true when the bar is shown (content is taller).
pub fn vscroll(ui: &mut Ui, id: egui::Id, area: Rect, track: Rect, content_h: f32, offset: &mut f32) -> bool {
    let view_h = area.height();
    let max = (content_h - view_h).max(0.0);
    if ui.rect_contains_pointer(area) || ui.rect_contains_pointer(track) {
        let d = ui.input(|i| i.smooth_scroll_delta.y);
        if d != 0.0 {
            *offset -= d;
        }
    }
    *offset = offset.clamp(0.0, max);
    if max <= 0.0 {
        return false;
    }
    let p = ui.painter();
    p.rect_filled(track, 0.0, t().scroll_track);
    let thumb_h = (view_h / content_h * track.height()).max(24.0).min(track.height());
    let travel = track.height() - thumb_h;
    let y0 = track.top() + if max > 0.0 { *offset / max * travel } else { 0.0 };
    let thumb = Rect::from_min_size(pos2(track.left(), y0), vec2(track.width(), thumb_h));
    let resp = ui.interact(track, id.with("scrollbar"), Sense::click_and_drag());
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        if travel > 0.0 {
            *offset = (*offset + dy / travel * max).clamp(0.0, max);
        }
    } else if resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            let page = view_h * 0.9;
            if pp.y < thumb.top() {
                *offset = (*offset - page).max(0.0);
            } else if pp.y > thumb.bottom() {
                *offset = (*offset + page).min(max);
            }
        }
    }
    let wide = resp.hovered() || resp.dragged();
    let w = if wide { 8.0 } else { 3.0 };
    let cx = (track.center().x).round();
    let r = Rect::from_min_max(pos2(cx - w / 2.0, thumb.top() + 2.0), pos2(cx + w / 2.0, thumb.bottom() - 2.0));
    ui.painter().rect_filled(r, if wide { 4.0 } else { 1.5 }, t().scroll_thumb);
    resp.widget_info(|| WidgetInfo::labeled(Role::ScrollBar, true, crate::tr("Scroll bar")));
    true
}

/// A collapsible section header (Libraries ▸ Folders, Filters ▸ Font Status): accent text, a 1 px accent
/// line from 8 px right of the text to `line_end`, and the double chevron. Click toggles.
pub fn section_header(ui: &mut Ui, rect: Rect, id: egui::Id, caption: &str, collapsed: bool, line_end: f32) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let p = ui.painter_at(rect);
    let cy = rect.center().y;
    let tr = ui_text(&p, pos2(rect.left() + 3.0, cy), Align2::LEFT_CENTER, caption, theme::body(theme::UI_SIZE), t().accent);
    let y = cy.round() + 0.5;
    if tr.right() + 8.0 < line_end {
        p.hline(tr.right() + 8.0..=line_end, y, Stroke::new(1.0, t().accent));
    }
    let chev = Rect::from_center_size(pos2(rect.left() + 226.0, cy), vec2(16.0, 16.0));
    let chev = if rect.width() < 236.0 { Rect::from_center_size(pos2(rect.right() - 11.0, cy), vec2(16.0, 16.0)) } else { chev };
    icons::paint(ui, if collapsed { "section-expand" } else { "section-collapse" }, chev, 16.0, Look::Normal);
    a11y_checked(&resp, Role::DisclosureTriangle, true, caption, !collapsed);
    resp
}

/// A filter check row (Filters panel, Glyph Filters): 12×12 box at x+4, label at x+19; checked rows get a
/// rounded `row.checked` highlight and a ✓.
pub fn check_row(ui: &mut Ui, rect: Rect, id: egui::Id, label: &str, checked: bool, translate: bool) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let p = ui.painter_at(rect.expand(1.0));
    if checked {
        p.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 3.0, t().row_checked);
    } else if resp.hovered() {
        p.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 3.0, t().row_hot);
    }
    let cy = rect.center().y;
    let bx = Rect::from_min_size(pos2(rect.left() + 4.0, (cy - 6.0).round()), vec2(12.0, 12.0));
    check_box_shape(ui, &p, bx, checked);
    let tp = pos2(rect.left() + 19.0, cy);
    if translate {
        ui_text(&p, tp, Align2::LEFT_CENTER, label, theme::body(theme::UI_SIZE), t().text);
    } else {
        data_text(ui, &p, tp, Align2::LEFT_CENTER, label, theme::body(theme::UI_SIZE), t().text);
    }
    a11y_checked(&resp, Role::CheckBox, true, label, checked);
    resp
}

/// The 12×12 check box of the reference (1 px border, rounded 2, white ✓ when checked).
pub fn check_box_shape(_ui: &Ui, p: &egui::Painter, bx: Rect, checked: bool) {
    if !checked {
        p.rect_filled(bx, 2.0, t().checkbox_bg);
        p.rect_stroke(bx, 2.0, Stroke::new(1.0, t().checkbox_border), StrokeKind::Inside);
    }
    if checked {
        let c = if t().light { t().text } else { Color32::WHITE };
        let pts = [pos2(bx.left() + 2.5, bx.center().y + 0.5), pos2(bx.left() + 5.0, bx.bottom() - 3.0), pos2(bx.right() - 2.5, bx.top() + 3.0)];
        p.line_segment([pts[0], pts[1]], Stroke::new(1.4, c));
        p.line_segment([pts[1], pts[2]], Stroke::new(1.4, c));
    }
}

/// A dialog check box: box + caption (with access key). Toggles `value`.
pub fn checkbox(ui: &mut Ui, value: &mut bool, caption: &str) -> Response {
    let font = theme::body(theme::UI_SIZE);
    let w = caption_width(ui, caption, font.clone()) + 20.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
    if resp.clicked() {
        *value = !*value;
    }
    let p = ui.painter();
    let bx = Rect::from_min_size(pos2(rect.left(), (rect.center().y - 6.0).round()), vec2(12.0, 12.0));
    if *value {
        p.rect_filled(bx, 2.0, t().accent_checked);
        p.rect_stroke(bx, 2.0, Stroke::new(1.0, t().accent), StrokeKind::Inside);
    }
    check_box_shape(ui, p, bx, *value);
    if resp.hovered() && !*value {
        p.rect_stroke(bx, 2.0, Stroke::new(1.0, t().accent), StrokeKind::Inside);
    }
    caption_text(ui, p, pos2(rect.left() + 19.0, rect.center().y), Align2::LEFT_CENTER, caption, font, t().text, ui.input(|i| i.modifiers.alt));
    a11y_checked(&resp, Role::CheckBox, true, caption, *value);
    resp
}

/// A radio button with a caption.
pub fn radio(ui: &mut Ui, selected: bool, caption: &str) -> Response {
    let font = theme::body(theme::UI_SIZE);
    let w = caption_width(ui, caption, font.clone()) + 22.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
    let p = ui.painter();
    let c = pos2(rect.left() + 6.5, rect.center().y);
    p.circle_filled(c, 6.0, t().checkbox_bg);
    p.circle_stroke(c, 6.0, Stroke::new(1.0, if resp.hovered() { t().accent } else { t().checkbox_border }));
    if selected {
        p.circle_filled(c, 3.0, t().accent);
    }
    caption_text(ui, p, pos2(rect.left() + 19.0, rect.center().y), Align2::LEFT_CENTER, caption, font, t().text, ui.input(|i| i.modifiers.alt));
    a11y_checked(&resp, Role::RadioButton, true, caption, selected);
    resp
}

/// A push button of the dialogs: 24 px tall, ≥ 80 px wide, radius 2; the default button is blue.
pub fn push_button(ui: &mut Ui, caption: &str, default: bool, enabled: bool) -> Response {
    let font = theme::body(theme::UI_SIZE);
    let w = (caption_width(ui, caption, font.clone()) + 24.0).max(80.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), sense);
    let p = ui.painter();
    let (fill, border) = if default {
        (t().accent_checked, t().accent)
    } else if resp.hovered() && enabled {
        (t().button_hot, t().button_border)
    } else {
        (t().button_bg, t().button_border)
    };
    let fill = if resp.is_pointer_button_down_on() && enabled { t().button_pressed } else { fill };
    p.rect_filled(rect, 2.0, fill);
    p.rect_stroke(rect, 2.0, Stroke::new(1.0, if resp.hovered() && enabled { t().accent } else { border }), StrokeKind::Inside);
    let color = if !enabled {
        t().text_disabled
    } else if default && !t().light {
        Color32::WHITE
    } else {
        t().text
    };
    caption_text(ui, p, rect.center(), Align2::CENTER_CENTER, caption, font, color, ui.input(|i| i.modifiers.alt));
    a11y(&resp, Role::Button, enabled, caption);
    resp
}

/// A 24×24 icon button (toolbar, tabs, panel headers). `toggled` draws the checked background.
pub fn icon_button(ui: &mut Ui, rect: Rect, id: egui::Id, icon: &str, label: &str, enabled: bool, toggled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let resp = ui.interact(rect, id, sense);
    let p = ui.painter();
    if toggled {
        p.rect_filled(rect, 3.0, t().accent_checked);
    } else if enabled && resp.is_pointer_button_down_on() {
        p.rect_filled(rect, 3.0, t().button_pressed);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, 3.0, t().menu_bar_hot);
    }
    icons::paint(ui, icon, rect, 16.0, if enabled { Look::Normal } else { Look::Disabled });
    if toggled {
        a11y_checked(&resp, Role::Button, enabled, label, true);
    } else {
        a11y(&resp, Role::Button, enabled, label);
    }
    resp
}

/// The blue slider (status bar preview size, glyph size, Options scale, variable axes): 4 px track,
/// accent fill left of the thumb, a hollow 11 px ring thumb. Returns the response; `value` is updated.
pub fn slider(ui: &mut Ui, rect: Rect, id: egui::Id, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) -> Response {
    let resp = ui.interact(rect.expand2(vec2(6.0, 4.0)), id, Sense::click_and_drag());
    let (lo, hi) = (*range.start(), *range.end());
    let x0 = rect.left() + 5.5;
    let x1 = rect.right() - 5.5;
    if let Some(pp) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
            let k = ((pp.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            *value = lo + k * (hi - lo);
        }
    }
    if resp.hovered() {
        let d = ui.input(|i| i.smooth_scroll_delta.y);
        if d != 0.0 {
            *value = (*value + d.signum() * (hi - lo) / 50.0).clamp(lo, hi);
        }
    }
    *value = value.clamp(lo, hi);
    let k = if hi > lo { (*value - lo) / (hi - lo) } else { 0.0 };
    let cx = x0 + k * (x1 - x0);
    let cy = rect.center().y;
    let p = ui.painter();
    let track = Rect::from_min_max(pos2(rect.left(), cy - 2.0), pos2(rect.right(), cy + 2.0));
    p.rect_filled(track, 2.0, t().slider_track);
    p.rect_filled(Rect::from_min_max(track.min, pos2(cx, track.bottom())), 2.0, t().accent);
    p.circle_filled(pos2(cx, cy), 5.5, t().panel_bg);
    p.circle_stroke(pos2(cx, cy), 5.0, Stroke::new(1.6, t().accent));
    let v = *value as f64;
    let label = crate::i18n::t(label);
    resp.widget_info(|| {
        let mut info = WidgetInfo::labeled(Role::Slider, true, label.clone());
        info.value = Some(v);
        info
    });
    resp
}

/// The frame of the edit boxes and combo boxes of the main window (1 px border, accent on focus/hover).
pub fn edit_frame(ui: &Ui, rect: Rect, focused: bool, hovered: bool) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, t().edit_bg);
    let c = if focused || hovered { t().accent } else { t().edit_border };
    p.rect_stroke(rect, 2.0, Stroke::new(1.0, c), StrokeKind::Inside);
}

/// A label whose text is data (font names, paths): laid out without translation.
pub fn data_label(ui: &mut Ui, text: &str, font: FontId, color: Color32) -> Response {
    let g = data_galley(ui, text, font, color);
    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, color);
    resp
}
