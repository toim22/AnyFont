//! The left panel: Libraries (My fonts, Folders, My Collections with the inline name editor) above the
//! splitter, Filters below it (UI-SPEC §5.1, §5.2).

use crate::app::{AnyFontApp, Focus};
use crate::commands::EditTarget;
use crate::icons::{self, Look};
use crate::theme::{self, t};
use crate::widgets;
use anyfont_core::catalog::{LibraryEntry, LibraryRef};
use anyfont_core::filters::{FilterGroup, FilterOption};
use egui::{pos2, vec2, Align2, Id, Key, Modifiers, Rect, Sense, Stroke, Ui, WidgetInfo, Role};

const TITLE_H: f32 = 34.0;
const HEADER_H: f32 = 28.0;
/// Library rows (the captured selection highlight of "My fonts" is 24 px tall).
const ROW_H: f32 = 24.0;
/// Caption centre of a library row, from its top (captured: cap height 4 px above the header's).
const ROW_TEXT_Y: f32 = 10.5;
const FILTER_HEADER_H: f32 = 23.0;
const FILTER_ROW_H: f32 = 26.0;
/// Room above and below the check boxes of an expanded filter group (captured: "Installed" 22 px below
/// "Font Status", "Embedding Rights" 30 px below "Duplicates").
const FILTER_GROUP_PAD: (f32, f32) = (1.0, 2.0);

/// Section keys of `AppSettings::collapsed_sections`.
const FOLDERS_KEY: &str = "Folders";
const COLLECTIONS_KEY: &str = "My Collections";

enum Line {
    Header(&'static str),
    Lib(LibraryEntry),
}

pub fn draw_left_panel(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect) {
    if app.settings.show_filter_list {
        let max_lib = (rect.height() - 80.0).max(100.0);
        let lib_h = app.settings.libraries_height.clamp(80.0, max_lib);
        let libs = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + lib_h));
        let filters = Rect::from_min_max(pos2(rect.left(), libs.bottom() + 1.0), rect.max);
        draw_libraries(app, ui, libs);
        // splitter
        let y = libs.bottom() + 0.5;
        ui.painter().hline(rect.left()..=rect.right() - 3.0, y, Stroke::new(1.0, t().line_divider));
        let sr = ui.interact(Rect::from_min_max(pos2(rect.left(), y - 3.0), pos2(rect.right() - 3.0, y + 3.0)), Id::unique("libraries-splitter"), Sense::drag());
        if sr.hovered() || sr.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if sr.dragged() {
            app.settings.libraries_height = (lib_h + sr.drag_delta().y).clamp(80.0, max_lib);
        }
        draw_filters(app, ui, filters);
    } else {
        draw_libraries(app, ui, rect);
    }
}

fn lines(app: &AnyFontApp) -> Vec<Line> {
    let mut v = Vec::new();
    let entries = app.catalog.libraries();
    v.push(Line::Header(FOLDERS_KEY));
    let folders_collapsed = app.settings.is_collapsed(FOLDERS_KEY);
    let collections_collapsed = app.settings.is_collapsed(COLLECTIONS_KEY);
    for e in &entries {
        match e.library {
            LibraryRef::MyFonts if !folders_collapsed => v.push(Line::Lib(e.clone())),
            LibraryRef::Folder(_) if !folders_collapsed && !e.path.as_deref().map(|p| app.is_content_folder(p)).unwrap_or(false) => {
                v.push(Line::Lib(e.clone()));
            }
            _ => {}
        }
    }
    v.push(Line::Header(COLLECTIONS_KEY));
    if !collections_collapsed {
        let mut cols: Vec<&LibraryEntry> = entries.iter().filter(|e| matches!(e.library, LibraryRef::Collection(_))).collect();
        cols.sort_by_key(|e| if e.is_favorites { crate::tr("Favorites").to_lowercase() } else { e.name.to_lowercase() });
        for e in cols {
            v.push(Line::Lib(e.clone()));
        }
    }
    v
}

fn entry_caption(e: &LibraryEntry) -> (String, bool) {
    match e.library {
        LibraryRef::MyFonts => ("My fonts".to_string(), true),
        LibraryRef::Collection(_) if e.is_favorites => ("Favorites".to_string(), true),
        _ => (e.name.clone(), false),
    }
}

