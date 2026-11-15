//! File ▸ Get More... – the online font catalog (owned by the services UI developer; UI-SPEC §7.2.7).
//!
//! The original hosts a store page in this frame (`736dc77a`); AnyFont shows the Google Fonts catalog there
//! instead (open licences, no account, nothing bought):
//!
//! - the catalog is `https://fonts.google.com/metadata/fonts` (its `)]}'` guard is stripped), cached in
//!   `%LOCALAPPDATA%\AnyFont\catalog.json` and refreshed once a day; offline, the cached copy is used, and
//!   with none the list says "Remote font catalog cannot connect to the internet.";
//! - search (family and designer), **Sort by:** Featured / Popular / Name / Newest, **Filter by:** Serif,
//!   Sans Serif, Display, Handwriting, Monospace, Free (always), My Library (already downloaded);
//! - each row draws the family name in the family: a subset with just those letters is fetched on demand
//!   from the Google Fonts CSS API (`text=`), cached in `%LOCALAPPDATA%\AnyFont\previews`, and rasterised
//!   here (ttf-parser + tiny-skia, no change to the global egui fonts); until then the name is shown in the UI font;
//! - Download fetches the family's files listed by `https://fonts.google.com/download/list?family=…` (the
//!   variable fonts when the family has them, else the static ones) and its licence into
//!   `content_dir/<Family>/`, with the operation-progress dialog (§7.2.12: "Downloading {}... {}%",
//!   Cancel / Close). `ui` returns the downloaded font files; the main window rescans and they appear in
//!   My fonts as Not installed.
//!
//! `ANYFONT_CATALOG_URL` (a URL or a local JSON file) replaces the catalog source for tests.

use crate::help::dlg;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// The catalog.
pub const CATALOG_URL: &str = "https://fonts.google.com/metadata/fonts";
/// The file list of one family (`?family=`).
pub const DOWNLOAD_LIST_URL: &str = "https://fonts.google.com/download/list";
/// The CSS API used for the preview subsets.
pub const CSS_URL: &str = "https://fonts.googleapis.com/css2";
/// The cached catalog is refreshed after a day.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The categories of the Filter by panel, in the catalog's words.
pub const CATEGORIES: [&str; 5] = ["Serif", "Sans Serif", "Display", "Handwriting", "Monospace"];

const NO_INTERNET: &str = "Remote font catalog cannot connect to the internet.";
const BAD_CATALOG: &str = "An error occurred while downloading the remote font catalog.";

// ------------------------------------------------------------------------------------------------
// catalog data
// ------------------------------------------------------------------------------------------------

/// One family of the catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct Family {
    pub name: String,
    /// "Sans Serif", "Serif", "Display", "Handwriting", "Monospace"
    pub category: String,
    pub designers: Vec<String>,
    /// Number of styles ("400", "400i", "700" …).
    pub styles: usize,
    /// 1 = most popular.
    pub popularity: u32,
    /// The catalog's own order ("Featured").
    pub default_sort: u32,
    /// "2012-09-30"
    pub date_added: String,
    /// The style the preview is drawn in: (weight, italic).
    pub preview_style: (u16, bool),
    /// Download size in bytes (0 when unknown).
    pub size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Featured,
    Popular,
    Name,
    Newest,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::Featured, Sort::Popular, Sort::Name, Sort::Newest];
    pub fn caption(self) -> &'static str {
        match self {
            Sort::Featured => "Featured",
            Sort::Popular => "Popular",
            Sort::Name => "Name",
            Sort::Newest => "Newest",
        }
    }
}

/// The JSON without Google's `)]}'` guard line.
pub fn strip_guard(text: &str) -> &str {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    match t.strip_prefix(")]}'") {
        Some(rest) => rest.trim_start(),
        None => t,
    }
}

/// The preview style of a family: regular when it has one, else the lightest upright, else the first.
fn preview_style(keys: &[String]) -> (u16, bool) {
    let parse = |k: &str| -> Option<(u16, bool)> {
        let italic = k.ends_with('i');
        k.trim_end_matches('i').parse::<u16>().ok().map(|w| (w, italic))
    };
    let styles: Vec<(u16, bool)> = keys.iter().filter_map(|k| parse(k)).collect();
    if styles.contains(&(400, false)) {
        return (400, false);
    }
    styles.iter().filter(|s| !s.1).min_by_key(|s| s.0.abs_diff(400)).or_else(|| styles.first()).copied().unwrap_or((400, false))
}

