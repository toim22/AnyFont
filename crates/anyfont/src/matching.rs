//! PANOSE font matching dialogs and the Type 1 replacement message (owned by the services UI developer).
//! - Exceptions dialog (from Options ▸ Font Matching ▸ Exceptions...) – `b3d42473`, UI-SPEC §7.2.18
//! - Font Matching Results dialog (after importing folders/collections that reference missing fonts) –
//!   `b12fbfa5`, §7.2.19
//! - Type 1 information message ("Replace fonts") – `3a3106a3`, §7.2.20, with "These fonts" for
//!   "This document" (§13.8)
//!
//! The logic is `anyfont_core::panose` (matching, Type 1 replacements) and the PANOSE tables of the font
//! database (exceptions, permanent substitutions). Nothing is written before OK.

use crate::help::dlg;
use anyfont_core::catalog::Catalog;
use anyfont_core::db::FontDatabase;
use anyfont_core::model::{FontId, FontRecord, Technology};
use anyfont_core::panose::{self, FontFilterType, MatchReason, MatchResult, MissingFont, PanoseException, PanoseMode, Substitution, SubstitutionStatus, SubstitutionType};
use egui::{pos2, vec2, Align2, Color32, Rect, Sense};

/// The Type 1 message text with "These fonts" in place of the dossier's "This document" (§13.8).
pub const TYPE1_TEXT: &str = "These fonts use Type 1 fonts, which may not be supported in the future. To avoid potential issues, consider updating the text to use different fonts, such as TrueType (TTF) or OpenType (OTF) fonts.";

/// Database property set by the Type 1 message's "Do not show again".
pub const TYPE1_HIDDEN_PROPERTY: &str = "type1_message_hidden";

/// True when the user asked not to see the Type 1 message again.
pub fn type1_message_suppressed(db: &FontDatabase) -> bool {
    db.property(TYPE1_HIDDEN_PROPERTY).ok().flatten().as_deref() == Some("1")
}

// ------------------------------------------------------------------------------------------------
// helpers
// ------------------------------------------------------------------------------------------------

const STYLE_WORDS: [&str; 23] = [
    "regular", "normal", "book", "italic", "oblique", "bold", "light", "thin", "hairline", "medium", "semibold", "demibold", "demi", "extrabold", "ultrabold", "black", "heavy", "extralight", "ultralight", "condensed", "narrow", "semilight", "extrablack",
];

fn weight_of(style: &str) -> u16 {
    let s = style.to_lowercase().replace([' ', '-'], "");
    for (w, n) in [("extralight", 200), ("ultralight", 200), ("semilight", 350), ("hairline", 100), ("thin", 100), ("light", 300), ("semibold", 600), ("demibold", 600), ("extrabold", 800), ("ultrabold", 800), ("extrablack", 950), ("medium", 500), ("bold", 700), ("black", 900), ("heavy", 900)] {
        if s.contains(w) {
            return n;
        }
    }
    400
}

/// A missing font from its name as an import reports it: "Family", "Family Bold Italic",
/// "Family (Bold Italic)", "Family, Bold" or a PostScript name ("Arial-BoldMT").
pub fn missing_from_name(name: &str) -> MissingFont {
    let name = name.trim();
    let (family, style) = if let Some(open) = name.rfind(" (").filter(|_| name.ends_with(')')) {
        (name[..open].trim().to_string(), name[open + 2..name.len() - 1].trim().to_string())
    } else if let Some((f, s)) = name.split_once(", ") {
        (f.trim().to_string(), s.trim().to_string())
    } else if !name.contains(' ') && name.contains('-') {
        let (f, s) = name.split_once('-').unwrap_or((name, ""));
        let s = s.trim_end_matches("MT").trim_end_matches("PS");
        let mut m = MissingFont { family: f.to_string(), style: s.to_string(), postscript_name: name.to_string(), weight: weight_of(s), italic: s.to_lowercase().contains("italic") || s.to_lowercase().contains("oblique"), ..Default::default() };
        if m.style.is_empty() {
            m.style = "Regular".into();
        }
        return m;
    } else {
        let words: Vec<&str> = name.split_whitespace().collect();
        let mut cut = words.len();
        while cut > 1 && STYLE_WORDS.contains(&words[cut - 1].to_lowercase().as_str()) {
            cut -= 1;
        }
        (words[..cut].join(" "), words[cut..].join(" "))
    };
    let lower = style.to_lowercase();
    MissingFont { weight: weight_of(&style), italic: lower.contains("italic") || lower.contains("oblique"), family, style, ..Default::default() }
}

