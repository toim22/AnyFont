# Phase 05: Data models (font database, settings, file formats)

Dossier sources:

- `05-data-models/file-formats/font-database.md`: the SQLite schema of the shipped and user databases.
- `05-data-models/file-formats/font-files.md`: what the font engine reads from a font file.
- `05-data-models/settings.md`.
- `05-data-models/file-formats/json-configs.md`.

The handlers that read and write this data are listed in `09-features/feature-catalog.md` and
`06-ui/handler-map.md`.

The AnyFont implementation is `crates/anyfont-core`. Its API is described in
[`../core-api.md`](../core-api.md).

## 1. Font database

### What the dossier says

`FontDatabase.sqlitedb` has 20 tables and 681 `FONTSDATA` rows on the analysed PC:

| Table | Columns | Role |
|---|---|---|
| `FONTSDATA` | 66 | one row per face, keyed by `DBShaID` (content hash). `UNIQUE(DBFamily, DBSubFamily, DBVersion, DBTech, DBWeight, DBWidth, DBItalic, DBManufacturer, DBDocumentID)` |
| `FONTVARIABLEAXES` | 9 | axis tag, attributes, value, min, max, default, names per face |
| `OPENTYPE` | 4 | `(DBFontRowId, DBFeature, DBScript, DBLanguage)` |
| `FONTFOLDERS` | 3 | `(DBEntryID, DBEnabled, DBFontFolder)`: monitored folders |
| `GROUPS` / `GROUPLINKS` | 1 / 2 | collections (`DBName`) and membership `(DBGroupRowId, DBFontRowId)` |
| `DBPROPERTIES` | 2 | key/value properties |
| `FONTSAMPLES`, `FONTPREVIEWCACHE`, `FONTCURVES` | 9 / 5 / 3 | rendered sample and preview curves (cache) |
| `KEYWORDS_ALL`, `KEYWORDS_<lang>` ×16 | 2 | search keywords per language |
| `sqlite_sequence` | 2 | SQLite bookkeeping |

A separate `Interprocess.sqlitedb` (`DBEVENTLISTENERS`, `PROTOCOLREQUESTS`, `REQUESTS`, `REQUESTSRESULTS`) carries
requests between the suite's processes.

### AnyFont

`anyfont_core::db::FontDatabase` creates one SQLite file. The UI team chooses its location, normally
`%LOCALAPPDATA%\AnyFont\fonts.db`. The file runs in WAL mode with a busy timeout, so the scanner thread and
the UI connection can use it at the same time.

| AnyFont table | Original | Notes |
|---|---|---|
| `properties(key, value)` | `DBPROPERTIES` | `schema_version` (2: `fonts.file_size` added; opening a version 1 file adds the column and fills it from `files.size`), `last_scan` |
| `files(path NOCASE UNIQUE, size, mtime, sha, format, error, scanned_at)` | `FONTSDATA.DBPaths` | one row per file location. Path, size and mtime drive the incremental rescan. All rows with the same `sha` are one font's `DBPaths` |
| `fonts(...)` | `FONTSDATA` | one row per face and per named instance. `key = "<sha>:<face>:<instance or ->"` is the `DBShaID` equivalent. Column mapping in §1.1 |
| `font_names(font_id, language, family, subfamily, full_name, typographic_family, typographic_subfamily)` | `DBLocales` + `KEYWORDS_<lang>` | localised names from the `name` table. They drive "Show Latin names" and localised search |
| `font_features(font_id, feature, script, language)` | `OPENTYPE` | same triple |
| `font_axes(font_id, ord, tag, name, min, def, max, value, hidden)` | `FONTVARIABLEAXES` | `value` is the instance coordinate. `hidden` comes from the attribute flag |
| `font_instances(font_id, ord, subfamily, postscript_name, coordinates)` | (in `FONTVARIABLEAXES` rows per instance) | every named instance of the file |
| `folders(path NOCASE UNIQUE, enabled, recursive)` | `FONTFOLDERS` | `recursive` is an AnyFont addition |
| `collections(name NOCASE UNIQUE, is_favorites)` | `GROUPS` | Favorites always exists |
| `collection_fonts(collection_id, font_key, added_at)` | `GROUPLINKS` | stores the content key, not the row id, so collections survive a cache rebuild |
| `mru(font_key, used_at)` | (in-memory list in the original) | View > Show recent |
| `panose_exceptions(missing, substitute)` | `PanosePreferences.xml` | exception list |
| `panose_substitutions(missing, substitute, kind, status)` | `PanosePreferences.xml` | permanent substitutions only |
| `installed(path NOCASE, protected)` | `DBInstalled`, `DBSystem` | installed set supplied by `anyfont-sys` |

