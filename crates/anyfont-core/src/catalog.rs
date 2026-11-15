//! In-memory snapshot of the database that answers every list query.
//!
//! The UI loads a [`Catalog`] from its [`FontDatabase`] at start and after each scan or edit, then calls
//! [`Catalog::query`] every time the library, filters, search text or view options change. Queries are pure
//! and fast (a few milliseconds for thousands of faces).
//!
//! List rules reproduced from the screenshots (`06-ui/screenshots`):
//!
//! * Libraries: "My fonts" (every local face: installed fonts and the fonts of the enabled user folders),
//!   each user folder (faces with a file under it, sub-folders when recursive), each collection.
//! * Grouping (View > Group by family): faces are grouped by typographic family ("Arial Narrow" and
//!   "Arial Black" under "Arial"); a family header row (with its style count) appears only when the group
//!   has more than one face. Order inside the list: group family, legacy family, weight, width, italic.
//! * Search: a flat list (no headers) of the faces whose names contain the text, ordered by group family,
//!   weight, width, italic, legacy family.
//! * Duplicates: copies (`DBDuplicate = 2`) are hidden unless "Show duplicate fonts" is on.
//! * Corrupt files are listed only when the "Corrupt" status filter is checked.
//! * Show recent: the recently used faces of the library are listed first, under a "Recent" header.
//! * Status text: `My fonts: 681 fonts`, `My fonts (filtered): 10 fonts`, `<folder name>: 2 fonts`.

use crate::db::{normalize_path, Collection, Folder, FontDatabase};
use crate::eligibility::{self, Eligibility, Selection};
use crate::error::Result;
use crate::filters::{self, FilterGroupOptions, FilterSet};
use crate::format;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Name of the "My fonts" library.
pub const MY_FONTS_NAME: &str = "My fonts";

/// A library of the left panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LibraryRef {
    /// Every local font.
    #[default]
    MyFonts,
    /// A user folder.
    Folder(FolderId),
    /// A collection (Favorites included).
    Collection(CollectionId),
}

/// List order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SortOrder {
    /// A to Z.
    #[default]
    Ascending,
    /// Z to A.
    Descending,
}

/// A list query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontQuery {
    /// Library.
    pub library: LibraryRef,
    /// Checked filters.
    pub filters: FilterSet,
    /// Search text (case-insensitive, matches family, full, PostScript and localised names).
    pub search: String,
    /// View > Group by family.
    pub group_by_family: bool,
    /// View > Show duplicate fonts.
    pub show_duplicates: bool,
    /// View > Show recent.
    pub show_recent: bool,
    /// View > Show Latin names.
    pub show_latin_names: bool,
    /// UI language (Windows LCID) used for localised family names.
    pub language: u16,
    /// Order.
    pub sort: SortOrder,
}

impl Default for FontQuery {
    fn default() -> Self {
        FontQuery {
            library: LibraryRef::MyFonts,
            filters: FilterSet::default(),
            search: String::new(),
            group_by_family: true,
            show_duplicates: false,
            show_recent: false,
            show_latin_names: false,
            language: 0x0409,
            sort: SortOrder::Ascending,
        }
    }
}

/// One row of the font list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ListRow {
    /// "Recent" section header.
    RecentHeader {
        /// Number of recent faces listed.
        count: usize,
    },
    /// Family header (only for families with more than one face).
    FamilyHeader {
        /// Family name as displayed.
        family: String,
        /// Number of faces ("12 styles").
        style_count: usize,
        /// The faces, in list order.
        fonts: Vec<FontId>,
    },
    /// A face.
    Font {
        /// Face id.
        id: FontId,
        /// Label, e.g. `Arial (Bold-Italic)`.
        label: String,
        /// Indented under a family header.
        indent: bool,
        /// Listed in the Recent section.
        recent: bool,
    },
}

/// Result of [`Catalog::query`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QueryResult {
    /// Rows to display.
    pub rows: Vec<ListRow>,
    /// Faces of the main list in display order (no Recent duplicates). "Select all" selects these.
    pub font_ids: Vec<FontId>,
    /// Faces of the Recent section.
    pub recent: Vec<FontId>,
    /// Number of faces (`font_ids.len()`).
    pub count: usize,
    /// Filters or search are active.
    pub filtered: bool,
    /// Library name ("My fonts", folder name, collection name).
    pub library_name: String,
    /// Status bar text.
    pub status_text: String,
}