/// The missing font a Type 1 face stands for (its family, style and PANOSE).
fn missing_from_font(f: &FontRecord) -> MissingFont {
    MissingFont {
        family: f.family.clone(),
        style: f.subfamily.clone(),
        postscript_name: f.postscript_name.clone(),
        weight: f.weight_class,
        italic: f.italic,
        panose: (!f.panose.is_empty()).then_some(f.panose),
        charset: None,
        technology: Some(f.technology),
    }
}

/// The families of the usable fonts, sorted, without repeats (the font combo boxes).
pub fn families(fonts: &[FontRecord]) -> Vec<String> {
    let mut v: Vec<String> = fonts.iter().filter(|f| f.is_usable()).map(|f| f.group_family().to_string()).filter(|s| !s.is_empty()).collect();
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// The face of `family` closest to the missing font's weight and slant.
fn best_face(fonts: &[FontRecord], family: &str, m: &MissingFont) -> Option<FontId> {
    fonts
        .iter()
        .filter(|f| f.is_usable() && f.technology != Technology::Type1 && f.group_family().eq_ignore_ascii_case(family))
        .min_by_key(|f| (f.italic != m.italic) as u32 * 1000 + u32::from(f.weight_class.abs_diff(if m.weight == 0 { 400 } else { m.weight })))
        .map(|f| f.id)
}

/// A combo box of font families with a filter box at the top of its list.
fn font_combo(ui: &mut egui::Ui, id: &str, name: &str, value: &mut String, list: &[String], width: f32, enabled: bool) -> bool {
    let mut changed = false;
    let filter_id = egui::Id::unique((id, "filter"));
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(id).width(width).selected_text(egui::text::LayoutJob::simple_singleline(value.clone(), dlg::body_font(ui), dlg::text_color(ui))).height(320.0).show_ui(ui, |ui| {
            let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search").desired_width(width - 16.0)).accessible_name("Search").request_focus();
            let q = filter.to_lowercase();
            for f in list.iter().filter(|f| q.is_empty() || f.to_lowercase().contains(&q)).take(400) {
                let job = egui::text::LayoutJob::simple_singleline(f.clone(), dlg::body_font(ui), dlg::text_color(ui));
                if ui.selectable_label(value == f, job).clicked() {
                    *value = f.clone();
                    changed = true;
                }
            }
            ui.data_mut(|d| d.insert_temp(filter_id, filter));
        })
        .response
        .accessible_name(name);
    });
    changed
}

/// A header row and the rows of a simple bordered table; returns the clicked row.
fn table(ui: &mut egui::Ui, id: &str, rect: Rect, headers: &[&str], widths: &[f32], rows: &[Vec<String>], selected: Option<usize>) -> Option<usize> {
    dlg::frame_rect(ui, rect);
    let text = dlg::text_color(ui);
    let f = dlg::font(12.0);
    let head = Rect::from_min_size(rect.min + vec2(1.0, 1.0), vec2(rect.width() - 2.0, 22.0));
    let dark = ui.visuals().dark_mode;
    ui.painter().rect_filled(head, 0.0, if dark { Color32::from_gray(0x33) } else { Color32::from_gray(0xE6) });
    let mut x = head.left() + 6.0;
    for (h, w) in headers.iter().zip(widths) {
        ui.painter().text(pos2(x, head.center().y), Align2::LEFT_CENTER, *h, f.clone(), text);
        x += w;
    }
    let mut clicked = None;
    dlg::at(ui, Rect::from_min_max(pos2(rect.left() + 1.0, head.bottom()), rect.max - vec2(1.0, 1.0)), |ui| {
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, row) in rows.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
                if selected == Some(i) {
                    ui.painter().rect_filled(r, 0.0, if dark { Color32::from_gray(0x52) } else { Color32::from_rgb(0xCC, 0xE4, 0xF7) });
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, if dark { Color32::from_gray(0x3A) } else { Color32::from_gray(0xEE) });
                }
                let mut x = r.left() + 5.0;
                for (cell, w) in row.iter().zip(widths) {
                    // cells are data (font names): not translated, except the type column (an interface word)
                    let clip = Rect::from_min_max(pos2(x, r.top()), pos2((x + w - 6.0).min(r.right()), r.bottom()));
                    let g = ui.painter().layout_job(egui::text::LayoutJob::simple_singleline(cell.clone(), f.clone(), text));
                    ui.painter().with_clip_rect(clip).galley(pos2(x, r.center().y - g.size().y / 2.0), g, text);
                    x += w;
                }
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
    });
    clicked
}

