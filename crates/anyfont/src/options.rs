//! Options (UI-SPEC §7.2.4, title-bar gear): Appearance ▸ Size (UI scale 75–225 %), Color ▸ Theme and Window
//! border, and AnyFont's Font Matching header (the three PANOSE modes and Exceptions..., lead decision 8).
//! Changes apply at once; there is no Cancel, as in the original.

use crate::app::AnyFontApp;
use crate::help::dlg;
use crate::settings::{SCALE_MAX, SCALE_MIN};
use crate::theme::{self, t, Scheme};
use crate::{icons, widgets};
use anyfont_core::PanoseMode;
use egui::{pos2, vec2, Align2, Color32, Id, Rect, Role, Sense, Stroke};

/// The Window border swatches: the theme's default first, then a small palette.
const BORDERS: [Option<[u8; 3]>; 9] = [
    None,
    Some([0x00, 0x00, 0x00]),
    Some([0x52, 0x52, 0x52]),
    Some([0x01, 0x7D, 0xBC]),
    Some([0x1D, 0x44, 0x70]),
    Some([0x66, 0xBF, 0x83]),
    Some([0xC4, 0x2B, 0x1C]),
    Some([0xF2, 0xC2, 0x1B]),
    Some([0xFF, 0xFF, 0xFF]),
];

fn swatch_color(c: Option<[u8; 3]>) -> Color32 {
    match c {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => t().window_border,
    }
}

/// A text header at `pos` (left centre).
fn header_at(ui: &egui::Ui, pos: egui::Pos2, text: &str, size: f32) {
    ui.painter().text(pos, Align2::LEFT_CENTER, text, theme::bold(size), t().text_strong);
}

/// The drop-down glyph of the original's combo boxes.
fn chevron(ui: &egui::Ui, rect: Rect, _v: &egui::style::WidgetVisuals, _open: bool) {
    icons::paint(ui, "chevron-down", Rect::from_center_size(rect.center(), vec2(10.0, 10.0)), 10.0, icons::Look::Normal);
}

