//! The content column: the search row (sample text combo, Search box), the information strip and the font
//! list (UI-SPEC §1.3, §5.3): family headers, face rows with the label and the preview rendered in the face
//! itself, the technology icon, the lock and the green installed bar, selection with the mouse and the
//! keyboard, type-ahead, virtualised rows, the hover tooltip, empty states and dragging fonts onto a
//! collection.

use crate::app::{AnyFontApp, Focus};
use crate::commands::EditTarget;
use crate::icons::{self, Look};
use crate::preview::{Content, Request};
use crate::theme::{self, t};
use crate::widgets;
use anyfont_core::catalog::{LibraryRef, ListRow};
use anyfont_core::model::{FontId, FontRecord};
use egui::{pos2, vec2, Align2, Color32, Id, Key, Modifiers, Rect, Sense, Stroke, Ui, WidgetInfo, Role};
use std::collections::HashSet;

/// A selectable row.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SelKey {
    Header(String),
    Font(FontId),
}

pub fn row_key(r: &ListRow) -> Option<SelKey> {
    match r {
        ListRow::FamilyHeader { family, .. } => Some(SelKey::Header(family.clone())),
        ListRow::Font { id, .. } => Some(SelKey::Font(*id)),
        ListRow::RecentHeader { .. } => None,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub keys: HashSet<SelKey>,
    /// The current font: the last focused face; it keeps its highlight after Deselect All.
    pub current: Option<FontId>,
    /// The keyboard row.
    pub cursor: Option<usize>,
    /// Start of a Shift range.
    pub anchor: Option<usize>,
    /// Ctrl+A.
    pub all: bool,
}

#[derive(Default)]
pub struct ListState {
    pub scroll: f32,
    pub scroll_to: Option<usize>,
    type_ahead: String,
    type_time: f64,
    pub sample_open: bool,
    sample_opened_frame: u64,
    suggest_dismissed: String,
    sample_before: Option<String>,
}

/// Row geometry of the list (UI-SPEC §5.3).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub tops: Vec<f32>,
    pub heights: Vec<f32>,
    /// A 1 px separator is drawn at the top of this row (a new family block).
    pub sep_before: Vec<bool>,
    pub total: f32,
}

pub const HEADER_H: f32 = 72.0;

pub fn face_height(preview: f32) -> f32 {
    40.0 + preview
}

impl Layout {
    pub fn build(rows: &[ListRow], preview: f32) -> Layout {
        let mut l = Layout::default();
        let mut y = 0.0;
        let mut prev_recent = false;
        for (i, r) in rows.iter().enumerate() {
            let (h, sep) = match r {
                ListRow::RecentHeader { .. } => (0.0, false),
                ListRow::FamilyHeader { .. } => (HEADER_H, i > 0 && y > 0.0),
                ListRow::Font { indent, recent, .. } => {
                    let new_block = !*indent && y > 0.0 && !(*recent && prev_recent);
                    let after_recent = prev_recent && !*recent;
                    (face_height(preview), new_block || after_recent)
                }
            };
            prev_recent = matches!(r, ListRow::Font { recent: true, .. }) || (prev_recent && matches!(r, ListRow::RecentHeader { .. }));
            l.tops.push(y);
            l.heights.push(h);
            l.sep_before.push(sep);
            y += h;
        }
        l.total = y;
        l
    }

    /// The row at content offset `y`.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        if y < 0.0 || y >= self.total {
            return None;
        }
        let i = self.tops.partition_point(|t| *t <= y);
        let mut i = i.saturating_sub(1);
        while i < self.tops.len() && self.heights[i] == 0.0 {
            i += 1;
        }
        (i < self.tops.len()).then_some(i)
    }
}