// ------------------------------------------------------------------------------------------------
// Exceptions (b3d42473)
// ------------------------------------------------------------------------------------------------

struct ExceptionsDlg {
    loaded: bool,
    original: Vec<PanoseException>,
    list: Vec<PanoseException>,
    missing: String,
    substitute: String,
    selected: Option<usize>,
}

/// The exception list after Add Exception: one entry per missing family (any case), sorted.
pub fn add_exception(list: &mut Vec<PanoseException>, missing: &str, substitute: &str) {
    let missing = missing.trim();
    list.retain(|e| !panose::normalize_name(&e.missing).eq(&panose::normalize_name(missing)));
    list.push(PanoseException { missing: missing.to_string(), substitute: substitute.trim().to_string() });
    list.sort_by_key(|e| e.missing.to_lowercase());
}

/// What OK writes: (exceptions to remove, exceptions to set).
pub fn exception_changes(original: &[PanoseException], list: &[PanoseException]) -> (Vec<String>, Vec<PanoseException>) {
    let removed = original.iter().filter(|o| !list.iter().any(|e| e.missing == o.missing)).map(|o| o.missing.clone()).collect();
    let set = list.iter().filter(|e| !original.contains(e)).cloned().collect();
    (removed, set)
}

// ------------------------------------------------------------------------------------------------
// Results (b12fbfa5)
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    /// Fonts referenced by an import but not present.
    Missing,
    /// Type 1 faces to replace in the collections ("Replace fonts").
    Type1,
}

struct Row {
    missing: MissingFont,
    label: String,
    /// The Type 1 face this row replaces (Purpose::Type1).
    type1: Option<FontId>,
    result: Option<MatchResult>,
    /// The suggested substitute family.
    suggested: Option<String>,
    use_suggested: bool,
    /// "Substitute font with:"
    chosen: String,
    filter: FontFilterType,
    save_exception: bool,
    status: SubstitutionStatus,
}

impl Row {
    fn substitute_family(&self) -> Option<String> {
        if self.use_suggested { self.suggested.clone() } else { Some(self.chosen.clone()).filter(|s| !s.is_empty()) }
    }

    fn kind(&self) -> SubstitutionType {
        match (&self.result, self.use_suggested) {
            (Some(r), true) => r.kind,
            _ => SubstitutionType::Exception,
        }
    }
}

struct ResultsDlg {
    purpose: Purpose,
    rows: Vec<Row>,
    /// Rows matched so far (the matching runs a few rows per frame so Stop can interrupt it).
    next: usize,
    loaded: bool,
    exceptions: Vec<PanoseException>,
    substitutions: Vec<Substitution>,
    /// The fonts searched (Type 1 faces left out when replacing them).
    fonts: Vec<FontRecord>,
    families: Vec<String>,
    selected: usize,
}

/// One resolved missing font, after OK in the results dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    /// The missing font as given to `open_results` (or the Type 1 face's name).
    pub missing: String,
    /// The substitute family and face, `None` when nothing was chosen.
    pub family: Option<String>,
    pub font: Option<FontId>,
    pub status: SubstitutionStatus,
}

// ------------------------------------------------------------------------------------------------
// the dialogs
// ------------------------------------------------------------------------------------------------

struct Type1Msg {
    fonts: Vec<FontId>,
    do_not_show: bool,
    checked: bool,
}

pub struct Matching {
    exceptions: Option<ExceptionsDlg>,
    results: Option<ResultsDlg>,
    type1: Option<Type1Msg>,
    default_font: String,
    resolved: Option<Vec<Resolved>>,
    families_cache: Option<(usize, Vec<String>)>,
}

impl Default for Matching {
    fn default() -> Self {
        Self::new()
    }
}

impl Matching {
    pub fn new() -> Self {
        Matching { exceptions: None, results: None, type1: None, default_font: "Arial".into(), resolved: None, families_cache: None }
    }

    /// The default font of the PANOSE preferences (`Settings::panose.default_font`), used when nothing matches.
    pub fn set_default_font(&mut self, family: &str) {
        self.default_font = family.to_string();
    }

    pub fn open_exceptions(&mut self) {
        self.exceptions = Some(ExceptionsDlg { loaded: false, original: vec![], list: vec![], missing: String::new(), substitute: String::new(), selected: None });
    }

