//! The main window: its state, the start-up (settings, database, first scan), the per-frame orchestration of
//! the bands (title bar, menu bar, toolbar, panels, status bar), background work (scan, font operations,
//! previews, the folder watcher, messages from later instances) and persistence.

use crate::fontlist::{self, Layout, ListState, SelKey, Selection};
use crate::menus::MenuState;
use crate::ops::Running;
use crate::preview::Previews;
use crate::settings::{AppSettings, Paths};
use crate::theme::{self, t, Scheme};
use crate::watcher::Watcher;
use crate::{dialogs, properties, splash};
use anyfont_core::catalog::{Catalog, FontQuery, LibraryRef, QueryResult};
use anyfont_core::db::FontDatabase;
use anyfont_core::filters::FilterSet;
use anyfont_core::model::{FolderId, FontId};
use anyfont_core::scan::{ScanHandle, ScanProgress, ScanRequest};
use anyfont_core::settings::LastLibrary;
use anyfont_core::Eligibility;
use anyfont_sys::FontSystem;
use egui::{pos2, vec2, Rect};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What `main.rs` hands to the main window.
pub struct StartOptions {
    pub fs: Arc<dyn FontSystem>,
    pub paths: Paths,
    pub settings: AppSettings,
    /// Font files or folders from the command line.
    pub files: Vec<PathBuf>,
    /// `/Lang:` value.
    pub lang: Option<String>,
    /// Show the splash (not with `NoSplash`).
    pub splash: bool,
    /// A crash report of the previous run to offer.
    pub crash_report: Option<PathBuf>,
    /// Messages from later instances (files to show, focus requests).
    pub incoming: Option<Receiver<anyfont_sys::single_instance::Message>>,
    /// Set when the updater asks to exit and run its setup.
    pub updater_out: Arc<Mutex<Option<PathBuf>>>,
}

/// Configuration of a window driven by the end-to-end tests: everything happens inside `sandbox_root`
/// (fonts, registry, AppData) and `data_dir` (settings, database); the user's profile is never touched.
pub struct TestConfig {
    /// The sandbox font system's root.
    pub sandbox_root: PathBuf,
    /// Settings and database folder.
    pub data_dir: PathBuf,
    /// Fonts seeded into a new sandbox (`None`: the default seed, copied from `%WINDIR%\Fonts`).
    pub seed: Option<Vec<PathBuf>>,
    /// A font system to use instead of opening `sandbox_root` (to keep a handle on the sandbox).
    pub system: Option<Arc<dyn FontSystem>>,
    /// The UI language ("en-US" by default, whatever the Windows language is).
    pub language: String,
    pub scheme: Scheme,
    /// Run the first scan (and every rescan) on the UI thread, so a test sees its result at once.
    pub synchronous_scan: bool,
    /// Files as if passed on the command line.
    pub files: Vec<PathBuf>,
    /// Settings to start with (default: the spec defaults, window not maximized).
    pub settings: Option<AppSettings>,
}

impl TestConfig {
    pub fn new(sandbox_root: &Path) -> TestConfig {
        TestConfig {
            sandbox_root: sandbox_root.to_path_buf(),
            data_dir: sandbox_root.join("anyfont-data"),
            seed: None,
            system: None,
            language: "en-US".into(),
            scheme: Scheme::Dark,
            synchronous_scan: true,
            files: Vec::new(),
            settings: None,
        }
    }
}

/// Where the keyboard goes in the main window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    List,
    Libraries,
}

/// The inline name editor of a collection row (UI-SPEC §5.1).
pub struct Rename {
    pub id: anyfont_core::model::CollectionId,
    pub text: String,
    /// Created by Add Collection: Esc removes it again.
    pub is_new: bool,
    pub started_frame: u64,
}

/// Files confirmed in Delete fonts (with the font names for the progress line and the faces).
pub(crate) type PendingDelete = (Vec<PathBuf>, std::collections::HashMap<PathBuf, String>, Vec<FontId>);