/// The sample text presets of the combo (UI-SPEC §5.3).
pub const SAMPLES: [&str; 5] = ["Font Name", "abcdefghijklmnopqrstuvwxyz", "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "1234567890", "The quick brown fox jumps over the lazy dog."];

/// The text a face's preview shows.
pub fn preview_text(app: &AnyFontApp, f: &FontRecord) -> String {
    let s = app.sample.trim();
    if s.is_empty() || s == "Font Name" || s == crate::tr("Font Name") {
        // the original's previews show the family ("Arial" for Arial Bold and Arial Italic too)
        let name = f.display_family(app.language(), app.settings.core.show_latin_names);
        if name.trim().is_empty() {
            f.label(app.language(), app.settings.core.show_latin_names)
        } else {
            name
        }
    } else {
        app.sample.clone()
    }
}

/// Axis coordinates of a face's preview (the Variable tab's values, a named instance, or the defaults).
pub fn variations(app: &AnyFontApp, f: &FontRecord) -> Vec<([u8; 4], f32)> {
    if !f.is_variable || f.axes.is_empty() {
        return Vec::new();
    }
    let tag = |s: &str| {
        let b = s.as_bytes();
        let mut t = [b' '; 4];
        for (i, c) in b.iter().take(4).enumerate() {
            t[i] = *c;
        }
        t
    };
    if let Some(v) = app.props.axis_values.get(&f.id) {
        return f.axes.iter().zip(v.iter()).map(|(a, v)| (tag(&a.tag), *v)).collect();
    }
    if let Some(inst) = f.instance_index.and_then(|i| f.instances.get(i as usize)) {
        return f.axes.iter().zip(inst.coordinates.iter()).map(|(a, v)| (tag(&a.tag), *v)).collect();
    }
    f.axes.iter().map(|a| (tag(&a.tag), a.value)).collect()
}

/// Draws the search row, the information strip and the list into `rect`.
pub fn draw_content(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect) {
    let row_y = rect.top() + 6.0;
    let combo = Rect::from_min_size(pos2(rect.left(), row_y), vec2(250.0, 24.0));
    let search = Rect::from_min_size(pos2(combo.right() + 8.0, row_y), vec2(250.0, 24.0));
    let sep_y = rect.top() + 40.0;
    let track_top = rect.top() + 38.0;
    ui.painter().hline(rect.left() + 2.0..=rect.right() - 18.0, sep_y - 0.5, Stroke::new(1.0, t().line_strong));

    let mut list_top = sep_y;
    if app.refresh_needed() {
        let strip = Rect::from_min_size(pos2(rect.left(), sep_y), vec2(rect.width() - 17.0, 32.0));
        info_strip(app, ui, strip);
        list_top = strip.bottom();
    }
    let list = Rect::from_min_max(pos2(rect.left(), list_top), pos2(rect.right() - 17.0, rect.bottom() - 8.0));
    let track = Rect::from_min_max(pos2(rect.right() - 17.0, track_top), pos2(rect.right(), rect.bottom() - 8.0));
    draw_list(app, ui, list, track);

    // the search row last: its drop-downs float over the list
    sample_combo(app, ui, combo);
    search_box(app, ui, search);
}

fn info_strip(app: &mut AnyFontApp, ui: &mut Ui, r: Rect) {
    let p = ui.painter();
    p.rect_filled(r.shrink2(vec2(0.0, 2.0)), 2.0, t().row_hot);
    icons::paint(ui, "about", Rect::from_center_size(pos2(r.left() + 14.0, r.center().y), vec2(16.0, 16.0)), 16.0, Look::Normal);
    widgets::ui_text(ui.painter(), pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, "The font folders have changed.", theme::body(theme::UI_SIZE), t().text);
    let b = Rect::from_min_size(pos2(r.right() - 92.0, r.center().y - 12.0), vec2(84.0, 24.0));
    let resp = egui::UiBuilder::new().max_rect(b);
    let clicked = ui.scope_builder(resp, |ui| widgets::tooltip(widgets::push_button(ui, "Refresh", false, true), "Refresh", Some("Refresh the font list.")).clicked()).inner;
    if clicked {
        app.start_scan(false);
    }
}

/// The child ui an edit box of the list header lives in. egui paints the hint text in the visuals' weak
/// text colour whatever colour the hint asks for, so the captured placeholder grey goes there.
fn hint_ui(ui: &mut Ui, edit_rect: Rect) -> Ui {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
    child.visuals_mut().weak_text_color = Some(t().text_placeholder);
    child
}

fn sample_combo(app: &mut AnyFontApp, ui: &mut Ui, r: Rect) {
    let id = EditTarget::Sample.id();
    let ctx = ui.ctx().clone();
    let focused = ctx.memory(|m| m.has_focus(id));
    let hovered = ui.rect_contains_pointer(r);
    widgets::edit_frame(ui, r, focused || app.list.sample_open, hovered);
    let chev = Rect::from_min_max(pos2(r.right() - 20.0, r.top()), r.max);
    let edit_rect = Rect::from_min_max(pos2(r.left() + 6.0, r.top() + 1.0), pos2(chev.left(), r.bottom() - 1.0));
    let before = app.sample.clone();
    let out = egui::TextEdit::singleline(&mut app.sample)
        .id(id)
        .hint_text(egui::RichText::new("Enter sample text").font(theme::body(theme::UI_SIZE)))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(edit_rect.width())
        .align(egui::Align2::LEFT_CENTER)
        .text_color(t().text)
        .font(theme::body(theme::UI_SIZE))
        .show(&mut hint_ui(ui, edit_rect));
    widgets::a11y(&out.response, Role::TextInput, true, "Enter sample text");
    crate::commands::remember_selection(&ctx, EditTarget::Sample);
    if out.response.gained_focus() {
        app.list.sample_before = Some(before.clone());
    }
    if app.sample != before {
        app.edit_undo.insert(EditTarget::Sample, before);
    }
    if out.response.lost_focus() {
        let s = app.sample.trim().to_string();
        app.settings.core.remember_sample(&s);
    }
    if out.response.secondary_clicked() {
        let p = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(r.left_bottom());
        app.menu.open(crate::menus::Source::Edit(EditTarget::Sample), p, false, app.frame_no);
    }
    // the chevron
    let cr = ui.interact(chev, Id::unique("sample-combo-button"), Sense::click());
    icons::paint(ui, "chevron-down", Rect::from_center_size(chev.center(), vec2(10.0, 10.0)), 10.0, Look::Normal);
    widgets::a11y(&cr, Role::ComboBox, true, "Enter sample text");
    if cr.clicked() {
        app.list.sample_open = !app.list.sample_open;
        app.list.sample_opened_frame = app.frame_no;
    }
    if app.list.sample_open {
        let mut items: Vec<String> = SAMPLES.iter().map(|s| s.to_string()).collect();
        for h in &app.settings.core.sample_history {
            if !items.iter().any(|x| x == h) {
                items.push(h.clone());
            }
        }
        let h = items.len() as f32 * 22.0 + 6.0;
        let pr = Rect::from_min_size(pos2(r.left(), r.bottom() + 1.0), vec2(r.width(), h));
        let mut pick: Option<String> = None;
        egui::Area::new(Id::unique("sample-combo-popup")).order(egui::Order::Foreground).fixed_pos(pr.min).show(&ctx, |ui| {
            let (full, _) = ui.allocate_exact_size(pr.size(), Sense::hover());
            ui.painter().rect_filled(full, 0.0, t().menu_bg);
            ui.painter().rect_stroke(full, 0.0, Stroke::new(1.0, t().accent), egui::StrokeKind::Inside);
            for (i, it) in items.iter().enumerate() {
                let ir = Rect::from_min_size(pos2(full.left() + 1.0, full.top() + 3.0 + i as f32 * 22.0), vec2(full.width() - 2.0, 22.0));
                let resp = ui.interact(ir, Id::unique(("sample-item", i)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(ir.shrink2(vec2(1.0, 0.0)), 2.0, t().menu_hot);
                }
                let translate = i < SAMPLES.len();
                let tp = pos2(ir.left() + 6.0, ir.center().y);
                if translate {
                    widgets::ui_text(ui.painter(), tp, Align2::LEFT_CENTER, it, theme::body(theme::UI_SIZE), t().text);
                } else {
                    widgets::data_text(ui, ui.painter(), tp, Align2::LEFT_CENTER, it, theme::body(theme::UI_SIZE), t().text);
                }
                widgets::a11y(&resp, Role::Button, true, it);
                if resp.clicked() {
                    pick = Some(it.clone());
                }
            }
        });
        if let Some(p) = pick {
            app.edit_undo.insert(EditTarget::Sample, app.sample.clone());
            app.sample = if p == SAMPLES[0] { String::new() } else if p == SAMPLES[4] { crate::tr(&p) } else { p };
            app.list.sample_open = false;
        } else {
            let clicked_outside = ctx.input(|i| i.pointer.any_pressed()) && app.frame_no > app.list.sample_opened_frame && !ctx.input(|i| i.pointer.interact_pos().map(|p| pr.contains(p) || chev.contains(p)).unwrap_or(false));
            if clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
                app.list.sample_open = false;
            }
        }
    }
}

fn search_box(app: &mut AnyFontApp, ui: &mut Ui, r: Rect) {
    let id = EditTarget::Search.id();
    let ctx = ui.ctx().clone();
    let focused = ctx.memory(|m| m.has_focus(id));
    widgets::edit_frame(ui, r, focused, ui.rect_contains_pointer(r));
    let edit_rect = Rect::from_min_max(pos2(r.left() + 6.0, r.top() + 1.0), pos2(r.right() - 6.0, r.bottom() - 1.0));
    let before = app.search.clone();
    let out = egui::TextEdit::singleline(&mut app.search)
        .id(id)
        .hint_text(egui::RichText::new("Search").font(theme::body(theme::UI_SIZE)))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(edit_rect.width())
        .align(egui::Align2::LEFT_CENTER)
        .text_color(t().text)
        .font(theme::body(theme::UI_SIZE))
        .show(&mut hint_ui(ui, edit_rect));
    widgets::a11y(&out.response, Role::TextInput, true, "Search");
    crate::commands::remember_selection(&ctx, EditTarget::Search);
    // Esc clears the box (the edit gives up the focus on that same Esc)
    if (out.response.has_focus() || out.response.lost_focus()) && ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.search.clear();
    }
    if app.search != before {
        app.edit_undo.insert(EditTarget::Search, before);
        app.requery();
        app.list.scroll = 0.0;
        if app.sel.keys.is_empty() {
            app.select_first();
        }
    }
    if out.response.secondary_clicked() {
        let p = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(r.left_bottom());
        app.menu.open(crate::menus::Source::Edit(EditTarget::Search), p, false, app.frame_no);
    }
    // search suggestions (UI-SPEC §11 #10): matching family names while typing
    let text = app.search.trim().to_string();
    if focused && !text.is_empty() && app.list.suggest_dismissed != text {
        let lang = app.language();
        let latin = app.settings.core.show_latin_names;
        let mut names: Vec<String> = Vec::new();
        for id in &app.result.font_ids {
            if let Some(f) = app.catalog.font(*id) {
                let n = f.display_family(lang, latin);
                if !names.contains(&n) && n.to_lowercase() != text.to_lowercase() {
                    names.push(n);
                }
            }
            if names.len() >= 8 {
                break;
            }
        }
        if !names.is_empty() {
            let w = r.width().max(240.0);
            let pr = Rect::from_min_size(pos2(r.left(), r.bottom() + 1.0), vec2(w, names.len() as f32 * 22.0 + 6.0));
            let mut pick = None;
            egui::Area::new(Id::unique("search-suggestions")).order(egui::Order::Foreground).fixed_pos(pr.min).show(&ctx, |ui| {
                let (full, _) = ui.allocate_exact_size(pr.size(), Sense::hover());
                ui.painter().rect_filled(full, 0.0, t().menu_bg);
                ui.painter().rect_stroke(full, 0.0, Stroke::new(1.0, t().menu_border), egui::StrokeKind::Inside);
                for (i, n) in names.iter().enumerate() {
                    let ir = Rect::from_min_size(pos2(full.left() + 1.0, full.top() + 3.0 + i as f32 * 22.0), vec2(full.width() - 2.0, 22.0));
                    let resp = ui.interact(ir, Id::unique(("suggestion", i)), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(ir.shrink2(vec2(1.0, 0.0)), 2.0, t().menu_hot);
                    }
                    widgets::data_text(ui, ui.painter(), pos2(ir.left() + 6.0, ir.center().y), Align2::LEFT_CENTER, n, theme::body(theme::UI_SIZE), t().text);
                    resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, n.clone()));
                    if resp.clicked() {
                        pick = Some(n.clone());
                    }
                }
            });
            if let Some(n) = pick {
                app.search = n.clone();
                app.list.suggest_dismissed = n;
                app.requery();
                app.select_first();
            } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                app.list.suggest_dismissed = text;
            }
        }
    }
}