    /// Missing font names (family + style) to resolve with PANOSE.
    pub fn open_results(&mut self, missing: Vec<String>) {
        self.open_results_for(missing.iter().map(|m| (m.clone(), missing_from_name(m))).collect());
    }

    /// [`Self::open_results`] with the details an import knows (PANOSE digits, character set, PostScript
    /// name): (name shown in the list, missing font).
    pub fn open_results_for(&mut self, missing: Vec<(String, MissingFont)>) {
        if missing.is_empty() {
            return;
        }
        let rows = missing.into_iter().map(|(label, m)| new_row(label, m, None)).collect();
        self.results = Some(ResultsDlg { purpose: Purpose::Missing, rows, next: 0, loaded: false, exceptions: vec![], substitutions: vec![], fonts: vec![], families: vec![], selected: 0 });
    }

    /// Type 1 fonts found in the current selection/library.
    pub fn open_type1_message(&mut self, font_ids: Vec<anyfont_core::model::FontId>) {
        if !font_ids.is_empty() {
            self.type1 = Some(Type1Msg { fonts: font_ids, do_not_show: false, checked: false });
        }
    }

    /// True while one of the dialogs is open.
    pub fn is_open(&self) -> bool {
        self.exceptions.is_some() || self.results.is_some() || self.type1.is_some()
    }

    /// The choices of the last results dialog closed with OK (once): what an import links instead of the
    /// missing fonts. `None` when there is nothing new.
    pub fn take_resolved(&mut self) -> Option<Vec<Resolved>> {
        self.resolved.take()
    }

    /// Options ▸ Font Matching (§11 #13, §13.8): the three modes, the default font and Exceptions...
    /// For the shell's Options dialog; returns true when `prefs` changed (the shell saves its settings).
    pub fn preferences_ui(&mut self, ui: &mut egui::Ui, prefs: &mut anyfont_core::settings::PanoseSettings, catalog: &Catalog) -> bool {
        let mut changed = false;
        for m in [PanoseMode::Text, PanoseMode::TextAndStyles, PanoseMode::NeverShow] {
            if ui.radio(prefs.mode == m, m.caption()).clicked() && prefs.mode != m {
                prefs.mode = m;
                changed = true;
            }
        }
        let list = self.families(catalog).to_vec();
        ui.horizontal(|ui| {
            ui.label("Default font:");
            if font_combo(ui, "panose_default_font", "Default font:", &mut prefs.default_font, &list, 220.0, true) {
                changed = true;
            }
        });
        if dlg::button(ui, "Exceptions...", true) {
            self.open_exceptions();
        }
        self.default_font = prefs.default_font.clone();
        changed
    }

    fn families(&mut self, catalog: &Catalog) -> &[String] {
        let n = catalog.fonts().len();
        if self.families_cache.as_ref().map(|c| c.0) != Some(n) {
            self.families_cache = Some((n, families(catalog.fonts())));
        }
        &self.families_cache.as_ref().expect("set above").1
    }

    /// Returns true when the database changed (substitutions/exceptions saved) so the caller reloads its catalog.
    pub fn ui(&mut self, ctx: &egui::Context, db: &mut FontDatabase, catalog: &Catalog) -> bool {
        let mut changed = false;
        if self.type1.is_some() {
            self.type1_ui(ctx, db, catalog);
        }
        if self.results.is_some() {
            changed |= self.results_ui(ctx, db, catalog);
        }
        if self.exceptions.is_some() {
            changed |= self.exceptions_ui(ctx, db, catalog);
        }
        changed
    }

    // -- Exceptions ------------------------------------------------------------------------------

