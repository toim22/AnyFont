//! The font properties panel (UI-SPEC §5.4): the tab strip, Glyphs (glyph filters, the glyph grid drawn in
//! the font, the glyph count and the cell size control), Font info and Variable font (axis sliders that drive
//! the list preview).

use crate::app::AnyFontApp;
use crate::icons::{self, Look};
use crate::preview::{Content, Request};
use crate::settings::{GLYPH_CELL_MAX, GLYPH_CELL_MIN};
use crate::theme::{self, t};
use crate::widgets;
use anyfont_core::glyphs::{GlyphCategory, GlyphEntry, GlyphFilter, GlyphList, GlyphOrigin};
use anyfont_core::model::{FontId, FontRecord};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Stroke, Ui, WidgetInfo, Role};
use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc::{channel, Receiver};

#[derive(Default)]
pub struct PropsState {
    /// The face whose glyphs are loaded (or loading).
    glyph_font: Option<FontId>,
    glyphs: Option<GlyphList>,
    loading: Option<Receiver<(FontId, Option<GlyphList>)>>,
    pub filters: BTreeSet<GlyphFilter>,
    selected_glyph: Option<usize>,
    /// The glyph alternates pop-up (`e4939644`): index (in the shown list) of the cell it belongs to.
    alt_popup: Option<usize>,
    /// Scroll the grid so that this shown index is visible (set by a pick in the alternates pop-up).
    scroll_to_glyph: Option<usize>,
    glyph_scroll: f32,
    filter_scroll: f32,
    filter_h: f32,
    info_scroll: f32,
    /// Variable tab values per face (not saved; they drive the list preview).
    pub axis_values: HashMap<FontId, Vec<f32>>,
    collapsed_scripts: bool,
    collapsed_common: bool,
}

impl PropsState {
    pub fn on_selection_changed(&mut self) {
        self.selected_glyph = None;
        self.alt_popup = None;
    }

    /// Takes the glyph list computed on the worker thread.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.loading {
            if let Ok((id, list)) = rx.try_recv() {
                if Some(id) == self.glyph_font {
                    self.glyphs = list;
                    self.glyph_scroll = 0.0;
                    self.alt_popup = None;
                }
                self.loading = None;
                ctx.request_repaint();
            }
        }
    }

    fn load(&mut self, ctx: &egui::Context, f: &FontRecord) {
        if self.glyph_font == Some(f.id) {
            return;
        }
        self.glyph_font = Some(f.id);
        self.glyphs = None;
        self.filters.clear();
        let Some(path) = f.path().map(|p| p.to_path_buf()) else { return };
        let (tx, rx) = channel();
        let id = f.id;
        let index = f.face_index;
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new().name("anyfont-glyphs".into()).spawn(move || {
            let list = anyfont_core::glyphs::enumerate_file(&path, index);
            let _ = tx.send((id, list));
            ctx.request_repaint();
        });
        self.loading = Some(rx);
    }
}

/// The selected faces (headers count as their faces) for the panel.
fn selected(app: &AnyFontApp) -> Vec<FontId> {
    let mut v = app.selected_faces();
    for id in app.header_faces() {
        if !v.contains(&id) {
            v.push(id);
        }
    }
    v
}

/// The face the panel describes: the current font, or the only selected face.
fn panel_font(app: &AnyFontApp) -> Option<FontId> {
    let sel = selected(app);
    if sel.len() == 1 {
        return Some(sel[0]);
    }
    app.sel.current.or_else(|| sel.first().copied())
}

const TABS: [(&str, &str, &str); 3] = [
    ("tab-glyphs", "Glyphs", "Preview and filter available glyphs for a selected font."),
    ("tab-font-info", "Font info", "Show detailed font information."),
    ("tab-variable", "Variable font", "Adjust variable font properties."),
];