fn empty_state(app: &mut AnyFontApp, ui: &mut Ui, list: Rect) {
    let filters = !app.filters.is_empty();
    let search = !app.search.trim().is_empty();
    let (text, clear, more) = if filters {
        ("No fonts found. Remove or change filters or explore the font packs available in Get More.", true, true)
    } else if search {
        ("No fonts found. Clear the Search box, type a different keyword, or explore the font packs available in Get More.", false, true)
    } else if let LibraryRef::Collection(_) = app.library {
        ("To populate this collection, drag fonts from an added folder or another collection.", false, false)
    } else {
        return;
    };
    let c = pos2(list.center().x, list.top() + (list.height() * 0.3).min(160.0));
    let galley = ui.painter().layout(crate::tr(text), theme::italic(theme::UI_SIZE), t().text_secondary, (list.width() - 40.0).max(100.0));
    let gr = Align2::CENTER_CENTER.anchor_size(c, galley.size());
    ui.painter().galley(gr.min, galley, t().text_secondary);
    let n = clear as i32 + more as i32;
    let mut x = c.x - (n as f32 * 32.0 - 8.0) / 2.0;
    let y = gr.bottom() + 12.0;
    if clear {
        let r = Rect::from_min_size(pos2(x, y), vec2(24.0, 24.0));
        let resp = widgets::icon_button(ui, r, Id::unique("empty-clear-filters"), "filter-clear", "Clear filters", true, false);
        if widgets::tooltip(resp, "Clear filters", Some("Remove all selected font filters.")).clicked() {
            app.filters.clear();
            app.requery();
            app.select_first();
        }
        x += 32.0;
    }
    if more {
        let r = Rect::from_min_size(pos2(x, y), vec2(24.0, 24.0));
        let resp = widgets::icon_button(ui, r, Id::unique("empty-get-more"), "get-more", "Get More", true, false);
        if widgets::tooltip(resp, "Get More", None).clicked() {
            app.getmore.open();
        }
    }
}