/// Reads the catalog (`familyMetadataList` of `metadata/fonts`).
pub fn parse_catalog(text: &str) -> Result<Vec<Family>, String> {
    let v: serde_json::Value = serde_json::from_str(strip_guard(text)).map_err(|e| e.to_string())?;
    let list = v["familyMetadataList"].as_array().ok_or("no familyMetadataList")?;
    let mut out = Vec::with_capacity(list.len());
    for f in list {
        let Some(name) = f["family"].as_str().filter(|s| !s.is_empty()) else { continue };
        let keys: Vec<String> = f["fonts"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        out.push(Family {
            name: name.to_string(),
            category: f["category"].as_str().unwrap_or("").to_string(),
            designers: f["designers"].as_array().map(|a| a.iter().filter_map(|d| d.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            styles: keys.len().max(1),
            popularity: f["popularity"].as_u64().unwrap_or(u64::from(u32::MAX)) as u32,
            default_sort: f["defaultSort"].as_u64().unwrap_or(u64::from(u32::MAX)) as u32,
            date_added: f["dateAdded"].as_str().unwrap_or("").to_string(),
            preview_style: preview_style(&keys),
            size: f["size"].as_u64().unwrap_or(0),
        });
    }
    if out.is_empty() {
        return Err("the catalog is empty".into());
    }
    Ok(out)
}

/// The rows to show: search (family or designer, any case), categories (none checked = all), My Library,
/// then the sort. Returns indices into `families`.
pub fn filter_sort(families: &[Family], search: &str, categories: &HashSet<String>, my_library: bool, downloaded: &HashSet<String>, sort: Sort) -> Vec<usize> {
    let q = search.trim().to_lowercase();
    let mut idx: Vec<usize> = families
        .iter()
        .enumerate()
        .filter(|(_, f)| q.is_empty() || f.name.to_lowercase().contains(&q) || f.designers.iter().any(|d| d.to_lowercase().contains(&q)))
        .filter(|(_, f)| categories.is_empty() || categories.contains(&f.category))
        .filter(|(_, f)| !my_library || downloaded.contains(&folder_name(&f.name)))
        .map(|(i, _)| i)
        .collect();
    match sort {
        Sort::Featured => idx.sort_by_key(|&i| (families[i].default_sort, families[i].popularity)),
        Sort::Popular => idx.sort_by_key(|&i| families[i].popularity),
        Sort::Name => idx.sort_by_key(|&i| families[i].name.to_lowercase()),
        Sort::Newest => idx.sort_by(|&a, &b| families[b].date_added.cmp(&families[a].date_added).then(families[a].popularity.cmp(&families[b].popularity))),
    }
    // a search puts names that start with the text first
    if !q.is_empty() {
        idx.sort_by_key(|&i| !families[i].name.to_lowercase().starts_with(&q));
    }
    idx
}

/// The folder of a family under the Content folder: the name without the characters Windows forbids.
pub fn folder_name(family: &str) -> String {
    let s: String = family.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() { "_".into() } else { s }
}

fn is_font_file(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".ttf") || n.ends_with(".otf")
}

/// The families already in the Content folder (a folder with at least one font file).
pub fn downloaded_families(content_dir: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(rd) = std::fs::read_dir(content_dir) else { return out };
    for e in rd.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let has_font = walk_fonts(&e.path()).next().is_some();
        if has_font {
            out.insert(e.file_name().to_string_lossy().into_owned());
        }
    }
    out
}

fn walk_fonts(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    let mut files = vec![];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if is_font_file(&e.file_name().to_string_lossy()) {
                    files.push(p);
                }
            }
        }
    }
    files.into_iter()
}

// ------------------------------------------------------------------------------------------------
// download list (manifest)
// ------------------------------------------------------------------------------------------------

/// What `download/list` says about one family.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Manifest {
    /// Files given inline (the licence, README): (name, contents).
    pub inline: Vec<(String, String)>,
    /// Font files to fetch: (name relative to the family folder, URL).
    pub fonts: Vec<(String, String)>,
}

/// Reads `download/list`: the variable fonts when there are any at the top level, else the static ones;
/// inline licence files are kept.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let v: serde_json::Value = serde_json::from_str(strip_guard(text)).map_err(|e| e.to_string())?;
    let m = &v["manifest"];
    let inline = m["files"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|f| Some((f["filename"].as_str()?.to_string(), f["contents"].as_str().unwrap_or("").to_string())))
                .filter(|(n, _)| safe_relative(n))
                .collect()
        })
        .unwrap_or_default();
    let refs: Vec<(String, String)> = m["fileRefs"]
        .as_array()
        .map(|a| a.iter().filter_map(|f| Some((f["filename"].as_str()?.to_string(), f["url"].as_str()?.to_string()))).collect())
        .unwrap_or_default();
    let fonts: Vec<(String, String)> = refs.into_iter().filter(|(n, u)| is_font_file(n) && safe_relative(n) && u.starts_with("https://")).collect();
    let top: Vec<(String, String)> = fonts.iter().filter(|(n, _)| !n.contains('/')).cloned().collect();
    let fonts = if top.is_empty() { fonts } else { top };
    if fonts.is_empty() {
        return Err("no font files".into());
    }
    Ok(Manifest { inline, fonts })
}

/// A file name from the manifest that stays inside the family folder.
fn safe_relative(name: &str) -> bool {
    !name.is_empty() && !name.starts_with(['/', '\\']) && !name.contains(':') && !name.split(['/', '\\']).any(|p| p == ".." || p.is_empty())
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The file list address of a family.
pub fn download_list_url(family: &str) -> String {
    format!("{DOWNLOAD_LIST_URL}?family={}", url_encode(family))
}

/// The CSS address of a preview subset: the family in its preview style with only the letters of `text`.
pub fn preview_css_url(f: &Family, text: &str) -> String {
    let fam = url_encode(&f.name).replace("%20", "+");
    let (w, italic) = f.preview_style;
    let spec = if italic { format!(":ital,wght@1,{w}") } else if w != 400 { format!(":wght@{w}") } else { String::new() };
    format!("{CSS_URL}?family={fam}{}&text={}", url_encode(&spec).replace("%3A", ":").replace("%40", "@").replace("%2C", ","), url_encode(text))
}

/// The first `url(...)` of a CSS answer.
pub fn css_font_url(css: &str) -> Option<String> {
    let a = css.find("url(")? + 4;
    let b = css[a..].find(')')? + a;
    let u = css[a..b].trim().trim_matches(['\'', '"']);
    u.starts_with("https://").then(|| u.to_string())
}

// ------------------------------------------------------------------------------------------------
// network and files
// ------------------------------------------------------------------------------------------------

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(secs))).build().into()
}