pub fn draw(app: &mut AnyFontApp, ui: &mut Ui, r: Rect) {
    let tab = app.settings.core.window.right_panel_tab.min(2) as usize;
    for (i, (icon, header, body)) in TABS.iter().enumerate() {
        let br = Rect::from_min_size(pos2(r.left() + 3.0 + i as f32 * 33.0, r.top() + 6.0), vec2(24.0, 24.0));
        let resp = widgets::icon_button(ui, br, Id::unique(("props-tab", i)), icon, header, true, tab == i);
        if widgets::tooltip(resp, header, Some(body)).clicked() {
            app.settings.core.window.right_panel_tab = i as u8;
        }
    }
    ui.painter().hline(r.left() + 2.0..=r.right() - 2.0, r.top() + 39.5, Stroke::new(1.0, t().line_strong));
    let strip = Rect::from_min_max(pos2(r.left(), r.bottom() - 32.0), r.max);
    ui.painter().hline(r.left()..=r.right(), strip.top() - 0.5, Stroke::new(1.0, t().line_strong));
    let body = Rect::from_min_max(pos2(r.left(), r.top() + 40.0), pos2(r.right(), strip.top() - 1.0));
    match tab {
        0 => glyphs_tab(app, ui, body, strip),
        1 => info_tab(app, ui, body),
        _ => variable_tab(app, ui, body),
    }
}

fn centered_note(ui: &Ui, r: Rect, lines: &[&str]) {
    let mut y = r.top() + (r.height() * 0.3).min(120.0);
    for l in lines {
        let g = ui.painter().layout(crate::tr(l), theme::italic(theme::UI_SIZE), t().text_secondary, r.width() - 24.0);
        let gr = Align2::CENTER_TOP.anchor_size(pos2(r.center().x, y), g.size());
        ui.painter().galley(gr.min, g.clone(), t().text_secondary);
        y = gr.bottom() + 8.0;
    }
}

