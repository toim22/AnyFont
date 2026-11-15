# anyfont-core API

`crates/anyfont-core` is the engine of AnyFont. It is pure Rust, with no UI and no Win32 calls. The egui
app (`anyfont`) and the Windows layer (`anyfont-sys`, see [`sys-api.md`](sys-api.md)) build on it. Every
module has `//!` docs that cite the dossier evidence. This page is the map.

Bad font files never cause an error or a panic. They become records with `corrupt: Some(reason)`.
Fallible calls return `anyfont_core::Result<T>` with `CoreError`:

- `Io{path, source}`
- `Db`
- `Json`
- `Xml`
- `InvalidFont`
- `NotAllowed`
- `DuplicateName`
- `Cancelled`

## 1. Threading model and typical start-up

```rust
use anyfont_core::*;

// UI thread: one connection for edits, one in-memory snapshot for queries.
let db = FontDatabase::open(&data_dir.join("fonts.db"))?;
let settings = Settings::load(&data_dir.join("settings.json"));

// Background scan: its own connection, progress via channel, cancellable.
let scan = spawn_scan(ScanRequest {
    db_path: data_dir.join("fonts.db"),
    installed: sys_installed_fonts(),       // Vec<InstalledFont> from anyfont-sys
    ..Default::default()
});
// every frame:
for ev in scan.poll() {
    match ev {
        ScanProgress::Started { total, to_parse } => {}
        ScanProgress::File { done, total, path } => {}
        ScanProgress::Finished(report) => { /* reload the snapshot */ }
    }
}
// scan.cancel(); scan.is_finished(); scan.join()

let catalog = Catalog::load(&db)?;          // reload after a scan or any db edit
let result = catalog.query(&FontQuery { library: LibraryRef::MyFonts, ..Default::default() });
result.status_text                           // "My fonts: 681 fonts"
```

Concurrency rules:

- `FontDatabase` is `Send` but not `Sync`. Keep it on the UI thread.
- The scan opens its own connection. SQLite runs in WAL mode with a 10 s busy timeout, so both connections can
  be used at the same time.
- `Catalog` is a plain value (`Clone`). Queries take a few ms.
- A cancelled scan keeps the files it already stored. The next scan continues incrementally, and the report
  has `cancelled = true`.

## 2. Font records (`model`)

`FontRecord` is one face, or one named instance of a variable font. Instances are separate list entries, so
Bahnschrift shows "Light Condensed" and the others.

| Group | Fields |
|---|---|
| identity | `id: FontId`, `key: FontKey` (`sha:face:instance`), `sha`, `face_index`, `face_count`, `instance_index`, `paths` (every copy, first = primary), `file_size`, `modified`, `format: FileFormat` |
| names (English) | `family` (ID 1), `subfamily` (ID 2 or instance name), `typographic_family/subfamily` (16/17), `full_name`, `postscript_name`, `version` ("Version 1.57"), `unique_id`, `copyright`, `trademark`, `manufacturer`, `designer`, `description`, `vendor_url`, `designer_url`, `license`, `license_url`, `sample_text`, `localized_names: Vec<LocalizedName>` |
| classification | `technology: Technology` (TrueType, OpenTypeTrueType, OpenTypePostScript, Type1), `outline`, `is_variable`, `color: ColorFormats`, `weight_class`, `width_class` (1–9), `italic`, `oblique`, `fs_selection`, `mac_style`, `fs_type`, `embedding: Embedding`, `code_page_range`, `unicode_range`, `panose: Panose`, `vendor_id`, `symbol`, `vertical`, `monospaced`, `glyph_count`, `units_per_em`, `features` (sorted tags), `feature_records`, `axes: Vec<VariationAxis>`, `instances: Vec<NamedInstance>`, `warnings` |
| state | `installed`, `protected` (lock icon), `duplicate: DuplicateState`, `corrupt: Option<String>`, `remote: Option<RemoteInfo>` (online-only, needs Download), `collections: Vec<CollectionId>` |

Helper methods:

- `group_family()` returns the typographic family, or the legacy family if there is none.
- `display_family(lang, show_latin)` and `display_legacy_family(lang, show_latin)`.
- `label(lang, show_latin)` builds the list label: `Arial (Regular, Normal)`, `Arial (Bold-Italic)`, `Arial Black`.
- `path()`, `is_usable()`, `needs_download()`, `weight_bucket()` (100–900).

Captions:

- `Technology::caption()`: "OpenType - TrueType". `Technology::is_opentype()` picks the icon: "O" for
  OpenType, "TT" otherwise.