fn draw_list(app: &mut AnyFontApp, ui: &mut Ui, list: Rect, track: Rect) {
    let ctx = ui.ctx().clone();
    let list_id = Id::unique("font-list");
    let bg = ui.interact(list, list_id, Sense::click());
    widgets::a11y(&bg, Role::Unknown, true, "Font list");
    if bg.clicked() || bg.secondary_clicked() {
        app.focus = Focus::List;
    }
    if app.result.rows.is_empty() {
        empty_state(app, ui, list);
        return;
    }
    let preview = app.settings.core.preview_size;
    if app.layout.heights.len() != app.result.rows.len() || app.layout.heights.iter().any(|h| *h > HEADER_H && (*h - face_height(preview)).abs() > 0.1) {
        app.layout = Layout::build(&app.result.rows, preview);
    }
    keyboard(app, ui, list);
    if let Some(i) = app.list.scroll_to.take() {
        if let (Some(top), Some(h)) = (app.layout.tops.get(i), app.layout.heights.get(i)) {
            if *top < app.list.scroll {
                app.list.scroll = *top;
            } else if top + h > app.list.scroll + list.height() {
                app.list.scroll = top + h - list.height();
            }
        }
    }
    let mut scroll = app.list.scroll;
    widgets::vscroll(ui, list_id, list, track, app.layout.total, &mut scroll);
    app.list.scroll = scroll;
    if app.layout.total <= list.height() {
        ui.painter().rect_filled(track, 0.0, t().panel_bg);
    }

    let painter = ui.painter_at(list);
    let ppp = ctx.pixels_per_point();
    let first = app.layout.row_at(app.list.scroll).unwrap_or(0);
    let in_font = app.settings.core.render_name_in_font;
    let modifiers = ctx.input(|i| i.modifiers);
    let mut clicked: Option<(usize, Modifiers, bool)> = None;
    let mut drag_start: Option<usize> = None;
    let mut tooltip_row: Option<(usize, egui::Response)> = None;
    let rows = app.result.rows.clone();
    for (i, row) in rows.iter().enumerate().skip(first) {
        let top = list.top() + app.layout.tops[i] - app.list.scroll;
        if top > list.bottom() {
            break;
        }
        let h = app.layout.heights[i];
        if h <= 0.0 {
            continue;
        }
        let r = Rect::from_min_size(pos2(list.left(), top), vec2(list.width(), h));
        let key = row_key(row);
        let selected = key.as_ref().map(|k| app.sel.keys.contains(k)).unwrap_or(false);
        let is_current = matches!((row, app.sel.current), (ListRow::Font { id, .. }, Some(c)) if *id == c) && app.sel.keys.is_empty();
        let resp = ui.interact(r.intersect(list), Id::unique(("font-row", i)), Sense::click_and_drag());
        if selected || is_current {
            painter.rect_filled(r, 0.0, t().list_selected);
        } else if resp.hovered() && app.drag_fonts.is_none() {
            painter.rect_filled(r, 0.0, t().row_hot);
        }
        if app.layout.sep_before[i] {
            painter.hline(list.left() + 2.0..=list.right(), r.top() + 0.5, Stroke::new(1.0, t().line_strong));
        }
        match row {
            ListRow::RecentHeader { .. } => {}
            ListRow::FamilyHeader { family, style_count, fonts } => {
                widgets::data_text(ui, &painter, pos2(r.left() + 2.0, r.center().y), Align2::LEFT_CENTER, family, theme::light(27.0), t().text);
                // the header carries the installed bar too (no lock, no technology icon)
                if fonts.iter().any(|id| app.catalog.font(*id).is_some_and(|f| f.installed)) {
                    let bar = Rect::from_min_max(pos2(r.right() - 15.0, r.top() + 4.0), pos2(r.right() - 2.0, r.bottom() - 4.0));
                    painter.rect_filled(bar, 0.0, t().installed_bar);
                }
                let label = format!("{family} ({style_count})");
                resp.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, label.clone()));
            }
            ListRow::Font { id, label, indent, .. } => {
                let Some(f) = app.catalog.font(*id) else { continue };
                let x = r.left() + if *indent { 40.0 } else { 2.0 };
                let dim = f.duplicate.is_duplicate();
                let text_c = if dim { t().text_secondary } else { t().text };
                widgets::data_text(ui, &painter, pos2(x, r.top() + 17.0), Align2::LEFT_CENTER, label, theme::body(theme::UI_SIZE), text_c);
                // preview
                let baseline = r.top() + 27.0 + preview;
                let max_w = (r.right() - x - 70.0).max(40.0);
                let text = preview_text(app, f);
                let mut drawn = false;
                if in_font {
                    if let Some(path) = f.path() {
                        let req = Request {
                            path: path.to_path_buf(),
                            face_index: f.face_index,
                            variations: variations(app, f),
                            content: Content::Text(text.clone()),
                            px: (preview * ppp).round(),
                            max_width: (max_w * ppp) as u32,
                        };
                        match app.previews.get(&req) {
                            Some(Ok(pv)) => {
                                let size = vec2(pv.size[0] as f32, pv.size[1] as f32) / ppp;
                                let min = pos2(x, (baseline - pv.baseline / ppp).round());
                                let tint = if dim { t().text_secondary } else { t().text };
                                painter.image(pv.texture.id(), Rect::from_min_size(min, size), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                                drawn = true;
                            }
                            Some(Err(())) => {}
                            None => drawn = true, // being rendered: leave the space empty for a moment
                        }
                    }
                }
                if !drawn {
                    let g = widgets::data_galley(ui, &text, theme::body(preview), text_c);
                    let asc = preview * 0.8;
                    painter.galley(pos2(x, baseline - asc - preview * 0.12), g, text_c);
                }
                // markers: technology icon, installed bar, lock
                let tech = tech_icon(f);
                icons::paint(ui, tech, Rect::from_center_size(pos2(r.right() - 33.0, r.center().y), vec2(19.0, 19.0)), 19.0, Look::Normal);
                if f.installed {
                    let bar = Rect::from_min_max(pos2(r.right() - 15.0, r.top() + 4.0), pos2(r.right() - 2.0, r.bottom() - 4.0));
                    painter.rect_filled(bar, 0.0, t().installed_bar);
                    if f.protected {
                        icons::paint(ui, "lock", Rect::from_center_size(bar.center(), vec2(10.0, 12.0)), 12.0, Look::Color(Color32::from_rgb(0x26, 0x26, 0x26)));
                    }
                }
                let name = label.clone();
                resp.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, name.clone()));
                if resp.hovered() && app.drag_fonts.is_none() {
                    tooltip_row = Some((i, resp.clone()));
                }
            }
        }
        if resp.clicked() {
            clicked = Some((i, modifiers, false));
        } else if resp.secondary_clicked() {
            clicked = Some((i, modifiers, true));
        } else if resp.double_clicked() {
            if let ListRow::Font { id, .. } = row {
                if let Some(f) = app.catalog.font(*id) {
                    let _ = app.db.touch_mru(&f.key, app.settings.core.mru_size.max(1));
                }
            }
        }
        if resp.drag_started() && matches!(row, ListRow::Font { .. } | ListRow::FamilyHeader { .. }) {
            drag_start = Some(i);
        }
    }
    if let Some((i, resp)) = tooltip_row {
        font_tooltip(app, i, resp);
    }
    if let Some((i, m, secondary)) = clicked {
        app.focus = Focus::List;
        ctx.memory_mut(|mem| mem.surrender_focus(EditTarget::Search.id()));
        ctx.memory_mut(|mem| mem.surrender_focus(EditTarget::Sample.id()));
        if secondary {
            let key = row_key(&rows[i]);
            if key.as_ref().map(|k| !app.sel.keys.contains(k)).unwrap_or(false) {
                click_row(app, i, Modifiers::NONE);
            }
            let p = ctx.input(|inp| inp.pointer.interact_pos()).unwrap_or(list.center());
            app.menu.open(crate::menus::Source::Font, p, false, app.frame_no);
        } else {
            click_row(app, i, m);
        }
    }
    if let Some(i) = drag_start {
        let key = row_key(&rows[i]);
        if key.as_ref().map(|k| !app.sel.keys.contains(k)).unwrap_or(false) {
            click_row(app, i, Modifiers::NONE);
        }
        let t = app.targets();
        if !t.is_empty() {
            app.drag_fonts = Some(t);
        }
    }
    if let Some(ids) = &app.drag_fonts {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
            let n = ids.len();
            let label = app.catalog.font(ids[0]).map(|f| if n > 1 { format!("{} (+{})", f.full_name, n - 1) } else { f.full_name.clone() }).unwrap_or_default();
            egui::Area::new(Id::unique("drag-label")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(&ctx, |ui| {
                let g = widgets::data_galley(ui, &label, theme::body(theme::UI_SIZE), t().text);
                let r = Rect::from_min_size(ui.max_rect().min, g.size() + vec2(12.0, 6.0));
                ui.painter().rect_filled(r, 2.0, t().tooltip_bg);
                ui.painter().galley(r.min + vec2(6.0, 3.0), g, t().text);
            });
        }
        if ctx.input(|i| !i.pointer.any_down()) {
            // dropped somewhere else than a collection (the Libraries panel handles collection rows)
            app.drag_fonts = None;
        }
    }
}