fn glyphs_tab(app: &mut AnyFontApp, ui: &mut Ui, body: Rect, strip: Rect) {
    let ctx = ui.ctx().clone();
    let sel = selected(app);
    let font = panel_font(app).and_then(|id| app.catalog.font(id)).cloned();
    if let Some(f) = &font {
        app.props.load(&ctx, f);
    }
    // filters part
    if app.props.filter_h <= 0.0 {
        app.props.filter_h = 190.0;
    }
    let fh = app.props.filter_h.clamp(60.0, (body.height() - 120.0).max(60.0));
    let ftop = body.top();
    let p = ui.painter();
    widgets::ui_text(p, pos2(body.left() + 4.0, ftop + 20.0), Align2::LEFT_CENTER, "Glyph Filters", theme::bold(theme::UI_SIZE), t().text_strong);
    let active = !app.props.filters.is_empty();
    let fr = Rect::from_center_size(pos2(body.right() - 15.0, ftop + 20.0), vec2(20.0, 20.0));
    let resp = ui.interact(fr, Id::unique("clear-glyph-filters"), if active { Sense::click() } else { Sense::hover() });
    icons::paint(ui, "filter-clear", fr, 16.0, if active { Look::FilterActive } else { Look::Disabled });
    widgets::a11y(&resp, Role::Button, active, "Remove all selected glyph filters.");
    if widgets::tooltip(resp, "Remove all selected glyph filters.", None).clicked() {
        app.props.filters.clear();
    }
    let farea = Rect::from_min_max(pos2(body.left(), ftop + 36.0), pos2(body.right() - 4.0, ftop + fh));
    let groups = app.props.glyphs.as_ref().map(|g| g.available_filters()).unwrap_or_default();
    let mut common: Vec<GlyphCategory> = groups.iter().find(|(s, _)| s == "Common").map(|(_, c)| c.clone()).unwrap_or_default();
    // alphabetical, as captured (Currency, Mathematical Symbols, Numbers, Punctuation & Separators, Symbols)
    common.sort_by_cached_key(|c| crate::tr(c.caption()).to_lowercase());
    let scripts: Vec<(String, Vec<GlyphCategory>)> = groups.iter().filter(|(s, _)| s != "Common").cloned().collect();
    let mut content_h = 0.0;
    if !common.is_empty() {
        content_h += 23.0 + if app.props.collapsed_common { 0.0 } else { common.len() as f32 * 26.0 };
    }
    if !scripts.is_empty() {
        content_h += 23.0 + if app.props.collapsed_scripts { 0.0 } else { scripts.len() as f32 * 26.0 };
    }
    let track = Rect::from_min_max(pos2(body.right() - 10.0, farea.top()), pos2(body.right() - 3.0, farea.bottom()));
    let mut fs = app.props.filter_scroll;
    widgets::vscroll(ui, Id::unique("glyph-filter-scroll"), farea, track, content_h, &mut fs);
    app.props.filter_scroll = fs;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(farea));
    child.set_clip_rect(farea);
    let mut y = farea.top() - app.props.filter_scroll;
    let w = (body.width() - 12.0).max(80.0);
    let line_end = body.left() + w - 26.0;
    let mut toggle: Vec<GlyphFilter> = Vec::new();
    let mut set_all: Option<(Vec<GlyphFilter>, bool)> = None;
    if !common.is_empty() {
        let hr = Rect::from_min_size(pos2(body.left(), y + 3.0), vec2(w, 20.0));
        if widgets::section_header(&mut child, hr, Id::unique("glyph-common"), "Common", app.props.collapsed_common, line_end).clicked() {
            app.props.collapsed_common = !app.props.collapsed_common;
        }
        y += 23.0;
        if !app.props.collapsed_common {
            for c in &common {
                let rr = Rect::from_min_size(pos2(body.left(), y), vec2(w, 26.0));
                y += 26.0;
                let gf = GlyphFilter { script: "Common".into(), category: *c };
                let checked = app.props.filters.contains(&gf);
                if widgets::check_row(&mut child, rr, Id::unique(("glyph-filter", "Common", c.caption())), c.caption(), checked, true).clicked() {
                    toggle.push(gf);
                }
            }
        }
    }
    if !scripts.is_empty() {
        let hr = Rect::from_min_size(pos2(body.left(), y + 3.0), vec2(w, 20.0));
        if widgets::section_header(&mut child, hr, Id::unique("glyph-scripts"), "Scripts", app.props.collapsed_scripts, line_end).clicked() {
            app.props.collapsed_scripts = !app.props.collapsed_scripts;
        }
        y += 23.0;
        if !app.props.collapsed_scripts {
            for (s, cats) in &scripts {
                let rr = Rect::from_min_size(pos2(body.left(), y), vec2(w, 26.0));
                y += 26.0;
                let fset: Vec<GlyphFilter> = cats.iter().map(|c| GlyphFilter { script: s.clone(), category: *c }).collect();
                let checked = fset.iter().all(|f| app.props.filters.contains(f));
                if widgets::check_row(&mut child, rr, Id::unique(("glyph-script", s.as_str())), s, checked, false).clicked() {
                    set_all = Some((fset, !checked));
                }
            }
        }
    }
    for f in toggle {
        if !app.props.filters.remove(&f) {
            app.props.filters.insert(f);
        }
    }
    if let Some((fs, on)) = set_all {
        for f in fs {
            if on {
                app.props.filters.insert(f);
            } else {
                app.props.filters.remove(&f);
            }
        }
    }
    // splitter
    let sy = ftop + fh + 0.5;
    ui.painter().hline(body.left()..=body.right(), sy, Stroke::new(1.0, t().line_divider));
    let sr = ui.interact(Rect::from_min_max(pos2(body.left(), sy - 3.0), pos2(body.right(), sy + 3.0)), Id::unique("glyph-splitter"), Sense::drag());
    if sr.hovered() || sr.dragged() {
        ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if sr.dragged() {
        app.props.filter_h = (fh + sr.drag_delta().y).clamp(60.0, body.height() - 120.0);
    }

    // preview part
    let ptop = sy + 1.0;
    widgets::ui_text(ui.painter(), pos2(body.left() + 4.0, ptop + 15.0), Align2::LEFT_CENTER, "Glyph Preview", theme::bold(theme::UI_SIZE), t().text_strong);
    let grid = Rect::from_min_max(pos2(body.left() + 2.0, ptop + 32.0), pos2(body.right() - 12.0, body.bottom()));
    let gtrack = Rect::from_min_max(pos2(body.right() - 10.0, grid.top()), pos2(body.right() - 3.0, grid.bottom()));
    let cell = app.settings.core.glyph_cell_size;
    let mut count_text = String::new();
    if sel.len() > 1 {
        centered_note(ui, grid, &["Multiple fonts are selected. To view a glyph set, choose only one font."]);
    } else if let Some(f) = &font {
        if f.needs_download() || f.paths.is_empty() {
            centered_note(ui, grid, &["Install or download the font to preview glyphs."]);
        } else if let Some(list) = app.props.glyphs.clone() {
            let shown: Vec<_> = list.filtered(&app.props.filters).cloned().collect();
            count_text = crate::fmt("{} glyphs", &[&shown.len().to_string()]);
            if shown.is_empty() {
                centered_note(ui, grid, &["No matching glyphs"]);
            } else {
                let cols = ((grid.width() / cell).floor() as usize).max(1);
                let rows = shown.len().div_ceil(cols);
                if let Some(i) = app.props.scroll_to_glyph.take() {
                    let top = (i / cols) as f32 * cell;
                    if top < app.props.glyph_scroll {
                        app.props.glyph_scroll = top;
                    } else if top + cell > app.props.glyph_scroll + grid.height() {
                        app.props.glyph_scroll = top + cell - grid.height();
                    }
                }
                let mut gs = app.props.glyph_scroll;
                widgets::vscroll(ui, Id::unique("glyph-grid-scroll"), grid, gtrack, rows as f32 * cell, &mut gs);
                app.props.glyph_scroll = gs;
                let ppp = ctx.pixels_per_point();
                let painter = ui.painter_at(grid);
                let first_row = (app.props.glyph_scroll / cell).floor() as usize;
                let path = f.path().map(|p| p.to_path_buf()).unwrap_or_default();
                let variations = crate::fontlist::variations(app, f);
                let glyph_req = |gid: u16| Request {
                    path: path.clone(),
                    face_index: f.face_index,
                    variations: variations.clone(),
                    content: Content::Glyph(gid),
                    px: (cell * 0.72 * ppp).round(),
                    max_width: (cell * ppp) as u32,
                };
                let mut clicked: Option<(usize, bool)> = None;
                let mut open_alternates: Option<(usize, Rect)> = None;
                for row in first_row..rows {
                    let y = grid.top() + row as f32 * cell - app.props.glyph_scroll;
                    if y > grid.bottom() {
                        break;
                    }
                    for col in 0..cols {
                        let i = row * cols + col;
                        let Some(g) = shown.get(i) else { break };
                        let cr = Rect::from_min_size(pos2(grid.left() + col as f32 * cell, y), vec2(cell, cell));
                        let resp = ui.interact(cr.intersect(grid), Id::unique(("glyph", i)), Sense::click());
                        // glyphs with alternates: a small triangle in the bottom-right corner opens the pop-up
                        let tri = (g.origin == GlyphOrigin::Encoded && !g.alternates.is_empty()).then(|| {
                            let tr = Rect::from_min_max(cr.max - vec2(11.0, 11.0), cr.max - vec2(1.0, 1.0));
                            let tresp = ui.interact(tr.intersect(grid), Id::unique(("glyph-alternates", i)), Sense::click());
                            (tr, tresp)
                        });
                        let hovered = resp.hovered() || tri.as_ref().is_some_and(|(_, t)| t.hovered());
                        let selected = app.props.selected_glyph == Some(i);
                        if hovered {
                            painter.rect_filled(cr, 0.0, t().row_hot);
                        }
                        if selected {
                            painter.hline(cr.x_range(), cr.top() + 0.5, Stroke::new(1.0, t().accent));
                            painter.hline(cr.x_range(), cr.bottom() - 0.5, Stroke::new(1.0, t().accent));
                        }
                        if g.origin == GlyphOrigin::Encoded && g.codepoint.is_some_and(char::is_whitespace) {
                            // the space (and U+00A0 and the other blanks): a green marker
                            space_marker(&painter, cr, cell);
                        } else if let Some(Ok(pv)) = app.previews.get(&glyph_req(g.glyph_id)) {
                            let size = vec2(pv.size[0] as f32, pv.size[1] as f32) / ppp;
                            painter.image(pv.texture.id(), Rect::from_center_size(cr.center(), size), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), t().text);
                        }
                        if let Some((tr, tresp)) = &tri {
                            let open = app.props.alt_popup == Some(i);
                            if hovered || selected || open {
                                let color = if tresp.hovered() || open { t().accent } else { t().text_secondary };
                                let pts = vec![pos2(tr.right(), tr.top() + 3.0), pos2(tr.right(), tr.bottom()), pos2(tr.left() + 3.0, tr.bottom())];
                                painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
                            }
                            tresp.widget_info(|| WidgetInfo::labeled(Role::Button, true, crate::tr("Show glyph alternates.")));
                            if widgets::tooltip(tresp.clone(), "Show glyph alternates.", None).clicked() {
                                open_alternates = Some((i, cr));
                            }
                        }
                        let name = g.caption();
                        resp.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, name.clone()));
                        let tip = g.caption();
                        let resp = resp.on_hover_ui(move |ui| {
                            widgets::data_label(ui, &tip, theme::body(theme::UI_SIZE), t().text);
                        });
                        if resp.double_clicked() {
                            clicked = Some((i, true));
                        } else if resp.clicked() {
                            clicked = Some((i, false));
                        }
                    }
                }
                if let Some((i, copy)) = clicked {
                    app.props.selected_glyph = Some(i);
                    app.props.alt_popup = None;
                    if copy {
                        copy_glyph(&ctx, &shown[i]);
                    }
                }
                if let Some((i, cr)) = open_alternates {
                    app.props.selected_glyph = Some(i);
                    app.props.alt_popup = if app.props.alt_popup == Some(i) { None } else { Some(i) };
                    ctx.data_mut(|d| d.insert_temp(Id::unique("glyph-alternates-anchor"), cr));
                }
                if let Some(base) = app.props.alt_popup {
                    let anchor = ctx.data(|d| d.get_temp::<Rect>(Id::unique("glyph-alternates-anchor")));
                    match (shown.get(base), anchor) {
                        (Some(g), Some(anchor)) if !g.alternates.is_empty() => {
                            if let Some(pick) = alternates_popup(app, &ctx, &shown, base, anchor, cell, &glyph_req) {
                                app.props.selected_glyph = Some(pick);
                                app.props.scroll_to_glyph = Some(pick);
                                app.props.alt_popup = None;
                            }
                        }
                        _ => app.props.alt_popup = None,
                    }
                }
                if let Some(i) = app.props.selected_glyph {
                    let ctrl_c = ctx.input(|inp| inp.events.iter().any(|e| matches!(e, egui::Event::Copy))) && ctx.memory(|m| m.focused().is_none());
                    if ctrl_c {
                        if let Some(g) = shown.get(i) {
                            copy_glyph(&ctx, g);
                        }
                    }
                }
            }
        }
    }
    // the panel's status strip
    if !count_text.is_empty() {
        widgets::data_text(ui, ui.painter(), pos2(strip.left() + 3.0, strip.center().y), Align2::LEFT_CENTER, &count_text, theme::body(theme::UI_SIZE), t().text);
    }
    let tip = "Change the glyph size in the Glyph Preview area.";
    let (changed, v) = crate::statusbar::size_control(ui, strip, "glyph-size", cell, GLYPH_CELL_MIN, GLYPH_CELL_MAX, true, tip);
    if changed {
        app.settings.core.glyph_cell_size = v;
    }
}