pub fn show(app: &mut AnyFontApp, ctx: &egui::Context) {
    if !app.options_open {
        return;
    }
    // geometry measured on the original (464 wide, body origin at 15, 36), plus the Font Matching header
    let (ok, dismissed) = dlg::window(ctx, "options", &crate::tr("Options"), vec2(464.0, 500.0), |ui| {
        let body = ui.max_rect();
        let (x0, y0) = (body.left(), body.top());
        let (_, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
        ui.painter().hline((body.left() - 15.0)..=(body.right() + 15.0), bar.top() - 9.5, Stroke::new(1.0, t().line_divider));

        header_at(ui, pos2(x0, y0 + 14.0), "Appearance", 15.0);
        header_at(ui, pos2(x0, y0 + 60.0), "Size", theme::UI_SIZE);
        // small square, slider with 7 ticks, large square, spin box, %, Reset to Default
        let cy = y0 + 87.0;
        ui.painter().rect_stroke(Rect::from_center_size(pos2(x0 + 8.0, cy), vec2(4.0, 4.0)), 0.0, Stroke::new(1.0, t().text), egui::StrokeKind::Inside);
        let track = Rect::from_min_max(pos2(x0 + 30.0, cy - 7.0), pos2(x0 + 165.0, cy + 7.0));
        for i in 0..7 {
            let x = track.left() + 5.5 + i as f32 * (track.width() - 11.0) / 6.0;
            ui.painter().vline(x, (cy + 5.0)..=(cy + 7.0), Stroke::new(1.0, t().text));
        }
        let mut v = app.settings.core.ui_scale_percent;
        let resp = widgets::slider(ui, track, Id::unique("options-scale"), &mut v, SCALE_MIN..=SCALE_MAX, "Size");
        if resp.drag_stopped() || resp.clicked() {
            v = (v / 25.0).round() * 25.0;
        }
        ui.painter().rect_stroke(Rect::from_min_size(pos2(x0 + 181.0, cy - 7.0), vec2(14.0, 14.0)), 0.0, Stroke::new(1.0, t().text), egui::StrokeKind::Inside);
        let spin = Rect::from_min_size(pos2(x0 + 221.0, cy - 11.0), vec2(52.0, 22.0));
        let dv = ui.put(spin, egui::DragValue::new(&mut v).range(SCALE_MIN..=SCALE_MAX).fixed_decimals(1).speed(1.0));
        widgets::a11y(&dv, Role::SpinButton, true, "Size");
        ui.painter().text(pos2(x0 + 284.0, cy), Align2::LEFT_CENTER, "%", theme::body(theme::UI_SIZE), t().text);
        let reset = Rect::from_min_max(pos2(x0 + 309.0, cy - 12.0), pos2(body.right(), cy + 12.0));
        if dlg::at_right(ui, reset, |ui| widgets::push_button(ui, "&Reset to Default", false, true).clicked()) {
            v = 100.0;
        }
        app.settings.core.ui_scale_percent = v.clamp(SCALE_MIN, SCALE_MAX);
        let hint = Rect::from_min_max(pos2(x0, y0 + 98.0), pos2(body.right(), y0 + 180.0));
        dlg::at(ui, hint, |ui| {
            ui.add(egui::Label::new(egui::RichText::new("Hint: The scaling level is relative to the scaling level of the operating system. For example, scaling by 200% will make the application user interface appear twice as large as the UI of the operating system. Note that excessive scaling may cause certain workspace elements to appear cropped.").color(t().text)).wrap());
        });

        header_at(ui, pos2(x0, y0 + 202.0), "Color", theme::UI_SIZE);
        ui.painter().text(pos2(x0, y0 + 229.0), Align2::LEFT_CENTER, "Theme:", theme::body(theme::UI_SIZE), t().text);
        let theme_rect = Rect::from_min_size(pos2(x0 + 105.0, y0 + 218.0), vec2(80.0, 22.0));
        let mut scheme = app.settings.scheme;
        dlg::at_left(ui, theme_rect, |ui| {
            egui::ComboBox::from_id_salt("options-theme").width(80.0).icon(chevron).selected_text(scheme.caption()).show_ui(ui, |ui| {
                for s in Scheme::ALL {
                    ui.selectable_value(&mut scheme, s, s.caption());
                }
            });
        });
        if scheme != app.settings.scheme {
            app.settings.scheme = scheme;
        }
        ui.painter().text(pos2(x0, y0 + 261.0), Align2::LEFT_CENTER, "Window border:", theme::body(theme::UI_SIZE), t().text);
        let border_rect = Rect::from_min_size(pos2(x0 + 105.0, y0 + 250.0), vec2(80.0, 22.0));
        let cur = app.settings.core.window_border_color;
        let combo = dlg::at_left(ui, border_rect, |ui| {
            egui::ComboBox::from_id_salt("options-border")
                .width(80.0)
                .icon(chevron)
                .selected_text("      ")
                .show_ui(ui, |ui| {
                    for b in BORDERS {
                        let (r, resp) = ui.allocate_exact_size(vec2(110.0, 20.0), Sense::click());
                        if resp.hovered() || b == cur {
                            ui.painter().rect_filled(r, 2.0, t().menu_hot);
                        }
                        let sw = Rect::from_min_max(pos2(r.left() + 4.0, r.top() + 3.0), pos2(r.right() - 4.0, r.bottom() - 3.0));
                        ui.painter().rect_filled(sw, 0.0, swatch_color(b));
                        ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.0, t().edit_border), egui::StrokeKind::Inside);
                        let label = match b {
                            Some([r_, g, bl]) => format!("#{r_:02X}{g:02X}{bl:02X}"),
                            None => "default".into(),
                        };
                        resp.widget_info(|| egui::WidgetInfo::selected(Role::ListBoxOption, true, b == cur, label.clone()));
                        if resp.clicked() {
                            app.settings.core.window_border_color = b;
                        }
                    }
                })
                .response
                .rect
        });
        let sw = Rect::from_min_max(pos2(combo.left() + 5.0, combo.top() + 4.0), pos2(combo.right() - 22.0, combo.bottom() - 4.0));
        ui.painter().rect_filled(sw, 0.0, swatch_color(app.settings.core.window_border_color));

        // AnyFont: the PANOSE font matching modes (lead decision 8)
        header_at(ui, pos2(x0, y0 + 306.0), "Font Matching", theme::UI_SIZE);
        let mode = app.settings.core.panose.mode;
        let radios = Rect::from_min_size(pos2(x0, y0 + 322.0), vec2(body.width(), 24.0));
        dlg::at_left(ui, radios, |ui| {
            for (m, cap) in [(PanoseMode::Text, "&Text"), (PanoseMode::TextAndStyles, "Text and &styles"), (PanoseMode::NeverShow, "&Never show")] {
                if widgets::radio(ui, mode == m, cap).clicked() {
                    app.settings.core.panose.mode = m;
                }
                ui.add_space(12.0);
            }
        });
        let exc = Rect::from_min_size(pos2(x0, y0 + 356.0), vec2(body.width(), 24.0));
        if dlg::at_left(ui, exc, |ui| widgets::push_button(ui, "&Exceptions...", false, true).clicked()) {
            app.matching.open_exceptions();
        }
        dlg::at_right(ui, bar, |ui| widgets::push_button(ui, "OK", true, true).clicked()) || dlg::enter_pressed(ui)
    });
    if ok || dismissed {
        app.options_open = false;
        app.save_settings();
    }
}