fn tech_icon(f: &FontRecord) -> &'static str {
    use anyfont_core::model::Technology;
    if f.is_variable {
        return "tech-variable";
    }
    match f.technology {
        Technology::TrueType => "tech-truetype",
        Technology::OpenTypeTrueType | Technology::OpenTypePostScript => "tech-opentype",
        Technology::Type1 => "tech-type1",
        Technology::Unknown => "tech-other",
    }
}

/// The font tooltip (UI-SPEC §7.2.9).
fn font_tooltip(app: &AnyFontApp, i: usize, resp: egui::Response) {
    let ListRow::Font { id, .. } = &app.result.rows[i] else { return };
    let Some(f) = app.catalog.font(*id) else { return };
    let name = f.full_name.clone();
    let installed = f.installed;
    let protected = f.protected;
    let dups: Vec<String> = if f.duplicate.is_duplicate() || app.settings.core.show_duplicates {
        app.catalog
            .fonts()
            .iter()
            .filter(|x| x.id != f.id && x.postscript_name == f.postscript_name && !f.postscript_name.is_empty() && x.duplicate.is_duplicate() != f.duplicate.is_duplicate() || (x.id != f.id && x.sha == f.sha && !f.sha.is_empty()))
            .filter_map(|x| x.path().map(|p| p.display().to_string()))
            .collect()
    } else {
        Vec::new()
    };
    resp.on_hover_ui(move |ui| {
        ui.set_max_width(420.0);
        widgets::data_label(ui, &name, theme::bold(theme::UI_SIZE), t().text);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
            if installed {
                ui.painter().rect_filled(Rect::from_center_size(r.center(), vec2(6.0, 12.0)), 0.0, t().installed_bar);
            }
            ui.label(if installed { "Installed" } else { "Not installed" });
        });
        if protected {
            ui.label("Protected System Font");
        }
        if !dups.is_empty() {
            ui.label("Duplicate(s):");
            for d in &dups {
                widgets::data_label(ui, d, theme::body(11.0), t().text);
            }
        }
    });
}