/// The green blank marker of the space cell (and of U+00A0 and the other blanks).
fn space_marker(painter: &egui::Painter, cr: Rect, cell: f32) {
    let c = cr.center();
    let w2 = cell * 0.22;
    let st = Stroke::new(1.0, t().glyph_baseline);
    painter.line_segment([pos2(c.x - w2, c.y), pos2(c.x - w2, c.y + 4.0)], st);
    painter.line_segment([pos2(c.x - w2, c.y + 4.0), pos2(c.x + w2, c.y + 4.0)], st);
    painter.line_segment([pos2(c.x + w2, c.y + 4.0), pos2(c.x + w2, c.y)], st);
}

/// The glyph alternates pop-up (`e4939644`: a pop-up list box with a drop shadow, `AltGlyphsList` at the
/// grid's cell size). It shows the glyph and its alternates in a row under the cell; a click picks one
/// (selects it in the grid), Esc or a click outside closes it. Returns the shown index of the pick.
fn alternates_popup(
    app: &mut AnyFontApp,
    ctx: &egui::Context,
    shown: &[GlyphEntry],
    base: usize,
    anchor: Rect,
    cell: f32,
    glyph_req: &dyn Fn(u16) -> Request,
) -> Option<usize> {
    let g = &shown[base];
    // the cells: the glyph itself, then its alternates, as they follow it in the grid
    let mut items: Vec<(usize, u16)> = vec![(base, g.glyph_id)];
    let mut k = base + 1;
    for &a in &g.alternates {
        while k < shown.len() && shown[k].origin == GlyphOrigin::Alternate && shown[k].glyph_id != a {
            k += 1;
        }
        if k < shown.len() && shown[k].glyph_id == a {
            items.push((k, a));
            k += 1;
        }
    }
    let screen = ctx.content_rect();
    let per_row = (((screen.width() - 16.0) / cell).floor() as usize).clamp(1, 8);
    let cols = items.len().min(per_row);
    let rows = items.len().div_ceil(cols);
    let size = vec2(cols as f32 * cell + 4.0, rows as f32 * cell + 4.0);
    let mut pos = pos2(anchor.left(), anchor.bottom() + 1.0);
    pos.x = pos.x.min(screen.right() - size.x - 2.0).max(screen.left() + 2.0);
    if pos.y + size.y > screen.bottom() - 2.0 {
        pos.y = anchor.top() - size.y - 1.0;
    }
    let ppp = ctx.pixels_per_point();
    let mut pick = None;
    let popup = Rect::from_min_size(pos, size);
    egui::Area::new(Id::unique("glyph-alternates-popup")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        let (full, _) = ui.allocate_exact_size(size, Sense::hover());
        let p = ui.painter();
        p.add(ui.visuals().popup_shadow.as_shape(full, 0.0));
        p.rect_filled(full, 0.0, t().menu_bg);
        p.rect_stroke(full, 0.0, Stroke::new(1.0, t().menu_border), egui::StrokeKind::Inside);
        for (n, (idx, gid)) in items.iter().enumerate() {
            let cr = Rect::from_min_size(pos2(full.left() + 2.0 + (n % cols) as f32 * cell, full.top() + 2.0 + (n / cols) as f32 * cell), vec2(cell, cell));
            let resp = ui.interact(cr, Id::unique(("glyph-alternate", n)), Sense::click());
            let selected = app.props.selected_glyph == Some(*idx);
            if resp.hovered() {
                p.rect_filled(cr, 0.0, t().menu_hot);
            }
            if selected {
                p.hline(cr.x_range(), cr.top() + 0.5, Stroke::new(1.0, t().accent));
                p.hline(cr.x_range(), cr.bottom() - 0.5, Stroke::new(1.0, t().accent));
            }
            if let Some(Ok(pv)) = app.previews.get(&glyph_req(*gid)) {
                let s = vec2(pv.size[0] as f32, pv.size[1] as f32) / ppp;
                p.image(pv.texture.id(), Rect::from_center_size(cr.center(), s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), t().text);
            }
            let name = shown[*idx].caption();
            resp.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, name.clone()));
            if resp.clicked() {
                pick = Some(*idx);
            }
        }
    });
    if pick.is_none() {
        let outside_press = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !popup.contains(p) && !anchor.contains(p))
        });
        if outside_press || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.props.alt_popup = None;
        }
    }
    pick
}