- `Embedding::caption()`: "Preview and Print", "Editable", "Installable" or "Restricted".

`InstalledFont { path, protected }` is the input from `anyfont-sys`.

## 3. Database (`db::FontDatabase`)

| Area | Calls |
|---|---|
| open | `open(path)`, `open_in_memory()`, `path()`, `property/set_property` |
| folders | `folders()` (sorted by name), `folder(id)`, `add_folder(path, recursive)` (an existing path returns the existing folder), `set_folder_enabled`, `set_folder_recursive`, `remove_folder` |
| collections | `collections()` (natural sort: "Collection 1", "Collection 2", "Favorites"), `collection(id)`, `collection_by_name`, `favorites()` (always exists), `next_collection_name()` ("Collection N"), `create_collection(None \| Some(name))`, `rename_collection` (Favorites: `NotAllowed`; name taken: `DuplicateName`), `remove_collection` (Favorites is emptied, not deleted), `add_to_collection(id, &[FontKey]) -> new count`, `remove_from_collection`, `collection_fonts(id)` |
| recent | `touch_mru(&key, max)`, `mru()`, `clear_mru()` |
| PANOSE | `panose_exceptions()`, `set_panose_exception`, `remove_panose_exception`, `substitutions()`, `set_substitution` (a `Temporary` one is not stored), `remove_substitution` |
| installed set | `set_installed(&[InstalledFont])`, `installed()` (the scan sets it) |
| fonts | `font_count()`, `load_fonts()`, `add_remote_font(&FontRecord)` (Get More catalog entry) |

`Folder { id, path, enabled, recursive }` has `name()` (the last path component, which is also the status
text name) and `contains(path)`. `Collection { id, name, is_favorites }`. `FAVORITES_NAME = "Favorites"`.

Collections store `FontKey`, not row ids, so they survive a rebuild of the database. The schema is in
[`phases/05-data-models.md`](phases/05-data-models.md).

## 4. Scanning (`scan`)

The scan works in this order:

1. `ScanRequest { db_path, installed, force, threads }` describes the work.
2. It collects the files: the installed set, then the enabled folders (recursive or not). A `.pfm` is
   replaced by its `.pfb`/`.pfa`.
3. It skips files whose path, size and mtime have not changed.
4. It parses the rest in parallel and stores them by content hash, so identical copies share their faces and
   a copy of an installed font counts as installed.
5. It drops vanished files and recomputes duplicates.

For a synchronous scan (tests, tools), call `scan(&mut db, &req, &mut |p| .., &cancel_flag)`.

`ScanReport` fields: `files_found`, `files_parsed`, `files_cached`, `files_removed`, `faces`, `corrupt`,
`cancelled`, `elapsed_ms`.

## 5. Queries (`catalog::Catalog`)

```rust
pub enum LibraryRef { MyFonts, Folder(FolderId), Collection(CollectionId) }
pub struct FontQuery {
    library, filters: FilterSet, search: String,
    group_by_family: bool /*true*/, show_duplicates: bool, show_recent: bool,
    show_latin_names: bool, language: u16 /*LCID, 0x0409*/, sort: SortOrder,
}
pub enum ListRow {
    RecentHeader { count },
    FamilyHeader { family, style_count, fonts: Vec<FontId> },   // only for families with 2+ faces
    Font { id, label, indent, recent },
}
pub struct QueryResult { rows, font_ids, recent, count, filtered, library_name, status_text }
```

What the query returns:

- **My fonts** holds every local face: installed fonts plus the fonts of the enabled folders.
- **A folder** holds the faces with a file under it.
- **A collection** holds its members.
- **Grouped list:** ordered by group family, then legacy family, weight, width, italic. Arial Black sits under
  Arial.
- **Search:** a flat list with no headers, ordered by family, then weight, width, italic.
- **Duplicates:** copies are hidden unless `show_duplicates` is set. Duplicate sets are evaluated inside each
  library.
- **Corrupt files:** listed only when the Corrupt status filter is checked.
- `status_text` reads "My fonts: 681 fonts" or "My fonts (filtered): 10 fonts".
- `first_font()` is the face that is auto-selected when a library opens.
- `font_ids` is the set that Select All selects.

Other calls:

| Call | Returns |
|---|---|
| `fonts()`, `font(id)`, `font_by_key(&key)`, `folders()`, `collections()`, `collection(id)`, `mru()` | stored data |
| `libraries()` | left panel entries: My fonts, then folders, then collections, each with a count |
| `library_name(&lib)`, `library_fonts(&lib)`, `visible_fonts(&lib, show_dup, with_corrupt)` | library contents |
| `available_filters(&lib)` | dynamic filter options (see §6) |
| `eligibility(lib, &faces, &header_faces, all_selected, current)` | an `Eligibility` (see §7) |
| `library_menu(lib)` | library context menu: install, uninstall, remove, rename |
| `family_of(id)` | faces of the same family |
| `catalog::font_info(&font, catalog.collections())` | Font info tab rows as `(caption, value)`: Family, Sub-family, File Path, File Size ("74.8 KB"), Version, Font technology, Code Page (one "1252  (ANSI - Latin I)" per line), Italic, Weight, Width, Embedding, Manufacturer, Designer, Vendor URL, Copyright, Collection, and Error for corrupt files |
| `catalog::variable_axes(&[&font])` | `Some(axes)` only for exactly one variable face |

When `variable_axes` returns `None`, show `catalog::NO_VARIABLE_FONT_TEXT`.

## 6. Filters (`filters`)

The groups appear in panel order:

| `FilterGroup` | Options |
|---|---|
| `FontStatus` | `Status(Installed \| NotInstalled \| ProtectedSystemFonts \| Duplicates \| Corrupt)` |
| `EmbeddingRights` | `Embedding(..)` |
| `FontTechnology` | `Technology(OpenTypePostScript \| OpenTypeTrueType \| TrueType \| Type1 \| Variable)` |
| `Weight` | `Weight(Light \| Regular \| Bold)` |
| `Width` | `Width(Condensed \| Normal \| Expanded)` |
| `Style` | `Style(Decorative \| Display \| Monospaced \| SansSerif \| Script \| Serif \| Symbol)` |
| `CharacterRange` | `CharRange(Arabic … Turkish)` |
| `OpenType` | `OpenType(OpenTypeGroup::…)`, 24 groups, each with `tags()` |

`FilterSet` is a `BTreeSet<FilterOption>`. Its methods are `with`, `set`, `toggle`, `is_checked`,
`clear`, `clear_group`, `is_empty` and `matches(&font)`. Options in the same group are combined with OR, and
groups are combined with AND.

`FilterOption::caption()` and `FilterGroup::caption()` return the panel strings.
`filters::available_options(fonts)` returns `Vec<FilterGroupOptions { group, options }>`:

- the four status options are always included;
- Corrupt is included only when the library has corrupt files;
- every other option is included only when at least one font matches it.

## 7. Command eligibility (`eligibility`)

`Eligibility` has these fields:

| Area | Fields |
|---|---|
| toolbar | `add_folder`, `add_collection`, `get_more`, `remove_folder_or_collection`, `install`, `uninstall`, `download`, `delete`, `show_duplicates`, `show_font_properties` |
| Font menu and context menu | `menu_install`, `menu_uninstall`, `menu_download`, `menu_delete` (a family header counts as all of its faces) |
| other items | `open_file_location`, `add_to_collection`, `remove_from_open_collection`, `remove_from_collection`, `deselect_all`, `rename_library` |

The toolbar fields reproduce all 13 captured states in `toolbar-states.json`, and a unit test checks them.
A selected family header enables the `menu_*` fields but not the toolbar buttons, as in the capture
(UI-SPEC §2.6 [decide]). `Selection` can also be built by hand and passed to `eligibility::evaluate`.

## 8. Glyphs tab (`glyphs`)

Get the glyph list with `glyphs::enumerate_file(path, face_index)`. It returns
`Option<GlyphList { glyphs: Vec<GlyphEntry> }>`, or `None` if the file cannot be read. Order of the list:

1. encoded glyphs in code-point order;
2. after each one, its GSUB single, alternate and ligature outputs (`GlyphOrigin::Alternate`);
3. the unencoded glyphs.

Fields of `GlyphEntry`:

- `glyph_id`
- `codepoint`
- `name`
- `origin`
- `script` ("Common", "Latin" … or `UNENCODED_GROUP`)
- `category: GlyphCategory`, one of Uppercase, Lowercase, Numbers, PunctuationAndSeparators, Currency,
  MathematicalSymbols, Marks, Symbols, Other. `caption()` gives the panel string.

Other calls:

- `GlyphList::count_text()` gives "262 glyphs".
- `available_filters()` returns `Vec<(script, Vec<GlyphCategory>)>`, with "Common" first.
- `filtered(&BTreeSet<GlyphFilter>)` applies the filters with OR.
- `enumerate_sfnt(bytes, index)` and `enumerate_type1(names)` build the list from bytes or Type 1 glyph
  names.

## 9. PANOSE matching (`panose`)

Preference and dialog types:

- `PanoseMode { NeverShow, Text, TextAndStyles }` has `caption()` and `shows_results(from_style)`.
- `SubstitutionType { Panose, Exception }`, `SubstitutionStatus { Permanent, Temporary }`,
  `FontFilterType { SameCodePage, ShowAll }`.
- `MissingFont { family, style, postscript_name, weight, italic, panose, charset, technology }`.
- `PanoseException { missing, substitute }`, `Substitution { missing, substitute, kind, status }`.

`find_substitute(&missing, catalog.fonts(), &exceptions, &substitutions, Some(default_font), filter)` returns
`MatchResult { substitute, reason: MatchReason, kind, candidates: Vec<Candidate { font_id, family,
panose_distance, name_similarity }> }`. The candidates come best first.

It tries these stages in order:

1. exception
2. stored substitution
3. exact name
4. same family, closest style
5. nearest PANOSE
6. similar name
7. default font

Helpers: `distance(&a, &b) -> Option<u32>` (the weighted metric; `None` when the family kinds differ),
`levenshtein`, `name_similarity`, `charset_code_page_bit`.

Type 1 support: `type1_replacements(fonts)` returns `Vec<Type1Replacement { type1, replacement }>` for
"Replace fonts". `TYPE1_MESSAGE` is the dialog text.

## 10. Import and export (`exchange`)

| Call | Does |
|---|---|
| `has_exportable(&db)` | tells whether Export has anything to write |
| `export_xml(&db, &catalog, &ExportOptions::all())` / `export_file(..., path)` | writes the export |
| `import_xml(&db, &catalog, &xml)` / `import_file(..., path)` | reads an export and returns `ImportReport { folders_added, folders_existing, collections_created, collections_merged, fonts_linked, fonts_missing }` |

Reload the `Catalog` after an import.

The format is the original's `<Export><MonitoredFolders/><UserGroups/>` XML. AnyFont adds a `key` attribute
for exact matches, and files written by the original import too.

## 11. Settings (`settings::Settings`)

- `Settings::load(path)` never fails. A missing or invalid file gives the defaults.
- `save(path)` writes atomically.

Fields:

| Area | Fields |
|---|---|
| window | `window: WindowState` (position, size, maximised, panel widths, right panel visible and tab) |
| preview | `preview_size`, `sample_text` (`SAMPLE_FONT_NAME` = "Font Name"), `sample_history`, `remember_sample()` |
| View toggles | `group_by_family`, `show_recent`, `render_name_in_font`, `show_latin_names`, `show_duplicates`, `show_font_properties` |
| Options dialog | `theme: Theme` (Dark, MediumDark, MediumLight, Light), `ui_scale_percent`, `ui_scale_enabled`, `ui_scale()`, `window_border_color` |
| PANOSE | `panose { mode, default_font }` |
| other | `update_mode`, `language`, `mru_size`, `last_library`, `expanded_filter_groups`, `glyph_cell_size` |

`SAMPLE_PRESETS` holds the five sample text presets.

## 12. Smaller helpers

| Module | Calls |
|---|---|
| `format` | `face_label`, `file_size`, `weight_name`, `width_name`, `yes_no`, `code_pages(range)` → `CodePage { number, description }.caption()`, `status_text`, `count_text` |
| `vendors` | `vendor_name(id)`, `vendor_url(&font)` (name ID 11, or the vendor table), `foundry(&font)` |
| `duplicates` | `duplicate_key`, `compute`, `mark` |
| `parse` | `parse_font_file(path) -> ParsedFile { format, sha, faces }`, `parse_font_bytes`, `is_font_path`, `FONT_EXTENSIONS` |
| `sfnt` | `SfntFace::open(bytes, index)?.validate()` (the original's validation rules), `NameTable`, `Os2`, `Fvar` |
| `woff` | `decode_woff` |
| `type1` | `parse_type1`, `parse_pfm`, `parse_afm` |

Supported formats: `.ttf .otf .ttc .otc .woff .pfb .pfa` (with `.pfm`/`.afm`), plus a lone `.pfm`, which is
reported as missing its outline file. WOFF2 is recognised but reported as unsupported, as a corrupt record
with the reason "WOFF2 files are not supported".
