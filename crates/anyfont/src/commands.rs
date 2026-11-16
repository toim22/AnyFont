//! Every command of the main window (menus, toolbar, context menus, shortcuts), the menus' items with
//! their enable rules (UI-SPEC §2, §6) and the dispatcher.

use crate::app::AnyFontApp;
use crate::dialogs::{self, Dialog};
use crate::menus::{Item, Source};
use crate::ops::OpKind;
use anyfont_core::catalog::LibraryRef;
use anyfont_core::model::{CollectionId, FontId};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    AddFolder,
    AddCollection,
    GetMore,
    RemoveLibrary(LibraryRef),
    Import,
    Export,
    Exit,
    SelectAll,
    DeselectAll,
    Install,
    Uninstall,
    Download,
    Delete,
    AddToCollection(CollectionId),
    RemoveFromOpenCollection,
    RemoveFromCollection(CollectionId),
    ToggleDuplicates,
    OpenLocation(PathBuf),
    Refresh,
    GroupByFamily,
    ShowRecent,
    RenderInFont,
    LatinNames,
    ToggleFilterList,
    ToggleProperties,
    ProductHelp,
    CheckUpdates,
    Feedback,
    ReportBug,
    About,
    Options,
    /// Ctrl+Alt+I: the "Inspectors" of the original = the font properties panel (UI-SPEC §8).
    Inspectors,
    /// Ctrl+Alt+Shift+I: egui's own inspection window (a debugging aid).
    DebugInspector,
    LibraryInstall(LibraryRef),
    LibraryUninstall(LibraryRef),
    RenameLibrary(CollectionId),
    Edit(EditTarget, EditCmd),
}

/// The text boxes with the edit context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditTarget {
    Sample,
    Search,
    Rename,
}

