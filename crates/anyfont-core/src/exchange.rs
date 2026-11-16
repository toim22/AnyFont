//! Import / Export of folders and collections (File > Import, File > Export).
//!
//! The original (`FontDS.OnExportGroups` / `HasGroupsForExport`, `OnImportGroups` / `ReadyForImport`)
//! writes an XML document:
//!
//! ```xml
//! <?xml version='1.0'?>
//! <Export>
//!   <MonitoredFolders><folder path="C:\Fonts\Temp" enabled="1"/></MonitoredFolders>
//!   <UserGroups>
//!     <group name="Collection 1">
//!       <font DBFamily="Algerian" DBSubFamily="Regular" DBVersion="Version 1.57" DBTech="OpenType-TrueType"
//!             DBWeight="400" DBWidth="5" DBItalic="0" DBManufacturer="..."/>
//!     </group>
//!   </UserGroups>
//! </Export>
//! ```
//!
//! and on import finds each font with
//! `SELECT DBEntryID FROM FONTSDATA WHERE DBFamily=? AND DBSubFamily=? AND DBVersion=? AND DBTech=? AND
//! DBWeight=? AND DBWidth=? AND DBItalic=? AND DBManufacturer=?`.
//!
//! AnyFont writes the same elements and attributes, plus `format="AnyFont" version="1"` on the root,
//! `recursive` on folders, `favorites="1"` on the Favorites group and `key` (content hash, face, instance)
//! on fonts. Import first matches by `key`, then by the eight attributes, so files exported by the original
//! program import as well. Fonts that are not in the database are counted as missing.

use crate::catalog::Catalog;
use crate::db::FontDatabase;
use crate::error::{CoreError, Result};
use crate::model::{CollectionId, FontKey, FontRecord};
use std::fmt::Write as _;
use std::path::Path;

/// What to export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportOptions {
    /// Include the user folders.
    pub folders: bool,
    /// Collections to include (`None` = all, Favorites included).
    pub collections: Option<Vec<CollectionId>>,
    /// Folders left out although they are library folders: the shell's own hidden folders (the AnyFont
    /// Content folder), which are not the user's.
    pub skip_folders: Vec<std::path::PathBuf>,
}

impl ExportOptions {
    /// Everything.
    pub fn all() -> Self {
        ExportOptions { folders: true, collections: None, skip_folders: Vec::new() }
    }

    /// Everything but these (hidden) folders.
    pub fn all_except(skip_folders: &[std::path::PathBuf]) -> Self {
        ExportOptions { skip_folders: skip_folders.to_vec(), ..Self::all() }
    }

    fn skips(&self, path: &Path) -> bool {
        let p = crate::db::normalize_path(path);
        self.skip_folders.iter().any(|s| crate::db::normalize_path(s) == p)
    }
}

/// Outcome of an import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Folders added.
    pub folders_added: usize,
    /// Folders already present.
    pub folders_existing: usize,
    /// Collections created.
    pub collections_created: usize,
    /// Collections that already existed (fonts merged into them).
    pub collections_merged: usize,
    /// Font references resolved and linked.
    pub fonts_linked: usize,
    /// Font references not found in the database.
    pub fonts_missing: usize,
    /// The font references not found, with the collection that wanted them (in document order).
    pub missing: Vec<MissingRef>,
}

/// A `<font>` of an imported collection that no face in the database matches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissingRef {
    /// The collection it was to be linked into.
    pub collection: CollectionId,
    /// `DBFamily` of the reference (empty when it only had a `key`).
    pub family: String,
    /// `DBSubFamily` of the reference.
    pub subfamily: String,
}

impl MissingRef {
    /// "Family Style" (or "Family" alone), as the font matching dialog lists it.
    pub fn name(&self) -> String {
        if self.subfamily.trim().is_empty() {
            self.family.trim().to_string()
        } else {
            format!("{} {}", self.family.trim(), self.subfamily.trim())
        }
    }
}

