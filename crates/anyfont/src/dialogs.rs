//! The shell's dialogs (UI-SPEC §7.2.5, §7.2.10–§7.2.14) in the shared dialog look (`help::dlg`): message
//! boxes, the remove confirmation, Delete fonts, the operation progress, the reboot prompt, "We can't find the
//! following:", the crash-report offer. Also the Install / Uninstall / Delete flow around `ops::Running`.

use crate::app::AnyFontApp;
use crate::help::dlg;
use crate::ops::{OpKind, Outcome, Running};
use crate::theme::{self, t};
use crate::widgets;
use anyfont_core::catalog::LibraryRef;
use anyfont_core::model::FontId;
use anyfont_sys::{Scope, UsedFontAction};
use egui::{pos2, vec2, Color32, Rect, Stroke};
use std::collections::HashMap;
use std::path::PathBuf;

pub enum Dialog {
    Message { title: String, text: String },
    RemoveLibrary { lib: LibraryRef, dont_show: bool },
    DeleteFonts { files: Vec<(PathBuf, bool)>, ids: Vec<FontId>, dont_show: bool },
    Reboot { kind: OpKind, files: Vec<PathBuf> },
    /// The UAC prompt of an all-users install was cancelled: offer to install only for the current user.
    InstallForMe { files: Vec<PathBuf>, names: HashMap<PathBuf, String>, ids: Vec<FontId> },
    Missing { items: Vec<String> },
    Failures { lines: Vec<String> },
    CrashOffer { path: PathBuf },
}

impl Dialog {
    pub fn message(title: &str, text: &str) -> Dialog {
        Dialog::Message { title: title.to_string(), text: text.to_string() }
    }
}

/// What a dialog's button does once the dialog is closed.
type DialogAction = Box<dyn FnOnce(&mut AnyFontApp, &egui::Context)>;

/// The fallback offered when the UAC prompt of an all-users install is cancelled.
pub const INSTALL_FOR_ME_TEXT: &str = "AnyFont could not get administrator permission to install the fonts for all users.\n\nDo you want to install them only for you? They will not be available to other users of this computer.";

/// An operation in progress and what it needs for its second pass.
pub struct OpFlow {
    pub ids: Vec<FontId>,
    pub names: HashMap<PathBuf, String>,
    /// Install scope of the operation (the reboot prompt's second pass keeps it).
    pub scope: Scope,
}

/// Starts Install / Uninstall for faces (only the eligible ones; nothing happens when none is).
pub fn start_fonts_op(app: &mut AnyFontApp, ctx: &egui::Context, kind: OpKind, ids: Vec<FontId>) {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut names: HashMap<PathBuf, String> = HashMap::new();
    let mut used_ids = Vec::new();
    // Uninstall works on the installed copies (a face found in a folder may also be installed elsewhere)
    let installed: Vec<PathBuf> = if kind == OpKind::Uninstall { app.installed().into_iter().map(|f| f.path).collect() } else { Vec::new() };
    for id in ids {
        let Some(f) = app.catalog.font(id) else { continue };
        let ok = match kind {
            OpKind::Install => !f.installed && f.is_usable(),
            OpKind::Uninstall => f.installed && !f.protected,
            OpKind::Delete => !f.paths.is_empty() && !f.protected,
        };
        if !ok {
            continue;
        }
        used_ids.push(id);
        let paths: Vec<PathBuf> = if kind == OpKind::Uninstall {
            f.paths.iter().filter(|p| installed.iter().any(|i| crate::app::same_path(i, p))).cloned().collect()
        } else {
            f.path().map(|p| vec![p.to_path_buf()]).unwrap_or_default()
        };
        for p in paths {
            names.entry(p.clone()).or_insert_with(|| f.full_name.clone());
            if !files.contains(&p) {
                files.push(p);
            }
        }
    }
    if files.is_empty() {
        return;
    }
    let scope = crate::ops::install_scope(app.fs.as_ref());
    run_files(app, ctx, kind, scope, files, UsedFontAction::Ask, names, used_ids);
}

#[allow(clippy::too_many_arguments)]
fn run_files(app: &mut AnyFontApp, ctx: &egui::Context, kind: OpKind, scope: Scope, files: Vec<PathBuf>, used: UsedFontAction, names: HashMap<PathBuf, String>, ids: Vec<FontId>) {
    if app.op.is_some() {
        return;
    }
    app.op = Some(Running::start(app.fs.clone(), kind, scope, files, used, names.clone(), ctx.clone()));
    app.op_flow = Some(OpFlow { ids, names, scope });
}