pub struct AnyFontApp {
    pub(crate) fs: Arc<dyn FontSystem>,
    pub(crate) paths: Paths,
    pub(crate) settings: AppSettings,
    saved_settings: AppSettings,
    save_due: Option<Instant>,
    pub(crate) db: FontDatabase,
    pub(crate) catalog: Catalog,
    pub(crate) result: QueryResult,
    pub(crate) layout: Layout,
    pub(crate) library: LibraryRef,
    pub(crate) filters: FilterSet,
    pub(crate) search: String,
    /// The sample text combo's text ("" = Font Name).
    pub(crate) sample: String,
    pub(crate) sel: Selection,
    pub(crate) list: ListState,
    pub(crate) previews: Previews,
    scan: Option<ScanHandle>,
    pub(crate) scan_progress: (usize, usize),
    /// The first scan finished (File ▸ Import is enabled).
    pub(crate) db_ready: bool,
    rescan_after: bool,
    sync_scan: bool,
    /// File ▸ Import: the file whose collections are imported once the scan of its folders ends (BUG-304).
    pub(crate) pending_import: Option<PathBuf>,
    /// Missing fonts of the last import ("Family Style") and the collections that wanted them: the
    /// substitutes chosen in Font Matching Results are linked there (BUG-102).
    pub(crate) import_links: Vec<(String, anyfont_core::model::CollectionId)>,
    /// Missing fonts waiting for the dialogs in front of them (We can't find the following:) to close.
    pub(crate) pending_results: Option<Vec<String>>,
    pub(crate) watcher: Watcher,
    pub(crate) op: Option<Running>,
    pub(crate) op_flow: Option<dialogs::OpFlow>,
    pub(crate) dialogs: VecDeque<dialogs::Dialog>,
    pub(crate) options_open: bool,
    pub(crate) inspector_open: bool,
    pub(crate) menu: MenuState,
    pub(crate) rename: Option<Rename>,
    pub(crate) libs_scroll: f32,
    pub(crate) filters_scroll: f32,
    pub(crate) props: properties::PropsState,
    pub(crate) about: crate::about::About,
    pub(crate) bugreport: crate::bugreport::BugReport,
    pub(crate) getmore: crate::getmore::GetMore,
    pub(crate) matching: crate::matching::Matching,
    pub(crate) updater: crate::updater::Updater,
    updater_out: Arc<Mutex<Option<PathBuf>>>,
    incoming: Option<Receiver<anyfont_sys::single_instance::Message>>,
    pub(crate) hwnd: Option<isize>,
    frame_ready: bool,
    pub(crate) splash: Option<splash::Splash>,
    pub(crate) frame_no: u64,
    pub(crate) focus: Focus,
    pub(crate) lang_setting: String,
    pending_files: Vec<PathBuf>,
    pub(crate) content_dir: PathBuf,
    pub(crate) drag_fonts: Option<Vec<FontId>>,
    applied_zoom: f32,
    applied_border: Option<Option<[u8; 3]>>,
    pub(crate) test_mode: bool,
    /// Edit-box undo snapshots (text before the last change), per box.
    pub(crate) edit_undo: std::collections::HashMap<crate::commands::EditTarget, String>,
    pub(crate) last_restored: Option<(egui::Pos2, egui::Vec2)>,
    /// A folder just added: look for Type 1 fonts in it when the scan is done (UI-SPEC §11 #14).
    pub(crate) type1_check: Option<FolderId>,
    /// Files confirmed in Delete fonts, deleted on the next frame.
    pub(crate) pending_delete: Option<PendingDelete>,
}

/// The Windows LCID of a UI language (localised family names, `FontQuery::language`).
pub fn lcid(l: crate::i18n::Lang) -> u16 {
    use crate::i18n::Lang::*;
    match l {
        EnUs => 0x0409,
        PtBr => 0x0416,
        ZhCn => 0x0804,
        ZhTw => 0x0404,
        CsCz => 0x0405,
        NlNl => 0x0413,
        FrFr => 0x040C,
        DeDe => 0x0407,
        ItIt => 0x0410,
        JaJp => 0x0411,
        PlPl => 0x0415,
        RuRu => 0x0419,
        EsEs => 0x0C0A,
        SvSe => 0x041D,
        TrTr => 0x041F,
    }
}

impl AnyFontApp {
    /// The window of the application (`main.rs`).
    pub fn new(cc: &eframe::CreationContext<'_>, opts: StartOptions) -> AnyFontApp {
        let mut settings = opts.settings;
        // language: /Lang: > the setup's choice (once) > the settings > Windows
        let mut lang_setting = settings.core.language.clone().unwrap_or_default();
        if let Some((l, stamp)) = crate::i18n::installer_choice() {
            if stamp != settings.installer_lang_stamp {
                settings.installer_lang_stamp = stamp;
                settings.core.language = Some(l.code().to_string());
                lang_setting = l.code().to_string();
            }
        }
        if let Some(l) = &opts.lang {
            lang_setting = l.clone();
        }
        let hwnd = crate::platform::hwnd_of(cc);
        if let Some(h) = hwnd {
            anyfont_sys::set_owner_window(h);
        }
        let mut app = AnyFontApp::build(&cc.egui_ctx, opts.fs, opts.paths, settings, lang_setting, false, opts.updater_out);
        app.hwnd = hwnd;
        app.incoming = opts.incoming;
        app.pending_files = opts.files;
        if opts.splash {
            app.splash = Some(splash::Splash::new());
        } else {
            app.restore_window(&cc.egui_ctx, false);
        }
        if let Some(p) = opts.crash_report {
            app.dialogs.push_back(dialogs::Dialog::CrashOffer { path: p });
        }
        app.updater.on_startup(&cc.egui_ctx, app.settings.core.update_mode);
        app.start_scan(false);
        app
    }

    /// A window for the headless end-to-end tests (see [`TestConfig`]).
    pub fn new_for_test(cc: &eframe::CreationContext<'_>, cfg: TestConfig) -> AnyFontApp {
        let fs: Arc<dyn FontSystem> = match cfg.system {
            Some(s) => s,
            None => {
                let config = match cfg.seed {
                    Some(seed) => anyfont_sys::SandboxConfig { seed },
                    None => anyfont_sys::SandboxConfig::default(),
                };
                Arc::new(anyfont_sys::SandboxFontSystem::open(cfg.sandbox_root.clone(), config).expect("sandbox font system"))
            }
        };
        let paths = Paths::in_dir(&cfg.data_dir, fs.paths().content_fonts_dir.clone());
        let mut settings = cfg.settings.unwrap_or_else(|| {
            let mut s = AppSettings::default();
            s.core.window.maximized = false;
            s
        });
        settings.scheme = cfg.scheme;
        settings.core.update_mode = anyfont_core::settings::UpdateMode::Never;
        let mut app = AnyFontApp::build(&cc.egui_ctx, fs, paths, settings, cfg.language, true, Arc::new(Mutex::new(None)));
        // the Updates dialog shows the mode of the settings, as after the real start-up (`on_startup`)
        app.updater.set_mode(app.settings.core.update_mode);
        app.sync_scan = cfg.synchronous_scan;
        app.pending_files = cfg.files;
        app.start_scan(false);
        app
    }