impl EditTarget {
    pub fn id(self) -> egui::Id {
        match self {
            EditTarget::Sample => egui::Id::unique("anyfont-sample-edit"),
            EditTarget::Search => egui::Id::unique("anyfont-search-edit"),
            EditTarget::Rename => egui::Id::unique("anyfont-rename-edit"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCmd {
    Undo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
}

/// A collection's caption ("Favorites" is a UI string, the others are data).
pub(crate) fn collection_caption(c: &anyfont_core::db::Collection) -> String {
    if c.is_favorites {
        crate::tr("Favorites")
    } else {
        c.name.clone()
    }
}

/// Middle-ellipsizes `text` to at most `max_chars` characters.
pub(crate) fn middle_ellipsis(text: &str, max_chars: usize) -> String {
    let n = text.chars().count();
    if n <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let head: String = text.chars().take(keep / 2).collect();
    let tail: String = text.chars().skip(n - (keep - keep / 2)).collect();
    format!("{head}...{tail}")
}

impl AnyFontApp {
    /// The items of a menu, from the current state.
    pub(crate) fn menu_items(&self, source: &Source) -> Vec<Item> {
        match source {
            Source::Bar(0) => self.file_menu(),
            Source::Bar(1) => self.edit_menu(),
            Source::Bar(2) | Source::Font => self.font_menu(),
            Source::Bar(3) => self.view_menu(),
            Source::Bar(4) => self.help_menu(),
            Source::Bar(_) => Vec::new(),
            Source::Library(lib) => self.library_menu(*lib),
            Source::Edit(t) => self.edit_box_menu(*t),
        }
    }

    fn file_menu(&self) -> Vec<Item> {
        // the hidden Content folder is AnyFont's, not the user's: it is never exported (BUG-303)
        let exportable = anyfont_core::exchange::has_exportable_except(&self.db, std::slice::from_ref(&self.content_dir)).unwrap_or(false);
        vec![
            Item::cmd("Add Folder...", Cmd::AddFolder).icon("add-folder"),
            Item::cmd("Add Collection", Cmd::AddCollection).icon("add-collection"),
            Item::cmd("Get More...", Cmd::GetMore).icon("get-more"),
            Item::Sep,
            Item::cmd("Remove Folder or Collection", Cmd::RemoveLibrary(self.library)).icon("remove").enabled(self.library != LibraryRef::MyFonts),
            Item::Sep,
            Item::cmd("Import Folders and Collections...", Cmd::Import).icon("import").enabled(self.db_ready),
            Item::cmd("Export Folders and Collections...", Cmd::Export).icon("export").enabled(exportable),
            Item::Sep,
            Item::cmd("E&xit", Cmd::Exit).icon("exit").shortcut("Alt+F4"),
        ]
    }

    fn edit_menu(&self) -> Vec<Item> {
        let e = self.eligibility();
        vec![
            Item::cmd("Select All", Cmd::SelectAll).shortcut("Ctrl+A").enabled(!self.result.font_ids.is_empty()),
            Item::cmd("Deselect All", Cmd::DeselectAll).shortcut("Ctrl+Shift+A").enabled(e.deselect_all),
        ]
    }

    /// The Font menu = the font row context menu (UI-SPEC §2.6, §6.1).
    fn font_menu(&self) -> Vec<Item> {
        let e = self.eligibility();
        let targets = self.targets();
        let mut cols: Vec<&anyfont_core::db::Collection> =
            self.catalog.collections().iter().filter(|c| self.library != LibraryRef::Collection(c.id)).collect();
        cols.sort_by_key(|c| collection_caption(c).to_lowercase());
        let add: Vec<Item> = cols.iter().map(|c| Item::data(&collection_caption(c), Cmd::AddToCollection(c.id))).collect();
        let mut items = vec![
            Item::cmd("Install", Cmd::Install).icon("install").enabled(e.menu_install),
            Item::cmd("Uninstall", Cmd::Uninstall).icon("uninstall").enabled(e.menu_uninstall),
            Item::cmd("Download", Cmd::Download).icon("download").enabled(e.menu_download),
            Item::Sep,
            Item::cmd("Delete", Cmd::Delete).icon("delete").enabled(e.menu_delete),
            Item::Sep,
            Item::sub("Add to collection", add).enabled(e.add_to_collection),
        ];
        match self.library {
            LibraryRef::Collection(id) => {
                let name = self.catalog.collection(id).map(collection_caption).unwrap_or_default();
                items.push(Item::data(&crate::fmt("Remove from {}", &[&name]), Cmd::RemoveFromOpenCollection).enabled(e.remove_from_open_collection));
            }
            _ => {
                let mut member: Vec<&anyfont_core::db::Collection> = self
                    .catalog
                    .collections()
                    .iter()
                    .filter(|c| targets.iter().any(|id| self.catalog.font(*id).map(|f| f.collections.contains(&c.id)).unwrap_or(false)))
                    .collect();
                member.sort_by_key(|c| collection_caption(c).to_lowercase());
                let sub: Vec<Item> = member.iter().map(|c| Item::data(&collection_caption(c), Cmd::RemoveFromCollection(c.id))).collect();
                items.push(Item::sub("Remove from collection", sub).enabled(e.remove_from_collection));
            }
        }
        let dup = self.settings.core.show_duplicates;
        items.push(Item::Sep);
        items.push(Item::cmd(if dup { "Hide duplicate fonts" } else { "Show duplicate fonts" }, Cmd::ToggleDuplicates).icon("show-duplicates").checked(dup));
        items.push(Item::Sep);
        let files: Vec<Item> = if e.open_file_location { self.location_files().into_iter().map(|p| Item::data(&p.0, Cmd::OpenLocation(p.1))).collect() } else { Vec::new() };
        items.push(Item::sub("Open font file location", files).enabled(e.open_file_location));
        items
    }

    /// The files of "Open font file location ▸": lower-case full paths, middle-ellipsized (~400 px).
    fn location_files(&self) -> Vec<(String, PathBuf)> {
        let mut out: Vec<(String, PathBuf)> = Vec::new();
        for id in self.targets() {
            let Some(f) = self.catalog.font(id) else { continue };
            let mut fonts = vec![f];
            if self.settings.core.show_duplicates {
                fonts.extend(self.catalog.fonts().iter().filter(|x| x.id != f.id && x.sha == f.sha && !x.sha.is_empty()));
            }
            for f in fonts {
                for p in &f.paths {
                    let label = middle_ellipsis(&p.display().to_string().to_lowercase(), 64);
                    if !out.iter().any(|(_, q)| q == p) {
                        out.push((label, p.clone()));
                    }
                }
            }
        }
        out
    }

    fn view_menu(&self) -> Vec<Item> {
        let c = &self.settings.core;
        vec![
            Item::cmd("Refresh the Font List", Cmd::Refresh).icon("refresh").enabled(self.refresh_needed()),
            Item::Sep,
            Item::cmd("Group by family", Cmd::GroupByFamily).checked(c.group_by_family),
            Item::cmd("Show recently used fonts", Cmd::ShowRecent).checked(c.show_recent),
            Item::cmd("Use font to display font name", Cmd::RenderInFont).checked(c.render_name_in_font),
            Item::cmd("Show Latin names", Cmd::LatinNames).checked(c.show_latin_names),
            Item::Sep,
            Item::cmd(if self.settings.show_filter_list { "Hide filter list" } else { "Show filter list" }, Cmd::ToggleFilterList).checked(self.settings.show_filter_list),
            Item::cmd(if c.show_font_properties { "Hide font properties" } else { "Show font properties" }, Cmd::ToggleProperties)
                .icon("font-properties")
                .checked(c.show_font_properties),
        ]
    }

    fn help_menu(&self) -> Vec<Item> {
        vec![
            Item::cmd("&Product Help...", Cmd::ProductHelp).icon("help"),
            Item::cmd("Check for &Updates...", Cmd::CheckUpdates).icon("title-updates"),
            Item::cmd("Submit Ideas and Feedback...", Cmd::Feedback),
            Item::cmd("Report a &Bug...", Cmd::ReportBug),
            Item::Sep,
            Item::cmd("&About AnyFont...", Cmd::About).icon("about"),
        ]
    }

    /// Context menu of a Libraries row (UI-SPEC §6.2).
    fn library_menu(&self, lib: LibraryRef) -> Vec<Item> {
        let m = self.catalog.library_menu(lib);
        let mut v = vec![
            Item::cmd("Install", Cmd::LibraryInstall(lib)).enabled(m.install),
            Item::cmd("Uninstall", Cmd::LibraryUninstall(lib)).icon("uninstall").enabled(m.uninstall),
            Item::cmd("Remove Folder or Collection", Cmd::RemoveLibrary(lib)).icon("delete").enabled(m.remove),
        ];
        if let (true, LibraryRef::Collection(id)) = (m.rename, lib) {
            v.push(Item::cmd("Rename", Cmd::RenameLibrary(id)));
        }
        v
    }

    /// The themed edit menu of the text boxes (UI-SPEC §6.3).
    fn edit_box_menu(&self, target: EditTarget) -> Vec<Item> {
        let text = self.edit_text(target).unwrap_or_default();
        let sel = menu_selection(target);
        let has_sel = sel.map(|r| !r.is_empty()).unwrap_or(false);
        let undo = self.edit_undo.get(&target).map(|u| *u != text).unwrap_or(false);
        vec![
            Item::cmd("Undo", Cmd::Edit(target, EditCmd::Undo)).enabled(undo),
            Item::Sep,
            Item::cmd("Cut", Cmd::Edit(target, EditCmd::Cut)).enabled(has_sel),
            Item::cmd("Copy", Cmd::Edit(target, EditCmd::Copy)).enabled(has_sel),
            Item::cmd("Paste", Cmd::Edit(target, EditCmd::Paste)),
            Item::cmd("Delete", Cmd::Edit(target, EditCmd::Delete)).enabled(has_sel),
            Item::Sep,
            Item::cmd("Select All", Cmd::Edit(target, EditCmd::SelectAll)).enabled(!text.is_empty()),
        ]
    }

    pub(crate) fn edit_text(&self, target: EditTarget) -> Option<String> {
        match target {
            EditTarget::Sample => Some(self.sample.clone()),
            EditTarget::Search => Some(self.search.clone()),
            EditTarget::Rename => self.rename.as_ref().map(|r| r.text.clone()),
        }
    }

    fn edit_text_mut(&mut self, target: EditTarget) -> Option<&mut String> {
        match target {
            EditTarget::Sample => Some(&mut self.sample),
            EditTarget::Search => Some(&mut self.search),
            EditTarget::Rename => self.rename.as_mut().map(|r| &mut r.text),
        }
    }

    // ------------------------------------------------------------------ dispatch

    pub(crate) fn run(&mut self, ctx: &egui::Context, cmd: Cmd) {
        log::debug!("command {cmd:?}");
        match cmd {
            Cmd::AddFolder => {
                let test_mode = self.test_mode;
                if let Some(dir) = crate::testing::pick(|| if test_mode { None } else { rfd::FileDialog::new().set_title(crate::tr("Add Folder")).pick_folder() }) {
                    self.add_folder(&dir);
                }
            }
            Cmd::AddCollection => self.add_collection(),
            Cmd::GetMore => self.getmore.open(),
            Cmd::RemoveLibrary(lib) => {
                if lib == LibraryRef::MyFonts {
                    return;
                }
                if self.settings.skip_remove_confirmation {
                    self.remove_library(lib);
                } else {
                    self.dialogs.push_back(Dialog::RemoveLibrary { lib, dont_show: false });
                }
            }
            Cmd::Import => {
                let test_mode = self.test_mode;
                let f = crate::testing::pick(|| if test_mode { None } else { rfd::FileDialog::new().add_filter(crate::tr("XML - Extensible Markup Language (*.xml)"), &["xml"]).pick_file() });
                if let Some(path) = f {
                    self.import(&path);
                }
            }
            Cmd::Export => {
                let test_mode = self.test_mode;
                let f = crate::testing::pick(|| {
                    if test_mode {
                        return None;
                    }
                    rfd::FileDialog::new()
                        .add_filter(crate::tr("XML - Extensible Markup Language (*.xml)"), &["xml"])
                        .set_file_name(crate::tr("AnyFont Folders and Collections.xml"))
                        .save_file()
                });
                if let Some(path) = f {
                    self.export(&path);
                }
            }
            Cmd::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Cmd::SelectAll => self.select_all(),
            Cmd::DeselectAll => self.deselect_all(),
            Cmd::Install => dialogs::start_fonts_op(self, ctx, OpKind::Install, self.targets()),
            Cmd::Uninstall => dialogs::start_fonts_op(self, ctx, OpKind::Uninstall, self.targets()),
            Cmd::Download => {
                // online placeholders do not exist in 0.1.0 (lead decision 7): Get More downloads
                self.getmore.open();
            }
            Cmd::Delete => dialogs::ask_delete(self, self.targets()),
            Cmd::AddToCollection(id) => {
                let keys = self.keys_of(&self.targets());
                if let Err(e) = self.db.add_to_collection(id, &keys) {
                    log::error!("add to collection: {e}");
                }
                self.reload();
            }
            Cmd::RemoveFromOpenCollection => {
                if let LibraryRef::Collection(id) = self.library {
                    let keys = self.keys_of(&self.targets());
                    if let Err(e) = self.db.remove_from_collection(id, &keys) {
                        log::error!("remove from collection: {e}");
                    }
                    self.reload();
                    if self.sel.keys.is_empty() {
                        self.select_first();
                    }
                }
            }
            Cmd::RemoveFromCollection(id) => {
                let keys = self.keys_of(&self.targets());
                if let Err(e) = self.db.remove_from_collection(id, &keys) {
                    log::error!("remove from collection: {e}");
                }
                self.reload();
            }
            Cmd::ToggleDuplicates => {
                self.settings.core.show_duplicates = !self.settings.core.show_duplicates;
                self.requery();
            }
            Cmd::OpenLocation(p) => {
                if let Err(e) = self.fs.open_location(&p) {
                    log::warn!("open location {}: {e}", p.display());
                }
            }
            Cmd::Refresh => self.start_scan(false),
            Cmd::GroupByFamily => {
                self.settings.core.group_by_family = !self.settings.core.group_by_family;
                self.requery();
            }
            Cmd::ShowRecent => {
                self.settings.core.show_recent = !self.settings.core.show_recent;
                self.requery();
            }
            Cmd::RenderInFont => self.settings.core.render_name_in_font = !self.settings.core.render_name_in_font,
            Cmd::LatinNames => {
                self.settings.core.show_latin_names = !self.settings.core.show_latin_names;
                self.requery();
            }
            Cmd::ToggleFilterList => self.settings.show_filter_list = !self.settings.show_filter_list,
            Cmd::ToggleProperties | Cmd::Inspectors => {
                self.settings.core.show_font_properties = !self.settings.core.show_font_properties;
                self.settings.core.window.right_panel_visible = self.settings.core.show_font_properties;
            }
            Cmd::DebugInspector => self.inspector_open = !self.inspector_open,
            Cmd::ProductHelp => {
                if self.test_mode {
                    log::info!("product help: {}", crate::help::help_url());
                    crate::testing::intercept(crate::testing::External::Url(crate::help::help_url()));
                } else {
                    crate::help::open_product_help();
                }
            }
            Cmd::CheckUpdates => self.updater.open(),
            Cmd::Feedback => self.bugreport.open(crate::bugreport::Kind::Idea),
            Cmd::ReportBug => self.bugreport.open(crate::bugreport::Kind::Bug),
            Cmd::About => self.about.open(),
            Cmd::Options => self.options_open = true,
            Cmd::LibraryInstall(lib) => {
                let ids: Vec<FontId> = self.catalog.library_fonts(&lib).iter().filter(|f| !f.installed && f.is_usable()).map(|f| f.id).collect();
                dialogs::start_fonts_op(self, ctx, OpKind::Install, ids);
            }
            Cmd::LibraryUninstall(lib) => {
                let ids: Vec<FontId> = self.catalog.library_fonts(&lib).iter().filter(|f| f.installed && !f.protected).map(|f| f.id).collect();
                dialogs::start_fonts_op(self, ctx, OpKind::Uninstall, ids);
            }
            Cmd::RenameLibrary(id) => {
                if let Some(c) = self.catalog.collection(id) {
                    if !c.is_favorites {
                        self.focus = crate::app::Focus::Libraries;
                        self.rename = Some(crate::app::Rename { id, text: c.name.clone(), is_new: false, started_frame: self.frame_no });
                    }
                }
            }
            Cmd::Edit(target, op) => self.edit_op(ctx, target, op),
        }
    }

    fn keys_of(&self, ids: &[FontId]) -> Vec<anyfont_core::model::FontKey> {
        ids.iter().filter_map(|id| self.catalog.font(*id)).map(|f| f.key.clone()).collect()
    }

    /// File ▸ Add Folder... with a chosen folder (also used by the tests).
    pub fn add_folder(&mut self, dir: &Path) {
        if !dir.is_dir() {
            self.dialogs.push_back(Dialog::message("AnyFont", &crate::tr("Folder not found. Please choose another location.")));
            return;
        }
        let folders = self.catalog.folders().to_vec();
        if folders.iter().any(|f| crate::app::same_path(&f.path, dir)) {
            self.dialogs.push_back(Dialog::message("AnyFont", &crate::tr("This folder has already been added.")));
            return;
        }
        if let Some(parent) = folders.iter().find(|f| f.recursive && f.contains(&dir.join("x")) && !self.is_content_folder(&f.path)) {
            let msg = crate::fmt("This folder has already been added as a subfolder of {}.", &[&parent.path.display().to_string()]);
            self.dialogs.push_back(Dialog::message("AnyFont", &msg));
            return;
        }
        match self.db.add_folder(dir, true) {
            Ok(f) => {
                log::info!("folder added: {}", dir.display());
                self.reload();
                self.open_library(LibraryRef::Folder(f.id));
                self.type1_check = Some(f.id);
                self.start_scan(false);
            }
            Err(e) => self.dialogs.push_back(Dialog::message("AnyFont", &crate::fmt("Error: {}", &[&e.to_string()]))),
        }
    }

    /// File ▸ Add Collection: "Collection {n}" in the inline name editor.
    pub fn add_collection(&mut self) {
        match self.db.create_collection(None) {
            Ok(c) => {
                self.reload();
                self.open_library(LibraryRef::Collection(c.id));
                self.focus = crate::app::Focus::Libraries;
                self.rename = Some(crate::app::Rename { id: c.id, text: c.name.clone(), is_new: true, started_frame: self.frame_no });
            }
            Err(e) => log::error!("add collection: {e}"),
        }
    }

    /// Removes a folder or collection from the Libraries (font files are not touched).
    pub(crate) fn remove_library(&mut self, lib: LibraryRef) {
        let r = match lib {
            LibraryRef::MyFonts => return,
            LibraryRef::Folder(id) => self.db.remove_folder(id),
            LibraryRef::Collection(id) => self.db.remove_collection(id),
        };
        if let Err(e) = r {
            log::error!("remove library: {e}");
        }
        if self.library == lib {
            self.library = LibraryRef::MyFonts;
        }
        self.reload();
        self.open_library(self.library);
        if matches!(lib, LibraryRef::Folder(_)) {
            self.start_scan(false);
        }
    }

    /// Commits the inline collection name. False when the name is refused (the editor stays open).
    pub(crate) fn commit_rename(&mut self) -> bool {
        let Some(r) = &self.rename else { return true };
        let name = r.text.trim().to_string();
        if name.is_empty() {
            return false;
        }
        let id = r.id;
        let unchanged = self.catalog.collection(id).map(|c| c.name == name).unwrap_or(false);
        if !unchanged {
            if let Err(e) = self.db.rename_collection(id, &name) {
                log::info!("rename refused: {e}");
                return false;
            }
        }
        self.rename = None;
        self.reload();
        self.open_library(LibraryRef::Collection(id));
        true
    }

    /// Esc in the inline editor: a new collection is removed again.
    pub(crate) fn cancel_rename(&mut self) {
        if let Some(r) = self.rename.take() {
            if r.is_new {
                let _ = self.db.remove_collection(r.id);
                self.reload();
                self.open_library(LibraryRef::MyFonts);
            }
        }
    }

    pub(crate) fn select_all(&mut self) {
        if self.result.font_ids.is_empty() {
            return;
        }
        self.sel.keys.clear();
        for r in &self.result.rows {
            if let Some(k) = crate::fontlist::row_key(r) {
                self.sel.keys.insert(k);
            }
        }
        self.sel.all = true;
        self.props.on_selection_changed();
    }

    pub(crate) fn deselect_all(&mut self) {
        self.sel.keys.clear();
        self.sel.all = false;
        self.props.on_selection_changed();
    }

    /// File ▸ Import with a chosen file (also used by the tests). The folders come first; the collections
    /// are imported when those folders have been scanned, so fonts in them are found (BUG-304).
    pub fn import(&mut self, path: &Path) {
        let text = std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        match anyfont_core::exchange::import_file_parts(&self.db, &self.catalog, path, anyfont_core::exchange::ImportParts::Folders) {
            Ok(report) => {
                log::info!("import {} (folders): {report:?}", path.display());
                let missing_folders = import_missing_folders(&text);
                if !missing_folders.is_empty() {
                    self.dialogs.push_back(Dialog::Missing { items: missing_folders });
                }
                self.pending_import = Some(path.to_path_buf());
                if report.folders_added > 0 {
                    self.reload();
                    // `scan_finished` imports the collections
                    self.start_scan(false);
                } else if !self.scanning() {
                    self.finish_import();
                }
            }
            Err(e) => self.dialogs.push_back(Dialog::message("AnyFont", &crate::fmt("Error: {}", &[&e.to_string()]))),
        }
    }

    /// The second half of File ▸ Import: the collections, with the fonts the database knows now; the
    /// fonts still missing go to PANOSE Font Matching Results.
    pub(crate) fn finish_import(&mut self) {
        let Some(path) = self.pending_import.take() else { return };
        match anyfont_core::exchange::import_file_parts(&self.db, &self.catalog, &path, anyfont_core::exchange::ImportParts::Collections) {
            Ok(report) => {
                log::info!("import {} (collections): {report:?}", path.display());
                self.reload();
                if !report.missing.is_empty() && self.settings.core.panose.mode.shows_results(false) {
                    let mut names: Vec<String> = Vec::new();
                    self.import_links.clear();
                    for m in &report.missing {
                        let name = m.name();
                        if name.is_empty() {
                            continue;
                        }
                        if !names.contains(&name) {
                            names.push(name.clone());
                        }
                        self.import_links.push((name, m.collection));
                    }
                    if !names.is_empty() {
                        // after "We can't find the following:", not on top of it
                        self.pending_results = Some(names);
                    }
                }
            }
            Err(e) => self.dialogs.push_back(Dialog::message("AnyFont", &crate::fmt("Error: {}", &[&e.to_string()]))),
        }
    }

    /// OK in Font Matching Results after an import: each chosen substitute takes its missing font's place in
    /// the imported collections (BUG-102).
    pub(crate) fn link_substitutes(&mut self, resolved: &[crate::matching::Resolved]) {
        let links = std::mem::take(&mut self.import_links);
        let mut changed = false;
        for r in resolved {
            let Some(key) = r.font.and_then(|id| self.catalog.font(id)).map(|f| f.key.clone()) else { continue };
            for (_, c) in links.iter().filter(|(name, _)| *name == r.missing) {
                match self.db.add_to_collection(*c, std::slice::from_ref(&key)) {
                    Ok(_) => changed = true,
                    Err(e) => log::warn!("linking {} into collection {c}: {e}", r.missing),
                }
            }
        }
        if changed {
            self.reload();
        }
    }

    /// File ▸ Export to a chosen file (also used by the tests).
    pub fn export(&mut self, path: &Path) {
        if let Err(e) = anyfont_core::exchange::export_file(&self.db, &self.catalog, &anyfont_core::exchange::ExportOptions::all_except(std::slice::from_ref(&self.content_dir)), path) {
            self.dialogs.push_back(Dialog::message("AnyFont", &crate::fmt("Error: {}", &[&e.to_string()])));
        }
    }

    fn edit_op(&mut self, ctx: &egui::Context, target: EditTarget, op: EditCmd) {
        let id = target.id();
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        let Some(before) = self.edit_text(target) else { return };
        let n = before.chars().count();
        // the selection when the menu opened: clicking the menu took the focus from the box, which collapsed it
        let range = take_menu_selection(target)
            .or_else(|| state.cursor.char_range())
            .map(|r| r.as_sorted_char_range())
            .map(|r| (r.start.0.min(n), r.end.0.min(n)))
            .unwrap_or((n, n));
        let selected: String = before.chars().skip(range.0).take(range.1 - range.0).collect();
        let replace = |text: &str, with: &str| -> (String, usize) {
            let head: String = text.chars().take(range.0).collect();
            let tail: String = text.chars().skip(range.1).collect();
            (format!("{head}{with}{tail}"), range.0 + with.chars().count())
        };
        let mut new_text: Option<(String, usize)> = None;
        match op {
            EditCmd::Undo => {
                if let Some(u) = self.edit_undo.get(&target).cloned() {
                    let l = u.chars().count();
                    new_text = Some((u, l));
                }
            }
            EditCmd::Copy => ctx.copy_text(selected),
            EditCmd::Cut => {
                ctx.copy_text(selected);
                new_text = Some(replace(&before, ""));
            }
            EditCmd::Delete => new_text = Some(replace(&before, "")),
            EditCmd::Paste => {
                let clip = if self.test_mode { None } else { arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) };
                if let Some(c) = clip {
                    let c = c.replace(['\r', '\n'], " ");
                    new_text = Some(replace(&before, &c));
                }
            }
            EditCmd::SelectAll => {
                state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
            }
        }
        if let Some((t, cur)) = new_text {
            self.edit_undo.insert(target, before);
            if let Some(s) = self.edit_text_mut(target) {
                *s = t;
            }
            state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::one(egui::text::CCursor::new(cur))));
            if target == EditTarget::Search {
                self.requery();
            }
        }
        state.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }
}

/// The last known selection of a text box (for the edit menu's enable rules).
fn selection_of(target: EditTarget) -> Option<egui::text_selection::CCursorRange> {
    EDIT_SEL.with(|m| m.borrow().get(&target).copied())
}

thread_local! {
    /// The selection of a text box when its edit menu opened (BUG-301).
    static MENU_SEL: std::cell::RefCell<std::collections::HashMap<EditTarget, egui::text_selection::CCursorRange>> = Default::default();
}

/// Keeps the selection of a text box as its edit menu opens: the menu's Cut, Copy and Delete act on it,
/// although the text box gives up its focus (and egui its selection) when the menu is clicked.
pub(crate) fn snapshot_selection(target: EditTarget) {
    MENU_SEL.with(|m| {
        let mut m = m.borrow_mut();
        match selection_of(target) {
            Some(r) => m.insert(target, r),
            None => m.remove(&target),
        };
    });
}

/// The selection the open edit menu acts on.
fn menu_selection(target: EditTarget) -> Option<egui::text_selection::CCursorRange> {
    MENU_SEL.with(|m| m.borrow().get(&target).copied()).or_else(|| selection_of(target))
}

fn take_menu_selection(target: EditTarget) -> Option<egui::text_selection::CCursorRange> {
    MENU_SEL.with(|m| m.borrow_mut().remove(&target))
}

thread_local! {
    /// The last known selection of each text box (updated while drawing them).
    pub(crate) static EDIT_SEL: std::cell::RefCell<std::collections::HashMap<EditTarget, egui::text_selection::CCursorRange>> = Default::default();
}

/// The state of a text box before the right mouse button reaches it, while that button presses, holds or
/// releases over `rect`: egui's text edit puts the cursor under any pointer button, which collapses the
/// selection the edit menu acts on (BUG-301). Hand it to [`keep_selection`] after drawing the box.
pub(crate) fn before_right_click(ctx: &egui::Context, target: EditTarget, rect: egui::Rect) -> Option<egui::text_edit::TextEditState> {
    use egui::PointerButton::{Primary, Secondary};
    let right = ctx.input(|i| {
        let p = &i.pointer;
        (p.button_pressed(Secondary) || p.button_down(Secondary) || p.button_released(Secondary))
            && !p.button_down(Primary)
            && p.interact_pos().is_some_and(|q| rect.contains(q))
    });
    if right {
        egui::text_edit::TextEditState::load(ctx, target.id())
    } else {
        None
    }
}

/// Puts back the state [`before_right_click`] saved (the selection survives the right click).
pub(crate) fn keep_selection(ctx: &egui::Context, target: EditTarget, saved: Option<egui::text_edit::TextEditState>) {
    if let Some(s) = saved {
        s.store(ctx, target.id());
    }
}

/// Remembers the selection of a text box (called after drawing it).
pub(crate) fn remember_selection(ctx: &egui::Context, target: EditTarget) {
    if let Some(s) = egui::text_edit::TextEditState::load(ctx, target.id()) {
        if let Some(r) = s.cursor.char_range() {
            EDIT_SEL.with(|m| {
                m.borrow_mut().insert(target, r);
            });
        }
    }
}

/// Missing folders (paths that do not exist) of an import file.
fn import_missing_folders(xml: &str) -> Vec<String> {
    let mut folders = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        if let Some(t) = tag.strip_prefix("folder ") {
            if let Some(p) = attr(t, "path") {
                if !Path::new(&p).is_dir() {
                    folders.push(p);
                }
            }
        }
        rest = &rest[end.min(rest.len())..];
    }
    folders
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    Some(
        tag[i..j]
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&#10;", "\n")
            .replace("&#13;", "\r")
            .replace("&#9;", "\t")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn ellipsis_keeps_both_ends() {
        let s = super::middle_ellipsis("c:\\windows\\fonts\\a-very-long-folder-name\\another-one\\font.ttf", 30);
        assert_eq!(s.chars().count(), 30);
        assert!(s.starts_with("c:\\windows"));
        assert!(s.ends_with("font.ttf"));
    }

    #[test]
    fn import_attrs() {
        assert_eq!(super::attr("font DBFamily=\"A &amp; B\" DBSubFamily=\"Bold\"/", "DBFamily").as_deref(), Some("A & B"));
    }
}