    fn exceptions_ui(&mut self, ctx: &egui::Context, db: &mut FontDatabase, catalog: &Catalog) -> bool {
        let list = self.families(catalog).to_vec();
        let Some(d) = &mut self.exceptions else { return false };
        if !d.loaded {
            d.original = db.panose_exceptions().unwrap_or_default();
            d.list = d.original.clone();
            d.loaded = true;
        }
        let (mut ok, mut cancel) = (false, false);
        let ((), dismissed) = dlg::window(ctx, "panose_exceptions", "PANOSE Font Matching Exceptions", vec2(460.0, 420.0), |ui| {
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at(ui, top, |ui| {
                ui.add_space(6.0);
                egui::Grid::new("exc_grid").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                    ui.label("Missing Font:");
                    let w = ui.available_width();
                    ui.add(egui::TextEdit::singleline(&mut d.missing).desired_width(w)).accessible_name("Missing Font:");
                    ui.end_row();
                    ui.label("Substituted Font:");
                    let w = ui.available_width();
                    font_combo(ui, "exc_substitute", "Substituted Font:", &mut d.substitute, &list, w - 8.0, true);
                    ui.end_row();
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if dlg::button(ui, "Remove Exception", d.selected.is_some()) {
                            if let Some(i) = d.selected.take() {
                                if i < d.list.len() {
                                    d.list.remove(i);
                                }
                            }
                        }
                        let can_add = !d.missing.trim().is_empty() && !d.substitute.trim().is_empty();
                        if dlg::button(ui, "Add Exception", can_add) {
                            add_exception(&mut d.list, &d.missing, &d.substitute);
                            d.selected = d.list.iter().position(|e| e.missing == d.missing.trim());
                            d.missing.clear();
                        }
                    });
                });
                ui.add_space(4.0);
                ui.label("List of current exceptions:");
                let rect = Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
                let rows: Vec<Vec<String>> = d.list.iter().map(|e| vec![e.missing.clone(), e.substitute.clone()]).collect();
                let half = rect.width() / 2.0;
                if let Some(i) = table(ui, "exc_list", rect, &["Missing Font", "Substituted Font"], &[half, half], &rows, d.selected) {
                    d.selected = Some(i);
                    d.missing = d.list[i].missing.clone();
                    d.substitute = d.list[i].substitute.clone();
                }
            });
            dlg::at_left(ui, bar, |ui| {
                if dlg::help_button(ui) {
                    crate::help::open_section("font-matching-exceptions");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = dlg::button(ui, "Cancel", true);
                    ok = dlg::default_button(ui, "OK", true);
                });
            });
        });
        let mut changed = false;
        if ok {
            let (removed, set) = exception_changes(&d.original, &d.list);
            for m in removed {
                changed |= db.remove_panose_exception(&m).is_ok();
            }
            for e in set {
                changed |= db.set_panose_exception(&e).is_ok();
            }
        }
        if ok || cancel || dismissed {
            self.exceptions = None;
        }
        changed
    }

    // -- Results ---------------------------------------------------------------------------------

    fn results_ui(&mut self, ctx: &egui::Context, db: &mut FontDatabase, catalog: &Catalog) -> bool {
        let all_families = self.families(catalog).to_vec();
        let default_font = self.default_font.clone();
        let Some(d) = &mut self.results else { return false };
        if !d.loaded {
            d.exceptions = db.panose_exceptions().unwrap_or_default();
            d.substitutions = db.substitutions().unwrap_or_default();
            d.fonts = match d.purpose {
                Purpose::Missing => catalog.fonts().to_vec(),
                Purpose::Type1 => catalog.fonts().iter().filter(|f| f.technology != Technology::Type1).cloned().collect(),
            };
            d.families = families(&d.fonts);
            d.loaded = true;
        }
        // match a few rows per frame
        let running = d.next < d.rows.len();
        if running {
            let end = (d.next + 4).min(d.rows.len());
            for i in d.next..end {
                let row = &mut d.rows[i];
                let res = panose::find_substitute(&row.missing, &d.fonts, &d.exceptions, &d.substitutions, Some(&default_font), row.filter);
                apply_result(row, res, &d.fonts);
            }
            d.next = end;
            ctx.request_repaint();
        }
        let running = d.next < d.rows.len();
        let mut stop = false;
        let (mut ok, mut cancel) = (false, false);
        let title = "PANOSE Font Matching Results";
        let ((), dismissed) = dlg::window(ctx, "panose_results", title, vec2(560.0, 560.0), |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at(ui, top, |ui| {
                ui.add_space(6.0);
                ui.add(egui::Label::new(match d.purpose {
                    Purpose::Missing => "The fonts below are not available on this computer. AnyFont suggests a substitute for each one. Select a font in the list to choose another substitute.",
                    Purpose::Type1 => "AnyFont suggests an OpenType or TrueType replacement for each Type 1 font. The replacements take the place of the Type 1 fonts in your collections. Select a font in the list to choose another replacement.",
                }).wrap());
                ui.add_space(4.0);
                if running {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label("Working, please wait...");
                        ui.add(egui::ProgressBar::new(d.next as f32 / d.rows.len().max(1) as f32).desired_width(300.0));
                        stop = dlg::button(ui, "Stop", true);
                        ui.add_space(6.0);
                        dlg::note(ui, "Stopping will end the PANOSE matching process. All missing fonts will revert to a default selection.");
                    });
                    return;
                }
                // the table
                let table_h = (ui.available_height() - 200.0).max(110.0);
                let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), table_h));
                let rows: Vec<Vec<String>> = d
                    .rows
                    .iter()
                    .map(|r| {
                        let kind = match r.kind() {
                            SubstitutionType::Panose => "PANOSE",
                            SubstitutionType::Exception => "Exception",
                        };
                        vec![r.label.clone(), r.substitute_family().unwrap_or_default(), kind.to_string()]
                    })
                    .collect();
                let w = rect.width();
                if let Some(i) = table(ui, "results_list", rect, &["Missing Font", "Substituted Font", "Type"], &[w * 0.4, w * 0.4, w * 0.2], &rows, Some(d.selected)) {
                    d.selected = i;
                }
                ui.advance_cursor_after_rect(rect);
                ui.add_space(8.0);
                // the selected row
                let sel = d.selected.min(d.rows.len() - 1);
                let row = &mut d.rows[sel];
                let suggested = row.suggested.clone().unwrap_or_default();
                if ui.radio(row.use_suggested, "Use the PANOSE suggested match").clicked() {
                    row.use_suggested = true;
                }
                ui.horizontal(|ui| {
                    if ui.radio(!row.use_suggested, "Substitute font with:").clicked() {
                        row.use_suggested = false;
                        if row.chosen.is_empty() {
                            row.chosen = suggested.clone();
                        }
                    }
                    // Same code page: the candidates of the match; Show all: every family
                    let list: Vec<String> = match (row.filter, &row.result) {
                        (FontFilterType::SameCodePage, Some(r)) => {
                            let mut v: Vec<String> = r.candidates.iter().map(|c| c.family.clone()).collect();
                            v.sort_by_key(|s| s.to_lowercase());
                            v.dedup();
                            v
                        }
                        _ => d.families.clone(),
                    };
                    let list = if list.is_empty() { all_families.clone() } else { list };
                    if font_combo(ui, "results_substitute", "Substitute font with:", &mut row.chosen, &list, 240.0, !row.use_suggested) {
                        row.use_suggested = false;
                    }
                });
                ui.indent("results_opts", |ui| {
                    let edit = !row.use_suggested;
                    ui.add_enabled_ui(edit, |ui| {
                        if ui.radio(row.filter == FontFilterType::SameCodePage, "Show fonts with the same code page").clicked() {
                            row.filter = FontFilterType::SameCodePage;
                        }
                        if ui.radio(row.filter == FontFilterType::ShowAll, "Show all fonts").clicked() {
                            row.filter = FontFilterType::ShowAll;
                        }
                        ui.checkbox(&mut row.save_exception, "Save this exception for future font matching");
                    });
                    if d.purpose == Purpose::Missing {
                        ui.label("Make this substitution:");
                        if ui.radio(row.status == SubstitutionStatus::Permanent, "Permanent - keep these fonts").clicked() {
                            row.status = SubstitutionStatus::Permanent;
                        }
                        if ui.radio(row.status == SubstitutionStatus::Temporary, "Temporary - use original fonts").clicked() {
                            row.status = SubstitutionStatus::Temporary;
                        }
                    }
                });
            });
            dlg::at_left(ui, bar, |ui| {
                if dlg::help_button(ui) {
                    crate::help::open_section(match d.purpose {
                        Purpose::Missing => "font-matching-results",
                        Purpose::Type1 => "type-1-fonts",
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = dlg::button(ui, "Cancel", true);
                    ok = dlg::default_button(ui, "OK", !running);
                });
            });
        });
        if stop {
            // the rest falls back to the default font
            let fallback = best_face(&d.fonts, &default_font, &MissingFont::default()).map(|_| default_font.clone());
            for row in &mut d.rows[d.next..] {
                row.suggested = fallback.clone();
                row.result = Some(MatchResult { substitute: None, reason: MatchReason::DefaultFont, kind: SubstitutionType::Panose, candidates: vec![] });
            }
            d.next = d.rows.len();
        }
        let mut changed = false;
        if ok {
            let mut resolved = vec![];
            for row in &d.rows {
                let family = row.substitute_family();
                let font = match (&family, row.use_suggested, &row.result) {
                    (Some(_), true, Some(r)) if r.substitute.is_some() => r.substitute,
                    (Some(f), _, _) => best_face(&d.fonts, f, &row.missing),
                    _ => None,
                };
                if let Some(fam) = &family {
                    if row.save_exception {
                        changed |= db.set_panose_exception(&PanoseException { missing: row.missing.family.clone(), substitute: fam.clone() }).is_ok();
                    }
                    if d.purpose == Purpose::Missing && row.status == SubstitutionStatus::Permanent {
                        changed |= db.set_substitution(&Substitution { missing: row.missing.family.clone(), substitute: fam.clone(), kind: row.kind(), status: SubstitutionStatus::Permanent }).is_ok();
                    }
                }
                // Replace fonts: the replacement takes the Type 1 face's place in its collections
                if let (Purpose::Type1, Some(t1), Some(new)) = (d.purpose, row.type1, font) {
                    if let (Some(old), Some(newf)) = (catalog.font(t1), catalog.font(new)) {
                        for c in &old.collections {
                            if db.add_to_collection(*c, std::slice::from_ref(&newf.key)).is_ok() && db.remove_from_collection(*c, std::slice::from_ref(&old.key)).is_ok() {
                                changed = true;
                            }
                        }
                    }
                }
                resolved.push(Resolved { missing: row.label.clone(), family, font, status: if d.purpose == Purpose::Type1 { SubstitutionStatus::Permanent } else { row.status } });
            }
            self.resolved = Some(resolved);
        }
        if ok || cancel || (dismissed && !running) {
            self.results = None;
        }
        changed
    }

    // -- Type 1 message --------------------------------------------------------------------------

    fn type1_ui(&mut self, ctx: &egui::Context, db: &mut FontDatabase, catalog: &Catalog) {
        let Some(m) = &mut self.type1 else { return };
        if !m.checked {
            m.checked = true;
            if type1_message_suppressed(db) {
                self.type1 = None;
                return;
            }
        }
        let mut replace = false;
        let ((), dismissed) = dlg::window(ctx, "type1_message", "AnyFont", vec2(480.0, 220.0), |ui| {
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at(ui, top, |ui| {
                ui.add_space(10.0);
                ui.horizontal_top(|ui| {
                    // the information icon
                    let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 14.0, dlg::ACCENT);
                    ui.painter().text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, "i", egui::FontId::proportional(20.0), Color32::WHITE);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Type 1 fonts detected").strong().size(14.0));
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(TYPE1_TEXT).wrap());
                    });
                });
            });
            dlg::at_left(ui, bar, |ui| {
                if dlg::help_button(ui) {
                    crate::help::open_section("type-1-fonts");
                }
                if ui.checkbox(&mut m.do_not_show, "Do not show again").changed() {
                    let _ = db.set_property(TYPE1_HIDDEN_PROPERTY, if m.do_not_show { "1" } else { "0" });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    replace = dlg::default_button(ui, "Replace fonts", true);
                });
            });
        });
        if replace {
            let ids = m.fonts.clone();
            self.type1 = None;
            self.open_type1_replacements(&ids, catalog);
        } else if dismissed {
            self.type1 = None;
        }
    }

    /// "Replace fonts": the results dialog with the Type 1 faces and their suggested replacements.
    fn open_type1_replacements(&mut self, ids: &[FontId], catalog: &Catalog) {
        let suggestions = panose::type1_replacements(catalog.fonts());
        let mut rows = vec![];
        for id in ids {
            let Some(f) = catalog.font(*id) else { continue };
            let label = format!("{} ({})", f.family, f.subfamily).replace(" ()", "");
            let mut row = new_row(label, missing_from_font(f), Some(*id));
            if let Some(Some(rep)) = suggestions.iter().find(|s| s.type1 == *id).map(|s| s.replacement) {
                row.suggested = catalog.font(rep).map(|r| r.group_family().to_string());
            }
            rows.push(row);
        }
        if !rows.is_empty() {
            self.results = Some(ResultsDlg { purpose: Purpose::Type1, rows, next: 0, loaded: false, exceptions: vec![], substitutions: vec![], fonts: vec![], families: vec![], selected: 0 });
        }
    }
}