    fn build(
        ctx: &egui::Context,
        fs: Arc<dyn FontSystem>,
        paths: Paths,
        settings: AppSettings,
        lang_setting: String,
        test_mode: bool,
        updater_out: Arc<Mutex<Option<PathBuf>>>,
    ) -> AnyFontApp {
        crate::i18n::init(&lang_setting);
        theme::install_fonts(ctx);
        theme::set_scheme(ctx, settings.scheme);
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
        });
        if let Some(dir) = paths.db_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut db_corrupt = false;
        let db = FontDatabase::open(&paths.db_file).unwrap_or_else(|e| {
            log::error!("cannot open the font database {}: {e}; using an empty one in memory", paths.db_file.display());
            db_corrupt = true;
            FontDatabase::open_in_memory().expect("in-memory database")
        });
        let content_dir = fs.content_folder();
        // the AnyFont Content folder (Get More downloads) is part of My fonts (lead decision 7)
        if content_dir.is_dir() {
            let known = db.folders().map(|f| f.iter().any(|x| same_path(&x.path, &content_dir))).unwrap_or(false);
            if !known {
                if let Err(e) = db.add_folder(&content_dir, true) {
                    log::warn!("cannot add the content folder: {e}");
                }
            }
        }
        let catalog = Catalog::load(&db).unwrap_or_default();
        let library = match &settings.core.last_library {
            LastLibrary::MyFonts => LibraryRef::MyFonts,
            LastLibrary::Folder(p) => catalog.folders().iter().find(|f| same_path(&f.path, Path::new(p))).map(|f| LibraryRef::Folder(f.id)).unwrap_or_default(),
            LastLibrary::Collection(n) => catalog.collections().iter().find(|c| &c.name == n).map(|c| LibraryRef::Collection(c.id)).unwrap_or_default(),
        };
        let sample = {
            let s = settings.core.sample_text.clone();
            if s == anyfont_core::settings::SAMPLE_FONT_NAME {
                String::new()
            } else {
                s
            }
        };
        let watcher = Watcher::start(ctx.clone(), Duration::from_secs(3));
        let cache_dir = paths.cache_dir.clone();
        let mut app = AnyFontApp {
            fs,
            paths,
            saved_settings: settings.clone(),
            settings,
            save_due: None,
            db,
            catalog,
            result: QueryResult {
                rows: Vec::new(),
                font_ids: Vec::new(),
                recent: Vec::new(),
                count: 0,
                filtered: false,
                library_name: String::new(),
                status_text: String::new(),
            },
            layout: Layout::default(),
            library,
            filters: FilterSet::default(),
            search: String::new(),
            sample,
            sel: Selection::default(),
            list: ListState::default(),
            previews: Previews::new(ctx),
            scan: None,
            scan_progress: (0, 0),
            db_ready: false,
            rescan_after: false,
            sync_scan: false,
            pending_import: None,
            import_links: Vec::new(),
            pending_results: None,
            watcher,
            op: None,
            op_flow: None,
            dialogs: VecDeque::new(),
            options_open: false,
            inspector_open: false,
            menu: MenuState::default(),
            rename: None,
            libs_scroll: 0.0,
            filters_scroll: 0.0,
            props: properties::PropsState::default(),
            about: crate::about::About::new(),
            bugreport: crate::bugreport::BugReport::new(),
            getmore: crate::getmore::GetMore::with_cache_dir(cache_dir),
            matching: crate::matching::Matching::new(),
            updater: crate::updater::Updater::new(),
            updater_out,
            incoming: None,
            hwnd: None,
            frame_ready: false,
            splash: None,
            frame_no: 0,
            focus: Focus::List,
            lang_setting,
            pending_files: Vec::new(),
            content_dir,
            drag_fonts: None,
            applied_zoom: 0.0,
            applied_border: None,
            test_mode,
            edit_undo: Default::default(),
            last_restored: None,
            type1_check: None,
            pending_delete: None,
        };
        if db_corrupt {
            app.dialogs.push_back(dialogs::Dialog::message(
                "AnyFont",
                "Your font database has been corrupted. To reset the database, you must restart AnyFont while holding down Shift.",
            ));
        }
        app.requery();
        app.select_first();
        app
    }

    // ------------------------------------------------------------------ data

    /// The UI language's LCID.
    pub(crate) fn language(&self) -> u16 {
        lcid(crate::i18n::lang())
    }

    /// Reloads the catalog from the database (after a scan or any edit) and re-runs the query.
    pub(crate) fn reload(&mut self) {
        match Catalog::load(&self.db) {
            Ok(c) => self.catalog = c,
            Err(e) => log::error!("cannot load the catalog: {e}"),
        }
        // the open library may be gone
        let exists = match self.library {
            LibraryRef::MyFonts => true,
            LibraryRef::Folder(id) => self.catalog.folders().iter().any(|f| f.id == id),
            LibraryRef::Collection(id) => self.catalog.collection(id).is_some(),
        };
        if !exists {
            self.library = LibraryRef::MyFonts;
        }
        self.watcher.watch(self.watched_folders());
        self.requery();
    }

    /// The folders the watcher polls: the font folders of Windows and every user folder.
    fn watched_folders(&self) -> Vec<PathBuf> {
        let dirs = self.fs.font_dirs();
        let mut v = vec![dirs.all_users.clone(), dirs.current_user.clone()];
        v.extend(self.catalog.folders().iter().filter(|f| f.enabled).map(|f| f.path.clone()));
        v
    }

    pub(crate) fn query(&self) -> FontQuery {
        FontQuery {
            library: self.library,
            filters: self.filters.clone(),
            search: self.search.trim().to_string(),
            group_by_family: self.settings.core.group_by_family,
            show_duplicates: self.settings.core.show_duplicates,
            show_recent: self.settings.core.show_recent,
            show_latin_names: self.settings.core.show_latin_names,
            language: self.language(),
            sort: Default::default(),
        }
    }

    /// Re-runs the list query; keeps the selection of rows that are still listed.
    pub(crate) fn requery(&mut self) {
        self.result = self.catalog.query(&self.query());
        self.layout = Layout::build(&self.result.rows, self.settings.core.preview_size);
        let rows = &self.result.rows;
        let present: std::collections::HashSet<SelKey> = rows.iter().filter_map(fontlist::row_key).collect();
        self.sel.keys.retain(|k| present.contains(k));
        if self.sel.all {
            self.sel.all = !self.result.font_ids.is_empty();
        }
        if let Some(c) = self.sel.current {
            if !self.result.font_ids.contains(&c) && !self.result.recent.contains(&c) {
                self.sel.current = None;
            }
        }
        self.sel.cursor = self.sel.cursor.filter(|c| *c < rows.len());
        self.sel.anchor = self.sel.anchor.filter(|c| *c < rows.len());
        self.props.on_selection_changed();
    }

    /// Opens a library: its first font is selected (UI-SPEC §5.1).
    pub(crate) fn open_library(&mut self, lib: LibraryRef) {
        if self.library != lib {
            self.library = lib;
            self.filters.clear();
            self.list.scroll = 0.0;
        }
        self.settings.core.last_library = match lib {
            LibraryRef::MyFonts => LastLibrary::MyFonts,
            LibraryRef::Folder(id) => self.catalog.folders().iter().find(|f| f.id == id).map(|f| LastLibrary::Folder(f.path.display().to_string())).unwrap_or_default(),
            LibraryRef::Collection(id) => self.catalog.collection(id).map(|c| LastLibrary::Collection(c.name.clone())).unwrap_or_default(),
        };
        self.sel = Selection::default();
        self.requery();
        self.select_first();
    }

    pub(crate) fn select_first(&mut self) {
        if let Some(id) = self.result.first_font() {
            if let Some(i) = self.result.rows.iter().position(|r| matches!(r, anyfont_core::ListRow::Font { id: x, recent: false, .. } if *x == id)) {
                self.sel.keys.clear();
                self.sel.keys.insert(SelKey::Font(id));
                self.sel.current = Some(id);
                self.sel.cursor = Some(i);
                self.sel.anchor = Some(i);
                self.sel.all = false;
                self.props.on_selection_changed();
            }
        }
    }

    /// Explicitly selected faces (font rows).
    pub(crate) fn selected_faces(&self) -> Vec<FontId> {
        if self.sel.all {
            return self.result.font_ids.clone();
        }
        let mut v: Vec<FontId> = self.sel.keys.iter().filter_map(|k| if let SelKey::Font(id) = k { Some(*id) } else { None }).collect();
        v.sort_by_key(|id| self.result.font_ids.iter().position(|x| x == id).unwrap_or(usize::MAX));
        v
    }

    /// Faces of the selected family header rows.
    pub(crate) fn header_faces(&self) -> Vec<FontId> {
        if self.sel.all {
            return Vec::new();
        }
        let mut v = Vec::new();
        for r in &self.result.rows {
            if let anyfont_core::ListRow::FamilyHeader { family, fonts, .. } = r {
                if self.sel.keys.contains(&SelKey::Header(family.clone())) {
                    v.extend(fonts.iter().copied());
                }
            }
        }
        v
    }

    /// The faces a command acts on: the selection (headers count as their faces), else the current font.
    pub(crate) fn targets(&self) -> Vec<FontId> {
        let mut v = self.selected_faces();
        for id in self.header_faces() {
            if !v.contains(&id) {
                v.push(id);
            }
        }
        if v.is_empty() {
            v.extend(self.sel.current);
        }
        v
    }

    pub(crate) fn eligibility(&self) -> Eligibility {
        self.catalog.eligibility(self.library, &self.selected_faces(), &self.header_faces(), self.sel.all, self.sel.current)
    }

    // ------------------------------------------------------------------ scanning

    /// The installed fonts in the core's terms.
    pub(crate) fn installed(&self) -> Vec<anyfont_core::model::InstalledFont> {
        match self.fs.installed_fonts() {
            Ok(v) => v.into_iter().map(|f| anyfont_core::model::InstalledFont::new(f.path, f.protected)).collect(),
            Err(e) => {
                log::error!("cannot read the installed fonts: {e}");
                Vec::new()
            }
        }
    }

    /// Starts a (re)scan of every library. A scan already running is followed by another one.
    pub(crate) fn start_scan(&mut self, force: bool) {
        if self.scan.is_some() {
            self.rescan_after = true;
            return;
        }
        let req = ScanRequest {
            db_path: self.paths.db_file.clone(),
            installed: self.installed(),
            force,
            threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8),
        };
        self.watcher.rebase();
        if self.sync_scan {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            match anyfont_core::scan::scan(&mut self.db, &req, &mut |_| {}, &cancel) {
                Ok(r) => log::info!("scan: {} files, {} faces", r.files_found, r.faces),
                Err(e) => log::error!("scan failed: {e}"),
            }
            self.scan_finished();
            return;
        }
        log::info!("scan started");
        self.scan = Some(anyfont_core::scan::spawn_scan(req));
    }

    /// True while a scan runs on its thread.
    pub(crate) fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    fn poll_scan(&mut self) {
        let Some(h) = &self.scan else { return };
        let mut finished = false;
        for p in h.poll() {
            match p {
                ScanProgress::Started { total, to_parse } => self.scan_progress = (0, to_parse.max(1).min(total.max(1))),
                ScanProgress::File { done, total, .. } => self.scan_progress = (done, total),
                ScanProgress::Finished(r) => {
                    log::info!("scan finished: {} files ({} parsed), {} faces in {} ms", r.files_found, r.files_parsed, r.faces, r.elapsed_ms);
                    finished = true;
                }
            }
        }
        if finished || h.is_finished() {
            if let Some(h) = self.scan.take() {
                if let Err(e) = h.join() {
                    log::error!("scan failed: {e}");
                }
            }
            self.scan_finished();
        }
    }

    fn scan_finished(&mut self) {
        self.db_ready = true;
        self.previews.clear();
        self.reload();
        if self.sel.keys.is_empty() && self.sel.current.is_none() {
            self.select_first();
        }
        if let Some(fid) = self.type1_check.take() {
            let ids: Vec<FontId> = self
                .catalog
                .library_fonts(&LibraryRef::Folder(fid))
                .iter()
                .filter(|f| f.technology == anyfont_core::model::Technology::Type1)
                .map(|f| f.id)
                .collect();
            if !ids.is_empty() {
                self.matching.open_type1_message(ids);
            }
        }
        if !self.pending_files.is_empty() {
            let files = std::mem::take(&mut self.pending_files);
            self.show_files(files);
        }
        if self.rescan_after {
            self.rescan_after = false;
            self.start_scan(false);
        } else if self.pending_import.is_some() {
            // the imported folders are scanned: their fonts can be linked now
            self.finish_import();
        }
    }

    /// Font files (or folders) from the command line or a later instance: their folder is added as a user
    /// folder (if it is not one yet), opened, and the file's font selected.
    pub(crate) fn show_files(&mut self, files: Vec<PathBuf>) {
        let mut folder: Option<PathBuf> = None;
        let mut file: Option<PathBuf> = None;
        for f in files {
            if f.is_dir() {
                folder = Some(f);
            } else if f.is_file() {
                folder = f.parent().map(Path::to_path_buf);
                file = Some(f);
            }
        }
        let Some(folder) = folder else { return };
        let existing = self.catalog.folders().iter().find(|x| same_path(&x.path, &folder) || x.contains(file.as_deref().unwrap_or(&folder.join("x")))).map(|x| x.id);
        let id: Option<FolderId> = match existing {
            Some(id) => Some(id),
            None => match self.db.add_folder(&folder, false) {
                Ok(f) => {
                    self.pending_files = file.iter().cloned().collect();
                    self.reload();
                    self.open_library(LibraryRef::Folder(f.id));
                    self.start_scan(false);
                    return;
                }
                Err(e) => {
                    log::warn!("cannot add {}: {e}", folder.display());
                    None
                }
            },
        };
        if let Some(id) = id {
            self.open_library(LibraryRef::Folder(id));
            if let Some(file) = file {
                let hit = self.catalog.fonts().iter().find(|f| f.paths.iter().any(|p| same_path(p, &file))).map(|f| f.id);
                if let Some(fid) = hit {
                    self.select_font(fid);
                }
            }
        }
    }

    /// Selects one face and scrolls it into view.
    pub(crate) fn select_font(&mut self, id: FontId) {
        if let Some(i) = self.result.rows.iter().position(|r| matches!(r, anyfont_core::ListRow::Font { id: x, recent: false, .. } if *x == id)) {
            self.sel = Selection::default();
            self.sel.keys.insert(SelKey::Font(id));
            self.sel.current = Some(id);
            self.sel.cursor = Some(i);
            self.sel.anchor = Some(i);
            self.list.scroll_to = Some(i);
            self.props.on_selection_changed();
        }
    }

    // ------------------------------------------------------------------ window

    /// Turns the start window into the main window: placement from the settings.
    pub(crate) fn restore_window(&mut self, ctx: &egui::Context, from_splash: bool) {
        if self.test_mode {
            return;
        }
        let w = &self.settings.core.window;
        let size = vec2(w.size[0].max(800.0), w.size[1].max(600.0));
        if from_splash {
            ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(vec2(800.0, 600.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
        let pos = w.position.map(|p| pos2(p[0], p[1])).filter(|p| {
            let mon = ctx.input(|i| i.viewport().monitor_size).unwrap_or(vec2(1920.0, 1080.0));
            p.x > -50.0 && p.y > -50.0 && p.x < mon.x * 2.0 && p.y < mon.y * 1.5
        });
        match pos {
            Some(p) => ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p)),
            None => {
                if let Some(mon) = ctx.input(|i| i.viewport().monitor_size) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos2(((mon.x - size.x) / 2.0).max(0.0), ((mon.y - size.y) / 2.0).max(0.0))));
                }
            }
        }
        if w.maximized {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
    }

    fn install_frame(&mut self, frame: &eframe::Frame) {
        if self.frame_ready || self.test_mode || self.splash.is_some() {
            return;
        }
        self.frame_ready = true;
        if self.hwnd.is_none() {
            self.hwnd = crate::platform::hwnd_of(frame);
        }
        if let Some(h) = self.hwnd {
            anyfont_sys::set_owner_window(h);
            crate::platform::install_frame(h);
        }
    }

    fn track_window(&mut self, ctx: &egui::Context) {
        let (max, min, outer, inner) = ctx.input(|i| {
            let v = i.viewport();
            (v.maximized.unwrap_or(false), v.minimized.unwrap_or(false), v.outer_rect, v.inner_rect)
        });
        if self.splash.is_some() || self.test_mode {
            return;
        }
        let w = &mut self.settings.core.window;
        if !min {
            w.maximized = max;
            if !max {
                if let (Some(o), Some(i)) = (outer, inner) {
                    w.position = Some([o.min.x, o.min.y]);
                    w.size = [i.width(), i.height()];
                    self.last_restored = Some((o.min, i.size()));
                }
            }
        }
    }

    fn apply_appearance(&mut self, ctx: &egui::Context) {
        if theme::scheme() != self.settings.scheme {
            theme::set_scheme(ctx, self.settings.scheme);
            self.previews.clear();
        }
        let zoom = (self.settings.core.ui_scale_percent / 100.0).clamp(0.75, 2.25);
        if (zoom - self.applied_zoom).abs() > 0.001 {
            self.applied_zoom = zoom;
            if !self.test_mode || (zoom - 1.0).abs() > 0.001 {
                ctx.set_zoom_factor(zoom);
            }
            self.previews.clear();
        }
        let border = self.settings.core.window_border_color;
        if self.applied_border != Some(border) && self.splash.is_none() {
            self.applied_border = Some(border);
            if let Some(h) = self.hwnd {
                let c = border.unwrap_or_else(|| {
                    let c = t().window_border;
                    [c.r(), c.g(), c.b()]
                });
                crate::platform::set_border_color(h, c);
            }
        }
        if crate::i18n::sync(&self.lang_setting) {
            theme::install_fonts(ctx);
        }
    }

    // ------------------------------------------------------------------ persistence

    fn persist(&mut self, now: Instant) {
        self.settings.core.sample_text = if self.sample.trim().is_empty() { anyfont_core::settings::SAMPLE_FONT_NAME.to_string() } else { self.sample.clone() };
        if self.settings != self.saved_settings {
            match self.save_due {
                None => self.save_due = Some(now + Duration::from_millis(800)),
                Some(due) if now >= due => self.save_settings(),
                _ => {}
            }
        }
    }

    pub(crate) fn save_settings(&mut self) {
        self.save_due = None;
        self.settings.sanitize();
        if let Err(e) = self.settings.save(&self.paths.settings_file) {
            log::error!("cannot save the settings {}: {e}", self.paths.settings_file.display());
        }
        self.saved_settings = self.settings.clone();
    }

    // ------------------------------------------------------------------ background work

    fn poll_background(&mut self, ctx: &egui::Context) {
        self.poll_scan();
        if let Some(rx) = &self.incoming {
            let msgs: Vec<_> = rx.try_iter().collect();
            for m in msgs {
                log::info!("message from another instance: {} file(s)", m.files.len());
                if let Some(h) = self.hwnd {
                    crate::platform::bring_to_front(h);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                if !m.files.is_empty() {
                    if self.db_ready {
                        self.show_files(m.files);
                    } else {
                        self.pending_files.extend(m.files);
                    }
                }
            }
        }
        dialogs::poll_operation(self, ctx);
        self.props.poll(ctx);
        if self.scan.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// The list needs a manual refresh (UI-SPEC §11 #5).
    pub(crate) fn refresh_needed(&self) -> bool {
        self.watcher.changed() && self.scan.is_none()
    }

    // ------------------------------------------------------------------ keyboard

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use crate::commands::Cmd;
        use egui::{Key, Modifiers};
        if ctx.memory(|m| m.top_modal_layer().is_some()) || self.menu.is_open() || self.rename.is_some() {
            return;
        }
        let text_focus = ctx.memory(|m| m.focused()).is_some() && ctx.egui_wants_keyboard_input();
        let mut run: Vec<Cmd> = Vec::new();
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT, Key::I) {
                run.push(Cmd::DebugInspector);
            }
            if i.consume_key(Modifiers::CTRL | Modifiers::ALT, Key::I) {
                run.push(Cmd::Inspectors);
            }
            if !text_focus {
                if i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::A) {
                    run.push(Cmd::DeselectAll);
                } else if i.consume_key(Modifiers::CTRL, Key::A) {
                    run.push(Cmd::SelectAll);
                }
                if i.consume_key(Modifiers::NONE, Key::F2) && self.focus == Focus::Libraries {
                    if let LibraryRef::Collection(id) = self.library {
                        run.push(Cmd::RenameLibrary(id));
                    }
                }
            }
            if i.consume_key(Modifiers::ALT, Key::F4) {
                run.push(Cmd::Exit);
            }
        });
        for c in run {
            self.run(ctx, c);
        }
    }

    // ------------------------------------------------------------------ frame

    fn main_ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, t().panel_bg);
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false)) && crate::platform::frame_installed();
        let title_h = if maximized { 25.0 } else { 32.0 };
        let title = Rect::from_min_size(full.min, vec2(full.width(), title_h));
        let menubar = Rect::from_min_size(pos2(full.left(), title.bottom()), vec2(full.width(), 24.0));
        let toolbar = Rect::from_min_size(pos2(full.left(), menubar.bottom()), vec2(full.width(), 33.0));
        let top_line = toolbar.bottom();
        let status = Rect::from_min_max(pos2(full.left(), full.bottom() - 32.0), full.max);
        let body = Rect::from_min_max(pos2(full.left(), top_line + 1.0), pos2(full.right(), status.top() - 1.0));

        crate::titlebar::draw(self, ui, title, maximized, frame);
        crate::toolbar::draw(self, ui, toolbar);
        ui.painter().hline(full.x_range(), top_line + 0.5, egui::Stroke::new(1.0, t().line_strong));

        // columns
        let left_w = self.settings.core.window.left_panel_width.clamp(crate::settings::LEFT_PANEL_MIN, (full.width() * 0.5).max(crate::settings::LEFT_PANEL_MIN));
        let left = Rect::from_min_max(body.min, pos2(body.left() + left_w, body.bottom()));
        let props_visible = self.settings.core.show_font_properties;
        let right_w = self.settings.core.window.right_panel_width.clamp(crate::settings::RIGHT_PANEL_MIN, crate::settings::RIGHT_PANEL_MAX.min(full.width() * 0.45));
        let right = props_visible.then(|| Rect::from_min_max(pos2(body.right() - right_w, body.top()), pos2(body.right(), status.bottom())));
        let content_right = right.map(|r| r.left() - 1.0).unwrap_or(body.right());
        let content = Rect::from_min_max(pos2(left.right() + 4.0, body.top()), pos2(content_right, body.bottom()));

        crate::libraries::draw_left_panel(self, ui, left);
        // left divider + splitter
        ui.painter().vline(left.right() + 0.5, (body.top() + 5.0)..=body.bottom() + 1.0, egui::Stroke::new(1.0, t().line_divider));
        let split = Rect::from_min_max(pos2(left.right() - 2.0, body.top()), pos2(left.right() + 3.0, body.bottom()));
        let sr = ui.interact(split, egui::Id::unique("left-splitter"), egui::Sense::drag());
        if sr.hovered() || sr.dragged() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if sr.dragged() {
            self.settings.core.window.left_panel_width = (left_w + sr.drag_delta().x).clamp(crate::settings::LEFT_PANEL_MIN, full.width() * 0.5);
        }

        crate::fontlist::draw_content(self, ui, content);

        if let Some(r) = right {
            ui.painter().vline(r.left() - 0.5, body.top()..=status.bottom(), egui::Stroke::new(1.0, t().line_divider));
            let rs = Rect::from_min_max(pos2(r.left() - 3.0, body.top()), pos2(r.left() + 2.0, status.top()));
            let rr = ui.interact(rs, egui::Id::unique("right-splitter"), egui::Sense::drag());
            if rr.hovered() || rr.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if rr.dragged() {
                self.settings.core.window.right_panel_width = (right_w - rr.drag_delta().x).clamp(crate::settings::RIGHT_PANEL_MIN, crate::settings::RIGHT_PANEL_MAX);
            }
            crate::properties::draw(self, ui, r);
        }

        // status bar (the line has a gap where the left divider meets it)
        let sl = status.top() - 0.5;
        ui.painter().hline(full.left()..=left.right() - 4.0, sl, egui::Stroke::new(1.0, t().line_strong));
        ui.painter().hline(left.right() + 4.0..=content_right, sl, egui::Stroke::new(1.0, t().line_strong));
        let sb = Rect::from_min_max(pos2(content.left(), status.top()), pos2(content_right, status.bottom()));
        crate::statusbar::draw(self, ui, sb);

        // the menu bar last among the bands: its popups float over everything
        crate::menus::menu_bar(ui, menubar, &mut self.menu, self.frame_no);
        self.show_menus(&ctx);
    }

    fn show_menus(&mut self, ctx: &egui::Context) {
        let Some(mut open) = self.menu.open.take() else { return };
        let items = self.menu_items(&open.source);
        let out = crate::menus::show(ctx, &mut open, &items, self.frame_no);
        match out {
            crate::menus::Outcome::None => self.menu.open = Some(open),
            crate::menus::Outcome::Close => self.menu.close(),
            crate::menus::Outcome::Run(cmd) => {
                self.menu.close();
                self.run(ctx, cmd);
            }
            crate::menus::Outcome::Neighbour(d) => {
                if let crate::menus::Source::Bar(i) = open.source {
                    let n = crate::menus::BAR.len() as i32;
                    let j = ((i as i32 + d + n) % n) as usize;
                    let x = egui::Area::new(egui::Id::unique("menubar-measure"))
                        .fixed_pos(pos2(-10000.0, -10000.0))
                        .show(ctx, |ui| crate::menus::bar_boxes(ui, Rect::from_min_size(pos2(0.0, 0.0), vec2(2000.0, 24.0)))[j].left())
                        .inner;
                    self.menu.open(crate::menus::Source::Bar(j), pos2(x - 1.0, open.pos.y), true, self.frame_no);
                    if let Some(o) = &mut self.menu.open {
                        o.hot = vec![Some(usize::MAX)];
                    }
                }
            }
        }
    }

    fn services(&mut self, ctx: &egui::Context) {
        self.about.ui(ctx);
        self.bugreport.ui(ctx);
        let downloaded = self.getmore.ui(ctx, &self.content_dir);
        if !downloaded.is_empty() {
            log::info!("{} font file(s) downloaded", downloaded.len());
            self.ensure_content_folder();
            self.start_scan(false);
        }
        if self.dialogs.is_empty() && !self.matching.is_open() {
            if let Some(missing) = self.pending_results.take() {
                self.matching.open_results(missing);
            }
        }
        if self.matching.ui(ctx, &mut self.db, &self.catalog) {
            self.reload();
        }
        if let Some(resolved) = self.matching.take_resolved() {
            self.link_substitutes(&resolved);
        }
        if let Some(req) = self.updater.ui(ctx) {
            match req {
                crate::updater::UpdaterRequest::ExitAndInstall(path) => {
                    *self.updater_out.lock() = Some(path);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if let Some(m) = self.updater.take_mode_change() {
            self.settings.core.update_mode = m;
        }
    }

    /// Makes sure the content folder is a (hidden) library folder after a download.
    pub(crate) fn ensure_content_folder(&mut self) {
        let dir = self.fs.content_folder();
        self.content_dir = dir.clone();
        if !self.catalog.folders().iter().any(|f| same_path(&f.path, &dir)) {
            let _ = self.db.add_folder(&dir, true);
            self.reload();
        }
    }

    /// True for the hidden content folder.
    pub(crate) fn is_content_folder(&self, p: &Path) -> bool {
        same_path(p, &self.content_dir)
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        if !self.inspector_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Inspectors").open(&mut open).default_size(vec2(480.0, 560.0)).vscroll(true).show(ctx, |ui| {
            egui::CollapsingHeader::new("Settings").show(ui, |ui| ctx.settings_ui(ui));
            egui::CollapsingHeader::new("Inspection").show(ui, |ui| ctx.inspection_ui(ui));
            egui::CollapsingHeader::new("Memory").show(ui, |ui| ctx.memory_ui(ui));
        });
        self.inspector_open = open;
    }
}

