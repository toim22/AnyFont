//! The custom title bar (UI-SPEC §4): app icon and title, the ?, Options and Updates buttons, minimize,
//! maximize/restore and close. With the Windows frame installed (`platform::install_frame`), the caption and
//! the maximize button are answered by `WM_NCHITTEST`, so dragging, double-click, Aero Snap and the snap
//! layouts of the maximize button are the system's own; otherwise (tests, other systems) egui drives them.

use crate::app::AnyFontApp;
use crate::commands::Cmd;
use crate::icons::{self, Look};
use crate::platform::{self, HitRegions};
use crate::theme::{self, t};
use crate::widgets;
use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense, Ui, ViewportCommand, Role};

pub fn draw(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect, maximized: bool, frame: &eframe::Frame) {
    let _ = frame;
    let ctx = ui.ctx().clone();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, t().title_bar_bg);
    let active = ctx.input(|i| i.viewport().focused.unwrap_or(true));
    let win_w = if maximized { 39.0 } else { 45.0 };
    let cust_w = if maximized { 44.0 } else { 50.0 };
    let close = Rect::from_min_max(pos2(rect.right() - win_w, rect.top()), rect.max);
    let maxb = close.translate(vec2(-win_w, 0.0));
    let minb = maxb.translate(vec2(-win_w, 0.0));
    let upd = Rect::from_min_max(pos2(minb.left() - cust_w, rect.top()), pos2(minb.left(), rect.bottom()));
    let gear = upd.translate(vec2(-cust_w, 0.0));
    let help = gear.translate(vec2(-cust_w, 0.0));

    // icon and title, on whole pixels (the maximized band is 25 px tall; captured: icon rows 4-19)
    let cy = rect.center().y.floor();
    // the button glyphs sit one pixel higher than the icon when maximized (captured rows 3-18)
    let by = if maximized { cy - 1.0 } else { cy };
    // ... and, maximized, the custom glyphs one pixel left and the window glyphs one pixel right of their
    // buttons' centres (captured columns)
    let (cust_dx, win_dx) = if maximized { (-1.0, 1.0) } else { (0.0, 0.0) };
    let icon_x = if maximized { 4.0 } else { 8.0 };
    let ir = Rect::from_min_size(pos2(rect.left() + icon_x, cy - 8.0), vec2(16.0, 16.0));
    if let Some(tex) = icons::app_icon(&ctx, 16.0, !active) {
        p.image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let title_c = if active { t().title_text } else { t().title_text_inactive };
    widgets::ui_text(p, pos2(ir.right() + 8.0, cy), Align2::LEFT_CENTER, "AnyFont", theme::body(theme::UI_SIZE), title_c);

    let glyph_look = |hot: bool| if active || hot { Look::Color(t().title_text) } else { Look::TitleBar };
    let win_look = |hot: bool| if active || hot { Look::Color(t().title_text) } else { Look::Window };

    // custom buttons
    let mut run: Option<Cmd> = None;
    for (r, icon, label, cmd) in [(help, "title-help", "Product Help", Cmd::ProductHelp), (gear, "title-options", "Options", Cmd::Options), (upd, "title-updates", "Updates", Cmd::CheckUpdates)] {
        let resp = ui.interact(r, Id::unique(("title-button", icon)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, t().title_hot);
        }
        icons::paint(ui, icon, Rect::from_center_size(pos2(r.center().x + cust_dx, by), vec2(16.0, 16.0)), 16.0, glyph_look(resp.hovered()));
        if icon == "title-updates" && app.updater.update_available() {
            ui.painter().circle_filled(r.center() + vec2(6.0, -6.0), 3.0, t().accent);
        }
        widgets::a11y(&resp, Role::Button, true, label);
        if widgets::tooltip(resp, label, None).clicked() {
            run = Some(cmd);
        }
    }
    // minimize
    let resp = ui.interact(minb, Id::unique("title-minimize"), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(minb, 0.0, t().title_hot);
    }
    icons::paint(ui, "window-minimize", Rect::from_center_size(pos2(minb.center().x + win_dx, by), vec2(16.0, 16.0)), 16.0, win_look(resp.hovered()));
    widgets::a11y(&resp, Role::Button, true, "Minimize");
    if widgets::tooltip(resp, "Minimize", None).clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
    // maximize / restore
    let installed = platform::frame_installed();
    let (nc_hot, nc_pressed) = platform::max_button_state();
    let resp = ui.interact(maxb, Id::unique("title-maximize"), Sense::click());
    let hot = resp.hovered() || (installed && (nc_hot || nc_pressed));
    if hot {
        ui.painter().rect_filled(maxb, 0.0, t().title_hot);
    }
    let window_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    icons::paint(ui, if window_max { "window-restore" } else { "window-maximize" }, Rect::from_center_size(pos2(maxb.center().x + win_dx, by), vec2(16.0, 16.0)), 16.0, win_look(hot));
    let max_label = if window_max { "Restore Down" } else { "Maximize" };
    widgets::a11y(&resp, Role::Button, true, max_label);
    if widgets::tooltip(resp, max_label, None).clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!window_max));
    }
    if platform::take_max_click() {
        ctx.request_repaint();
    }
    // close
    let resp = ui.interact(close, Id::unique("title-close"), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(close, 0.0, t().close_hot);
    }
    icons::paint(ui, "window-close", Rect::from_center_size(pos2(close.center().x + win_dx, by), vec2(16.0, 16.0)), 16.0, if resp.hovered() { Look::Color(Color32::WHITE) } else { win_look(false) });
    widgets::a11y(&resp, Role::Button, true, "Close");
    if widgets::tooltip(resp, "Close", None).clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    // caption: drag, double-click, system menu
    let caption = Rect::from_min_max(rect.min, pos2(help.left(), rect.bottom()));
    let ppp = ctx.pixels_per_point();
    let px = |v: f32| (v * ppp).round() as i32;
    platform::set_hit_regions(HitRegions {
        caption: [px(caption.left()), px(caption.top()), px(caption.right()), px(caption.bottom())],
        max_button: [px(maxb.left()), px(maxb.top()), px(maxb.right()), px(maxb.bottom())],
        border: if window_max { 0 } else { px(8.0) },
    });
    if !installed {
        let resp = ui.interact(caption, Id::unique("title-caption"), Sense::click_and_drag());
        if resp.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!window_max));
        } else if resp.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if resp.secondary_clicked() {
            if let (Some(h), Some(pos)) = (app.hwnd, ctx.input(|i| i.pointer.interact_pos())) {
                let outer = ctx.input(|i| i.viewport().inner_rect).map(|r| r.min).unwrap_or_default();
                platform::show_system_menu(h, ((outer.x + pos.x) * ppp) as i32, ((outer.y + pos.y) * ppp) as i32);
            }
        }
        if !app.test_mode && !window_max {
            resize_edges(ui, ui.max_rect());
        }
    }
    if let Some(c) = run {
        app.run(&ctx, c);
    }
}