/// Which parts of a document [`import_xml_parts`] imports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportParts {
    /// Folders and collections ([`import_xml`]).
    All,
    /// The folders only: the caller scans them before it imports the collections, whose fonts may be in
    /// those folders.
    Folders,
    /// The collections only.
    Collections,
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            '\t' => o.push_str("&#9;"),
            c if (c as u32) < 0x20 => {}
            c => o.push(c),
        }
    }
    o
}

/// True when there is something to export (`HasGroupsForExport`): a folder or a non-empty collection.
pub fn has_exportable(db: &FontDatabase) -> Result<bool> {
    has_exportable_except(db, &[])
}

/// [`has_exportable`] not counting these (hidden) folders.
pub fn has_exportable_except(db: &FontDatabase, skip_folders: &[std::path::PathBuf]) -> Result<bool> {
    let opts = ExportOptions::all_except(skip_folders);
    if db.folders()?.iter().any(|f| !opts.skips(&f.path)) {
        return Ok(true);
    }
    for c in db.collections()? {
        if !db.collection_fonts(c.id)?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn font_element(out: &mut String, f: &FontRecord) {
    let _ = writeln!(
        out,
        "      <font DBFamily=\"{}\" DBSubFamily=\"{}\" DBVersion=\"{}\" DBTech=\"{}\" DBWeight=\"{}\" DBWidth=\"{}\" \
         DBItalic=\"{}\" DBManufacturer=\"{}\" key=\"{}\"/>",
        esc(&f.family),
        esc(&f.subfamily),
        esc(&f.version),
        f.technology.db_name(),
        f.weight_class,
        f.width_class,
        f.italic as u8,
        esc(&f.manufacturer),
        esc(f.key.as_str())
    );
}

/// Builds the export document.
pub fn export_xml(db: &FontDatabase, catalog: &Catalog, options: &ExportOptions) -> Result<String> {
    let mut out = String::from("<?xml version='1.0'?>\n<Export format=\"AnyFont\" version=\"1\">\n");
    if options.folders {
        out.push_str("  <MonitoredFolders>\n");
        for f in db.folders()?.iter().filter(|f| !options.skips(&f.path)) {
            let _ = writeln!(
                out,
                "    <folder path=\"{}\" enabled=\"{}\" recursive=\"{}\"/>",
                esc(&f.path.to_string_lossy()),
                f.enabled as u8,
                f.recursive as u8
            );
        }
        out.push_str("  </MonitoredFolders>\n");
    }
    out.push_str("  <UserGroups>\n");
    for c in db.collections()? {
        if let Some(only) = &options.collections {
            if !only.contains(&c.id) {
                continue;
            }
        }
        let fav = if c.is_favorites { " favorites=\"1\"" } else { "" };
        let _ = writeln!(out, "    <group name=\"{}\"{fav}>", esc(&c.name));
        for key in db.collection_fonts(c.id)? {
            match catalog.font_by_key(&key) {
                Some(f) => font_element(&mut out, f),
                None => {
                    let _ = writeln!(out, "      <font key=\"{}\"/>", esc(key.as_str()));
                }
            }
        }
        out.push_str("    </group>\n");
    }
    out.push_str("  </UserGroups>\n</Export>\n");
    Ok(out)
}

/// Writes the export document to `path`.
pub fn export_file(db: &FontDatabase, catalog: &Catalog, options: &ExportOptions, path: &Path) -> Result<()> {
    let xml = export_xml(db, catalog, options)?;
    std::fs::write(path, xml).map_err(|e| CoreError::io(path, e))
}

fn attr_bool(v: Option<&str>, default: bool) -> bool {
    match v.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s == "0" || s == "false" || s == "no" => false,
        Some(s) if s == "1" || s == "true" || s == "yes" || s == "-1" => true,
        _ => default,
    }
}

fn find_font<'a>(catalog: &'a Catalog, node: roxmltree::Node) -> Option<&'a FontRecord> {
    if let Some(k) = node.attribute("key") {
        if let Some(f) = catalog.font_by_key(&FontKey(k.to_string())) {
            return Some(f);
        }
    }
    let family = node.attribute("DBFamily")?;
    let eq = |a: Option<&str>, b: &str| a.map(|a| a.trim().eq_ignore_ascii_case(b.trim())).unwrap_or(true);
    let num = |a: Option<&str>| a.and_then(|v| v.trim().parse::<i64>().ok());
    let candidates: Vec<&FontRecord> = catalog
        .fonts()
        .iter()
        .filter(|f| f.corrupt.is_none())
        .filter(|f| f.family.eq_ignore_ascii_case(family.trim()))
        .filter(|f| eq(node.attribute("DBSubFamily"), &f.subfamily))
        .filter(|f| eq(node.attribute("DBTech"), f.technology.db_name()))
        .filter(|f| num(node.attribute("DBWeight")).map(|w| w == f.weight_class as i64).unwrap_or(true))
        .filter(|f| num(node.attribute("DBWidth")).map(|w| w == f.width_class as i64).unwrap_or(true))
        .filter(|f| node.attribute("DBItalic").map(|i| attr_bool(Some(i), false) == f.italic).unwrap_or(true))
        .filter(|f| eq(node.attribute("DBManufacturer"), &f.manufacturer))
        .collect();
    // exact version first, then any version (a newer copy of the same face)
    candidates
        .iter()
        .find(|f| eq(node.attribute("DBVersion"), &f.version))
        .or_else(|| candidates.first())
        .copied()
}