fn new_row(label: String, missing: MissingFont, type1: Option<FontId>) -> Row {
    Row { missing, label, type1, result: None, suggested: None, use_suggested: true, chosen: String::new(), filter: FontFilterType::SameCodePage, save_exception: false, status: SubstitutionStatus::Temporary }
}

/// Takes a match into a row; a Type 1 row keeps the replacement `type1_replacements` suggested.
fn apply_result(row: &mut Row, res: MatchResult, fonts: &[FontRecord]) {
    let fam = res.substitute.and_then(|id| fonts.iter().find(|f| f.id == id)).map(|f| f.group_family().to_string());
    if row.suggested.is_none() || row.type1.is_none() {
        row.suggested = fam;
    }
    row.result = Some(res);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_into_family_and_style() {
        let m = missing_from_name("Arial Bold Italic");
        assert_eq!((m.family.as_str(), m.style.as_str(), m.weight, m.italic), ("Arial", "Bold Italic", 700, true));
        let m = missing_from_name("Futura (Light)");
        assert_eq!((m.family.as_str(), m.style.as_str(), m.weight), ("Futura", "Light", 300));
        let m = missing_from_name("Times New Roman");
        assert_eq!((m.family.as_str(), m.style.as_str(), m.weight), ("Times New Roman", "", 400));
        let m = missing_from_name("Arial-BoldMT");
        assert_eq!((m.family.as_str(), m.postscript_name.as_str(), m.weight), ("Arial", "Arial-BoldMT", 700));
        let m = missing_from_name("Gill Sans, SemiBold");
        assert_eq!((m.family.as_str(), m.weight), ("Gill Sans", 600));
        // a single word stays a family even if it is a style word
        assert_eq!(missing_from_name("Black").family, "Black");
    }

    #[test]
    fn exceptions_add_replace_and_diff() {
        let mut list = vec![PanoseException { missing: "Helvetica".into(), substitute: "Arial".into() }];
        let original = list.clone();
        add_exception(&mut list, " helvetica ", "Liberation Sans");
        add_exception(&mut list, "Futura", "Century Gothic");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].missing, "Futura");
        assert_eq!(list[1].substitute, "Liberation Sans");
        let (removed, set) = exception_changes(&original, &list);
        assert_eq!(removed, vec!["Helvetica".to_string()]);
        assert_eq!(set.len(), 2);
        let (removed, set) = exception_changes(&list, &list);
        assert!(removed.is_empty() && set.is_empty());
    }

    #[test]
    fn type1_text_says_these_fonts() {
        assert!(TYPE1_TEXT.starts_with("These fonts"));
        assert!(!TYPE1_TEXT.contains("This document"));
    }

    fn font(id: i64, family: &str, weight: u16, italic: bool, tech: Technology) -> FontRecord {
        FontRecord { id, family: family.into(), weight_class: weight, italic, technology: tech, paths: vec![format!("C:/f/{id}.ttf").into()], key: anyfont_core::model::FontKey(format!("k{id}")), ..Default::default() }
    }

    #[test]
    fn families_and_faces() {
        let fonts = vec![font(1, "Arial", 400, false, Technology::TrueType), font(2, "Arial", 700, false, Technology::TrueType), font(3, "arial", 400, true, Technology::TrueType), font(4, "Zapf", 400, false, Technology::Type1)];
        assert_eq!(families(&fonts), vec!["Arial".to_string(), "Zapf".to_string()]);
        let bold = MissingFont { weight: 700, ..Default::default() };
        assert_eq!(best_face(&fonts, "Arial", &bold), Some(2));
        assert_eq!(best_face(&fonts, "Zapf", &bold), None, "a Type 1 face is never a replacement");
    }

    /// The whole Exceptions/Results flow against an in-memory database.
    #[test]
    fn results_ok_writes_exceptions_and_permanent_substitutions() {
        let db = FontDatabase::open_in_memory().unwrap();
        let catalog = Catalog::load(&db).unwrap();
        let mut m = Matching::new();
        m.open_results(vec!["Helvetica Bold".into()]);
        let d = m.results.as_mut().unwrap();
        d.loaded = true;
        d.next = 1;
        let row = &mut d.rows[0];
        row.use_suggested = false;
        row.chosen = "Arial".into();
        row.save_exception = true;
        row.status = SubstitutionStatus::Permanent;
        // what OK does, without a window
        let row = &d.rows[0];
        let fam = row.substitute_family().unwrap();
        db.set_panose_exception(&PanoseException { missing: row.missing.family.clone(), substitute: fam.clone() }).unwrap();
        db.set_substitution(&Substitution { missing: row.missing.family.clone(), substitute: fam, kind: row.kind(), status: SubstitutionStatus::Permanent }).unwrap();
        assert_eq!(db.panose_exceptions().unwrap()[0].missing, "Helvetica");
        assert_eq!(db.substitutions().unwrap()[0].kind, SubstitutionType::Exception);
        let _ = catalog;
        assert!(!type1_message_suppressed(&db));
        db.set_property(TYPE1_HIDDEN_PROPERTY, "1").unwrap();
        assert!(type1_message_suppressed(&db));
    }
}