impl QueryResult {
    /// The face selected when the library opens (the first face of the list).
    pub fn first_font(&self) -> Option<FontId> {
        self.font_ids.first().copied()
    }
}

/// An entry of the left panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LibraryEntry {
    /// Library.
    pub library: LibraryRef,
    /// Name.
    pub name: String,
    /// Faces (duplicates hidden, corrupt excluded).
    pub count: usize,
    /// The Favorites collection.
    pub is_favorites: bool,
    /// Folder path (folders only).
    pub path: Option<std::path::PathBuf>,
}

/// Fills `installed` / `protected` from the installed set (path comparison, case-insensitive).
pub fn apply_installed(fonts: &mut [FontRecord], installed: &[InstalledFont]) {
    let set: HashMap<String, bool> = installed
        .iter()
        .map(|f| (normalize_path(&crate::parse::canonical_font_path(&f.path)), f.protected))
        .chain(installed.iter().map(|f| (normalize_path(&f.path), f.protected)))
        .collect();
    for f in fonts {
        let mut inst = false;
        let mut prot = false;
        for p in &f.paths {
            if let Some(&pr) = set.get(&normalize_path(p)) {
                inst = true;
                prot |= pr;
            }
        }
        f.installed = inst;
        f.protected = prot;
    }
}

/// The snapshot.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    fonts: Vec<FontRecord>,
    index: HashMap<FontId, usize>,
    by_key: HashMap<FontKey, FontId>,
    folders: Vec<Folder>,
    collections: Vec<Collection>,
    members: HashMap<CollectionId, Vec<FontId>>,
    mru: Vec<FontId>,
}

impl Catalog {
    /// Loads the snapshot from the database.
    pub fn load(db: &FontDatabase) -> Result<Catalog> {
        let fonts = db.load_fonts()?;
        let installed = db.installed()?;
        let folders = db.folders()?;
        let collections = db.collections()?;
        let links = db.all_collection_links()?;
        let mru = db.mru()?;
        Ok(Catalog::from_parts(fonts, &installed, folders, collections, links, mru))
    }

    /// Builds a snapshot from its parts (tests, tools).
    pub fn from_parts(
        mut fonts: Vec<FontRecord>,
        installed: &[InstalledFont],
        folders: Vec<Folder>,
        collections: Vec<Collection>,
        links: Vec<(CollectionId, FontKey)>,
        mru: Vec<FontKey>,
    ) -> Catalog {
        apply_installed(&mut fonts, installed);
        crate::duplicates::mark(&mut fonts);
        let index: HashMap<FontId, usize> = fonts.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
        let by_key: HashMap<FontKey, FontId> = fonts.iter().map(|f| (f.key.clone(), f.id)).collect();
        let mut members: HashMap<CollectionId, Vec<FontId>> = HashMap::new();
        for (c, k) in links {
            if let Some(&id) = by_key.get(&k) {
                let v = members.entry(c).or_default();
                if !v.contains(&id) {
                    v.push(id);
                }
                if let Some(&i) = index.get(&id) {
                    if !fonts[i].collections.contains(&c) {
                        fonts[i].collections.push(c);
                    }
                }
            }
        }
        let mru = mru.iter().filter_map(|k| by_key.get(k).copied()).collect();
        Catalog { fonts, index, by_key, folders, collections, members, mru }
    }