/// Hooks for the end-to-end tests (`tests/*.rs`) and automation: the state a test checks, and the
/// operations whose file pickers a test cannot drive.
impl AnyFontApp {
    /// The status line text ("My fonts: 12 fonts").
    pub fn status_text(&self) -> String {
        crate::statusbar::status_text(self)
    }
    /// The labels of the font list rows in display order (family headers and faces).
    pub fn row_labels(&self) -> Vec<String> {
        use anyfont_core::ListRow;
        self.result
            .rows
            .iter()
            .filter_map(|r| match r {
                ListRow::FamilyHeader { family, .. } => Some(family.clone()),
                ListRow::Font { label, .. } => Some(label.clone()),
                ListRow::RecentHeader { .. } => None,
            })
            .collect()
    }
    /// The faces of the font list with their installed state, in display order.
    pub fn face_states(&self) -> Vec<(String, bool)> {
        self.result
            .rows
            .iter()
            .filter_map(|r| match r {
                anyfont_core::ListRow::Font { id, label, .. } => self.catalog.font(*id).map(|f| (label.clone(), f.installed)),
                _ => None,
            })
            .collect()
    }
    /// The number of faces the current library shows.
    pub fn font_count(&self) -> usize {
        self.result.count
    }
    /// The user folders and collections (names as the Libraries panel shows them, content folder excluded).
    pub fn library_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.catalog.folders().iter().filter(|f| !self.is_content_folder(&f.path)).map(|f| f.name()).collect();
        v.extend(self.catalog.collections().iter().map(crate::commands::collection_caption));
        v
    }
    /// File ▸ Add Folder with the folder already picked.
    pub fn add_folder_path(&mut self, dir: &Path) {
        self.add_folder(dir);
    }
    /// File ▸ Import with the file already picked.
    pub fn import_path(&mut self, file: &Path) {
        self.import(file);
    }
    /// File ▸ Export with the file already picked.
    pub fn export_path(&mut self, file: &Path) {
        self.export(file);
    }
    /// The number of queued dialogs (messages, confirmations).
    pub fn pending_dialogs(&self) -> usize {
        self.dialogs.len()
    }
    /// True while a font operation (install, uninstall, delete) is running.
    pub fn operation_running(&self) -> bool {
        self.op.is_some() || self.pending_delete.is_some()
    }
    /// The font system the window works on (the sandbox in tests).
    pub fn font_system(&self) -> Arc<dyn FontSystem> {
        self.fs.clone()
    }
    /// The settings as they are now (saved on a change after a short delay, and on exit).
    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }
}