fn user_agent() -> String {
    // a plain agent: the CSS API then answers with TrueType files
    format!("AnyFont/{}", env!("CARGO_PKG_VERSION"))
}

fn get_bytes(url: &str, secs: u64) -> Result<Vec<u8>, String> {
    let url = &crate::testing::redirect(url);
    let mut r = agent(secs).get(url).header("User-Agent", &user_agent()).call().map_err(|_| NO_INTERNET.to_string())?;
    if r.status().as_u16() != 200 {
        return Err(format!("HTTP {}", r.status().as_u16()));
    }
    r.body_mut().with_config().limit(256 << 20).read_to_vec().map_err(|_| NO_INTERNET.to_string())
}

fn get_text(url: &str) -> Result<String, String> {
    get_bytes(url, 30).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `%LOCALAPPDATA%\AnyFont`.
pub fn cache_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("AnyFont")
}

fn catalog_override() -> Option<String> {
    std::env::var("ANYFONT_CATALOG_URL").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Loaded catalog: the families and whether it is an old copy because the catalog could not be reached.
type Loaded = Result<(Vec<Family>, bool), String>;

/// Reads the catalog: the override, else the cache when it is less than a day old, else the web (written
/// to the cache), else an older cache.
fn load_catalog(force: bool) -> Loaded {
    if let Some(src) = catalog_override() {
        let text = if src.starts_with("http://") || src.starts_with("https://") { get_text(&src)? } else { std::fs::read_to_string(&src).map_err(|_| BAD_CATALOG.to_string())? };
        return parse_catalog(&text).map(|f| (f, false)).map_err(|_| BAD_CATALOG.to_string());
    }
    let cache = cache_dir().join("catalog.json");
    let fresh = std::fs::metadata(&cache).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < MAX_AGE);
    if fresh && !force {
        if let Ok(f) = std::fs::read_to_string(&cache).map_err(|e| e.to_string()).and_then(|t| parse_catalog(&t)) {
            return Ok((f, false));
        }
    }
    match get_text(CATALOG_URL).and_then(|t| parse_catalog(&t).map(|f| (f, t))) {
        Ok((f, text)) => {
            let _ = std::fs::create_dir_all(cache_dir());
            let tmp = cache.with_extension("tmp");
            if std::fs::write(&tmp, strip_guard(&text)).is_ok() {
                let _ = std::fs::rename(&tmp, &cache);
            }
            Ok((f, false))
        }
        Err(e) => match std::fs::read_to_string(&cache).map_err(|e| e.to_string()).and_then(|t| parse_catalog(&t)) {
            Ok(f) => Ok((f, true)),
            Err(_) => Err(if e == NO_INTERNET { NO_INTERNET.to_string() } else { BAD_CATALOG.to_string() }),
        },
    }
}

/// Fetches (or reads from the cache) the preview subset of a family.
fn fetch_preview(f: &Family) -> Option<Vec<u8>> {
    let dir = cache_dir().join("previews");
    let path = dir.join(format!("{}.ttf", folder_name(&f.name)));
    if let Ok(b) = std::fs::read(&path) {
        if !b.is_empty() {
            return Some(b);
        }
    }
    let css = get_text(&preview_css_url(f, &f.name)).ok()?;
    let bytes = get_bytes(&css_font_url(&css)?, 30).ok()?;
    ttf_parser::Face::parse(&bytes, 0).ok()?;
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, &bytes);
    Some(bytes)
}

/// The family name rasterised in the family, as a white mask (tinted when drawn), at most `max` pixels.
pub fn render_text(font: &[u8], text: &str, px: f32, max: egui::Vec2) -> Option<egui::ColorImage> {
    struct Builder<'a> {
        pb: &'a mut tiny_skia::PathBuilder,
        x: f32,
        y: f32,
        s: f32,
    }
    impl ttf_parser::OutlineBuilder for Builder<'_> {
        fn move_to(&mut self, x: f32, y: f32) {
            self.pb.move_to(self.x + x * self.s, self.y - y * self.s);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.pb.line_to(self.x + x * self.s, self.y - y * self.s);
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            self.pb.quad_to(self.x + x1 * self.s, self.y - y1 * self.s, self.x + x * self.s, self.y - y * self.s);
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            self.pb.cubic_to(self.x + x1 * self.s, self.y - y1 * self.s, self.x + x2 * self.s, self.y - y2 * self.s, self.x + x * self.s, self.y - y * self.s);
        }
        fn close(&mut self) {
            self.pb.close();
        }
    }
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let upem = f32::from(face.units_per_em());
    let glyphs: Vec<ttf_parser::GlyphId> = text.chars().map(|c| face.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0))).collect();
    let advance: f32 = glyphs.iter().map(|g| f32::from(face.glyph_hor_advance(*g).unwrap_or(0))).sum();
    let (asc, desc) = (f32::from(face.ascender()), f32::from(face.descender()));
    let height = (asc - desc).max(1.0);
    if advance <= 0.0 {
        return None;
    }
    let px = px.min((max.x - 4.0) * upem / advance).min((max.y - 4.0) * upem / height).max(4.0);
    let s = px / upem;
    let w = (advance * s).ceil() as u32 + 4;
    let h = (height * s).ceil() as u32 + 4;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    let mut pb = tiny_skia::PathBuilder::new();
    let mut pen = 2.0;
    for g in &glyphs {
        let mut b = Builder { pb: &mut pb, x: pen, y: 2.0 + asc * s, s };
        let _ = face.outline_glyph(*g, &mut b);
        pen += f32::from(face.glyph_hor_advance(*g).unwrap_or(0)) * s;
    }
    let path = pb.finish()?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;
    pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    Some(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()))
}

