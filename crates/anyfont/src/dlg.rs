//! The look shared by AnyFont's dialogs (UI-SPEC §7.1, `docs/design-tokens.md`): a modal window with its
//! own 32 px title row and a single × button, 1 px border, the dialog background, push buttons 24–25 px
//! tall and at least 80 px wide, and the blue default button (`#1D4470` fill, `#017DBC` border).
//!
//! The colours of surfaces and text come from the global egui style the shell sets for the chosen theme
//! (`ctx.global_style().visuals`); only the accent tokens, which are the same in every theme, are constants here.
//! Owned by the services UI developer; the shell may reuse every public item.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Frame, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};

/// `button_default_bg` / `selection_blue`: fill of the default button.
pub const DEFAULT_FILL: Color32 = Color32::from_rgb(0x1D, 0x44, 0x70);
/// `button_default_border` / `accent`: border of the default button, focus colour, section header text.
pub const ACCENT: Color32 = Color32::from_rgb(0x01, 0x7D, 0xBC);
/// `dialog_separator`, `info_box_border`, `menu_border` (Dark theme).
pub const SEPARATOR: Color32 = Color32::from_rgb(0x52, 0x52, 0x52);
/// `button_bg` / `combo_bg` (Dark theme).
pub const BUTTON_FILL: Color32 = Color32::from_rgb(0x3C, 0x3C, 0x3C);
/// `about_banner_border`.
pub const BANNER_BORDER: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
/// The Windows red of a hovered × (`titlebar.close.hot`).
pub const CLOSE_HOT: Color32 = Color32::from_rgb(0xC4, 0x2B, 0x1C);
/// Error text.
pub const ERROR: Color32 = Color32::from_rgb(0xE0, 0x70, 0x60);
/// Title row height.
pub const TITLE_H: f32 = 32.0;
/// Body padding (UI-SPEC §7.1: 16 px; the captured About box uses 15 px at the sides).
pub const PAD: f32 = 15.0;
/// Push button height and minimum width.
pub const BUTTON_H: f32 = 24.0;
pub const BUTTON_W: f32 = 80.0;

/// True when the global style is a light theme.
pub fn is_light(ctx: &egui::Context) -> bool {
    !ctx.global_style().visuals.dark_mode
}

/// Normal text colour of the theme (`text` = `#CACED4` in Dark).
pub fn text_color(ui: &Ui) -> Color32 {
    ui.visuals().text_color()
}

/// Dimmed text (notes, categories, disabled captions).
pub fn dim_color(ui: &Ui) -> Color32 {
    ui.visuals().weak_text_color()
}

/// Separator / border colour of the theme.
pub fn line_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { SEPARATOR } else { Color32::from_gray(0xC8) }
}

/// Fill of a normal push button.
pub fn button_fill(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { BUTTON_FILL } else { Color32::from_gray(0xE1) }
}

/// The body font of the theme.
pub fn body_font(ui: &Ui) -> FontId {
    egui::TextStyle::Body.resolve(ui.style())
}

/// A body font of another size.
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Shows a modal dialog of `size` (title row included) with the title row and its × button. `body`
/// gets the content rectangle (inside the padding). Returns its result and whether the dialog was
/// dismissed (× clicked, or Esc while it is the top dialog and no pop-up is open).
pub fn window<R>(ctx: &egui::Context, id: &str, title: &str, size: Vec2, body: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    window_ex(ctx, id, title, size, true, body)
}