The following original tables are not reproduced, by design:

- `KEYWORDS_*`: search runs over the in-memory `Catalog`, over English and localised names.
- `FONTSAMPLES`, `FONTPREVIEWCACHE`, `FONTCURVES`: the UI renders with the font itself through egui's font
  system, so no curve cache is needed.
- `Interprocess.sqlitedb`: AnyFont is a single process.

### 1.1 `FONTSDATA` columns → AnyFont

Fields are `FontRecord` fields. Columns are in the `fonts` table unless noted otherwise.

| Original column(s) | AnyFont |
|---|---|
| `DBEntryID` | `id` |
| `DBShaID` | `sha` + `key` (face and instance make a face unique inside a collection file) |
| `DBDefaultFamily`, `DBFamily` | `family` (name ID 1, English) |
| `DBPreferredFamily` | `typographic_family` (ID 16) |
| `DBDefaultSubFamily`, `DBSubFamily` | `subfamily` (ID 2, or the instance name) |
| `DBPreferredSubFamily` | `typographic_subfamily` (ID 17) |
| `DBFullFontName` | `full_name` (ID 4) |
| `DBPostScriptFontName`, `DBPostScriptWithStyleFontName` | `postscript_name` (ID 6, or the instance PostScript name) |
| `DBDefaultFamilyStandardized`, `DBFamilyStandardized` | computed on demand: `panose::normalize_name` |
| `DBLocales` | `font_names` table |
| `DBLevenshteinDistance` | computed on demand: `panose::levenshtein` / `name_similarity` |
| `DBVersion` | `version` (ID 5) |
| `DBTech` | `technology` (same strings: `TrueType`, `OpenType-TrueType`, `OpenType-PostScript`, `Type1`) |
| `DBGdiPitchFamily` | `monospaced` + `panose` (GDI pitch/family is derived by `anyfont-sys` when needed) |
| `DBCdrFntWeight` | `weight_bucket` (weight class rounded to 100–900) |
| `DBWeight` | `weight_class` |
| `DBWidth` | `width_class` |
| `DBItalic`, `DBOblique` | `italic`, `oblique` |
| `DBEmbedding` | `embedding` (1 Preview and Print, 2 Editable, 3 Installable, 4 Restricted) |
| `DBDesigner`, `DBVendorURL`, `DBCopyrights`, `DBTrademark`, `DBManufacturer` | `designer`, `vendor_url`, `copyright`, `trademark`, `manufacturer` |
| `DBulCodePageRange1/2` | `code_page1/2` → `code_page_range` |
| `DBulUnicodeRange1..4` | `unicode1..4` → `unicode_range` |
| `DBPanose0..9` | `panose0..9` → `panose` |
| `DBSymbol`, `DBVertical`, `DBVariable` | `symbol`, `vertical`, `is_variable` |
| `DBInstalled`, `DBInstalledCache` | `installed`, computed from the `installed` table by path |
| `DBSystem` | `protected` (`installed.protected`) |
| `DBAvailable`, `DBAvailableCache` | `is_usable()` (not corrupt and has a file) |
| `DBPaths` | `files` table → `paths` |
| `DBAssetURL`, `DBMetadataURL`, `DBProviderName`, `DBRemoteFont` | `remote_asset_url`, `remote_metadata_url`, `remote_provider` → `remote: Option<RemoteInfo>` |
| `DBDuplicate` | `duplicate` (0 unique, 1 master, 2 copy) |
| `DBTemporaryMapped`, `DBEmbedded`, `DBDocumentID`, `DBMappedRowId`, `DBMappedRowIdVertical`, `DBPartial`, `DBUserUploadedFont` | not used. They belong to document-embedded fonts and font mapping inside the drawing application, which a standalone font manager does not have |