enum DownloadMsg {
    Progress(f32),
    Done(Result<Vec<PathBuf>, String>),
}

/// Downloads one family into `dest` (its folder); `cancel` stops after the current file and removes what
/// this download wrote.
fn download_family(family: &str, dest: &Path, tx: &Sender<DownloadMsg>, cancel: &AtomicBool) -> Result<Vec<PathBuf>, String> {
    let text = get_text(&download_list_url(family))?;
    let m = parse_manifest(&text)?;
    let existed = dest.exists();
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut written: Vec<PathBuf> = vec![];
    let cleanup = |written: &[PathBuf]| {
        for p in written {
            let _ = std::fs::remove_file(p);
        }
        if !existed {
            let _ = std::fs::remove_dir_all(dest);
        }
    };
    for (name, contents) in &m.inline {
        let p = dest.join(name);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&p, contents).is_ok() {
            written.push(p);
        }
    }
    let total = m.fonts.len();
    let mut fonts = vec![];
    for (i, (name, url)) in m.fonts.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cleanup(&written);
            return Err(String::new());
        }
        let data = match get_bytes(url, 300) {
            Ok(d) => d,
            Err(e) => {
                cleanup(&written);
                return Err(e);
            }
        };
        // a variable font's name keeps its axes ("Roboto[wdth,wght].ttf"), which Windows accepts
        let p = dest.join(name.rsplit('/').next().unwrap_or(name));
        if let Err(e) = std::fs::write(&p, &data) {
            cleanup(&written);
            return Err(e.to_string());
        }
        written.push(p.clone());
        fonts.push(p);
        let _ = tx.send(DownloadMsg::Progress((i + 1) as f32 / total as f32));
    }
    Ok(fonts)
}

// ------------------------------------------------------------------------------------------------
// window
// ------------------------------------------------------------------------------------------------

struct Download {
    family: String,
    progress: f32,
    rx: Receiver<DownloadMsg>,
    cancel: Arc<AtomicBool>,
    /// `None` while running; the error ("" = cancelled) or success when done.
    done: Option<Result<(), String>>,
}

pub struct GetMore {
    open: bool,
    families: Vec<Family>,
    /// The catalog is an older copy (no connection).
    stale: bool,
    error: Option<String>,
    loading: Option<Receiver<Loaded>>,
    search: String,
    sort: Sort,
    categories: HashSet<String>,
    my_library: bool,
    downloaded: HashSet<String>,
    downloaded_for: Option<PathBuf>,
    rows: Vec<usize>,
    rows_key: Option<(String, Sort, Vec<String>, bool, usize, usize)>,
    preview_tx: Option<Sender<Family>>,
    preview_rx: Option<Receiver<(String, Option<Vec<u8>>)>>,
    previews_requested: HashSet<String>,
    preview_fonts: HashMap<String, Option<Vec<u8>>>,
    textures: HashMap<String, Option<egui::TextureHandle>>,
    download: Option<Download>,
    scroll_to_top: bool,
}

impl Default for GetMore {
    fn default() -> Self {
        Self::new()
    }
}

