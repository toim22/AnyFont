//! The SQLite font database: font cache, libraries, collections, MRU and PANOSE tables.
//!
//! AnyFont's equivalent of the original's font database (dossier
//! `05-data-models/file-formats/font-database.md`: `FONTSDATA` with 66 columns, `FONTFOLDERS`, `GROUPS`,
//! `GROUPLINKS`, `OPENTYPE`, `FONTVARIABLEAXES`, `DBPROPERTIES`, the keyword tables ...). The mapping of
//! every original table/column is documented in `docs/phases/05-data-models.md`. Differences by design:
//!
//! * files and faces are separate tables (`files` keyed by path with size + mtime for incremental rescans,
//!   `fonts` keyed by content hash + face + instance, like `DBShaID`; all paths of a hash are the
//!   original's `DBPaths`);
//! * collection membership is stored by [`FontKey`], so a collection survives a rebuild of the cache;
//! * keyword tables are not needed: search runs over the in-memory [`crate::Catalog`].
//!
//! One [`FontDatabase`] owns one connection (`Send`, not `Sync`). The database runs in WAL mode with a busy
//! timeout so a scan on a worker thread (its own connection) and the UI connection can coexist.

use crate::error::{CoreError, Result};
use crate::model::*;
use crate::panose::{PanoseException, Substitution, SubstitutionStatus, SubstitutionType};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Name of the collection that always exists.
pub const FAVORITES_NAME: &str = "Favorites";