    /// Every face.
    pub fn fonts(&self) -> &[FontRecord] {
        &self.fonts
    }
    /// One face.
    pub fn font(&self, id: FontId) -> Option<&FontRecord> {
        self.index.get(&id).map(|&i| &self.fonts[i])
    }
    /// Face by key.
    pub fn font_by_key(&self, key: &FontKey) -> Option<&FontRecord> {
        self.by_key.get(key).and_then(|id| self.font(*id))
    }
    /// User folders.
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }
    /// Collections (sorted by name).
    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }
    /// Collection by id.
    pub fn collection(&self, id: CollectionId) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }
    /// Recently used faces, most recent first.
    pub fn mru(&self) -> &[FontId] {
        &self.mru
    }

    /// Name of a library.
    pub fn library_name(&self, lib: &LibraryRef) -> String {
        match lib {
            LibraryRef::MyFonts => MY_FONTS_NAME.to_string(),
            LibraryRef::Folder(id) => self.folders.iter().find(|f| f.id == *id).map(|f| f.name()).unwrap_or_default(),
            LibraryRef::Collection(id) => self.collection(*id).map(|c| c.name.clone()).unwrap_or_default(),
        }
    }

    /// True when the face belongs to the library.
    pub fn in_library(&self, f: &FontRecord, lib: &LibraryRef) -> bool {
        match lib {
            LibraryRef::MyFonts => !f.paths.is_empty(),
            LibraryRef::Folder(id) => match self.folders.iter().find(|x| x.id == *id) {
                Some(folder) => f.paths.iter().any(|p| folder.contains(p)),
                None => false,
            },
            LibraryRef::Collection(id) => f.collections.contains(id),
        }
    }

    /// Faces of a library (before filters; duplicates and corrupt files included).
    pub fn library_fonts(&self, lib: &LibraryRef) -> Vec<&FontRecord> {
        match lib {
            LibraryRef::Collection(id) => {
                self.members.get(id).map(|v| v.iter().filter_map(|i| self.font(*i)).collect()).unwrap_or_default()
            }
            _ => self.fonts.iter().filter(|f| self.in_library(f, lib)).collect(),
        }
    }

    /// Faces of a library as listed before filters: corrupt files only with `with_corrupt`, and without
    /// `show_duplicates` the extra copies hidden. Duplicate sets are evaluated inside the library, so a
    /// folder holding only a copy of an installed font still lists it.
    pub fn visible_fonts(&self, lib: &LibraryRef, show_duplicates: bool, with_corrupt: bool) -> Vec<&FontRecord> {
        let fonts: Vec<&FontRecord> =
            self.library_fonts(lib).into_iter().filter(|f| f.corrupt.is_none() || with_corrupt).collect();
        if show_duplicates {
            return fonts;
        }
        let states = crate::duplicates::compute_refs(&fonts);
        fonts.into_iter().zip(states).filter(|(_, s)| *s != DuplicateState::Copy).map(|(f, _)| f).collect()
    }

    /// The left panel: "My fonts" and the folders (Folders section), then the collections.
    pub fn libraries(&self) -> Vec<LibraryEntry> {
        let count = |lib: &LibraryRef| self.visible_fonts(lib, false, false).len();
        let mut v = vec![LibraryEntry {
            library: LibraryRef::MyFonts,
            name: MY_FONTS_NAME.into(),
            count: count(&LibraryRef::MyFonts),
            is_favorites: false,
            path: None,
        }];
        for f in &self.folders {
            let lib = LibraryRef::Folder(f.id);
            v.push(LibraryEntry { library: lib, name: f.name(), count: count(&lib), is_favorites: false, path: Some(f.path.clone()) });
        }
        for c in &self.collections {
            let lib = LibraryRef::Collection(c.id);
            v.push(LibraryEntry { library: lib, name: c.name.clone(), count: count(&lib), is_favorites: c.is_favorites, path: None });
        }
        v
    }

    /// Filter options to show for a library (dynamic, see [`filters::available_options`]).
    pub fn available_filters(&self, lib: &LibraryRef) -> Vec<FilterGroupOptions> {
        filters::available_options(self.library_fonts(lib))
    }

    fn matches_search(f: &FontRecord, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let hay = |s: &str| s.to_lowercase().contains(needle);
        hay(f.group_family())
            || hay(&f.family)
            || hay(&f.full_name)
            || hay(&f.postscript_name)
            || hay(&f.subfamily)
            || f.localized_names.iter().any(|n| {
                [&n.family, &n.typographic_family, &n.full_name].iter().any(|x| x.as_deref().map(hay).unwrap_or(false))
            })
    }

    /// Runs a query.
    pub fn query(&self, q: &FontQuery) -> QueryResult {
        let needle = q.search.trim().to_lowercase();
        let wants_corrupt = q.filters.wants_corrupt();
        let mut fonts: Vec<&FontRecord> = self
            .visible_fonts(&q.library, q.show_duplicates, wants_corrupt)
            .into_iter()
            .filter(|f| q.filters.matches(f))
            .filter(|f| Self::matches_search(f, &needle))
            .collect();
        let lang = q.language;
        let latin = q.show_latin_names;
        let key_grouped = |f: &FontRecord| {
            (
                f.display_family(lang, latin).to_lowercase(),
                f.display_legacy_family(lang, latin).to_lowercase(),
                f.weight_class,
                f.width_class,
                f.italic,
                f.subfamily.to_lowercase(),
                f.id,
            )
        };
        let key_search = |f: &FontRecord| {
            (
                f.display_family(lang, latin).to_lowercase(),
                f.weight_class,
                f.width_class,
                f.italic,
                f.display_legacy_family(lang, latin).to_lowercase(),
                f.subfamily.to_lowercase(),
                f.id,
            )
        };
        let flat = !needle.is_empty() || !q.group_by_family;
        if needle.is_empty() {
            fonts.sort_by_cached_key(|f| key_grouped(f));
        } else {
            fonts.sort_by_cached_key(|f| key_search(f));
        }
        if q.sort == SortOrder::Descending {
            // reverse family order, keep the style order inside a family
            let mut groups: Vec<Vec<&FontRecord>> = Vec::new();
            for f in fonts {
                let fam = f.display_family(lang, latin).to_lowercase();
                match groups.last_mut() {
                    Some(g) if g[0].display_family(lang, latin).to_lowercase() == fam => g.push(f),
                    _ => groups.push(vec![f]),
                }
            }
            groups.reverse();
            fonts = groups.into_iter().flatten().collect();
        }

        let mut rows = Vec::new();
        let in_list: HashSet<FontId> = fonts.iter().map(|f| f.id).collect();
        let recent: Vec<FontId> =
            if q.show_recent { self.mru.iter().copied().filter(|id| in_list.contains(id)).collect() } else { Vec::new() };
        if !recent.is_empty() {
            rows.push(ListRow::RecentHeader { count: recent.len() });
            for id in &recent {
                if let Some(f) = self.font(*id) {
                    rows.push(ListRow::Font { id: *id, label: f.label(lang, latin), indent: true, recent: true });
                }
            }
        }
        if flat {
            for f in &fonts {
                rows.push(ListRow::Font { id: f.id, label: f.label(lang, latin), indent: false, recent: false });
            }
        } else {
            let mut i = 0;
            while i < fonts.len() {
                let fam = fonts[i].display_family(lang, latin);
                let fam_l = fam.to_lowercase();
                let mut j = i + 1;
                while j < fonts.len() && fonts[j].display_family(lang, latin).to_lowercase() == fam_l {
                    j += 1;
                }
                let group = &fonts[i..j];
                if group.len() > 1 {
                    rows.push(ListRow::FamilyHeader {
                        family: fam,
                        style_count: group.len(),
                        fonts: group.iter().map(|f| f.id).collect(),
                    });
                    for f in group {
                        rows.push(ListRow::Font { id: f.id, label: f.label(lang, latin), indent: true, recent: false });
                    }
                } else {
                    let f = group[0];
                    rows.push(ListRow::Font { id: f.id, label: f.label(lang, latin), indent: false, recent: false });
                }
                i = j;
            }
        }
        let font_ids: Vec<FontId> = fonts.iter().map(|f| f.id).collect();
        let filtered = !q.filters.is_empty() || !needle.is_empty();
        let library_name = self.library_name(&q.library);
        let count = font_ids.len();
        QueryResult {
            status_text: format::status_text(&library_name, count, filtered),
            rows,
            font_ids,
            recent,
            count,
            filtered,
            library_name,
        }
    }

    /// Eligibility of the commands for a selection (`faces`: selected font rows; `header_faces`: the faces
    /// of the selected family header rows; `current`: the current font).
    pub fn eligibility(
        &self,
        library: LibraryRef,
        faces: &[FontId],
        header_faces: &[FontId],
        all_selected: bool,
        current: Option<FontId>,
    ) -> Eligibility {
        let is_fav = match library {
            LibraryRef::Collection(id) => self.collection(id).map(|c| c.is_favorites).unwrap_or(false),
            _ => false,
        };
        let sel = Selection {
            library,
            library_is_favorites: is_fav,
            faces: faces.iter().filter_map(|id| self.font(*id)).collect(),
            header_faces: header_faces.iter().filter_map(|id| self.font(*id)).collect(),
            all_selected,
            current: current.and_then(|id| self.font(id)),
        };
        eligibility::evaluate(&sel)
    }

    /// Library context menu state.
    pub fn library_menu(&self, library: LibraryRef) -> eligibility::LibraryMenu {
        let is_fav = match library {
            LibraryRef::Collection(id) => self.collection(id).map(|c| c.is_favorites).unwrap_or(false),
            _ => false,
        };
        eligibility::library_menu(&library, is_fav, &self.library_fonts(&library))
    }

    /// Faces of the same group family as `id` (the Variable tab and "select family").
    pub fn family_of(&self, id: FontId) -> Vec<&FontRecord> {
        let Some(f) = self.font(id) else { return Vec::new() };
        let fam = f.group_family().to_lowercase();
        self.fonts.iter().filter(|x| x.group_family().to_lowercase() == fam).collect()
    }
}