impl GetMore {
    pub fn new() -> Self {
        GetMore {
            open: false,
            families: vec![],
            stale: false,
            error: None,
            loading: None,
            search: String::new(),
            sort: Sort::Featured,
            categories: HashSet::new(),
            my_library: false,
            downloaded: HashSet::new(),
            downloaded_for: None,
            rows: vec![],
            rows_key: None,
            preview_tx: None,
            preview_rx: None,
            previews_requested: HashSet::new(),
            preview_fonts: HashMap::new(),
            textures: HashMap::new(),
            download: None,
            scroll_to_top: false,
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.downloaded_for = None;
        if self.loading.is_none() && (self.families.is_empty() || self.stale) {
            self.start_loading(false);
        }
    }

    /// True while the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// True while a family is being downloaded.
    pub fn is_downloading(&self) -> bool {
        self.download.as_ref().is_some_and(|d| d.done.is_none())
    }

    fn start_loading(&mut self, force: bool) {
        let (tx, rx) = channel();
        self.loading = Some(rx);
        self.error = None;
        std::thread::spawn(move || {
            let _ = tx.send(load_catalog(force));
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(Ok((f, stale))) => {
                    self.families = f;
                    self.stale = stale;
                    self.error = None;
                    self.loading = None;
                    self.rows_key = None;
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.loading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
                Err(_) => self.loading = None,
            }
        }
        if let Some(rx) = &self.preview_rx {
            let mut any = false;
            while let Ok((name, data)) = rx.try_recv() {
                self.preview_fonts.insert(name, data);
                any = true;
            }
            if any {
                ctx.request_repaint();
            }
        }
    }

    /// Queues the preview of a family (one background thread fetches them in order).
    fn request_preview(&mut self, f: &Family, ctx: &egui::Context) {
        if !self.previews_requested.insert(f.name.clone()) {
            return;
        }
        if self.preview_tx.is_none() {
            let (tx, rx) = channel::<Family>();
            let (rtx, rrx) = channel();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                while let Ok(f) = rx.recv() {
                    let data = fetch_preview(&f);
                    if rtx.send((f.name.clone(), data)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
            self.preview_tx = Some(tx);
            self.preview_rx = Some(rrx);
        }
        if let Some(tx) = &self.preview_tx {
            let _ = tx.send(f.clone());
        }
    }

    fn preview_texture(&mut self, ctx: &egui::Context, f: &Family) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(&f.name) {
            return t.clone();
        }
        let data = match self.preview_fonts.get(&f.name) {
            None => {
                self.request_preview(f, ctx);
                return None;
            }
            Some(d) => d.clone(),
        };
        let ppp = ctx.pixels_per_point();
        let tex = data.and_then(|d| render_text(&d, &f.name, 34.0 * ppp, vec2(157.0, 70.0) * ppp)).map(|img| ctx.load_texture(format!("getmore_{}", f.name), img, egui::TextureOptions::LINEAR));
        self.textures.insert(f.name.clone(), tex.clone());
        tex
    }

    fn start_download(&mut self, family: &str, content_dir: &Path, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let dest = content_dir.join(folder_name(family));
        let name = family.to_string();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = download_family(&name, &dest, &tx, &c);
            let _ = tx.send(DownloadMsg::Done(r));
            ctx.request_repaint();
        });
        self.download = Some(Download { family: family.to_string(), progress: 0.0, rx, cancel, done: None });
    }

    /// `content_dir`: where downloaded fonts go (anyfont_sys content folder). Returns the files downloaded
    /// this frame; the main window then rescans so they appear in My fonts as Not installed.
    pub fn ui(&mut self, ctx: &egui::Context, content_dir: &std::path::Path) -> Vec<PathBuf> {
        let mut finished = vec![];
        // a download goes on (and reports its files) even when the window was closed
        if let Some(d) = &mut self.download {
            let mut done_files = None;
            while let Ok(m) = d.rx.try_recv() {
                match m {
                    DownloadMsg::Progress(p) => d.progress = p,
                    DownloadMsg::Done(r) => {
                        d.done = Some(r.as_ref().map(|_| ()).map_err(|e| e.clone()));
                        done_files = r.ok();
                    }
                }
            }
            if let Some(files) = done_files {
                finished = files;
                self.downloaded_for = None;
            }
            if d.done.is_none() {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if !self.open {
            if self.download.as_ref().is_some_and(|d| d.done.is_some()) {
                self.download = None;
            }
            return finished;
        }
        self.poll(ctx);
        if self.downloaded_for.as_deref() != Some(content_dir) {
            self.downloaded = downloaded_families(content_dir);
            self.downloaded_for = Some(content_dir.to_path_buf());
            self.rows_key = None;
        }
        let mut cats: Vec<String> = self.categories.iter().cloned().collect();
        cats.sort();
        let key = (self.search.clone(), self.sort, cats, self.my_library, self.families.len(), self.downloaded.len());
        if self.rows_key.as_ref() != Some(&key) {
            self.rows = filter_sort(&self.families, &self.search, &self.categories, self.my_library, &self.downloaded, self.sort);
            self.rows_key = Some(key);
            self.scroll_to_top = true;
        }

        let mut to_download: Option<String> = None;
        let progress_open = self.download.is_some();
        let ((), dismissed) = dlg::window(ctx, "get_more", "Get More", vec2(1058.0, 652.0), |ui| {
            let full = ui.max_rect();
            let filter_w = 165.0;
            let gap = 30.0;
            let list_area = Rect::from_min_max(pos2(full.left() + 20.0, full.top() + 40.0), pos2(full.right() - filter_w - gap - 4.0, full.bottom() - 20.0));
            let filter_area = Rect::from_min_max(pos2(list_area.right() + gap, list_area.top()), pos2(full.right() - 4.0, list_area.top() + 350.0));
            ui.add_enabled_ui(!progress_open, |ui| {
                self.top_row(ui, Rect::from_min_size(list_area.min, vec2(list_area.width(), 32.0)));
                let list = Rect::from_min_max(pos2(list_area.left(), list_area.top() + 34.0), list_area.max);
                to_download = self.list_ui(ui, list);
                self.filter_ui(ui, filter_area);
            });
        });
        if let Some(f) = to_download {
            self.start_download(&f, content_dir, ctx);
        }
        if self.progress_ui(ctx) {
            self.download = None;
        }
        if dismissed && !progress_open {
            self.open = false;
        }
        finished
    }

    /// Search box with ×, Search button, Sort by.
    fn top_row(&mut self, ui: &mut egui::Ui, r: Rect) {
        let dark = ui.visuals().dark_mode;
        ui.painter().rect_filled(r, 0.0, if dark { Color32::from_gray(0x2B) } else { Color32::from_gray(0xE8) });
        dlg::at_left(ui, r.shrink2(vec2(8.0, 4.0)), |ui| {
            let te = egui::TextEdit::singleline(&mut self.search).hint_text("Search").desired_width(150.0);
            let resp = ui.add(te).accessible_name("Search");
            if !self.search.is_empty() && ui.add(egui::Button::new("×").frame(false)).on_hover_text("Clear").clicked() {
                self.search.clear();
                resp.request_focus();
            }
            if dlg::button(ui, "Search", true) {
                // the list already follows the box; the button keeps the original's layout
                self.rows_key = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("getmore_sort").selected_text(self.sort.caption()).width(110.0).show_ui(ui, |ui| {
                    for s in Sort::ALL {
                        ui.selectable_value(&mut self.sort, s, s.caption());
                    }
                })
                .response
                .accessible_name("Sort by:");
                ui.label("Sort by:");
            });
        });
    }

    fn filter_ui(&mut self, ui: &mut egui::Ui, r: Rect) {
        dlg::frame_rect(ui, r);
        let head = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(r.width() - 6.0, 24.0));
        let dark = ui.visuals().dark_mode;
        ui.painter().rect_filled(head, 0.0, if dark { Color32::from_gray(0x3A) } else { Color32::from_gray(0xDD) });
        ui.painter().text(pos2(head.left() + 6.0, head.center().y), Align2::LEFT_CENTER, "Filter by:", dlg::font(12.0), dlg::text_color(ui));
        dlg::at(ui, Rect::from_min_max(pos2(r.left() + 10.0, head.bottom() + 8.0), r.max - vec2(6.0, 6.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for c in CATEGORIES {
                let mut on = self.categories.contains(c);
                if ui.checkbox(&mut on, c).changed() {
                    if on {
                        self.categories.insert(c.to_string());
                    } else {
                        self.categories.remove(c);
                    }
                }
            }
            let mut free = true;
            ui.add_enabled(false, egui::Checkbox::new(&mut free, "Free"));
            dlg::separator(ui);
            ui.checkbox(&mut self.my_library, "My Library");
        });
    }

    /// The result list; returns the family whose Download was clicked.
    fn list_ui(&mut self, ui: &mut egui::Ui, r: Rect) -> Option<String> {
        let dark = ui.visuals().dark_mode;
        let bg = if dark { Color32::from_gray(0x20) } else { Color32::from_gray(0xF7) };
        ui.painter().rect_filled(r, 0.0, bg);
        let mut clicked = None;
        if self.families.is_empty() {
            dlg::at(ui, r.shrink(16.0), |ui| {
                if self.loading.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                    });
                } else if let Some(e) = &self.error {
                    dlg::note(ui, e);
                    ui.add_space(6.0);
                    if dlg::button(ui, "Retry", true) {
                        self.start_loading(true);
                    }
                }
            });
            return None;
        }
        let row_h = 93.0;
        let ctx = ui.ctx().clone();
        let rows = self.rows.clone();
        let reset = std::mem::take(&mut self.scroll_to_top);
        dlg::at(ui, r, |ui| {
            if self.stale {
                dlg::note(ui, NO_INTERNET);
            }
            let mut sa = egui::ScrollArea::vertical().id_salt("getmore_list").auto_shrink([false, false]);
            if reset {
                sa = sa.vertical_scroll_offset(0.0);
            }
            sa.show_rows(ui, row_h, rows.len(), |ui, range| {
                // the rows are placed by hand from the top of the visible range (an allocation per row
                // can come back shorter than asked inside the scroll area)
                let pitch = row_h + ui.spacing().item_spacing.y;
                let top = ui.max_rect().top();
                let w = ui.available_width();
                let first = range.start;
                for i in range.clone() {
                    let f = self.families[rows[i]].clone();
                    let row = Rect::from_min_size(pos2(ui.max_rect().left(), top + (i - first) as f32 * pitch), vec2(w, row_h));
                    if let Some(name) = self.row_ui(ui, &ctx, row, &f) {
                        clicked = Some(name);
                    }
                }
                let used = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(w, (range.len() as f32 * pitch - ui.spacing().item_spacing.y).max(0.0)));
                ui.allocate_rect(used, Sense::hover());
            });
            if rows.is_empty() {
                ui.add_space(12.0);
                dlg::note(ui, "No fonts found.");
            }
        });
        clicked
    }

    fn row_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, row: Rect, f: &Family) -> Option<String> {
        let text = dlg::text_color(ui);
        let dim = dlg::dim_color(ui);
        let p = ui.painter().clone();
        p.hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, dlg::line_color(ui)));
        // thumbnail 165×78: the name in the family, dark on a light card
        let thumb = Rect::from_min_size(pos2(row.left() + 8.0, row.top() + 7.0), vec2(165.0, 78.0));
        p.rect_filled(thumb, 0.0, Color32::from_gray(0xF2));
        match self.preview_texture(ctx, f) {
            Some(tex) => {
                let size = tex.size_vec2() / ctx.pixels_per_point();
                let ir = Rect::from_center_size(thumb.center(), size);
                p.image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_gray(0x18));
            }
            None => {
                // the name is data: not translated
                let job = egui::text::LayoutJob::simple(f.name.clone(), dlg::font(15.0), Color32::from_gray(0x50), thumb.width() - 12.0);
                let g = p.layout_job(job);
                p.galley(thumb.center() - g.size() / 2.0, g, Color32::from_gray(0x50));
            }
        }
        // texts
        let x = thumb.right() + 16.0;
        let right_w = 150.0;
        let tw = (row.right() - right_w - x).max(80.0);
        let name = p.layout_job(egui::text::LayoutJob::simple_singleline(f.name.clone(), egui::FontId::proportional(15.0), ui.visuals().strong_text_color()));
        p.galley(pos2(x, row.top() + 8.0), name, text);
        p.text(pos2(x, row.top() + 37.0), Align2::LEFT_CENTER, f.category.as_str(), dlg::font(12.0), dim);
        let designer = if f.designers.is_empty() { String::new() } else { f.designers.join(", ") };
        let desc = if f.styles == 1 { format!("{designer} · 1 style") } else { format!("{designer} · {} styles", f.styles) };
        let desc = desc.trim_start_matches(" · ").to_string();
        // Painter::layout is translated ("{} · {} styles"); the designers fill the placeholder
        let g = p.layout(desc, dlg::font(12.0), text, tw);
        p.with_clip_rect(Rect::from_min_size(pos2(x, row.top() + 50.0), vec2(tw, 36.0))).galley(pos2(x, row.top() + 52.0), g, text);
        // right: Free and the Download button
        let rr = Rect::from_min_max(pos2(row.right() - right_w, row.top()), pos2(row.right() - 12.0, row.bottom()));
        p.text(pos2(rr.right(), row.top() + 30.0), Align2::RIGHT_CENTER, "Free", egui::FontId::proportional(14.0), ui.visuals().strong_text_color());
        let br = Rect::from_min_size(pos2(rr.right() - 100.0, row.top() + 50.0), vec2(100.0, 24.0));
        let have = self.downloaded.contains(&folder_name(&f.name));
        let caption = if have { "Downloaded" } else { "Download" };
        let tip = format!("{} styles · {}", f.styles, size_caption(f.size));
        let mut clicked = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(br), |ui| {
            clicked = dlg::button_at(ui, br, caption, false, !have);
        })
        .response
        .on_hover_text(tip);
        clicked.then(|| f.name.clone())
    }

    /// The operation-progress dialog of a download (§7.2.12). True when it was closed.
    fn progress_ui(&mut self, ctx: &egui::Context) -> bool {
        let Some(d) = &mut self.download else { return false };
        if !self.open && d.done.is_none() {
            return false;
        }
        let mut close = false;
        dlg::window_ex(ctx, "getmore_progress", "AnyFont", vec2(420.0, 150.0), false, |ui| {
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            dlg::at(ui, top, |ui| {
                ui.add_space(8.0);
                match &d.done {
                    Some(Err(e)) if !e.is_empty() => {
                        dlg::error(ui, &format!("Unable to download {}.", d.family));
                        if e != NO_INTERNET {
                            dlg::note(ui, e);
                        } else {
                            dlg::note(ui, "An Internet connection is required. Please connect to the Internet and try again.");
                        }
                    }
                    _ => {
                        let pct = (d.progress * 100.0).round() as u32;
                        ui.label(format!("Downloading {}... {}%", d.family, pct));
                        ui.add(egui::ProgressBar::new(d.progress).desired_width(ui.available_width()));
                    }
                }
            });
            dlg::at_right(ui, bar, |ui| {
                if d.done.is_none() {
                    if dlg::button(ui, "Cancel", !d.cancel.load(Ordering::Relaxed)) {
                        d.cancel.store(true, Ordering::Relaxed);
                    }
                } else if dlg::default_button(ui, "Close", true) || dlg::enter_pressed(ui) {
                    close = true;
                }
            });
        });
        // a cancelled download closes by itself
        close || matches!(&d.done, Some(Err(e)) if e.is_empty())
    }
}