/// Edit ▸ Delete: the confirmation (unless switched off), then the operation.
pub fn ask_delete(app: &mut AnyFontApp, ids: Vec<FontId>) {
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    let mut del_ids = Vec::new();
    for id in ids {
        let Some(f) = app.catalog.font(id) else { continue };
        if f.paths.is_empty() || f.protected {
            continue;
        }
        del_ids.push(id);
        for p in &f.paths {
            if !files.iter().any(|(q, _)| q == p) {
                files.push((p.clone(), true));
            }
        }
    }
    if files.is_empty() {
        return;
    }
    app.dialogs.push_back(Dialog::DeleteFonts { files, ids: del_ids, dont_show: false });
    if app.settings.skip_delete_confirmation {
        // confirmed at once with every file
        if let Some(Dialog::DeleteFonts { files, ids, .. }) = app.dialogs.pop_back() {
            let names = names_of(app, &ids);
            app.pending_delete = Some((files.into_iter().map(|(p, _)| p).collect(), names, ids));
        }
    }
}

fn names_of(app: &AnyFontApp, ids: &[FontId]) -> HashMap<PathBuf, String> {
    let mut m = HashMap::new();
    for id in ids {
        if let Some(f) = app.catalog.font(*id) {
            for p in &f.paths {
                m.entry(p.clone()).or_insert_with(|| f.full_name.clone());
            }
        }
    }
    m
}

/// Takes the operation's progress; at its end, the reboot prompt, the failure list and a rescan.
pub fn poll_operation(app: &mut AnyFontApp, ctx: &egui::Context) {
    if let Some((files, names, ids)) = app.pending_delete.take() {
        run_files(app, ctx, OpKind::Delete, Scope::AllUsers, files, UsedFontAction::Ask, names, ids);
    }
    let Some(op) = &mut app.op else { return };
    if !op.poll() {
        return;
    }
    let outcome = op.outcome.clone().unwrap_or_default();
    let used = op.used;
    let kind = op.kind;
    let scope = op.scope;
    app.op = None;
    finish(app, kind, scope, used, outcome);
}

fn finish(app: &mut AnyFontApp, kind: OpKind, scope: Scope, used: UsedFontAction, o: Outcome) {
    if kind == OpKind::Install && o.succeeded > 0 {
        if let Some(flow) = &app.op_flow {
            for id in &flow.ids {
                if let Some(f) = app.catalog.font(*id) {
                    let _ = app.db.touch_mru(&f.key, app.settings.core.mru_size.max(1));
                }
            }
        }
    }
    if used == UsedFontAction::Ask && !o.in_use.is_empty() {
        app.dialogs.push_back(Dialog::Reboot { kind, files: o.in_use.clone() });
    } else if matches!(used, UsedFontAction::RebootNow) {
        // (the sandbox of the tests only records the request)
        if let Err(e) = app.fs.reboot_now() {
            log::error!("reboot: {e}");
        }
    }
    if kind == OpKind::Install && scope == Scope::AllUsers && !o.elevation_cancelled_files.is_empty() {
        // the UAC prompt was cancelled: offer the per-user install, which needs no administrator rights
        let (names, ids) = app.op_flow.as_ref().map(|f| (f.names.clone(), f.ids.clone())).unwrap_or_default();
        app.dialogs.push_back(Dialog::InstallForMe { files: o.elevation_cancelled_files.clone(), names, ids });
    } else if o.elevation_cancelled {
        app.dialogs.push_back(Dialog::message("AnyFont", &crate::tr("Insufficient Permissions")));
    }
    if !o.failures.is_empty() {
        let names = app.op_flow.as_ref().map(|f| f.names.clone()).unwrap_or_default();
        let lines = o
            .failures
            .iter()
            .map(|(p, e)| format!("{} ({}): {e}", names.get(p).cloned().unwrap_or_else(|| p.display().to_string()), p.display()))
            .collect();
        app.dialogs.push_back(Dialog::Failures { lines });
    }
    if app.dialogs.iter().all(|d| !matches!(d, Dialog::Reboot { .. } | Dialog::InstallForMe { .. })) {
        app.op_flow = None;
    }
    app.start_scan(false);
}

fn warning_icon(ui: &egui::Ui, at: egui::Pos2) {
    let p = ui.painter();
    let pts = vec![pos2(at.x + 16.0, at.y), pos2(at.x + 32.0, at.y + 28.0), pos2(at.x, at.y + 28.0)];
    p.add(egui::Shape::convex_polygon(pts, Color32::from_rgb(0xF2, 0xC2, 0x1B), Stroke::NONE));
    p.text(pos2(at.x + 16.0, at.y + 17.0), egui::Align2::CENTER_CENTER, "!", theme::bold(17.0), Color32::from_rgb(0x20, 0x20, 0x20));
}