/// The "Font info" tab of the right panel: caption/value pairs in screenshot order.
pub fn font_info(f: &FontRecord, collections: &[Collection]) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = vec![
        ("Family", f.group_family().to_string()),
        ("Sub-family", f.typographic_subfamily.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| f.subfamily.clone())),
        ("File Path", f.path().map(|p| p.display().to_string()).unwrap_or_default()),
        ("File Size", if f.paths.is_empty() { String::new() } else { format::file_size(f.file_size) }),
        ("Version", f.version.clone()),
        ("Font technology", f.technology.caption().to_string()),
        (
            "Code Page",
            format::code_pages(f.code_page_range).iter().map(|c| c.caption()).collect::<Vec<_>>().join("\n"),
        ),
        ("Italic", format::yes_no(f.italic).to_string()),
        ("Weight", format::weight_name(f.weight_class).to_string()),
        ("Width", format::width_name(f.width_class).to_string()),
        ("Embedding", f.embedding.caption().to_string()),
        ("Manufacturer", crate::vendors::foundry(f).unwrap_or_default()),
        ("Designer", f.designer.clone()),
        ("Vendor URL", crate::vendors::vendor_url(f).unwrap_or_default()),
        ("Copyright", f.copyright.clone()),
        (
            "Collection",
            collections.iter().filter(|c| f.collections.contains(&c.id)).map(|c| c.name.clone()).collect::<Vec<_>>().join(", "),
        ),
    ];
    if let Some(reason) = &f.corrupt {
        v.push(("Error", reason.clone()));
    }
    v
}