/// [`window`] with a choice of the × button (`closable` false: the operation-progress look, no ×, and
/// Esc does nothing).
pub fn window_ex<R>(ctx: &egui::Context, id: &str, title: &str, size: Vec2, closable: bool, body: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    let mut dismissed = false;
    // never larger than the main window
    let screen = ctx.content_rect().size();
    let size = vec2(size.x.min(screen.x - 8.0).max(200.0), size.y.min(screen.y - 8.0).max(120.0));
    let visuals = ctx.global_style().visuals.clone();
    let bg = visuals.window_fill;
    let border = if visuals.dark_mode { Color32::from_gray(0x48) } else { Color32::from_gray(0xA0) };
    let resp = egui::Modal::new(Id::unique(("anyfont_dlg", id)))
        .backdrop_color(Color32::from_black_alpha(70))
        .frame(Frame::new().fill(bg).stroke(Stroke::new(1.0, border)).corner_radius(0.0).inner_margin(0.0))
        .show(ctx, |ui| {
            let (full, _) = ui.allocate_exact_size(size, Sense::hover());
            let title_rect = Rect::from_min_size(full.min, vec2(size.x, TITLE_H));
            let text = ui.visuals().text_color();
            ui.painter().text(pos2(title_rect.left() + 12.0, title_rect.center().y), Align2::LEFT_CENTER, title, font(12.0), text);
            if closable && close_button(ui, Rect::from_min_max(pos2(title_rect.right() - 46.0, title_rect.top()), title_rect.right_bottom()), Id::unique(("anyfont_dlg_x", id))).clicked() {
                dismissed = true;
            }
            let body_rect = Rect::from_min_max(pos2(full.left() + PAD, title_rect.bottom() + 4.0), pos2(full.right() - PAD, full.bottom() - 12.0));
            ui.scope_builder(UiBuilder::new().max_rect(body_rect).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                ui.set_clip_rect(body_rect.expand(3.0));
                body(ui)
            })
            .inner
        });
    if closable && resp.is_top_modal && !resp.any_popup_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        dismissed = true;
    }
    (resp.inner, dismissed)
}

/// The × of a title row: a glyph in the title colour, the Windows red behind it on hover.
pub fn close_button(ui: &mut Ui, r: Rect, id: Id) -> Response {
    let resp = ui.interact(r, id, Sense::click()).on_hover_text("Close");
    let hot = resp.hovered();
    if hot {
        ui.painter().rect_filled(r, 0.0, CLOSE_HOT);
    }
    let c = r.center();
    let s = 5.0;
    let color = if hot { Color32::WHITE } else { ui.visuals().text_color() };
    let st = Stroke::new(1.0, color);
    ui.painter().line_segment([pos2(c.x - s, c.y - s), pos2(c.x + s, c.y + s)], st);
    ui.painter().line_segment([pos2(c.x - s, c.y + s), pos2(c.x + s, c.y - s)], st);
    resp
}

/// The default (blue) push button. Its caption is never wrapped.
pub fn default_button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    let b = egui::Button::new(egui::RichText::new(text).color(Color32::WHITE))
        .fill(DEFAULT_FILL)
        .stroke(Stroke::new(1.0, ACCENT))
        .corner_radius(CornerRadius::same(2))
        .min_size(vec2(BUTTON_W, BUTTON_H));
    ui.add_enabled(enabled, b).clicked()
}

/// A normal push button (Cancel, System Info..., Check Now).
pub fn button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    let fill = button_fill(ui);
    let b = egui::Button::new(text).fill(fill).stroke(Stroke::new(1.0, fill)).corner_radius(CornerRadius::same(2)).min_size(vec2(BUTTON_W, BUTTON_H));
    ui.add_enabled(enabled, b).clicked()
}

/// A push button at an exact rectangle (the captured layouts of About and System Info).
pub fn button_at(ui: &mut Ui, r: Rect, text: &str, default: bool, enabled: bool) -> bool {
    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
        ui.spacing_mut().button_padding.x = 4.0;
        let fill = if default { DEFAULT_FILL } else { button_fill(ui) };
        let stroke = if default { Stroke::new(1.0, ACCENT) } else { Stroke::new(1.0, fill) };
        let rt = if default { egui::RichText::new(text).color(Color32::WHITE) } else { egui::RichText::new(text) };
        let b = egui::Button::new(rt).fill(fill).stroke(stroke).corner_radius(CornerRadius::same(2));
        ui.add_enabled_ui(enabled, |ui| ui.add_sized(r.size(), b).clicked()).inner
    })
    .inner
}