/// Case-insensitive path comparison.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    anyfont_core::db::normalize_path(a) == anyfont_core::db::normalize_path(b)
}

impl eframe::App for AnyFontApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_no += 1;
        let now = Instant::now();
        self.apply_appearance(&ctx);
        self.poll_background(&ctx);
        self.previews.begin_frame(&ctx);

        if self.splash.is_some() {
            if splash::show(self, ui) {
                self.previews.end_frame();
                return;
            }
            self.restore_window(&ctx, true);
        }
        self.install_frame(frame);
        self.track_window(&ctx);

        self.main_ui(ui, frame);
        self.shortcuts(&ctx);
        dialogs::show(self, &ctx);
        crate::options::show(self, &ctx);
        self.services(&ctx);
        self.inspector(&ctx);

        self.previews.end_frame();
        self.persist(now);
        if self.save_due.is_some() {
            ctx.request_repaint_after(Duration::from_millis(900));
        }
    }

    fn on_exit(&mut self) {
        if let Some(h) = &self.scan {
            h.cancel();
        }
        if let Some(op) = &self.op {
            op.cancel();
        }
        self.settings.core.sample_text = if self.sample.trim().is_empty() { anyfont_core::settings::SAMPLE_FONT_NAME.to_string() } else { self.sample.clone() };
        self.save_settings();
        log::info!("AnyFont closed");
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = t().panel_bg;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }
}