/// Text of the Variable tab when it cannot show axes.
pub const NO_VARIABLE_FONT_TEXT: &str =
    "You don't have a variable font selected, or you've selected multiple fonts.";

/// Axes for the Variable tab: `Some` only when exactly one variable face is selected.
pub fn variable_axes<'a>(selected: &[&'a FontRecord]) -> Option<&'a [VariationAxis]> {
    match selected {
        [one] if one.is_variable && !one.axes.is_empty() => Some(&one.axes),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::{FilterOption, StatusFilter, WeightFilter};

    fn face(id: i64, family: &str, typo: Option<&str>, sub: &str, w: u16, italic: bool, path: &str) -> FontRecord {
        FontRecord {
            id,
            key: FontKey(format!("k{id}")),
            sha: format!("s{id}"),
            family: family.into(),
            typographic_family: typo.map(str::to_string),
            subfamily: sub.into(),
            weight_class: w,
            width_class: 5,
            italic,
            paths: vec![path.into()],
            code_page_range: [1, 0],
            ..Default::default()
        }
    }

    fn sample() -> Catalog {
        let fonts = vec![
            face(1, "Arial", None, "Regular", 400, false, "C:/Windows/Fonts/arial.ttf"),
            face(2, "Arial", None, "Bold Italic", 700, true, "C:/Windows/Fonts/arialbi.ttf"),
            face(3, "Arial", None, "Bold", 700, false, "C:/Windows/Fonts/arialbd.ttf"),
            face(4, "Arial Black", Some("Arial"), "Regular", 900, false, "C:/Windows/Fonts/ariblk.ttf"),
            face(5, "Algerian", None, "Regular", 400, false, "C:/Windows/Fonts/ALGER.TTF"),
            face(6, "Agency FB", None, "Regular", 400, false, "C:/temp/fm/agency.ttf"),
            face(7, "Agency FB", None, "Regular", 400, false, "C:/Windows/Fonts/AGENCYR.TTF"),
            FontRecord { corrupt: Some("bad".into()), ..face(8, "Broken", None, "", 400, false, "C:/temp/fm/broken.ttf") },
        ];
        let installed: Vec<InstalledFont> = ["arial.ttf", "arialbi.ttf", "arialbd.ttf", "ariblk.ttf"]
            .iter()
            .map(|f| InstalledFont::new(format!("C:\\Windows\\Fonts\\{f}"), true))
            .chain(["ALGER.TTF", "AGENCYR.TTF"].iter().map(|f| InstalledFont::new(format!("C:\\Windows\\Fonts\\{f}"), false)))
            .collect();
        let folders = vec![Folder { id: 1, path: "C:\\temp\\fm".into(), enabled: true, recursive: true }];
        let collections = vec![
            Collection { id: 1, name: "Collection 1".into(), is_favorites: false },
            Collection { id: 2, name: "Favorites".into(), is_favorites: true },
        ];
        let links = vec![(1, FontKey("k5".into())), (1, FontKey("k1".into()))];
        Catalog::from_parts(fonts, &installed, folders, collections, links, vec![FontKey("k5".into())])
    }

    #[test]
    fn grouped_list_and_status() {
        let c = sample();
        let r = c.query(&FontQuery::default());
        assert_eq!(r.status_text, "My fonts: 6 fonts", "copy and corrupt hidden");
        let labels: Vec<String> = r
            .rows
            .iter()
            .map(|row| match row {
                ListRow::FamilyHeader { family, style_count, .. } => format!("[{family} {style_count}]"),
                ListRow::Font { label, indent, .. } => format!("{}{label}", if *indent { "  " } else { "" }),
                ListRow::RecentHeader { .. } => "[recent]".into(),
            })
            .collect();
        assert_eq!(
            labels,
            vec![
                "Agency FB (Regular, Normal)",
                "Algerian (Regular, Normal)",
                "[Arial 4]",
                "  Arial (Regular, Normal)",
                "  Arial (Bold)",
                "  Arial (Bold-Italic)",
                "  Arial Black",
            ]
        );
        assert_eq!(r.first_font(), Some(7));
    }

    #[test]
    fn search_is_flat_and_filters_mark_filtered() {
        let c = sample();
        let r = c.query(&FontQuery { search: "aria".into(), ..Default::default() });
        assert!(r.rows.iter().all(|x| matches!(x, ListRow::Font { indent: false, .. })));
        assert_eq!(r.font_ids, vec![1, 3, 2, 4]);
        assert_eq!(r.status_text, "My fonts (filtered): 4 fonts");
        let q = FontQuery {
            filters: FilterSet::new().with(FilterOption::Weight(WeightFilter::Bold)),
            ..Default::default()
        };
        assert_eq!(c.query(&q).count, 3);
        let corrupt = FontQuery {
            filters: FilterSet::new().with(FilterOption::Status(StatusFilter::Corrupt)),
            ..Default::default()
        };
        assert_eq!(c.query(&corrupt).font_ids, vec![8]);
    }

    #[test]
    fn libraries_folders_collections_recent_duplicates() {
        let c = sample();
        let folder = c.query(&FontQuery { library: LibraryRef::Folder(1), ..Default::default() });
        assert_eq!(folder.status_text, "fm: 1 font");
        let dup = c.query(&FontQuery { library: LibraryRef::Folder(1), show_duplicates: true, ..Default::default() });
        assert_eq!(dup.count, 1, "the folder copy is the master (installed copy is the other file)");
        let all = c.query(&FontQuery { show_duplicates: true, ..Default::default() });
        assert_eq!(all.count, 7);
        let coll = c.query(&FontQuery { library: LibraryRef::Collection(1), show_recent: true, ..Default::default() });
        assert_eq!(coll.recent, vec![5]);
        assert!(matches!(coll.rows[0], ListRow::RecentHeader { count: 1 }));
        assert_eq!(coll.font_ids, vec![5, 1]);
        let names: Vec<String> = c.libraries().into_iter().map(|l| l.name).collect();
        assert_eq!(names, vec!["My fonts", "fm", "Collection 1", "Favorites"]);
        assert_eq!(c.font(1).unwrap().collections, vec![1]);
        let e = c.eligibility(LibraryRef::MyFonts, &[1], &[], false, None);
        assert!(!e.uninstall, "protected system font");
        let e = c.eligibility(LibraryRef::MyFonts, &[5], &[], false, None);
        assert!(e.uninstall && e.delete);
        let info = font_info(c.font(5).unwrap(), c.collections());
        assert_eq!(info[0], ("Family", "Algerian".to_string()));
        assert_eq!(info.last().unwrap().1, "Collection 1");
    }
}