/// The "?" help button at the left of a PANOSE dialog's button row.
pub fn help_button(ui: &mut Ui) -> bool {
    let fill = button_fill(ui);
    ui.add(egui::Button::new("?").fill(fill).stroke(Stroke::new(1.0, fill)).corner_radius(CornerRadius::same(2)).min_size(vec2(BUTTON_H, BUTTON_H)))
        .on_hover_text("Product Help")
        .clicked()
}

/// A full-width 1 px separator with some room around it.
pub fn separator(ui: &mut Ui) {
    ui.add_space(6.0);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, line_color(ui)));
    ui.add_space(6.0);
}

/// Bold heading text.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(13.0));
}

/// Dimmed explanatory text, wrapped.
pub fn note(ui: &mut Ui, text: &str) {
    let c = dim_color(ui);
    ui.add(egui::Label::new(egui::RichText::new(text).color(c)).wrap());
}

/// Error text, wrapped.
pub fn error(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).color(ERROR)).wrap());
}

/// A bordered box (the Source ID box, list frames).
pub fn frame_rect(ui: &Ui, r: Rect) {
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, line_color(ui)), StrokeKind::Inside);
}

/// Splits the content rectangle into the part above the bottom button row and the row itself
/// (`row_h` tall, 10 px gap). Returns (top, bottom).
pub fn split_bottom(ui: &Ui, row_h: f32) -> (Rect, Rect) {
    let full = ui.max_rect();
    let bar = Rect::from_min_max(pos2(full.left(), full.bottom() - row_h), full.max);
    let top = Rect::from_min_max(full.min, pos2(full.right(), bar.top() - 10.0));
    (top, bar)
}

/// Runs `add` in a child ui at `r` with a top-down layout.
pub fn at<R>(ui: &mut Ui, r: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(UiBuilder::new().max_rect(r).layout(egui::Layout::top_down(egui::Align::Min)), add).inner
}

/// Runs `add` in a child ui at `r` laid out right to left (bottom button rows: the default button first).
pub fn at_right<R>(ui: &mut Ui, r: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(UiBuilder::new().max_rect(r).layout(egui::Layout::right_to_left(egui::Align::Center)), add).inner
}

/// Runs `add` in a child ui at `r` laid out left to right.
pub fn at_left<R>(ui: &mut Ui, r: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)), add).inner
}

/// True when Enter was pressed and no text edit has the focus (Enter triggers the default button).
pub fn enter_pressed(ui: &Ui) -> bool {
    let editing = ui.ctx().memory(|m| m.focused().is_some()) && ui.ctx().egui_wants_keyboard_input();
    !editing && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// Opens a URL (or a local file) in the default browser.
pub fn open_url(url: &str) -> bool {
    #[cfg(windows)]
    {
        // ShellExecute, not `cmd /C start`: a URL keeps its `&` and `%` as they are
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let r = unsafe { ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL) };
        // values above 32 mean success
        r.0 as isize > 32
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().is_ok()
    }
}

/// No console window for a child process (`CREATE_NO_WINDOW`).
pub fn hide_console(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// A short message box owned by a services dialog (System Info ▸ Save result, download errors).
#[derive(Clone, Debug, Default)]
pub struct Message {
    pub title: String,
    pub text: String,
}

impl Message {
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        Message { title: title.into(), text: text.into() }
    }

    /// Shows the message; true when it was closed (OK, Enter, Esc or ×).
    pub fn ui(&self, ctx: &egui::Context, id: &str) -> bool {
        let w = 420.0;
        let lines = (self.text.chars().count() as f32 / 60.0).ceil().max(1.0) + self.text.matches('\n').count() as f32;
        let h = (TITLE_H + 70.0 + lines * 17.0).min(400.0);
        let (ok, dismissed) = window(ctx, id, &self.title, vec2(w, h), |ui| {
            let (top, bar) = split_bottom(ui, BUTTON_H);
            at(ui, top, |ui| {
                ui.add_space(6.0);
                ui.add(egui::Label::new(self.text.as_str()).wrap());
            });
            let mut ok = at_right(ui, bar, |ui| default_button(ui, "OK", true));
            ok |= enter_pressed(ui);
            ok
        });
        ok || dismissed
    }
}