/// Imports a document: adds its folders, creates (or merges into) its collections and links the fonts
/// found in `catalog`. Reload the catalog afterwards.
pub fn import_xml(db: &FontDatabase, catalog: &Catalog, xml: &str) -> Result<ImportReport> {
    import_xml_parts(db, catalog, xml, ImportParts::All)
}

/// [`import_xml`] of some parts of the document only.
pub fn import_xml_parts(db: &FontDatabase, catalog: &Catalog, xml: &str, parts: ImportParts) -> Result<ImportReport> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| CoreError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if !root.has_tag_name("Export") {
        return Err(CoreError::Xml("not an Import/Export file (root element is not <Export>)".into()));
    }
    let mut report = ImportReport::default();
    let existing: Vec<String> =
        db.folders()?.iter().map(|f| crate::db::normalize_path(&f.path)).collect();
    let folders = root.children().filter(|n| n.has_tag_name("MonitoredFolders") && parts != ImportParts::Collections);
    for section in folders {
        for f in section.children().filter(|n| n.has_tag_name("folder")) {
            let Some(path) = f.attribute("path").filter(|p| !p.trim().is_empty()) else { continue };
            let p = Path::new(path.trim());
            if existing.contains(&crate::db::normalize_path(p)) {
                report.folders_existing += 1;
                continue;
            }
            let folder = db.add_folder(p, attr_bool(f.attribute("recursive"), true))?;
            if !attr_bool(f.attribute("enabled"), true) {
                db.set_folder_enabled(folder.id, false)?;
            }
            report.folders_added += 1;
        }
    }
    let groups = root.children().filter(|n| n.has_tag_name("UserGroups") && parts != ImportParts::Folders);
    for section in groups {
        for g in section.children().filter(|n| n.has_tag_name("group")) {
            let name = g.attribute("name").unwrap_or("").trim();
            let favorites = attr_bool(g.attribute("favorites"), false) || name == crate::db::FAVORITES_NAME;
            let collection = if favorites {
                report.collections_merged += 1;
                db.ensure_favorites()?
            } else if let Some(c) = db.collection_by_name(name)? {
                report.collections_merged += 1;
                c
            } else {
                report.collections_created += 1;
                db.create_collection(if name.is_empty() { None } else { Some(name) })?
            };
            let mut keys = Vec::new();
            for f in g.children().filter(|n| n.has_tag_name("font")) {
                match find_font(catalog, f) {
                    Some(font) => keys.push(font.key.clone()),
                    None => {
                        report.fonts_missing += 1;
                        report.missing.push(MissingRef {
                            collection: collection.id,
                            family: f.attribute("DBFamily").unwrap_or_default().to_string(),
                            subfamily: f.attribute("DBSubFamily").unwrap_or_default().to_string(),
                        });
                    }
                }
            }
            report.fonts_linked += keys.len();
            db.add_to_collection(collection.id, &keys)?;
        }
    }
    Ok(report)
}