/// A click on row `i` with modifiers (UI-SPEC §5.3 Selection).
pub fn click_row(app: &mut AnyFontApp, i: usize, m: Modifiers) {
    let Some(key) = app.result.rows.get(i).and_then(row_key) else { return };
    if app.sel.all {
        app.sel.all = false;
    }
    if m.shift {
        let a = app.sel.anchor.unwrap_or(i);
        let (lo, hi) = (a.min(i), a.max(i));
        if !m.command {
            app.sel.keys.clear();
        }
        for j in lo..=hi {
            if let Some(k) = app.result.rows.get(j).and_then(row_key) {
                app.sel.keys.insert(k);
            }
        }
    } else if m.command {
        if !app.sel.keys.remove(&key) {
            app.sel.keys.insert(key.clone());
        }
        app.sel.anchor = Some(i);
    } else {
        app.sel.keys.clear();
        app.sel.keys.insert(key.clone());
        app.sel.anchor = Some(i);
    }
    app.sel.cursor = Some(i);
    if let SelKey::Font(id) = key {
        app.sel.current = Some(id);
    }
    app.props.on_selection_changed();
}

fn keyboard(app: &mut AnyFontApp, ui: &Ui, list: Rect) {
    let ctx = ui.ctx();
    if app.focus != Focus::List || ctx.memory(|m| m.focused().is_some()) || ctx.memory(|m| m.top_modal_layer().is_some()) || app.menu.is_open() || app.rename.is_some() || app.menu.bar_focus.is_some() {
        return;
    }
    let n = app.result.rows.len();
    if n == 0 {
        return;
    }
    let page = ((list.height() / face_height(app.settings.core.preview_size)).floor() as usize).max(1);
    let (mv, m, space, menu, typed) = ctx.input_mut(|i| {
        let m = i.modifiers;
        let mut mv: Option<i64> = None;
        for (k, d) in [(Key::ArrowDown, 1i64), (Key::ArrowUp, -1), (Key::PageDown, page as i64), (Key::PageUp, -(page as i64)), (Key::Home, i64::MIN), (Key::End, i64::MAX)] {
            if i.key_pressed(k) {
                mv = Some(d);
            }
        }
        let space = i.key_pressed(Key::Space) && m.command;
        let menu = (i.key_pressed(Key::F10) && m.shift) || i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Menu, pressed: true, .. }));
        let typed: String = i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect();
        (mv, m, space, menu, typed)
    });
    let selectable = |app: &AnyFontApp, j: usize| app.layout.heights.get(j).map(|h| *h > 0.0).unwrap_or(false);
    if let Some(d) = mv {
        let cur = app.sel.cursor.unwrap_or(0) as i64;
        let mut j = match d {
            i64::MIN => 0,
            i64::MAX => n as i64 - 1,
            d => (cur + d).clamp(0, n as i64 - 1),
        } as usize;
        while j < n && !selectable(app, j) {
            j += 1;
        }
        let j = j.min(n - 1);
        if m.shift {
            let a = app.sel.anchor.unwrap_or(cur as usize);
            app.sel.keys.clear();
            app.sel.all = false;
            for k in a.min(j)..=a.max(j) {
                if let Some(key) = app.result.rows.get(k).and_then(row_key) {
                    app.sel.keys.insert(key);
                }
            }
            app.sel.cursor = Some(j);
            if let Some(SelKey::Font(id)) = app.result.rows.get(j).and_then(row_key) {
                app.sel.current = Some(id);
            }
            app.props.on_selection_changed();
        } else if m.command {
            app.sel.cursor = Some(j);
        } else {
            click_row(app, j, Modifiers::NONE);
        }
        app.list.scroll_to = Some(j);
    }
    if space {
        if let Some(c) = app.sel.cursor {
            click_row(app, c, Modifiers::COMMAND);
        }
    }
    if menu {
        let y = app.sel.cursor.and_then(|c| app.layout.tops.get(c)).map(|t| list.top() + t - app.list.scroll + 20.0).unwrap_or(list.top());
        app.menu.open(crate::menus::Source::Font, pos2(list.left() + 40.0, y.clamp(list.top(), list.bottom())), true, app.frame_no);
    }
    if !typed.is_empty() && !m.command && !m.alt {
        let now = ctx.input(|i| i.time);
        if now - app.list.type_time > 1.0 {
            app.list.type_ahead.clear();
        }
        app.list.type_time = now;
        app.list.type_ahead.push_str(&typed.to_lowercase());
        let prefix = app.list.type_ahead.clone();
        let start = app.sel.cursor.map(|c| if prefix.chars().count() == 1 { c + 1 } else { c }).unwrap_or(0);
        let lang = app.language();
        let latin = app.settings.core.show_latin_names;
        let name_of = |app: &AnyFontApp, j: usize| -> String {
            match &app.result.rows[j] {
                ListRow::FamilyHeader { family, .. } => family.to_lowercase(),
                ListRow::Font { id, .. } => app.catalog.font(*id).map(|f| f.label(lang, latin).to_lowercase()).unwrap_or_default(),
                _ => String::new(),
            }
        };
        let hit = (0..n).map(|k| (start + k) % n).find(|j| selectable(app, *j) && name_of(app, *j).starts_with(&prefix));
        if let Some(j) = hit {
            click_row(app, j, Modifiers::NONE);
            app.list.scroll_to = Some(j);
        }
    }
}