/// Schema version stored in `properties`.
///
/// - 1: first release layout.
/// - 2: `fonts.file_size` (the Font info tab's File Size). Older databases get the column and a backfill
///   from `files.size` when they are opened (see `FontDatabase::migrate`).
pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS properties (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE COLLATE NOCASE,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    sha TEXT NOT NULL,
    format TEXT NOT NULL,
    error TEXT,
    scanned_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS files_sha ON files(sha);
CREATE TABLE IF NOT EXISTS fonts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    sha TEXT NOT NULL,
    face_index INTEGER NOT NULL,
    face_count INTEGER NOT NULL,
    instance_index INTEGER,
    format TEXT NOT NULL,
    family TEXT NOT NULL,
    subfamily TEXT NOT NULL,
    typographic_family TEXT,
    typographic_subfamily TEXT,
    full_name TEXT NOT NULL,
    postscript_name TEXT NOT NULL,
    version TEXT NOT NULL,
    unique_id TEXT NOT NULL,
    copyright TEXT NOT NULL,
    trademark TEXT NOT NULL,
    manufacturer TEXT NOT NULL,
    designer TEXT NOT NULL,
    description TEXT NOT NULL,
    vendor_url TEXT NOT NULL,
    designer_url TEXT NOT NULL,
    license TEXT NOT NULL,
    license_url TEXT NOT NULL,
    sample_text TEXT NOT NULL,
    technology TEXT NOT NULL,
    outline INTEGER NOT NULL,
    is_variable INTEGER NOT NULL,
    color INTEGER NOT NULL,
    weight_class INTEGER NOT NULL,
    weight_bucket INTEGER NOT NULL,
    width_class INTEGER NOT NULL,
    italic INTEGER NOT NULL,
    oblique INTEGER NOT NULL,
    fs_selection INTEGER NOT NULL,
    mac_style INTEGER NOT NULL,
    fs_type INTEGER NOT NULL,
    embedding INTEGER NOT NULL,
    code_page1 INTEGER NOT NULL, code_page2 INTEGER NOT NULL,
    unicode1 INTEGER NOT NULL, unicode2 INTEGER NOT NULL, unicode3 INTEGER NOT NULL, unicode4 INTEGER NOT NULL,
    panose0 INTEGER NOT NULL, panose1 INTEGER NOT NULL, panose2 INTEGER NOT NULL, panose3 INTEGER NOT NULL,
    panose4 INTEGER NOT NULL, panose5 INTEGER NOT NULL, panose6 INTEGER NOT NULL, panose7 INTEGER NOT NULL,
    panose8 INTEGER NOT NULL, panose9 INTEGER NOT NULL,
    vendor_id TEXT NOT NULL,
    symbol INTEGER NOT NULL,
    vertical INTEGER NOT NULL,
    monospaced INTEGER NOT NULL,
    glyph_count INTEGER NOT NULL,
    units_per_em INTEGER NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    duplicate INTEGER NOT NULL DEFAULT 0,
    corrupt TEXT,
    warnings TEXT NOT NULL DEFAULT '[]',
    remote_asset_url TEXT,
    remote_metadata_url TEXT,
    remote_provider TEXT
);
CREATE INDEX IF NOT EXISTS fonts_sha ON fonts(sha);
CREATE TABLE IF NOT EXISTS font_names (
    font_id INTEGER NOT NULL REFERENCES fonts(id) ON DELETE CASCADE,
    language INTEGER NOT NULL,
    family TEXT, subfamily TEXT, full_name TEXT, typographic_family TEXT, typographic_subfamily TEXT
);
CREATE INDEX IF NOT EXISTS font_names_font ON font_names(font_id);
CREATE TABLE IF NOT EXISTS font_features (
    font_id INTEGER NOT NULL REFERENCES fonts(id) ON DELETE CASCADE,
    feature TEXT NOT NULL, script TEXT NOT NULL, language TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS font_features_font ON font_features(font_id);
CREATE TABLE IF NOT EXISTS font_axes (
    font_id INTEGER NOT NULL REFERENCES fonts(id) ON DELETE CASCADE,
    ord INTEGER NOT NULL, tag TEXT NOT NULL, name TEXT NOT NULL,
    min REAL NOT NULL, def REAL NOT NULL, max REAL NOT NULL, value REAL NOT NULL, hidden INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS font_axes_font ON font_axes(font_id);
CREATE TABLE IF NOT EXISTS font_instances (
    font_id INTEGER NOT NULL REFERENCES fonts(id) ON DELETE CASCADE,
    ord INTEGER NOT NULL, subfamily TEXT NOT NULL, postscript_name TEXT, coordinates TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS font_instances_font ON font_instances(font_id);
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE COLLATE NOCASE,
    enabled INTEGER NOT NULL DEFAULT 1,
    recursive INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    is_favorites INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS collection_fonts (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    font_key TEXT NOT NULL,
    added_at INTEGER NOT NULL DEFAULT 0,
    UNIQUE (collection_id, font_key)
);
CREATE TABLE IF NOT EXISTS mru (font_key TEXT PRIMARY KEY, used_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS panose_exceptions (missing TEXT PRIMARY KEY COLLATE NOCASE, substitute TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS panose_substitutions (
    missing TEXT PRIMARY KEY COLLATE NOCASE, substitute TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS installed (path TEXT PRIMARY KEY COLLATE NOCASE, protected INTEGER NOT NULL);
"#;

/// A user folder (left panel, "Folders" section).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Folder {
    /// Id.
    pub id: FolderId,
    /// Absolute path.
    pub path: PathBuf,
    /// Monitored (the original's `FONTFOLDERS.DBEnabled`).
    pub enabled: bool,
    /// Include sub-folders.
    pub recursive: bool,
}

/// Lower-case path with `/` separators and no trailing separator, for comparisons.
pub fn normalize_path(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/").to_lowercase();
    s.trim_end_matches('/').to_string()
}

impl Folder {
    /// Name shown in the left panel and the status bar: the last path component.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }
    /// True when `file` lies in this folder (directly, or at any depth when recursive).
    pub fn contains(&self, file: &Path) -> bool {
        let root = normalize_path(&self.path);
        let f = normalize_path(file);
        let Some(rest) = f.strip_prefix(&root).and_then(|r| r.strip_prefix('/')) else { return false };
        !rest.is_empty() && (self.recursive || !rest.contains('/'))
    }
}

/// A collection (Favorites included).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Collection {
    /// Id.
    pub id: CollectionId,
    /// Name.
    pub name: String,
    /// The Favorites collection.
    pub is_favorites: bool,
}

/// The font database.
pub struct FontDatabase {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for FontDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontDatabase").field("path", &self.path).finish()
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn outline_to_i64(o: Outline) -> i64 {
    match o {
        Outline::TrueType => 0,
        Outline::Cff => 1,
        Outline::Cff2 => 2,
        Outline::Bitmap => 3,
        Outline::Type1 => 4,
        Outline::None => 5,
    }
}
fn outline_from_i64(v: i64) -> Outline {
    match v {
        0 => Outline::TrueType,
        1 => Outline::Cff,
        2 => Outline::Cff2,
        3 => Outline::Bitmap,
        4 => Outline::Type1,
        _ => Outline::None,
    }
}

impl FontDatabase {
    /// Opens (creating if needed) the database at `path`.
    pub fn open(path: &Path) -> Result<FontDatabase> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let conn = Connection::open(path)?;
        let db = FontDatabase { conn, path: Some(path.to_path_buf()) };
        db.init()?;
        Ok(db)
    }

    /// In-memory database (tests, previews).
    pub fn open_in_memory() -> Result<FontDatabase> {
        let db = FontDatabase { conn: Connection::open_in_memory()?, path: None };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        self.conn.busy_timeout(std::time::Duration::from_secs(10))?;
        if self.path.is_some() {
            let _: String = self.conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        }
        self.conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")?;
        self.conn.execute_batch(SCHEMA)?;
        self.migrate()?;
        self.conn.execute(
            "INSERT OR IGNORE INTO collections(name, is_favorites) SELECT ?1, 1 \
             WHERE NOT EXISTS (SELECT 1 FROM collections WHERE is_favorites = 1)",
            [FAVORITES_NAME],
        )?;
        Ok(())
    }

    /// Brings a database written by an older AnyFont up to [`SCHEMA_VERSION`].
    fn migrate(&self) -> Result<()> {
        let stored: Option<i64> = self.property("schema_version")?.and_then(|v| v.parse().ok());
        if stored.is_some_and(|v| v >= SCHEMA_VERSION) {
            return Ok(());
        }
        // version 2: fonts.file_size, filled from the size of the face's first file location
        let has_size: bool = self
            .conn
            .query_row("SELECT COUNT(*) FROM pragma_table_info('fonts') WHERE name = 'file_size'", [], |r| r.get::<_, i64>(0))?
            > 0;
        if !has_size {
            self.conn.execute_batch("ALTER TABLE fonts ADD COLUMN file_size INTEGER NOT NULL DEFAULT 0;")?;
        }
        self.conn.execute(
            "UPDATE fonts SET file_size = COALESCE((SELECT size FROM files WHERE files.sha = fonts.sha ORDER BY id LIMIT 1), 0)              WHERE file_size = 0",
            [],
        )?;
        self.set_property("schema_version", &SCHEMA_VERSION.to_string())?;
        Ok(())
    }

    /// Path of the database file (`None` for in-memory databases).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Raw connection (advanced use; the schema is documented in `docs/phases/05-data-models.md`).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Mutable raw connection (transactions).
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    // ---- properties ------------------------------------------------------------------------------

    /// Reads a property (`DBPROPERTIES`).
    pub fn property(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM properties WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    /// Writes a property.
    pub fn set_property(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO properties(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- folders -------------------------------------------------------------------------------

    /// User folders, ordered by name.
    pub fn folders(&self) -> Result<Vec<Folder>> {
        let mut st = self.conn.prepare("SELECT id, path, enabled, recursive FROM folders")?;
        let mut v: Vec<Folder> = st
            .query_map([], |r| {
                Ok(Folder {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    enabled: r.get(2)?,
                    recursive: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        v.sort_by_key(|f| (f.name().to_lowercase(), f.id));
        Ok(v)
    }

    /// One folder.
    pub fn folder(&self, id: FolderId) -> Result<Option<Folder>> {
        Ok(self.folders()?.into_iter().find(|f| f.id == id))
    }

    /// Adds a folder (Add Folder). Adding an existing path returns the existing folder.
    pub fn add_folder(&self, path: &Path, recursive: bool) -> Result<Folder> {
        let p = path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
        if p.is_empty() {
            return Err(CoreError::NotAllowed("empty folder path".into()));
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO folders(path, enabled, recursive) VALUES (?1, 1, ?2)",
            params![p, recursive],
        )?;
        let id: i64 = self.conn.query_row("SELECT id FROM folders WHERE path = ?1", [&p], |r| r.get(0))?;
        self.folder(id)?.ok_or_else(|| CoreError::NotAllowed("folder vanished".into()))
    }

    /// Enables or disables monitoring of a folder.
    pub fn set_folder_enabled(&self, id: FolderId, enabled: bool) -> Result<()> {
        self.expect_changed(self.conn.execute("UPDATE folders SET enabled = ?2 WHERE id = ?1", params![id, enabled])?, "folder")
    }

    /// Includes or excludes sub-folders.
    pub fn set_folder_recursive(&self, id: FolderId, recursive: bool) -> Result<()> {
        self.expect_changed(
            self.conn.execute("UPDATE folders SET recursive = ?2 WHERE id = ?1", params![id, recursive])?,
            "folder",
        )
    }

    /// Removes a folder from the library (files are not touched; the next scan drops its fonts).
    pub fn remove_folder(&self, id: FolderId) -> Result<()> {
        self.expect_changed(self.conn.execute("DELETE FROM folders WHERE id = ?1", [id])?, "folder")
    }

    fn expect_changed(&self, n: usize, what: &str) -> Result<()> {
        if n == 0 {
            Err(CoreError::NotAllowed(format!("unknown {what}")))
        } else {
            Ok(())
        }
    }

    // ---- collections ---------------------------------------------------------------------------

    /// Collections sorted by name (Favorites included, as in the left panel: "Collection 1" before
    /// "Favorites").
    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut st = self.conn.prepare("SELECT id, name, is_favorites FROM collections")?;
        let mut v: Vec<Collection> = st
            .query_map([], |r| Ok(Collection { id: r.get(0)?, name: r.get(1)?, is_favorites: r.get(2)? }))?
            .collect::<std::result::Result<_, _>>()?;
        v.sort_by(|a, b| natural_cmp(&a.name, &b.name).then(a.id.cmp(&b.id)));
        Ok(v)
    }

    /// One collection.
    pub fn collection(&self, id: CollectionId) -> Result<Option<Collection>> {
        Ok(self
            .conn
            .query_row("SELECT id, name, is_favorites FROM collections WHERE id = ?1", [id], |r| {
                Ok(Collection { id: r.get(0)?, name: r.get(1)?, is_favorites: r.get(2)? })
            })
            .optional()?)
    }

    /// The Favorites collection (always exists).
    pub fn favorites(&self) -> Result<Collection> {
        Ok(self.conn.query_row("SELECT id, name, is_favorites FROM collections WHERE is_favorites = 1", [], |r| {
            Ok(Collection { id: r.get(0)?, name: r.get(1)?, is_favorites: true })
        })?)
    }

    /// Default name of a new collection: "Collection N" with the smallest free N >= 1.
    pub fn next_collection_name(&self) -> Result<String> {
        let names: Vec<String> = self.collections()?.into_iter().map(|c| c.name.to_lowercase()).collect();
        let mut n = 1;
        loop {
            let candidate = format!("Collection {n}");
            if !names.contains(&candidate.to_lowercase()) {
                return Ok(candidate);
            }
            n += 1;
        }
    }

    /// Creates a collection (Add Collection). `None` uses [`FontDatabase::next_collection_name`].
    pub fn create_collection(&self, name: Option<&str>) -> Result<Collection> {
        let name = match name.map(str::trim) {
            Some(n) if !n.is_empty() => n.to_string(),
            Some(_) => return Err(CoreError::NotAllowed("a collection name cannot be empty".into())),
            None => self.next_collection_name()?,
        };
        if self.collection_by_name(&name)?.is_some() {
            return Err(CoreError::DuplicateName(name));
        }
        self.conn.execute("INSERT INTO collections(name, is_favorites) VALUES (?1, 0)", [&name])?;
        Ok(Collection { id: self.conn.last_insert_rowid(), name, is_favorites: false })
    }

    /// Finds a collection by name (case-insensitive).
    pub fn collection_by_name(&self, name: &str) -> Result<Option<Collection>> {
        Ok(self
            .conn
            .query_row("SELECT id, name, is_favorites FROM collections WHERE name = ?1", [name.trim()], |r| {
                Ok(Collection { id: r.get(0)?, name: r.get(1)?, is_favorites: r.get(2)? })
            })
            .optional()?)
    }

    /// Renames a collection. Favorites cannot be renamed.
    pub fn rename_collection(&self, id: CollectionId, name: &str) -> Result<()> {
        let c = self.collection(id)?.ok_or_else(|| CoreError::NotAllowed("unknown collection".into()))?;
        if c.is_favorites {
            return Err(CoreError::NotAllowed("Favorites cannot be renamed".into()));
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::NotAllowed("a collection name cannot be empty".into()));
        }
        if let Some(other) = self.collection_by_name(name)? {
            if other.id != id {
                return Err(CoreError::DuplicateName(name.to_string()));
            }
        }
        self.conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, name])?;
        Ok(())
    }

    /// Removes a collection (Remove Folder or Collection). Favorites always exists: removing it empties it.
    pub fn remove_collection(&self, id: CollectionId) -> Result<()> {
        let c = self.collection(id)?.ok_or_else(|| CoreError::NotAllowed("unknown collection".into()))?;
        if c.is_favorites {
            self.conn.execute("DELETE FROM collection_fonts WHERE collection_id = ?1", [id])?;
        } else {
            self.conn.execute("DELETE FROM collection_fonts WHERE collection_id = ?1", [id])?;
            self.conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    /// Adds fonts to a collection; returns how many were new.
    pub fn add_to_collection(&self, id: CollectionId, keys: &[FontKey]) -> Result<usize> {
        if self.collection(id)?.is_none() {
            return Err(CoreError::NotAllowed("unknown collection".into()));
        }
        let t = now();
        let mut n = 0;
        let mut st = self
            .conn
            .prepare("INSERT OR IGNORE INTO collection_fonts(collection_id, font_key, added_at) VALUES (?1, ?2, ?3)")?;
        for k in keys {
            n += st.execute(params![id, k.as_str(), t])?;
        }
        Ok(n)
    }

    /// Removes fonts from a collection; returns how many were removed.
    pub fn remove_from_collection(&self, id: CollectionId, keys: &[FontKey]) -> Result<usize> {
        let mut n = 0;
        let mut st = self.conn.prepare("DELETE FROM collection_fonts WHERE collection_id = ?1 AND font_key = ?2")?;
        for k in keys {
            n += st.execute(params![id, k.as_str()])?;
        }
        Ok(n)
    }

    /// Font keys of a collection, in insertion order.
    pub fn collection_fonts(&self, id: CollectionId) -> Result<Vec<FontKey>> {
        let mut st = self
            .conn
            .prepare("SELECT font_key FROM collection_fonts WHERE collection_id = ?1 ORDER BY added_at, rowid")?;
        let v = st.query_map([id], |r| Ok(FontKey(r.get(0)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    /// Every membership `(collection, key)`.
    pub fn all_collection_links(&self) -> Result<Vec<(CollectionId, FontKey)>> {
        let mut st = self.conn.prepare("SELECT collection_id, font_key FROM collection_fonts ORDER BY added_at, rowid")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, FontKey(r.get(1)?))))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    // ---- MRU -----------------------------------------------------------------------------------

    /// Records the use of a font (applied to text / previewed), keeping at most `max` entries.
    pub fn touch_mru(&self, key: &FontKey, max: usize) -> Result<()> {
        let t: i64 = self.conn.query_row("SELECT COALESCE(MAX(used_at), 0) + 1 FROM mru", [], |r| r.get(0))?;
        self.conn.execute(
            "INSERT INTO mru(font_key, used_at) VALUES (?1, ?2) ON CONFLICT(font_key) DO UPDATE SET used_at = excluded.used_at",
            params![key.as_str(), t.max(now())],
        )?;
        self.conn.execute(
            "DELETE FROM mru WHERE font_key NOT IN (SELECT font_key FROM mru ORDER BY used_at DESC LIMIT ?1)",
            [max as i64],
        )?;
        Ok(())
    }

    /// Recently used fonts, most recent first.
    pub fn mru(&self) -> Result<Vec<FontKey>> {
        let mut st = self.conn.prepare("SELECT font_key FROM mru ORDER BY used_at DESC")?;
        let v = st.query_map([], |r| Ok(FontKey(r.get(0)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    /// Clears the MRU list.
    pub fn clear_mru(&self) -> Result<()> {
        self.conn.execute("DELETE FROM mru", [])?;
        Ok(())
    }

    // ---- PANOSE --------------------------------------------------------------------------------

    /// The exception list.
    pub fn panose_exceptions(&self) -> Result<Vec<PanoseException>> {
        let mut st = self.conn.prepare("SELECT missing, substitute FROM panose_exceptions ORDER BY missing")?;
        let v = st
            .query_map([], |r| Ok(PanoseException { missing: r.get(0)?, substitute: r.get(1)? }))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    /// Adds or replaces an exception (Add Exception).
    pub fn set_panose_exception(&self, e: &PanoseException) -> Result<()> {
        self.conn.execute(
            "INSERT INTO panose_exceptions(missing, substitute) VALUES (?1, ?2) \
             ON CONFLICT(missing) DO UPDATE SET substitute = excluded.substitute",
            params![e.missing.trim(), e.substitute.trim()],
        )?;
        Ok(())
    }

    /// Removes an exception (Remove Exception).
    pub fn remove_panose_exception(&self, missing: &str) -> Result<()> {
        self.conn.execute("DELETE FROM panose_exceptions WHERE missing = ?1", [missing.trim()])?;
        Ok(())
    }

    /// Stored substitutions (only permanent ones are persisted).
    pub fn substitutions(&self) -> Result<Vec<Substitution>> {
        let mut st = self.conn.prepare("SELECT missing, substitute, kind FROM panose_substitutions ORDER BY missing")?;
        let v = st
            .query_map([], |r| {
                let kind: String = r.get(2)?;
                Ok(Substitution {
                    missing: r.get(0)?,
                    substitute: r.get(1)?,
                    kind: if kind == "Exception" { SubstitutionType::Exception } else { SubstitutionType::Panose },
                    status: SubstitutionStatus::Permanent,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    /// Stores a substitution. Temporary substitutions are not persisted (and remove a stored one).
    pub fn set_substitution(&self, s: &Substitution) -> Result<()> {
        if s.status == SubstitutionStatus::Temporary {
            return self.remove_substitution(&s.missing);
        }
        let kind = match s.kind {
            SubstitutionType::Panose => "Panose",
            SubstitutionType::Exception => "Exception",
        };
        self.conn.execute(
            "INSERT INTO panose_substitutions(missing, substitute, kind, status) VALUES (?1, ?2, ?3, 'Permanent') \
             ON CONFLICT(missing) DO UPDATE SET substitute = excluded.substitute, kind = excluded.kind",
            params![s.missing.trim(), s.substitute.trim(), kind],
        )?;
        Ok(())
    }

    /// Removes a stored substitution.
    pub fn remove_substitution(&self, missing: &str) -> Result<()> {
        self.conn.execute("DELETE FROM panose_substitutions WHERE missing = ?1", [missing.trim()])?;
        Ok(())
    }

    // ---- installed set -------------------------------------------------------------------------

    /// Replaces the set of installed font files (supplied by the Windows layer).
    pub fn set_installed(&mut self, fonts: &[InstalledFont]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM installed", [])?;
        {
            let mut st = tx.prepare("INSERT OR REPLACE INTO installed(path, protected) VALUES (?1, ?2)")?;
            for f in fonts {
                st.execute(params![f.path.to_string_lossy(), f.protected])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The installed set.
    pub fn installed(&self) -> Result<Vec<InstalledFont>> {
        let mut st = self.conn.prepare("SELECT path, protected FROM installed ORDER BY path")?;
        let v = st
            .query_map([], |r| Ok(InstalledFont { path: PathBuf::from(r.get::<_, String>(0)?), protected: r.get(1)? }))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    // ---- fonts ---------------------------------------------------------------------------------

    /// Number of face records.
    pub fn font_count(&self) -> Result<usize> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM fonts", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// Loads every face with its paths, names, features, axes and instances. Library state
    /// (`installed`, `protected`, `collections`) is filled by [`crate::Catalog`].
    pub fn load_fonts(&self) -> Result<Vec<FontRecord>> {
        let mut paths: HashMap<String, Vec<PathBuf>> = HashMap::new();
        {
            let mut st = self.conn.prepare("SELECT sha, path FROM files ORDER BY id")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (sha, p) = row?;
                paths.entry(sha).or_default().push(PathBuf::from(p));
            }
        }
        let mut fonts: Vec<FontRecord> = {
            let mut st = self.conn.prepare(&format!("SELECT {FONT_COLUMNS} FROM fonts ORDER BY id"))?;
            let v = st.query_map([], row_to_font)?.collect::<std::result::Result<Vec<_>, _>>()?;
            v
        };
        let index: HashMap<FontId, usize> = fonts.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
        for f in &mut fonts {
            if let Some(p) = paths.get(&f.sha) {
                f.paths = p.clone();
            }
        }
        {
            let mut st = self.conn.prepare(
                "SELECT font_id, language, family, subfamily, full_name, typographic_family, typographic_subfamily \
                 FROM font_names ORDER BY rowid",
            )?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    LocalizedName {
                        language: r.get::<_, i64>(1)? as u16,
                        family: r.get(2)?,
                        subfamily: r.get(3)?,
                        full_name: r.get(4)?,
                        typographic_family: r.get(5)?,
                        typographic_subfamily: r.get(6)?,
                    },
                ))
            })?;
            for row in rows {
                let (id, n) = row?;
                if let Some(&i) = index.get(&id) {
                    fonts[i].localized_names.push(n);
                }
            }
        }
        {
            let mut st = self.conn.prepare("SELECT font_id, feature, script, language FROM font_features ORDER BY rowid")?;
            let rows = st.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, FeatureRecord { feature: r.get(1)?, script: r.get(2)?, language: r.get(3)? }))
            })?;
            for row in rows {
                let (id, fr) = row?;
                if let Some(&i) = index.get(&id) {
                    fonts[i].feature_records.push(fr);
                }
            }
        }
        {
            let mut st = self
                .conn
                .prepare("SELECT font_id, tag, name, min, def, max, value, hidden FROM font_axes ORDER BY font_id, ord")?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    VariationAxis {
                        tag: r.get(1)?,
                        name: r.get(2)?,
                        min: r.get::<_, f64>(3)? as f32,
                        default: r.get::<_, f64>(4)? as f32,
                        max: r.get::<_, f64>(5)? as f32,
                        value: r.get::<_, f64>(6)? as f32,
                        hidden: r.get(7)?,
                    },
                ))
            })?;
            for row in rows {
                let (id, a) = row?;
                if let Some(&i) = index.get(&id) {
                    fonts[i].axes.push(a);
                }
            }
        }
        {
            let mut st = self.conn.prepare(
                "SELECT font_id, subfamily, postscript_name, coordinates FROM font_instances ORDER BY font_id, ord",
            )?;
            let rows = st.query_map([], |r| {
                let coords: String = r.get(3)?;
                Ok((
                    r.get::<_, i64>(0)?,
                    NamedInstance {
                        subfamily: r.get(1)?,
                        postscript_name: r.get(2)?,
                        coordinates: serde_json::from_str(&coords).unwrap_or_default(),
                    },
                ))
            })?;
            for row in rows {
                let (id, inst) = row?;
                if let Some(&i) = index.get(&id) {
                    fonts[i].instances.push(inst);
                }
            }
        }
        for f in &mut fonts {
            let mut tags: Vec<String> = f.feature_records.iter().map(|r| r.feature.clone()).collect();
            tags.sort();
            tags.dedup();
            f.features = tags;
        }
        Ok(fonts)
    }

    /// Stores duplicate states computed by [`crate::duplicates`] (`DBDuplicate`).
    pub fn store_duplicates(&mut self, states: &[(FontId, DuplicateState)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare("UPDATE fonts SET duplicate = ?2 WHERE id = ?1")?;
            for (id, s) in states {
                st.execute(params![id, s.db_value()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Adds an online-only font (Get More / Download catalog). Returns its id.
    pub fn add_remote_font(&self, rec: &FontRecord) -> Result<FontId> {
        let mut r = rec.clone();
        r.paths.clear();
        if r.remote.is_none() {
            return Err(CoreError::NotAllowed("not a remote font".into()));
        }
        insert_font(&self.conn, &r)
    }
}

/// Natural, case-insensitive ordering ("Collection 2" < "Collection 10").
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut v: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match v.last_mut() {
                Some((kind, buf)) if *kind == d => buf.push(c),
                _ => v.push((d, c.to_string())),
            }
        }
        v
    }
    let (ca, cb) = (chunks(&a.to_lowercase()), chunks(&b.to_lowercase()));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let ord = match (x.0, y.0) {
            (true, true) => {
                let (xa, ya) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                xa.len().cmp(&ya.len()).then_with(|| xa.cmp(ya))
            }
            _ => x.1.cmp(&y.1),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    ca.len().cmp(&cb.len())
}

const FONT_COLUMNS: &str = "id, key, sha, face_index, face_count, instance_index, format, family, subfamily, \
typographic_family, typographic_subfamily, full_name, postscript_name, version, unique_id, copyright, trademark, \
manufacturer, designer, description, vendor_url, designer_url, license, license_url, sample_text, technology, \
outline, is_variable, color, weight_class, width_class, italic, oblique, fs_selection, mac_style, fs_type, \
embedding, code_page1, code_page2, unicode1, unicode2, unicode3, unicode4, panose0, panose1, panose2, panose3, \
panose4, panose5, panose6, panose7, panose8, panose9, vendor_id, symbol, vertical, monospaced, glyph_count, \
units_per_em, duplicate, corrupt, warnings, remote_asset_url, remote_metadata_url, remote_provider, file_size";

fn row_to_font(r: &Row) -> rusqlite::Result<FontRecord> {
    let mut panose = [0u8; 10];
    for (i, p) in panose.iter_mut().enumerate() {
        *p = r.get::<_, i64>(43 + i)? as u8;
    }
    let warnings: String = r.get(61)?;
    let asset: Option<String> = r.get(62)?;
    Ok(FontRecord {
        id: r.get(0)?,
        key: FontKey(r.get(1)?),
        sha: r.get(2)?,
        face_index: r.get::<_, i64>(3)? as u32,
        face_count: r.get::<_, i64>(4)? as u32,
        instance_index: r.get::<_, Option<i64>>(5)?.map(|v| v as u16),
        format: FileFormat::from_db_name(&r.get::<_, String>(6)?),
        family: r.get(7)?,
        subfamily: r.get(8)?,
        typographic_family: r.get(9)?,
        typographic_subfamily: r.get(10)?,
        full_name: r.get(11)?,
        postscript_name: r.get(12)?,
        version: r.get(13)?,
        unique_id: r.get(14)?,
        copyright: r.get(15)?,
        trademark: r.get(16)?,
        manufacturer: r.get(17)?,
        designer: r.get(18)?,
        description: r.get(19)?,
        vendor_url: r.get(20)?,
        designer_url: r.get(21)?,
        license: r.get(22)?,
        license_url: r.get(23)?,
        sample_text: r.get(24)?,
        technology: Technology::from_db_name(&r.get::<_, String>(25)?),
        outline: outline_from_i64(r.get(26)?),
        is_variable: r.get(27)?,
        color: ColorFormats(r.get::<_, i64>(28)? as u8),
        weight_class: r.get::<_, i64>(29)? as u16,
        width_class: r.get::<_, i64>(30)? as u16,
        italic: r.get(31)?,
        oblique: r.get(32)?,
        fs_selection: r.get::<_, i64>(33)? as u16,
        mac_style: r.get::<_, i64>(34)? as u16,
        fs_type: r.get::<_, i64>(35)? as u16,
        embedding: Embedding::from_db_value(r.get(36)?),
        code_page_range: [r.get::<_, i64>(37)? as u32, r.get::<_, i64>(38)? as u32],
        unicode_range: [
            r.get::<_, i64>(39)? as u32,
            r.get::<_, i64>(40)? as u32,
            r.get::<_, i64>(41)? as u32,
            r.get::<_, i64>(42)? as u32,
        ],
        panose: Panose(panose),
        vendor_id: r.get(53)?,
        symbol: r.get(54)?,
        vertical: r.get(55)?,
        monospaced: r.get(56)?,
        glyph_count: r.get::<_, i64>(57)? as u32,
        units_per_em: r.get::<_, i64>(58)? as u16,
        duplicate: DuplicateState::from_db_value(r.get(59)?),
        corrupt: r.get(60)?,
        warnings: serde_json::from_str(&warnings).unwrap_or_default(),
        remote: asset.map(|asset_url| RemoteInfo {
            asset_url,
            metadata_url: r.get::<_, Option<String>>(63).ok().flatten().unwrap_or_default(),
            provider: r.get::<_, Option<String>>(64).ok().flatten().unwrap_or_default(),
        }),
        file_size: r.get::<_, i64>(65)?.max(0) as u64,
        ..Default::default()
    })
}

/// Inserts one face with its child rows (used by the scanner). Returns the row id.
pub(crate) fn insert_font(conn: &Connection, f: &FontRecord) -> Result<FontId> {
    let p = f.panose.0;
    let remote = f.remote.as_ref();
    conn.execute(
        "INSERT OR REPLACE INTO fonts (key, sha, face_index, face_count, instance_index, format, family, subfamily, \
         typographic_family, typographic_subfamily, full_name, postscript_name, version, unique_id, copyright, \
         trademark, manufacturer, designer, description, vendor_url, designer_url, license, license_url, sample_text, \
         technology, outline, is_variable, color, weight_class, weight_bucket, width_class, italic, oblique, \
         fs_selection, mac_style, fs_type, embedding, code_page1, code_page2, unicode1, unicode2, unicode3, unicode4, \
         panose0, panose1, panose2, panose3, panose4, panose5, panose6, panose7, panose8, panose9, vendor_id, symbol, \
         vertical, monospaced, glyph_count, units_per_em, duplicate, corrupt, warnings, remote_asset_url, \
         remote_metadata_url, remote_provider, file_size) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
         ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, \
         ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54, ?55, ?56, ?57, ?58, \
         ?59, ?60, ?61, ?62, ?63, ?64, ?65, ?66)",
        rusqlite::params_from_iter(
            [
                rusqlite::types::Value::from(f.key.0.clone()),
                f.sha.clone().into(),
                (f.face_index as i64).into(),
                (f.face_count as i64).into(),
                f.instance_index.map(|v| v as i64).into(),
                f.format.db_name().to_string().into(),
                f.family.clone().into(),
                f.subfamily.clone().into(),
                f.typographic_family.clone().into(),
                f.typographic_subfamily.clone().into(),
                f.full_name.clone().into(),
                f.postscript_name.clone().into(),
                f.version.clone().into(),
                f.unique_id.clone().into(),
                f.copyright.clone().into(),
                f.trademark.clone().into(),
                f.manufacturer.clone().into(),
                f.designer.clone().into(),
                f.description.clone().into(),
                f.vendor_url.clone().into(),
                f.designer_url.clone().into(),
                f.license.clone().into(),
                f.license_url.clone().into(),
                f.sample_text.clone().into(),
                f.technology.db_name().to_string().into(),
                outline_to_i64(f.outline).into(),
                (f.is_variable as i64).into(),
                (f.color.0 as i64).into(),
                (f.weight_class as i64).into(),
                (f.weight_bucket() as i64).into(),
                (f.width_class as i64).into(),
                (f.italic as i64).into(),
                (f.oblique as i64).into(),
                (f.fs_selection as i64).into(),
                (f.mac_style as i64).into(),
                (f.fs_type as i64).into(),
                f.embedding.db_value().into(),
                (f.code_page_range[0] as i64).into(),
                (f.code_page_range[1] as i64).into(),
                (f.unicode_range[0] as i64).into(),
                (f.unicode_range[1] as i64).into(),
                (f.unicode_range[2] as i64).into(),
                (f.unicode_range[3] as i64).into(),
                (p[0] as i64).into(),
                (p[1] as i64).into(),
                (p[2] as i64).into(),
                (p[3] as i64).into(),
                (p[4] as i64).into(),
                (p[5] as i64).into(),
                (p[6] as i64).into(),
                (p[7] as i64).into(),
                (p[8] as i64).into(),
                (p[9] as i64).into(),
                f.vendor_id.clone().into(),
                (f.symbol as i64).into(),
                (f.vertical as i64).into(),
                (f.monospaced as i64).into(),
                (f.glyph_count as i64).into(),
                (f.units_per_em as i64).into(),
                f.duplicate.db_value().into(),
                f.corrupt.clone().into(),
                serde_json::to_string(&f.warnings).unwrap_or_else(|_| "[]".into()).into(),
                remote.map(|r| r.asset_url.clone()).into(),
                remote.map(|r| r.metadata_url.clone()).into(),
                remote.map(|r| r.provider.clone()).into(),
                (f.file_size.min(i64::MAX as u64) as i64).into(),
            ],
        ),
    )?;
    let id = conn.last_insert_rowid();
    {
        let mut st = conn.prepare(
            "INSERT INTO font_names(font_id, language, family, subfamily, full_name, typographic_family, \
             typographic_subfamily) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for n in &f.localized_names {
            st.execute(params![
                id,
                n.language as i64,
                n.family,
                n.subfamily,
                n.full_name,
                n.typographic_family,
                n.typographic_subfamily
            ])?;
        }
    }
    {
        let mut st = conn.prepare("INSERT INTO font_features(font_id, feature, script, language) VALUES (?1, ?2, ?3, ?4)")?;
        for r in &f.feature_records {
            st.execute(params![id, r.feature, r.script, r.language])?;
        }
    }
    {
        let mut st = conn.prepare(
            "INSERT INTO font_axes(font_id, ord, tag, name, min, def, max, value, hidden) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for (i, a) in f.axes.iter().enumerate() {
            st.execute(params![
                id,
                i as i64,
                a.tag,
                a.name,
                a.min as f64,
                a.default as f64,
                a.max as f64,
                a.value as f64,
                a.hidden
            ])?;
        }
    }
    {
        let mut st = conn.prepare(
            "INSERT INTO font_instances(font_id, ord, subfamily, postscript_name, coordinates) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (i, inst) in f.instances.iter().enumerate() {
            st.execute(params![
                id,
                i as i64,
                inst.subfamily,
                inst.postscript_name,
                serde_json::to_string(&inst.coordinates).unwrap_or_else(|_| "[]".into())
            ])?;
        }
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorites_and_collection_names() {
        let db = FontDatabase::open_in_memory().unwrap();
        let fav = db.favorites().unwrap();
        assert_eq!(fav.name, FAVORITES_NAME);
        let c1 = db.create_collection(None).unwrap();
        assert_eq!(c1.name, "Collection 1");
        let c2 = db.create_collection(None).unwrap();
        assert_eq!(c2.name, "Collection 2");
        assert!(matches!(db.create_collection(Some("collection 1")), Err(CoreError::DuplicateName(_))));
        db.remove_collection(c1.id).unwrap();
        assert_eq!(db.next_collection_name().unwrap(), "Collection 1");
        let names: Vec<String> = db.collections().unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["Collection 2", "Favorites"]);
        assert!(db.rename_collection(fav.id, "X").is_err());
        let k = FontKey("abc:0:-".into());
        assert_eq!(db.add_to_collection(fav.id, &[k.clone(), k.clone()]).unwrap(), 1);
        db.remove_collection(fav.id).unwrap();
        assert!(db.collection_fonts(fav.id).unwrap().is_empty(), "removing Favorites empties it");
        assert!(db.favorites().is_ok());
        db.rename_collection(c2.id, "Display faces").unwrap();
        assert_eq!(db.collection(c2.id).unwrap().unwrap().name, "Display faces");
    }

    #[test]
    fn folders_mru_panose_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fonts.db");
        {
            let mut db = FontDatabase::open(&path).unwrap();
            let f = db.add_folder(Path::new("C:\\Fonts\\Temp\\"), false).unwrap();
            assert_eq!(f.name(), "Temp");
            assert_eq!(db.add_folder(Path::new("c:\\fonts\\temp"), true).unwrap().id, f.id);
            for i in 0..5 {
                db.touch_mru(&FontKey(format!("k{i}")), 3).unwrap();
            }
            db.set_panose_exception(&PanoseException { missing: "Helvetica".into(), substitute: "Arial".into() })
                .unwrap();
            db.set_substitution(&Substitution {
                missing: "Futura".into(),
                substitute: "Century Gothic".into(),
                kind: SubstitutionType::Panose,
                status: SubstitutionStatus::Permanent,
            })
            .unwrap();
            db.set_installed(&[InstalledFont::new("C:/Windows/Fonts/arial.ttf", true)]).unwrap();
        }
        let db = FontDatabase::open(&path).unwrap();
        assert_eq!(db.folders().unwrap().len(), 1);
        assert_eq!(db.mru().unwrap(), vec![FontKey("k4".into()), FontKey("k3".into()), FontKey("k2".into())]);
        assert_eq!(db.panose_exceptions().unwrap()[0].substitute, "Arial");
        assert_eq!(db.substitutions().unwrap()[0].substitute, "Century Gothic");
        assert!(db.installed().unwrap()[0].protected);
    }

    #[test]
    fn folder_membership() {
        let f = Folder { id: 1, path: "C:\\Fonts".into(), enabled: true, recursive: false };
        assert!(f.contains(Path::new("c:/fonts/a.ttf")));
        assert!(!f.contains(Path::new("c:/fonts/sub/a.ttf")));
        assert!(!f.contains(Path::new("c:/fontsx/a.ttf")));
        let r = Folder { recursive: true, ..f };
        assert!(r.contains(Path::new("C:\\Fonts\\sub\\a.ttf")));
    }

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("Collection 2", "Collection 10"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("collection 1", "Favorites"), std::cmp::Ordering::Less);
    }

    #[test]
    fn font_round_trip() {
        let db = FontDatabase::open_in_memory().unwrap();
        let rec = FontRecord {
            key: FontKey::new("s", 0, Some(1)),
            sha: "s".into(),
            family: "Bahnschrift".into(),
            subfamily: "Light Condensed".into(),
            typographic_family: Some("Bahnschrift".into()),
            weight_class: 300,
            width_class: 3,
            panose: Panose([2, 11, 5, 2, 4, 2, 4, 2, 2, 4]),
            code_page_range: [0x2000_009f, 0],
            unicode_range: [0xa000_02ff, 0x4000_207b, 0, 0],
            is_variable: true,
            feature_records: vec![FeatureRecord { feature: "liga".into(), script: "latn".into(), language: "dflt".into() }],
            axes: vec![VariationAxis { tag: "wght".into(), name: "Weight".into(), min: 300.0, default: 400.0, max: 700.0, value: 300.0, hidden: false }],
            instances: vec![NamedInstance { subfamily: "Light".into(), postscript_name: None, coordinates: vec![300.0, 100.0] }],
            localized_names: vec![LocalizedName { language: 0x411, family: Some("ベ".into()), ..Default::default() }],
            warnings: vec!["w".into()],
            file_size: 76_595,
            ..Default::default()
        };
        let id = insert_font(db.connection(), &rec).unwrap();
        let back = db.load_fonts().unwrap().remove(0);
        let expected = FontRecord { id, features: vec!["liga".into()], ..rec };
        assert_eq!(back, expected);
        assert_eq!(back.file_size, 76_595, "the File Size of the Font info tab survives a restart");
    }

    /// A version 1 database (no `fonts.file_size`) is migrated on open: the column is added, filled from
    /// `files.size`, and the schema version is bumped.
    #[test]
    fn migrates_version_1_file_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fonts.db");
        {
            let db = FontDatabase::open(&path).unwrap();
            let rec = FontRecord { key: FontKey::new("abc", 0, None), sha: "abc".into(), family: "Algerian".into(), ..Default::default() };
            insert_font(db.connection(), &rec).unwrap();
            let c = db.connection();
            c.execute(
                "INSERT INTO files(path, size, mtime, sha, format) VALUES ('c:/windows/fonts/alger.ttf', 76595, 0, 'abc', 'ttf')",
                [],
            )
            .unwrap();
            // turn it back into a version 1 file
            c.execute_batch("ALTER TABLE fonts DROP COLUMN file_size; UPDATE properties SET value = '1' WHERE key = 'schema_version';")
                .unwrap();
        }
        let db = FontDatabase::open(&path).unwrap();
        assert_eq!(db.property("schema_version").unwrap().as_deref(), Some(SCHEMA_VERSION.to_string().as_str()));
        let f = db.load_fonts().unwrap().remove(0);
        assert_eq!(f.file_size, 76_595);
        assert_eq!(crate::format::file_size(f.file_size), "74.8 KB");
        // opening again is a no-op
        drop(db);
        assert_eq!(FontDatabase::open(&path).unwrap().load_fonts().unwrap()[0].file_size, 76_595);
    }
}