fn draw_libraries(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    let p = ui.painter_at(rect);
    widgets::ui_text(&p, pos2(rect.left() + 8.0, rect.top() + 17.0), Align2::LEFT_CENTER, "Libraries", theme::bold(theme::UI_SIZE), t().text_strong);
    let area = Rect::from_min_max(pos2(rect.left(), rect.top() + TITLE_H), pos2(rect.right() - 4.0, rect.bottom()));
    let lines = lines(app);
    let content_h: f32 = lines.iter().map(|l| if matches!(l, Line::Header(_)) { HEADER_H } else { ROW_H }).sum();
    let track = Rect::from_min_max(pos2(rect.right() - 10.0, area.top()), pos2(rect.right() - 3.0, area.bottom()));
    let mut scroll = app.libs_scroll;
    widgets::vscroll(ui, Id::unique("libraries-scroll"), area, track, content_h, &mut scroll);
    app.libs_scroll = scroll;
    let row_w = (rect.width() - 4.0).clamp(100.0, 237.0);
    let clip = ui.painter_at(area);
    // a click on the empty part of the panel gives it the keyboard (registered first: the rows lie on top)
    let bg = ui.interact(area, Id::unique("libraries-bg"), Sense::click());
    let mut y = area.top() - app.libs_scroll;
    let mut open_menu: Option<(LibraryRef, egui::Pos2)> = None;
    let mut open_lib: Option<LibraryRef> = None;
    let mut toggle: Option<&'static str> = None;
    let mut lib_rows: Vec<(LibraryRef, Rect)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        match line {
            Line::Header(key) => {
                let hr = Rect::from_min_size(pos2(rect.left(), y), vec2(row_w, HEADER_H));
                if hr.bottom() > area.top() && hr.top() < area.bottom() {
                    let text_r = Rect::from_min_max(pos2(hr.left(), hr.top() + 15.0 - 10.0), pos2(hr.right(), hr.top() + 15.0 + 10.0));
                    let collapsed = app.settings.is_collapsed(key);
                    let resp = widgets::section_header(ui, text_r.intersect(area), Id::unique(("lib-section", *key)), key, collapsed, rect.left() + 211.0);
                    if resp.clicked() {
                        toggle = Some(key);
                    }
                }
                y += HEADER_H;
            }
            Line::Lib(e) => {
                let r = Rect::from_min_size(pos2(rect.left(), y), vec2(row_w, ROW_H));
                y += ROW_H;
                lib_rows.push((e.library, r));
                if r.bottom() <= area.top() || r.top() >= area.bottom() {
                    continue;
                }
                let resp = ui.interact(r.intersect(area), Id::unique(("library-row", i, format!("{:?}", e.library))), Sense::click());
                let selected = app.library == e.library;
                let drop_target = app.drag_fonts.is_some() && matches!(e.library, LibraryRef::Collection(_)) && resp.contains_pointer();
                if selected {
                    clip.rect_filled(r, 3.0, t().library_selected);
                } else if drop_target {
                    clip.rect_filled(r, 3.0, t().accent_checked);
                } else if resp.hovered() && app.drag_fonts.is_none() {
                    clip.rect_filled(r, 3.0, t().row_hot);
                }
                let renaming = matches!((&app.rename, e.library), (Some(rn), LibraryRef::Collection(id)) if rn.id == id);
                let (cap, translate) = entry_caption(e);
                if renaming {
                    rename_editor(app, ui, r);
                } else if translate {
                    widgets::ui_text(&clip, pos2(r.left() + 4.0, r.top() + ROW_TEXT_Y), Align2::LEFT_CENTER, &cap, theme::body(theme::UI_SIZE), t().text);
                } else {
                    widgets::data_text(ui, &clip, pos2(r.left() + 4.0, r.top() + ROW_TEXT_Y), Align2::LEFT_CENTER, &cap, theme::body(theme::UI_SIZE), t().text);
                }
                let a11y_name = if translate { crate::tr(&cap) } else { cap.clone() };
                resp.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, a11y_name.clone()));
                let resp = match e.library {
                    LibraryRef::MyFonts => widgets::tooltip(resp, "Includes all installed fonts, all fonts in your added folders, and fonts downloaded from the Online library", None),
                    LibraryRef::Folder(_) => {
                        let path = e.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                        resp.on_hover_ui(move |ui| {
                            widgets::data_label(ui, &path, theme::body(theme::UI_SIZE), t().text);
                        })
                    }
                    _ => resp,
                };
                if resp.clicked() && !renaming {
                    open_lib = Some(e.library);
                }
                if resp.secondary_clicked() {
                    open_lib = Some(e.library);
                    open_menu = Some((e.library, ctx.input(|i| i.pointer.interact_pos()).unwrap_or(r.center())));
                }
                if drop_target && ctx.input(|i| i.pointer.any_released()) {
                    if let (Some(ids), LibraryRef::Collection(cid)) = (app.drag_fonts.take(), e.library) {
                        let keys: Vec<_> = ids.iter().filter_map(|id| app.catalog.font(*id)).map(|f| f.key.clone()).collect();
                        if let Err(err) = app.db.add_to_collection(cid, &keys) {
                            log::error!("drop on collection: {err}");
                        }
                        app.reload();
                    }
                }
            }
        }
    }
    if let Some(key) = toggle {
        let c = app.settings.is_collapsed(key);
        app.settings.set_collapsed(key, !c);
    }
    if let Some(lib) = open_lib {
        app.focus = Focus::Libraries;
        if app.library != lib {
            if app.rename.is_some() {
                app.commit_rename();
            }
            app.open_library(lib);
        }
    }
    if let Some((lib, p)) = open_menu {
        app.menu.open(crate::menus::Source::Library(lib), p, false, app.frame_no);
    }
    if bg.clicked() {
        app.focus = Focus::Libraries;
    }
    libraries_keyboard(app, ui, &lib_rows);
}