fn size_caption(bytes: u64) -> String {
    if bytes == 0 {
        String::new()
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = r#")]}'
{"familyMetadataList":[
 {"family":"ABeeZee","category":"Sans Serif","designers":["Anja Meiners"],"fonts":{"400":{},"400i":{}},"popularity":186,"defaultSort":184,"dateAdded":"2012-09-30","size":46514},
 {"family":"Roboto","category":"Sans Serif","designers":["Christian Robertson"],"fonts":{"100":{},"400":{},"700":{}},"popularity":2,"defaultSort":1,"dateAdded":"2013-01-08","size":509764},
 {"family":"Buda","category":"Display","designers":["Adobe"],"fonts":{"300":{}},"popularity":900,"defaultSort":700,"dateAdded":"2011-01-01"},
 {"family":"Zen Italic Only","category":"Handwriting","designers":[],"fonts":{"400i":{}},"popularity":50,"defaultSort":40,"dateAdded":"2024-05-01"},
 {"family":"","category":"Serif"}
]}"#;

    #[test]
    fn catalog_parses_without_the_guard() {
        let f = parse_catalog(CATALOG).unwrap();
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].name, "ABeeZee");
        assert_eq!(f[0].styles, 2);
        assert_eq!(f[0].preview_style, (400, false));
        assert_eq!(f[2].preview_style, (300, false));
        assert_eq!(f[3].preview_style, (400, true));
        assert!(parse_catalog("{}").is_err());
        assert_eq!(strip_guard(")]}'\n{}"), "{}");
    }

    #[test]
    fn search_filter_and_sort() {
        let f = parse_catalog(CATALOG).unwrap();
        let none = HashSet::new();
        let names = |idx: Vec<usize>| idx.into_iter().map(|i| f[i].name.clone()).collect::<Vec<_>>();
        assert_eq!(names(filter_sort(&f, "", &none, false, &none, Sort::Featured)), ["Roboto", "Zen Italic Only", "ABeeZee", "Buda"]);
        assert_eq!(names(filter_sort(&f, "", &none, false, &none, Sort::Popular)), ["Roboto", "Zen Italic Only", "ABeeZee", "Buda"]);
        assert_eq!(names(filter_sort(&f, "", &none, false, &none, Sort::Name)), ["ABeeZee", "Buda", "Roboto", "Zen Italic Only"]);
        assert_eq!(names(filter_sort(&f, "", &none, false, &none, Sort::Newest))[0], "Zen Italic Only");
        assert_eq!(names(filter_sort(&f, "anja", &none, false, &none, Sort::Name)), ["ABeeZee"]);
        assert_eq!(names(filter_sort(&f, "B", &none, false, &none, Sort::Name)), ["Buda", "ABeeZee", "Roboto"]);
        let display: HashSet<String> = ["Display".to_string()].into();
        assert_eq!(names(filter_sort(&f, "", &display, false, &none, Sort::Name)), ["Buda"]);
        let mine: HashSet<String> = ["Roboto".to_string()].into();
        assert_eq!(names(filter_sort(&f, "", &none, true, &mine, Sort::Name)), ["Roboto"]);
    }

    #[test]
    fn manifest_prefers_variable_fonts_and_stays_in_the_folder() {
        let list = r#")]}'
{"zipName":"Roboto.zip","manifest":{"files":[{"filename":"OFL.txt","contents":"licence"},{"filename":"../evil.txt","contents":"x"}],
"fileRefs":[{"filename":"static/Roboto-Regular.ttf","url":"https://fonts.gstatic.com/s/a.ttf"},
{"filename":"Roboto[wdth,wght].ttf","url":"https://fonts.gstatic.com/s/v.ttf"},
{"filename":"C:/x.ttf","url":"https://fonts.gstatic.com/s/x.ttf"},
{"filename":"README.txt","url":"https://fonts.gstatic.com/s/readme"}]}}"#;
        let m = parse_manifest(list).unwrap();
        assert_eq!(m.inline, vec![("OFL.txt".to_string(), "licence".to_string())]);
        assert_eq!(m.fonts, vec![("Roboto[wdth,wght].ttf".to_string(), "https://fonts.gstatic.com/s/v.ttf".to_string())]);
        let statics = r#"{"manifest":{"files":[],"fileRefs":[{"filename":"ABeeZee-Regular.ttf","url":"https://a/1.ttf"},{"filename":"ABeeZee-Italic.ttf","url":"https://a/2.ttf"}]}}"#;
        assert_eq!(parse_manifest(statics).unwrap().fonts.len(), 2);
        assert!(parse_manifest(r#"{"manifest":{"files":[],"fileRefs":[]}}"#).is_err());
    }

    #[test]
    fn addresses() {
        let f = parse_catalog(CATALOG).unwrap();
        assert_eq!(download_list_url("Roboto Slab"), "https://fonts.google.com/download/list?family=Roboto%20Slab");
        assert_eq!(preview_css_url(&f[1], "Roboto"), "https://fonts.googleapis.com/css2?family=Roboto&text=Roboto");
        assert_eq!(preview_css_url(&f[2], "Buda"), "https://fonts.googleapis.com/css2?family=Buda:wght@300&text=Buda");
        assert_eq!(preview_css_url(&f[3], "Z"), "https://fonts.googleapis.com/css2?family=Zen+Italic+Only:ital,wght@1,400&text=Z");
        let css = "@font-face {\n  src: url(https://fonts.gstatic.com/l/font?kit=abc&v=v23) format('truetype');\n}";
        assert_eq!(css_font_url(css).as_deref(), Some("https://fonts.gstatic.com/l/font?kit=abc&v=v23"));
        assert_eq!(css_font_url("nothing"), None);
        assert_eq!(folder_name("Foo: Bar?"), "Foo_ Bar_");
    }

    #[test]
    fn downloaded_families_need_a_font_file() {
        let dir = std::env::temp_dir().join(format!("anyfont-getmore-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Roboto/static")).unwrap();
        std::fs::create_dir_all(dir.join("Empty")).unwrap();
        std::fs::write(dir.join("Roboto/static/Roboto-Regular.ttf"), b"x").unwrap();
        std::fs::write(dir.join("Empty/OFL.txt"), b"x").unwrap();
        let d = downloaded_families(&dir);
        assert!(d.contains("Roboto") && !d.contains("Empty"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_rasterises_a_system_font() {
        // any TrueType font of the build machine; skipped where there is none
        let Some(bytes) = ["C:/Windows/Fonts/arial.ttf", "C:/Windows/Fonts/segoeui.ttf"].iter().find_map(|p| std::fs::read(p).ok()) else { return };
        let img = render_text(&bytes, "AnyFont", 34.0, vec2(157.0, 70.0)).unwrap();
        assert!(img.size[0] <= 157 && img.size[1] <= 70 && img.size[0] > 40);
        assert!(img.pixels.iter().any(|p| p.a() > 200));
    }

    /// The offline path: a local catalog file through `ANYFONT_CATALOG_URL`.
    #[test]
    fn local_catalog_override() {
        let path = std::env::temp_dir().join(format!("anyfont-catalog-test-{}.json", std::process::id()));
        std::fs::write(&path, CATALOG).unwrap();
        std::env::set_var("ANYFONT_CATALOG_URL", &path);
        let r = load_catalog(false);
        std::env::remove_var("ANYFONT_CATALOG_URL");
        let _ = std::fs::remove_file(&path);
        let (f, stale) = r.unwrap();
        assert_eq!((f.len(), stale), (4, false));
    }
}