/// Resize from the window edges when the system frame is not installed.
fn resize_edges(ui: &mut Ui, full: Rect) {
    use egui::{CursorIcon, ResizeDirection as D};
    let b = 5.0;
    let zones = [
        (Rect::from_min_max(full.min, pos2(full.left() + b, full.top() + b)), D::NorthWest, CursorIcon::ResizeNwSe),
        (Rect::from_min_max(pos2(full.right() - b, full.top()), pos2(full.right(), full.top() + b)), D::NorthEast, CursorIcon::ResizeNeSw),
        (Rect::from_min_max(pos2(full.left(), full.bottom() - b), pos2(full.left() + b, full.bottom())), D::SouthWest, CursorIcon::ResizeNeSw),
        (Rect::from_min_max(pos2(full.right() - b, full.bottom() - b), full.max), D::SouthEast, CursorIcon::ResizeNwSe),
        (Rect::from_min_max(full.min, pos2(full.right(), full.top() + 3.0)), D::North, CursorIcon::ResizeVertical),
        (Rect::from_min_max(pos2(full.left(), full.bottom() - b), full.max), D::South, CursorIcon::ResizeVertical),
        (Rect::from_min_max(full.min, pos2(full.left() + b, full.bottom())), D::West, CursorIcon::ResizeHorizontal),
        (Rect::from_min_max(pos2(full.right() - b, full.top()), full.max), D::East, CursorIcon::ResizeHorizontal),
    ];
    for (i, (r, dir, cursor)) in zones.into_iter().enumerate() {
        let resp = ui.interact(r, Id::unique(("resize-edge", i)), Sense::drag());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if resp.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}