fn rename_editor(app: &mut AnyFontApp, ui: &mut Ui, r: Rect) {
    let ctx = ui.ctx().clone();
    let id = EditTarget::Rename.id();
    let er = Rect::from_min_max(pos2(r.left() + 2.0, r.center().y - 12.0), pos2((r.left() + 235.0).min(r.right()), r.center().y + 12.0));
    ui.painter().rect_filled(er, 0.0, t().edit_bg);
    ui.painter().rect_stroke(er, 0.0, Stroke::new(1.0, t().accent), egui::StrokeKind::Inside);
    let Some(rn) = app.rename.as_mut() else { return };
    let first = rn.started_frame + 1 >= app.frame_no;
    let inner = Rect::from_min_max(pos2(er.left() + 4.0, er.top() + 1.0), pos2(er.right() - 4.0, er.bottom() - 1.0));
    let out = egui::TextEdit::singleline(&mut rn.text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(inner.width())
        .align(egui::Align2::LEFT_CENTER)
        .text_color(t().text)
        .font(theme::body(theme::UI_SIZE))
        .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    widgets::a11y(&out.response, Role::TextInput, true, "Name:");
    crate::commands::remember_selection(&ctx, EditTarget::Rename);
    if first {
        out.response.request_focus();
        let mut st = out.state.clone();
        let n = rn.text.chars().count();
        st.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
        st.store(&ctx, id);
        return;
    }
    if out.response.secondary_clicked() {
        let p = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(er.left_bottom());
        app.menu.open(crate::menus::Source::Edit(EditTarget::Rename), p, false, app.frame_no);
        return;
    }
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    let enter = out.response.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter));
    if esc {
        app.cancel_rename();
    } else if (enter || (out.response.lost_focus() && !app.menu.is_open())) && !app.commit_rename() {
        // refused (empty or duplicate name): the editor stays open
        out.response.request_focus();
    }
}

fn libraries_keyboard(app: &mut AnyFontApp, ui: &Ui, rows: &[(LibraryRef, Rect)]) {
    let ctx = ui.ctx();
    if app.focus != Focus::Libraries || ctx.memory(|m| m.focused().is_some()) || ctx.memory(|m| m.top_modal_layer().is_some()) || app.menu.is_open() || app.rename.is_some() || app.menu.bar_focus.is_some() {
        return;
    }
    if rows.is_empty() {
        return;
    }
    let cur = rows.iter().position(|(l, _)| *l == app.library).unwrap_or(0);
    let (up, down, left, right, menu) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowLeft),
            i.consume_key(Modifiers::NONE, Key::ArrowRight),
            i.consume_key(Modifiers::SHIFT, Key::F10) || i.consume_key(Modifiers::NONE, Key::Menu),
        )
    });
    let section = |l: &LibraryRef| if matches!(l, LibraryRef::Collection(_)) { COLLECTIONS_KEY } else { FOLDERS_KEY };
    if up && cur > 0 {
        app.open_library(rows[cur - 1].0);
    } else if down && cur + 1 < rows.len() {
        app.open_library(rows[cur + 1].0);
    }
    if left {
        app.settings.set_collapsed(section(&app.library), true);
    }
    if right {
        app.settings.set_collapsed(section(&app.library), false);
    }
    if menu {
        let r = rows[cur].1;
        app.menu.open(crate::menus::Source::Library(app.library), r.center(), true, app.frame_no);
    }
}