fn list_box(ui: &mut egui::Ui, id: &str, items: &[String], height: f32) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, height), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, t().edit_bg);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t().edit_border), egui::StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(4.0)));
    egui::ScrollArea::vertical().id_salt(id).max_height(height - 8.0).auto_shrink([false, false]).show(&mut child, |ui| {
        for it in items {
            widgets::data_label(ui, it, theme::body(theme::UI_SIZE), t().text);
        }
    });
}

/// Draws the first queued dialog and the progress dialog.
pub fn show(app: &mut AnyFontApp, ctx: &egui::Context) {
    // the progress dialog (modal, no ×) while an operation runs
    if let Some(op) = &app.op {
        let text = op.progress_text();
        let frac = op.percent() as f32 / 100.0;
        let (cancel, _) = dlg::window_ex(ctx, "progress", "AnyFont", vec2(420.0, 150.0), false, |ui| {
            ui.add_space(8.0);
            widgets::data_label(ui, &text, theme::body(theme::UI_SIZE), t().text);
            ui.add_space(8.0);
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 0.0, t().slider_track);
            ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(r.width() * frac, r.height())), 0.0, t().accent);
            let (_, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at_right(ui, bar, |ui| dlg::button(ui, &crate::tr("Cancel"), true))
        });
        if cancel {
            op.cancel();
        }
        return;
    }
    let Some(d) = app.dialogs.front_mut() else { return };
    let mut close = false;
    let mut action: Option<DialogAction> = None;
    match d {
        Dialog::Message { title, text } => {
            close = dlg::Message::new(crate::tr(title), crate::tr(text)).ui(ctx, "shell-message");
        }
        Dialog::RemoveLibrary { lib, dont_show } => {
            let lib = *lib;
            let text = if matches!(lib, LibraryRef::Collection(_)) { "Are you sure you want to remove this collection?" } else { "Are you sure you want to remove this folder?" };
            let ((yes, no), dismissed) = dlg::window(ctx, "remove-library", &crate::tr("AnyFont"), vec2(421.0, 199.0), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(10.0);
                    let r = ui.max_rect();
                    warning_icon(ui, r.min + vec2(4.0, 6.0));
                    let tr = Rect::from_min_max(pos2(r.left() + 50.0, r.top() + 8.0), r.max);
                    dlg::at(ui, tr, |ui| {
                        ui.add(egui::Label::new(crate::tr(text)).wrap());
                        ui.add_space(14.0);
                        widgets::checkbox(ui, dont_show, "Do not show this message again");
                    });
                });
                let yes = dlg::at_right(ui, bar, |ui| {
                    let no = widgets::push_button(ui, "&No", false, true).clicked();
                    ui.add_space(8.0);
                    let yes = widgets::push_button(ui, "&Yes", true, true).clicked();
                    (yes, no)
                });
                let (y, n) = yes;
                (y || dlg::enter_pressed(ui) || ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::Y)), n || ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::N)))
            });
            if yes {
                let skip = *dont_show;
                action = Some(Box::new(move |app, _| {
                    app.settings.skip_remove_confirmation = skip;
                    app.remove_library(lib);
                }));
                close = true;
            } else if no || dismissed {
                close = true;
            }
        }
        Dialog::DeleteFonts { files, ids, dont_show } => {
            let text = if files.len() == 1 {
                "Are you sure you want to delete this font?\n\nRead-only fonts cannot be deleted."
            } else {
                "Are you sure you want to delete these fonts?\n\nRead-only fonts cannot be deleted."
            };
            let h = (260.0 + files.len().min(8) as f32 * 18.0).min(480.0);
            let ((ok, cancel), dismissed) = dlg::window(ctx, "delete-fonts", &crate::tr("Delete fonts"), vec2(480.0, h), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(crate::tr(text)).wrap());
                    ui.add_space(8.0);
                    let mut all = files.iter().all(|(_, c)| *c);
                    if widgets::checkbox(ui, &mut all, "Select all").clicked() {
                        for f in files.iter_mut() {
                            f.1 = all;
                        }
                    }
                    let lh = (files.len().min(8) as f32 * 20.0 + 8.0).max(48.0);
                    let w = ui.available_width();
                    let (r, _) = ui.allocate_exact_size(vec2(w, lh), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0.0, t().edit_bg);
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t().edit_border), egui::StrokeKind::Inside);
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(4.0)));
                    egui::ScrollArea::vertical().id_salt("delete-files").auto_shrink([false, false]).show(&mut child, |ui| {
                        for (i, (p, c)) in files.iter_mut().enumerate() {
                            let resp = data_checkbox(ui, c, &p.display().to_string(), i);
                            let _ = resp;
                        }
                    });
                    ui.add_space(8.0);
                    widgets::checkbox(ui, dont_show, "Do not show this message again.");
                });
                let any = files.iter().any(|(_, c)| *c);
                dlg::at_right(ui, bar, |ui| {
                    let cancel = widgets::push_button(ui, "Cancel", false, true).clicked();
                    ui.add_space(8.0);
                    let ok = widgets::push_button(ui, "OK", true, any).clicked() || (any && dlg::enter_pressed(ui));
                    (ok, cancel)
                })
            });
            if ok {
                let chosen: Vec<PathBuf> = files.iter().filter(|(_, c)| *c).map(|(p, _)| p.clone()).collect();
                let ids = ids.clone();
                let skip = *dont_show;
                action = Some(Box::new(move |app, _| {
                    app.settings.skip_delete_confirmation = skip;
                    let names = names_of(app, &ids);
                    app.pending_delete = Some((chosen, names, ids));
                }));
                close = true;
            } else if cancel || dismissed {
                close = true;
            }
        }
        Dialog::Reboot { kind, files } => {
            let kind = *kind;
            let names = app.op_flow.as_ref().map(|f| f.names.clone()).unwrap_or_default();
            // one line per font: a font's files (its installed copy and the file in a folder) share its name
            let mut items: Vec<String> = Vec::new();
            for name in files.iter().map(|p| names.get(p).cloned().unwrap_or_else(|| p.display().to_string())) {
                if !items.contains(&name) {
                    items.push(name);
                }
            }
            let one = items.len() == 1;
            let text = match (kind, one) {
                (OpKind::Delete, true) => "The following font cannot be deleted because it is in use by one or more applications. To delete this font, you must first restart your computer.\n\nDo you want to restart your computer now?",
                (OpKind::Delete, false) => "The following fonts cannot be deleted because they are in use by one or more applications. To delete these fonts, you must first restart your computer.\n\nDo you want to restart your computer now?\n\nClick Ignore to continue without deleting the listed fonts.",
                (_, true) => "The following font cannot be uninstalled because it is in use by one or more applications. To uninstall this font, you must first restart your computer.\n\nDo you want to restart your computer now?",
                (_, false) => "The following fonts cannot be uninstalled because they are in use by one or more applications. To uninstall these fonts, you must first restart your computer.\n\nDo you want to restart your computer now?\n\nClick Ignore to continue without uninstalling the listed fonts.",
            };
            let (choice, dismissed) = dlg::window(ctx, "reboot-prompt", &crate::tr("AnyFont"), vec2(520.0, 330.0), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(crate::tr(text)).wrap());
                    ui.add_space(8.0);
                    list_box(ui, "reboot-list", &items, 72.0);
                });
                dlg::at_right(ui, bar, |ui| {
                    let mut c = None;
                    if widgets::push_button(ui, "Cancel", false, true).clicked() {
                        c = Some(UsedFontAction::Cancel);
                    }
                    ui.add_space(6.0);
                    if !one && widgets::push_button(ui, "&Ignore", false, true).clicked() {
                        c = Some(UsedFontAction::Ignore);
                    }
                    ui.add_space(6.0);
                    if widgets::push_button(ui, "Reboot &later", false, true).clicked() {
                        c = Some(UsedFontAction::RebootLater);
                    }
                    ui.add_space(6.0);
                    if widgets::push_button(ui, "Reboot &now", true, true).clicked() || dlg::enter_pressed(ui) {
                        c = Some(UsedFontAction::RebootNow);
                    }
                    c
                })
            });
            let choice = if dismissed { Some(UsedFontAction::Cancel) } else { choice };
            if let Some(c) = choice {
                let files = files.clone();
                close = true;
                action = Some(Box::new(move |app, ctx| {
                    let flow = app.op_flow.take();
                    if matches!(c, UsedFontAction::RebootNow | UsedFontAction::RebootLater) {
                        let (names, ids, scope) = flow.map(|f| (f.names, f.ids, f.scope)).unwrap_or((HashMap::new(), Vec::new(), Scope::AllUsers));
                        run_files(app, ctx, kind, scope, files, c, names, ids);
                    }
                }));
            }
        }
        Dialog::InstallForMe { files, names, ids } => {
            let ((me, cancel), dismissed) = dlg::window(ctx, "install-for-me", &crate::tr("AnyFont"), vec2(460.0, 200.0), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(10.0);
                    ui.add(egui::Label::new(crate::tr(INSTALL_FOR_ME_TEXT)).wrap());
                });
                dlg::at_right(ui, bar, |ui| {
                    let cancel = widgets::push_button(ui, "Cancel", false, true).clicked();
                    ui.add_space(8.0);
                    let me = widgets::push_button(ui, "&Install for me", true, true).clicked()
                        || dlg::enter_pressed(ui)
                        || ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::I));
                    (me, cancel)
                })
            });
            if me {
                let (files, names, ids) = (files.clone(), names.clone(), ids.clone());
                action = Some(Box::new(move |app, ctx| {
                    app.op_flow = None;
                    run_files(app, ctx, OpKind::Install, Scope::CurrentUser, files, UsedFontAction::Ask, names, ids);
                }));
                close = true;
            } else if cancel || dismissed {
                app.op_flow = None;
                close = true;
            }
        }
        Dialog::Missing { items } => {
            let h = (200.0 + items.len().min(8) as f32 * 18.0).min(420.0);
            let (ok, dismissed) = dlg::window(ctx, "missing", &crate::tr("AnyFont"), vec2(440.0, h), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(8.0);
                    ui.label("We can't find the following:");
                    ui.add_space(6.0);
                    let lh = ui.available_height() - 4.0;
                    list_box(ui, "missing-list", items, lh.max(40.0));
                });
                dlg::at_right(ui, bar, |ui| widgets::push_button(ui, "OK", true, true).clicked()) || dlg::enter_pressed(ui)
            });
            close = ok || dismissed;
        }
        Dialog::Failures { lines } => {
            let h = (200.0 + lines.len().min(8) as f32 * 18.0).min(420.0);
            let (ok, dismissed) = dlg::window(ctx, "failures", &crate::tr("AnyFont"), vec2(520.0, h), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(8.0);
                    ui.label("Some fonts could not be processed:");
                    ui.add_space(6.0);
                    let lh = ui.available_height() - 4.0;
                    list_box(ui, "failure-list", lines, lh.max(40.0));
                });
                dlg::at_right(ui, bar, |ui| widgets::push_button(ui, "OK", true, true).clicked()) || dlg::enter_pressed(ui)
            });
            close = ok || dismissed;
        }
        Dialog::CrashOffer { path } => {
            let path = path.clone();
            let ((yes, no), dismissed) = dlg::window(ctx, "crash-offer", &crate::tr("AnyFont"), vec2(440.0, 170.0), |ui| {
                let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
                dlg::at(ui, top, |ui| {
                    ui.add_space(10.0);
                    ui.add(egui::Label::new(crate::tr("AnyFont closed unexpectedly the last time. Do you want to send a bug report?")).wrap());
                });
                dlg::at_right(ui, bar, |ui| {
                    let no = widgets::push_button(ui, "&No", false, true).clicked();
                    ui.add_space(8.0);
                    let yes = widgets::push_button(ui, "&Yes", true, true).clicked() || dlg::enter_pressed(ui);
                    (yes, no)
                })
            });
            if yes {
                action = Some(Box::new(move |app, _| {
                    let text = std::fs::read_to_string(&path).unwrap_or_default();
                    let _ = anyfont_sys::crash::mark_handled(&path);
                    app.bugreport.open_with_crash(text);
                }));
                close = true;
            } else if no || dismissed {
                let _ = anyfont_sys::crash::mark_handled(&path);
                close = true;
            }
        }
    }
    if close {
        app.dialogs.pop_front();
    }
    if let Some(a) = action {
        a(app, ctx);
    }
}

/// A check box whose caption is data (a file path).
fn data_checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str, i: usize) -> egui::Response {
    let g = widgets::data_galley(ui, label, theme::body(theme::UI_SIZE), t().text);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 22.0, 20.0), egui::Sense::click());
    if resp.clicked() {
        *value = !*value;
    }
    let bx = Rect::from_min_size(pos2(r.left(), (r.center().y - 6.0).round()), vec2(12.0, 12.0));
    if *value {
        ui.painter().rect_filled(bx, 2.0, t().accent_checked);
    }
    widgets::check_box_shape(ui, ui.painter(), bx, *value);
    ui.painter().galley(pos2(r.left() + 19.0, r.center().y - g.size().y / 2.0), g, t().text);
    let l = label.to_string();
    let v = *value;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, v, l.clone()));
    let _ = i;
    resp
}