/// Reads and imports a file.
pub fn import_file(db: &FontDatabase, catalog: &Catalog, path: &Path) -> Result<ImportReport> {
    import_file_parts(db, catalog, path, ImportParts::All)
}

/// [`import_file`] of some parts of the document only.
pub fn import_file_parts(db: &FontDatabase, catalog: &Catalog, path: &Path, parts: ImportParts) -> Result<ImportReport> {
    let bytes = std::fs::read(path).map_err(|e| CoreError::io(path, e))?;
    let text = String::from_utf8_lossy(&bytes);
    import_xml_parts(db, catalog, text.trim_start_matches('\u{feff}'), parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn rec(sha: &str, family: &str, sub: &str, weight: u16) -> FontRecord {
        FontRecord {
            key: FontKey::new(sha, 0, None),
            sha: sha.into(),
            family: family.into(),
            subfamily: sub.into(),
            version: "Version 1.0".into(),
            manufacturer: "Foundry & Co \"Q\"".into(),
            weight_class: weight,
            width_class: 5,
            paths: vec![format!("C:/f/{sha}.ttf").into()],
            ..Default::default()
        }
    }

    fn db_with_fonts() -> FontDatabase {
        let db = FontDatabase::open_in_memory().unwrap();
        for r in [rec("a", "Alpha", "Regular", 400), rec("b", "Beta <x>", "Bold", 700)] {
            crate::db::insert_font(db.connection(), &r).unwrap();
            db.connection()
                .execute(
                    "INSERT INTO files(path, size, mtime, sha, format) VALUES (?1, 1, 1, ?2, 'ttf')",
                    rusqlite::params![r.paths[0].to_string_lossy(), r.sha],
                )
                .unwrap();
        }
        db
    }

    #[test]
    fn round_trip() {
        let db = db_with_fonts();
        db.add_folder(Path::new("C:\\Fonts\\Temp"), false).unwrap();
        let c = db.create_collection(Some("Display & Co")).unwrap();
        db.add_to_collection(c.id, &[FontKey::new("a", 0, None), FontKey::new("b", 0, None)]).unwrap();
        db.add_to_collection(db.favorites().unwrap().id, &[FontKey::new("b", 0, None)]).unwrap();
        let cat = Catalog::load(&db).unwrap();
        assert!(has_exportable(&db).unwrap());
        let xml = export_xml(&db, &cat, &ExportOptions::all()).unwrap();
        assert!(xml.starts_with("<?xml version='1.0'?>\n<Export"));
        assert!(xml.contains("DBFamily=\"Beta &lt;x&gt;\""));

        let other = db_with_fonts();
        let cat2 = Catalog::load(&other).unwrap();
        let r = import_xml(&other, &cat2, &xml).unwrap();
        assert_eq!(r.folders_added, 1);
        assert_eq!((r.collections_created, r.collections_merged), (1, 1));
        assert_eq!((r.fonts_linked, r.fonts_missing), (3, 0));
        let c2 = other.collection_by_name("Display & Co").unwrap().unwrap();
        assert_eq!(other.collection_fonts(c2.id).unwrap().len(), 2);
        assert_eq!(other.collection_fonts(other.favorites().unwrap().id).unwrap(), vec![FontKey::new("b", 0, None)]);
        let f = &other.folders().unwrap()[0];
        assert!(!f.recursive && f.enabled);
        // importing again merges without duplicates
        let again = import_xml(&other, &cat2, &xml).unwrap();
        assert_eq!((again.folders_existing, again.collections_created), (1, 0));
        assert_eq!(other.collection_fonts(c2.id).unwrap().len(), 2);
    }

    #[test]
    fn imports_original_format_by_attributes() {
        let db = db_with_fonts();
        let cat = Catalog::load(&db).unwrap();
        let xml = r#"<?xml version='1.0'?><Export><MonitoredFolders><folder path="D:\Type" enabled="0"/></MonitoredFolders>
            <UserGroups><group name="Imported"><font DBFamily="alpha" DBSubFamily="Regular" DBVersion="Version 9"
            DBTech="TrueType" DBWeight="400" DBWidth="5" DBItalic="0" DBManufacturer="Foundry &amp; Co &quot;Q&quot;"/>
            <font DBFamily="Gamma" DBSubFamily="Regular" DBVersion="" DBTech="TrueType" DBWeight="400" DBWidth="5"
            DBItalic="0" DBManufacturer=""/></group></UserGroups></Export>"#;
        let r = import_xml(&db, &cat, xml).unwrap();
        assert_eq!((r.fonts_linked, r.fonts_missing), (1, 1));
        assert!(!db.folders().unwrap()[0].enabled);
        assert!(import_xml(&db, &cat, "<Other/>").is_err());
        assert!(import_xml(&db, &cat, "not xml").is_err());
    }

    #[test]
    fn import_in_two_parts_reports_the_missing_references() {
        let db = db_with_fonts();
        let cat = Catalog::load(&db).unwrap();
        let xml = r#"<?xml version='1.0'?><Export><MonitoredFolders><folder path="D:\Type"/></MonitoredFolders>
            <UserGroups><group name="G"><font DBFamily="Alpha" DBSubFamily="Regular"/><font DBFamily="Gamma" DBSubFamily="Bold"/></group></UserGroups></Export>"#;
        let r = import_xml_parts(&db, &cat, xml, ImportParts::Folders).unwrap();
        assert_eq!((r.folders_added, r.collections_created, r.fonts_missing), (1, 0, 0));
        assert!(db.collection_by_name("G").unwrap().is_none());
        let r = import_xml_parts(&db, &cat, xml, ImportParts::Collections).unwrap();
        assert_eq!((r.folders_added, r.folders_existing, r.collections_created, r.fonts_linked, r.fonts_missing), (0, 0, 1, 1, 1));
        let g = db.collection_by_name("G").unwrap().unwrap();
        assert_eq!(r.missing, vec![MissingRef { collection: g.id, family: "Gamma".into(), subfamily: "Bold".into() }]);
        assert_eq!(r.missing[0].name(), "Gamma Bold");
    }

    /// The shell's hidden Content folder is neither exported nor counted as something to export (BUG-303).
    #[test]
    fn skipped_folders_are_not_exported() {
        let db = FontDatabase::open_in_memory().unwrap();
        let content = std::path::PathBuf::from(r"C:\Users\u\Documents\AnyFont\Content\Fonts");
        db.add_folder(&content, true).unwrap();
        let cat = Catalog::load(&db).unwrap();
        assert!(has_exportable(&db).unwrap());
        assert!(!has_exportable_except(&db, std::slice::from_ref(&content)).unwrap());
        let xml = export_xml(&db, &cat, &ExportOptions::all_except(std::slice::from_ref(&content))).unwrap();
        assert!(!xml.contains("Content"), "{xml}");
        db.add_folder(Path::new(r"D:\Mine"), true).unwrap();
        assert!(has_exportable_except(&db, std::slice::from_ref(&content)).unwrap());
        let xml = export_xml(&db, &cat, &ExportOptions::all_except(&[content])).unwrap();
        assert!(xml.contains("Mine") && !xml.contains("Content"), "{xml}");
    }
}
