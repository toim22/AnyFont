# Phase 09: Features

Dossier: `09-features/feature-catalog.md` and `06-ui/handler-map.md`.

Each row links a feature to its evidence in the dossier, the AnyFont code that implements it, and the test that
covers it. The core rows (`crates/anyfont-core`) are below. The UI (`anyfont`) and Windows layer
(`anyfont-sys`) rows belong in their own sections, which their owners add.

## Core (`anyfont-core`)

Tests named `module::tests::x` are unit tests in `src/`. Tests named `integration::x` are in
`crates/anyfont-core/tests/integration.rs`. Run them with `cargo test -p anyfont-core`.

| Feature | Dossier evidence | AnyFont | Test |
|---|---|---|---|
| sfnt validation: TTC face, numTables, head magic, unitsPerEm, glyphDataFormat, maxp flavour, cmap version, table matrix, quirk list | `05-data-models/file-formats/font-files.md` §6; `Initialize` 0x18017F470, `ReadOffsetTable` 0x18017FC90 | `sfnt::SfntFace::open`, `validate` | `sfnt::tests::*` (one per rule), `ttc_face_selection` |
| Font formats: TTF, OTF, TTC/OTC, WOFF | font-files.md; catalog §1 | `parse::parse_font_file`, `woff::decode_woff` | `integration::parses_fixture_fonts`, `collection_faces`, `woff_fixture_round_trip`, `parse_every_windows_font` (ignored, 478 files / 670 faces / 0 corrupt on the dev PC) |
| WOFF2 | not in the original | recognised and reported as unsupported (Corrupt) | `integration::woff_fixture_round_trip` |
| Type 1 `.pfb/.pfa/.pfm/.afm` | font-files.md (Type 1); `Type1InfoMessageDS` | `type1::*`, `parse` merging companions | `type1::tests::*`, `parse::tests::type1_record` |
| Font record and Font info fields (Family, Sub-family, File Path, File Size, Version, Font technology, Code Page, Italic, Weight, Width, Embedding, Manufacturer, Designer, Copyright, Collection) | `FONTSDATA` columns; screenshots `06-panels/font-info` | `FontRecord`, `catalog::font_info`, `format::*` | `format::tests::*`, `catalog::tests::libraries_folders_collections_recent_duplicates` |
| Technology rule (TrueType / OpenType - TrueType / OpenType - PostScript / Type 1) | `CFontDatabase::s_strTrueType` …; screenshots (Arial vs Algerian) | `parse::build_sfnt_records` (DSIG rule) | `integration::system_fonts_parse_when_available` |
| Embedding rights | `GetFontEmbeddingPermissions` 0x18017CE60 | `Embedding::from_fs_type` | `model::tests::embedding_from_fs_type`, `filters::tests::*` |
| Font database (20 tables, `FONTSDATA` 66 columns) | `05-data-models/file-formats/font-database.md` | `db::FontDatabase` (schema in Phase 05) | `db::tests::font_round_trip`, `folders_mru_panose_persist` |
| Incremental rescan off the UI thread, progress, cancel | `FONTFOLDERS` monitoring, database refresh | `scan::scan`, `spawn_scan`, `ScanHandle` | `integration::incremental_rescan_folders_duplicates_and_cancel`, `spawned_scan_reports_progress` |
| Identical copies are one font (copy of an installed font shows as installed) | `DBShaID` + `DBPaths`; temp-folder screenshots | content-hash `FontKey`, `files` table | `integration::incremental_rescan_…` |
| Libraries: My fonts, user folders (optional recursion), collections | catalog §1; `FONTFOLDERS`, `GROUPS`; screenshots | `Catalog::library_fonts`, `libraries`, `Folder::contains` | `db::tests::folder_membership`, `catalog::tests::*` |
| Add Folder / Remove Folder | `FontDS.OnAddFolder`, `RemoveFolderOrCollection` | `FontDatabase::add_folder`, `remove_folder`, `set_folder_enabled/recursive` | `db::tests::folders_mru_panose_persist` |
| Collections: Favorites always exists; "Collection N" default names; rename; remove (Favorites is emptied) | `GROUPS`/`GROUPLINKS`; toolbar-states (Remove enabled for Favorites) | `create_collection`, `next_collection_name`, `rename_collection`, `remove_collection`, `add/remove_to_collection` | `db::tests::favorites_and_collection_names` |
| Status text "My fonts: 681 fonts", "(filtered)", folder name | status bar screenshots | `format::status_text`, `QueryResult::status_text` | `catalog::tests::*` |
| Group by family; header only for 2+ faces; style count; Arial Black under Arial | font list screenshots | `Catalog::query` | `catalog::tests::grouped_list_and_status` |
| Search (flat list) | search screenshots | `Catalog::query` | `catalog::tests::search_is_flat_and_filters_mark_filtered` |
| Face labels `Arial (Regular, Normal)`, `Arial Black`, `Bahnschrift Light Condensed` | font list screenshots | `format::face_label` | `format::tests::labels_match_screenshots`, `integration::bahnschrift_named_instances_when_available` |
| Show Latin names | View menu; `DBLocales` | `FontRecord::display_family`, `FontQuery::show_latin_names` | `model::tests::latin_names_and_weight_bucket`, `db::tests::font_round_trip` |
| Show recent (MRU) | View menu | `touch_mru`, `mru`, `FontQuery::show_recent` | `catalog::tests::libraries_…`, `integration::collections_mru_export_import_on_scanned_db` |
| Show duplicate fonts; Duplicates filter | `DBDuplicate`, `DBDefaultFamilyIndex` | `duplicates::compute`, `Catalog::visible_fonts` | `duplicates::tests::*`, `integration::incremental_rescan_…` |
| Filters: Font Status, Embedding Rights, Font Technology, Weight, Width, Style, Character Range, OpenType (OR within, AND across) | `sub_1800DDB60`, predicates at 0x180113B50 … 0x180116770 | `filters::FilterSet`, `FilterOption::matches` | `filters::tests::*` |
| Dynamic filter options (only present values) | `SELECT DISTINCT` probes | `filters::available_options`, `Catalog::available_filters` | `filters::tests::status_and_availability`, `integration::spawned_scan_reports_progress` |
| Toolbar / menu eligibility (Install, Uninstall, Delete, Download, Remove Folder or Collection …) | `FontDS.*Enabled`; `06-ui/screenshots/90-reports/toolbar-states.json` | `eligibility::evaluate`, `Catalog::eligibility`, `library_menu` | `eligibility::tests::toolbar_states_match_capture` (all 13 states), `install_download_and_context_rules` |
| Opening a library selects its first font | toolbar-states (library contexts) | `QueryResult::first_font` | `catalog::tests::grouped_list_and_status` |
| Import / Export Folders and Collections | `FontDS.OnExportGroups`, `HasGroupsForExport`, `OnImportGroups`, `ReadyForImport` | `exchange::*` | `exchange::tests::round_trip`, `imports_original_format_by_attributes`, `integration::collections_mru_export_import_on_scanned_db` |
| Vendor URL, SelectedVendor, SelectedFoundry | `FontDS` properties; achVendID | `vendors::vendor_url`, `foundry`, `vendor_name` | `vendors::tests::vendor_lookup_and_fallback` |
| PANOSE matching: 3 modes, default font, exceptions, permanent/temporary substitutions, SameCodePage / ShowAll, ranking | catalog §3; `WPanose*` 0x1801A6A40, 0x1801A7840 | `panose::find_substitute`, `distance`, `PanoseMode`; persistence in `db` | `panose::tests::*`, `db::tests::folders_mru_panose_persist`, `integration::panose_matching_on_real_fonts` |
| Type 1 information and "Replace fonts" | `Type1InfoMessageDS` | `panose::type1_replacements`, `TYPE1_MESSAGE` | `panose::tests::type1_replacement_suggestion` |
| Glyphs tab: encoded glyphs, their alternates, unencoded glyphs, "N glyphs", script/category filters | glyph docker (`WGlyphDockerDS`); screenshots | `glyphs::enumerate_file`, `GlyphList::available_filters`, `filtered` | `glyphs::tests::*`, `integration::glyph_list_of_real_font` |
| Variable tab: axes (name, value) and named instances as list entries | `FONTVARIABLEAXES`; Bahnschrift screenshots | `FontRecord::axes`, `instances`, `catalog::variable_axes`, `NO_VARIABLE_FONT_TEXT` | `integration::variable_font_instances_and_axes`, `bahnschrift_named_instances_when_available` |
| Settings: View toggles, preview size and sample text, theme, UI scale, border colour, PANOSE preferences | `05-data-models/settings.md`; Options dialog screenshots | `settings::Settings::load` / `save` | `settings::tests::round_trip_and_bad_files` |
| Bad fonts never crash | (robustness) | corrupt records; `catch_unwind` around parsing in scans | `parse::tests::corrupt_bytes_never_panic`, `integration::incremental_rescan_…` |

### Not implemented in the core

| Item | Reason |
|---|---|
| WOFF2 decoding | needs Brotli plus the `glyf`/`loca` transform. The original does not read web fonts either. WOFF2 files are listed as Corrupt, with a reason |
| `FONTSAMPLES` / `FONTPREVIEWCACHE` / `FONTCURVES` caches | the UI renders with the font itself |
| `KEYWORDS_*` tables | search runs in memory over English and localised names |
| Document-mapping columns (`DBDocumentID`, `DBTemporaryMapped`, `DBMappedRowId…`, `DBEmbedded`, `DBPartial`, `DBUserUploadedFont`) | they belong to the drawing application that hosts the original font manager |
| The original PANOSE distance weights | not recoverable from the dossier. AnyFont's weighted metric is documented in Phase 04 §8 |