AnyFont also stores:

- `glyph_count`, `units_per_em`, `outline`, `color`, `fs_selection`, `mac_style`, `fs_type`;
- `unique_id`, `description`, `designer_url`, `license`, `license_url`, `sample_text`, `vendor_id`;
- `face_index`, `face_count`, `instance_index`, `format`;
- `corrupt`, the reason a file could not be read. The original drops such files, while AnyFont lists them
  under the "Corrupt" status filter.
- `warnings`: non-fatal validation findings.

## 2. Font file data (`font-files.md`)

### What the dossier says

The original's font engine:

- validates the sfnt container: TTC face selection, `numTables`, `head` magic and `unitsPerEm`, `maxp` flavour,
  `cmap` version, the table matrix per flavour, and a list of known problem families;
- reads `name`, `OS/2`, `head`, `post`, `fvar`/`STAT`, `GSUB`/`GPOS`;
- reads Type 1 `.pfb`/`.pfm`/`.afm`.

### AnyFont

| Module | Reads |
|---|---|
| `sfnt` | the validation rules, one test per rule (see Phase 04 §1), plus its own `name`/`OS/2`/`fvar`/`STAT` readers |
| `parse` | `ttf-parser` 0.25 for `cmap`, `post`, `GSUB`/`GPOS` scripts, languages and features, and glyph names |
| `woff` | WOFF 1.0, by decompressing to sfnt. WOFF2 is detected and reported as unsupported, because it needs Brotli and the `glyf` transform |
| `type1` | `.pfb`/`.pfa` (eexec decryption, `/CharStrings` names), `.pfm` (Windows metrics: face name, weight, italic, charset), `.afm` |

`FontRecord` is the in-memory model: every field above plus the library state.

## 3. Settings

### What the dossier says

`05-data-models/settings.md` and the Options dialog captures cover:

- View toggles: Group by family, Show recent, Show Latin names, Show duplicate fonts, Show font properties;
- preview size and sample text (`FONTSAMPLES` presets: Font Name, a–z, A–Z, 0–9, pangram);
- Options: UI scale % (slider 100.0 plus a checkbox), Theme (Light/Dark strings), Window border colour;
- PANOSE preferences: mode, default font, exceptions.

### AnyFont

`settings::Settings` is serde JSON stored at a path the caller chooses. It has `version: 1`.

| Field | Default |
|---|---|
| `window` (position, size, maximised, panel widths, right panel visible and tab) | 1280×800 |
| `preview_size` | 24 |
| `sample_text` | `"Font Name"` |
| `sample_history` | at most 20 entries |
| `group_by_family` | true |
| `show_recent` | true |
| `render_name_in_font` | true |
| `show_latin_names` | false |
| `show_duplicates` | false |
| `show_font_properties` | true |
| `theme` | Dark; the others are Light, MediumLight, MediumDark |
| `ui_scale_percent`, `ui_scale_enabled` | 100, false |
| `window_border_color` | none |
| `panose.mode` | Text |
| `panose.default_font` | "Arial" |
| `update_mode` | Notify |
| `language` | none (system) |
| `mru_size` | 10 |
| `last_library` | My fonts |
| `expanded_filter_groups` | empty |
| `glyph_cell_size` | 48 |

Load and save behaviour:

- Unknown fields are ignored and missing fields take their default.
- Out-of-range values are clamped.
- A damaged file loads as the defaults.
- Save writes a temporary file and renames it.

## 4. Import/Export file

### What the dossier says

`FontDS.OnExportGroups` writes `<?xml version='1.0'?><Export>`, which contains
`<MonitoredFolders><folder path enabled/></MonitoredFolders>` and
`<UserGroups><group name><font DBFamily DBSubFamily DBVersion DBTech DBWeight DBWidth DBItalic DBManufacturer/>`.
Import matches fonts with the eight attributes.

### AnyFont

`exchange` writes the same elements and attributes, and adds:

- `format="AnyFont" version="1"` on the root;
- `recursive` on `folder`;
- `favorites="1"` on the Favorites group;
- `key` on `font`.

Import matches fonts by `key` first, then by the eight attributes. If no font has the same version, it accepts
the same face with another version. Files written by the original therefore import as well.