/// Filter group collapse key; Font Status starts expanded, the others collapsed (UI-SPEC §5.2).
fn filter_collapsed(app: &AnyFontApp, g: FilterGroup) -> bool {
    let default = g != FilterGroup::FontStatus;
    default ^ app.settings.is_collapsed(&format!("filter:{}", g.caption()))
}

fn draw_filters(app: &mut AnyFontApp, ui: &mut Ui, rect: Rect) {
    let p = ui.painter_at(rect);
    widgets::ui_text(&p, pos2(rect.left() + 8.0, rect.top() + 14.0), Align2::LEFT_CENTER, "Filters", theme::bold(theme::UI_SIZE), t().text_strong);
    let active = !app.filters.is_empty();
    let fr = Rect::from_center_size(pos2(rect.left() + 226.0, rect.top() + 15.0), vec2(20.0, 20.0));
    let fr = if rect.width() < 240.0 { Rect::from_center_size(pos2(rect.right() - 14.0, rect.top() + 15.0), vec2(20.0, 20.0)) } else { fr };
    let resp = ui.interact(fr, Id::unique("clear-filters"), if active { Sense::click() } else { Sense::hover() });
    if active && resp.hovered() {
        ui.painter().rect_filled(fr, 3.0, t().menu_bar_hot);
    }
    icons::paint(ui, "filter-clear", fr, 16.0, if active { Look::FilterActive } else { Look::Disabled });
    widgets::a11y(&resp, Role::Button, active, "Clear filters");
    if widgets::tooltip(resp, "Clear filters", Some("Remove all selected font filters.")).clicked() {
        app.filters.clear();
        app.requery();
        app.select_first();
    }
    let area = Rect::from_min_max(pos2(rect.left(), rect.top() + 33.0), pos2(rect.right() - 4.0, rect.bottom()));
    let groups = app.catalog.available_filters(&app.library);
    let mut content_h = 0.0;
    for g in &groups {
        content_h += FILTER_HEADER_H;
        if !filter_collapsed(app, g.group) {
            content_h += g.options.len() as f32 * FILTER_ROW_H + FILTER_GROUP_PAD.0 + FILTER_GROUP_PAD.1;
        }
    }
    let track = Rect::from_min_max(pos2(rect.right() - 10.0, area.top()), pos2(rect.right() - 3.0, area.bottom()));
    let mut scroll = app.filters_scroll;
    widgets::vscroll(ui, Id::unique("filters-scroll"), area, track, content_h, &mut scroll);
    app.filters_scroll = scroll;
    let row_w = (rect.width() - 4.0).clamp(100.0, 237.0);
    let mut y = area.top() - app.filters_scroll;
    let mut toggle_group: Option<FilterGroup> = None;
    let mut toggle_opt: Option<FilterOption> = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    child.set_clip_rect(area);
    for g in &groups {
        let hr = Rect::from_min_size(pos2(rect.left(), y), vec2(row_w, FILTER_HEADER_H));
        y += FILTER_HEADER_H;
        let collapsed = filter_collapsed(app, g.group);
        if hr.bottom() > area.top() && hr.top() < area.bottom() {
            let text_r = Rect::from_min_max(pos2(hr.left(), hr.top() + 13.0 - 10.0), pos2(hr.right(), hr.top() + 13.0 + 10.0));
            if widgets::section_header(&mut child, text_r, Id::unique(("filter-group", g.group.caption())), g.group.caption(), collapsed, rect.left() + 211.0).clicked() {
                toggle_group = Some(g.group);
            }
        }
        if collapsed {
            continue;
        }
        y += FILTER_GROUP_PAD.0;
        for o in &g.options {
            let r = Rect::from_min_size(pos2(rect.left(), y), vec2(row_w, FILTER_ROW_H));
            y += FILTER_ROW_H;
            if r.bottom() <= area.top() || r.top() >= area.bottom() {
                continue;
            }
            let checked = app.filters.is_checked(o);
            if widgets::check_row(&mut child, r, Id::unique(("filter", format!("{o:?}"))), o.caption(), checked, true).clicked() {
                toggle_opt = Some(*o);
            }
        }
        y += FILTER_GROUP_PAD.1;
    }
    if let Some(g) = toggle_group {
        let key = format!("filter:{}", g.caption());
        let c = app.settings.is_collapsed(&key);
        app.settings.set_collapsed(&key, !c);
    }
    if let Some(o) = toggle_opt {
        app.filters.toggle(o);
        app.requery();
        if app.sel.keys.is_empty() {
            app.select_first();
        }
    }
}