fn copy_glyph(ctx: &egui::Context, g: &anyfont_core::glyphs::GlyphEntry) {
    if let Some(c) = g.codepoint {
        ctx.copy_text(c.to_string());
    }
}

/// Values of these captions are UI strings (translated); the others are font data.
const TRANSLATED_VALUES: [&str; 5] = ["Italic", "Font technology", "Weight", "Width", "Embedding"];

fn info_tab(app: &mut AnyFontApp, ui: &mut Ui, body: Rect) {
    let Some(f) = panel_font(app).and_then(|id| app.catalog.font(id)).cloned() else { return };
    let rows = anyfont_core::catalog::font_info(&f, app.catalog.collections());
    let label_right = body.left() + 108.0;
    let value_left = body.left() + 118.0;
    let value_w = (body.right() - 12.0 - value_left).max(40.0);
    let area = body.shrink2(vec2(0.0, 4.0));
    let painter = ui.painter_at(area);
    let mut y = area.top() + 10.0 - app.props.info_scroll;
    let start_y = y;
    for (cap, value) in &rows {
        let label = crate::tr(&format!("{cap}:"));
        widgets::data_text(ui, &painter, pos2(label_right, y + 8.0), Align2::RIGHT_CENTER, &label, theme::body(theme::UI_SIZE), t().text);
        let value = match *cap {
            "File Path" => value.to_lowercase(),
            "Collection" => value.replace("Favorites", &crate::tr("Favorites")),
            _ if TRANSLATED_VALUES.contains(cap) => value.lines().map(crate::tr).collect::<Vec<_>>().join("\n"),
            _ => value.clone(),
        };
        let (h, full) = if *cap == "File Path" {
            let shown = end_ellipsis(ui, &value, value_w);
            let g = widgets::data_galley(ui, &shown, theme::body(theme::UI_SIZE), t().text);
            let h = g.size().y;
            painter.galley(pos2(value_left, y), g, t().text);
            (h, Some(value.clone()))
        } else {
            let job = egui::text::LayoutJob::simple(value.clone(), theme::body(theme::UI_SIZE), t().text, value_w);
            let g = ui.painter().layout_job(job);
            let h = g.size().y;
            painter.galley(pos2(value_left, y), g, t().text);
            (h, None)
        };
        if let Some(full) = full {
            let r = Rect::from_min_size(pos2(value_left, y), vec2(value_w, h.max(16.0)));
            let resp = ui.interact(r.intersect(area), Id::unique("info-file-path"), Sense::hover());
            resp.on_hover_ui(move |ui| {
                widgets::data_label(ui, &full, theme::body(theme::UI_SIZE), t().text);
            });
        }
        y += (h + 18.0).max(34.0);
    }
    // the vendor link (UI-SPEC §11 #8)
    let url = anyfont_core::vendors::vendor_url(&f).unwrap_or_default();
    if !url.is_empty() {
        let g = widgets::data_galley(ui, &url, theme::body(theme::UI_SIZE), t().hyperlink);
        let r = Rect::from_min_size(pos2(value_left, y), g.size());
        let resp = ui.interact(r.intersect(area), Id::unique("vendor-url"), Sense::click());
        painter.galley(r.min, g, t().hyperlink);
        if resp.hovered() {
            painter.hline(r.x_range(), r.bottom(), Stroke::new(1.0, t().hyperlink));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let u = url.clone();
        resp.widget_info(|| WidgetInfo::labeled(Role::Link, true, u.clone()));
        if resp.clicked() && !app.test_mode {
            crate::help::dlg::open_url(&url);
        }
        y += 34.0;
    }
    let content = y - start_y + 10.0;
    let track = Rect::from_min_max(pos2(body.right() - 10.0, area.top()), pos2(body.right() - 3.0, area.bottom()));
    let mut s = app.props.info_scroll;
    widgets::vscroll(ui, Id::unique("font-info-scroll"), area, track, content, &mut s);
    app.props.info_scroll = s;
}

fn end_ellipsis(ui: &Ui, text: &str, width: f32) -> String {
    let font = theme::body(theme::UI_SIZE);
    if widgets::data_galley(ui, text, font.clone(), t().text).size().x <= width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 1 {
        n -= 1;
        let s: String = chars[..n].iter().collect::<String>() + "...";
        if widgets::data_galley(ui, &s, font.clone(), t().text).size().x <= width {
            return s;
        }
    }
    "...".into()
}

fn variable_tab(app: &mut AnyFontApp, ui: &mut Ui, body: Rect) {
    let sel = selected(app);
    let fonts: Vec<&FontRecord> = sel.iter().filter_map(|id| app.catalog.font(*id)).collect();
    let axes = anyfont_core::catalog::variable_axes(&fonts).map(|a| a.to_vec());
    let Some(axes) = axes else {
        centered_note(ui, body, &[anyfont_core::catalog::NO_VARIABLE_FONT_TEXT, "Choose one variable font to see and adjust its properties."]);
        return;
    };
    let f = fonts[0].clone();
    let mut values = app.props.axis_values.get(&f.id).cloned().unwrap_or_else(|| {
        if let Some(inst) = f.instance_index.and_then(|i| f.instances.get(i as usize)) {
            inst.coordinates.clone()
        } else {
            axes.iter().map(|a| a.value).collect()
        }
    });
    values.resize(axes.len(), 0.0);
    let mut changed = false;
    let mut y = body.top() + 14.0;
    for (i, a) in axes.iter().enumerate() {
        if a.hidden {
            continue;
        }
        let name = if a.name.is_empty() { a.tag.clone() } else { a.name.clone() };
        widgets::data_text(ui, ui.painter(), pos2(body.left() + 8.0, y + 8.0), Align2::LEFT_CENTER, &name, theme::body(theme::UI_SIZE), t().text);
        y += 22.0;
        let track = Rect::from_min_max(pos2(body.left() + 10.0, y), pos2(body.right() - 82.0, y + 16.0));
        let mut v = values[i];
        let resp = widgets::slider(ui, track, Id::unique(("axis", i)), &mut v, a.min..=a.max, &name);
        let _ = resp;
        let spin = Rect::from_min_size(pos2(body.right() - 72.0, y - 4.0), vec2(62.0, 24.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(spin));
        let dv = child.add_sized(spin.size(), egui::DragValue::new(&mut v).range(a.min..=a.max).fixed_decimals(1).speed((a.max - a.min) / 300.0));
        widgets::a11y(&dv, Role::SpinButton, true, &name);
        if (v - values[i]).abs() > f32::EPSILON {
            values[i] = v;
            changed = true;
        }
        y += 34.0;
    }
    if changed || !app.props.axis_values.contains_key(&f.id) {
        app.props.axis_values.insert(f.id, values);
    }
}
