# Phase 13: Evidence (traceability matrix)

Dossier: `13-evidence/evidence-log.csv` (claim → file/address → evidence id), `09-features/feature-catalog.md`
(F-01…F-07 and sections 2–6), `09-features/handler-map.md` / `handler-coverage.csv` (handler addresses),
`14-gaps/consistency.md`. AnyFont side: [`09-features.md`](09-features.md), the tests in `crates/*/src` (unit) and
`crates/*/tests` (integration and headless end-to-end), and [`../TESTING.md`](../TESTING.md).

Each row goes **feature → dossier evidence → AnyFont code → automated tests → status**.

- Addresses are the dossier's IDA addresses (`CdrFnt.dll` unless another module is named).
- Test names: `module::tests::name` is a unit test in `src/`; `file::name` is a test function in
  `crates/<crate>/tests/<file>.rs`. `integration::` is `anyfont-core/tests/integration.rs`; `sandbox::`, `guard::`,
  `env_sandbox::` are in `anyfont-sys/tests/`; the `e2e_*`, `operations`, `screens`, `startup` files are in
  `crates/anyfont/tests/`. `e2e-real` is the job of `.github/workflows/e2e-real.yml` (real installer and real
  font system on a disposable runner).
- `commands::name` is the `AnyFontApp` method defined in `commands.rs`.
- Status: **covered** (automated test of the behaviour), **partial** (only part is tested, or only below the UI),
  **gap** (implemented, no automated test), **bug** (a test exists and is `#[ignore = "BUG-n"]`, see
  `docs/QA-BUGS.md`), **n/a** (not implemented on purpose; the reason is given).

## 1. Dossier feature catalog

### 1.1 Font library management (F-01 … F-07)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| F-01 Install fonts | item `5682b3c4`, `FontDS.OnInstallFonts` 0x1800F1010 (`sub_1800F11C0`/`sub_1800F1200`), `InstallEnabled`; install worker 0x1802010F0; `AddFontResourceExW` 0x180201070; `WM_FONTCHANGE` 0x1802010D0 | `Cmd::Install` → `dialogs::start_fonts_op` → `ops::Running::start` → `FontSystem::install_with` (`anyfont-sys/src/engine.rs`); `eligibility::evaluate` | `e2e_operations::install_and_uninstall_from_the_toolbar_font_menu_and_context_menu`, `install_a_whole_family_from_its_header`, `progress_caption_and_cancel_of_an_install`; `operations::install_and_uninstall_in_the_sandbox`, `cancelled_uac_offers_install_for_me`; `sandbox::per_user_install_uninstall_round_trip`, `all_users_install_uses_system_folder_and_relative_data`, `file_already_in_fonts_folder_is_registered_in_place`, `file_name_collision_gets_numbered_copy`, `different_file_with_same_name_replaces_the_old_registration`; `engine::tests::unique_names_follow_dossier_pattern`; `e2e_selftest::the_real_system_script_passes_on_the_sandbox_backend`; `e2e-real` (`install`, `helper_install`) | covered |
| F-01 Install a library | item `b0c15ea3`, `FontDS.OnInstallLibrary` 0x1800EF6D0, `GetOnInstallLibraryEnable` 0x1800EF8F0 | `Cmd::LibraryInstall`, `Catalog::library_menu` | `e2e_libraries::library_menu_installs_and_uninstalls_a_whole_folder_and_collection`, `e2e_menus::library_context_menu_install_on_a_folder_of_new_fonts`, `library_context_menus` | covered |
| F-01 Reboot prompt | `FontInteractionDS.OnRebootNow` / `OnRebootLater` / `OnIgnore` (registrar 0x180182580, 0x180183FF0, 0x180184080, 0x180184110, 0x1801841A0); reboot 0x180202780 | `dialogs.rs` reboot prompt, `UsedFontAction`, `FontSystem::reboot_now`, `process_pending` | `e2e_operations::uninstall_in_use_reboot_now`, `uninstall_in_use_reboot_later`, `uninstall_in_use_ignore_and_cancel_keep_the_fonts`, `delete_in_use_asks_for_a_restart`; `sandbox::in_use_fonts_follow_the_used_font_action`, `all_users_in_use_font_is_deleted_at_reboot`, `open_location_and_reboot_are_recorded_only`; `protect::tests::open_file_is_in_use` | covered (the real reboot is never performed by a test) |
| F-02 Uninstall fonts / library | item `c087b613`, `FontDS.OnUninstallFonts` 0x1800F1610, filter 0x1800F17B0; library `1ee0bd61`, `OnUninstallLibrary` 0x1800EFA20; uninstall 0x180202390 | `Cmd::Uninstall`, `Cmd::LibraryUninstall` → `FontSystem::uninstall_with` | `e2e_operations::install_and_uninstall_from_the_toolbar_font_menu_and_context_menu`, `progress_captions_of_uninstall_and_delete`; `sandbox::per_user_install_uninstall_round_trip`, `fonts_outside_the_fonts_folders_are_kept_on_uninstall`; `e2e-real` (`uninstall`, `helper_uninstall`) | covered |
| F-02/F-03 Protected system fonts | DACL rule 0x1802028E0; "Protected System Font(s)" category 0x180113A60 | `anyfont-sys/src/protect.rs`, `win/acl.rs`; `FontSystem::is_protected`; lock marker in `fontlist.rs` | `e2e_operations::protected_system_fonts_cannot_be_uninstalled_or_deleted`; `sandbox::seeded_sandbox_has_protected_system_fonts`, `protected_fonts_are_never_uninstalled_or_deleted`, `replacing_a_protected_font_is_refused`; `protect::tests::critical_names`, `acl_rule`; `win::acl::tests::shipped_font_is_protected_and_temp_file_is_not`; `e2e-real` (`protected_refused`) | covered |
| F-03 Delete fonts | item `1fa770a0`, `FontDS.OnDeleteFonts` 0x1800F2190 (+0x1800FB0A0, 0x1800FB1E0, 0x1800FB200), filter 0x1800F26A0; `DeleteToRecycleBin` (`CrlUtils` 0x1800A2660) | `Cmd::Delete` → `dialogs::ask_delete` → `FontSystem::delete_with`; `win/shell.rs` `recycle` | `e2e_operations::delete_with_its_confirmation_dialog`, `delete_of_an_installed_font_uninstalls_it`, `failures_are_listed_read_only_file_and_invalid_font`; `operations::delete_asks_and_removes_the_file`; `sandbox::delete_recycles_and_uninstalls_first`, `delete_refuses_read_only_missing_and_busy_files`; `e2e-real` (`delete_copy`, Recycle Bin check) | covered |
| F-03 Delete a collection (`AssetListDS.OnDeleteGroup` 0x1800481E0) | `CrlGenericUI` | `commands::remove_library`, `FontDatabase::remove_collection` | `e2e_libraries::remove_with_confirmation_and_do_not_show_again`, `do_not_show_again_survives_a_restart`, `favorites_add_remove_and_remove_favorites_itself`; `e2e_libraries::removing_favorites_removes_it_for_good` | covered; **bug** BUG-306 (Favorites is only emptied) |
| F-04 Download fonts | item `163573c4`, `FontDS.OnDownloadFonts`, `DownloadFonts` 0x1800F1AC0, `DownloadEnabled`, visible ← `WPasmDataSource.IsUserAllowedToUseInternet`; `DownloadProgressDS` | Get More (`getmore.rs`: `parse_catalog`, `filter_sort`, `parse_manifest`, `download_list_url`) → Content folder → My fonts as *Not installed*; `Cmd::Download` for online placeholders; `FontDatabase::add_remote_font`; progress through `ops`/`dialogs` | `e2e_getmore_matching::get_more_search_filters_download_and_install`, `get_more_offline_then_retry`, `get_more_download_failure_is_reported_and_nothing_is_left`; `getmore::tests::*` (7); `eligibility::tests::install_download_and_context_rules`; `e2e_menus::every_menu_and_the_toolbar_in_the_13_contexts` (Download disabled as captured) | covered (the per-font cloud URL of the original is not reproduced, Phase 14) |
| F-05 Upload / share fonts | `UploadFontsDS.*` (0x1800CBDD0, 0x1800B8E20), `WPasmDataSource.InvokeShareYourExperienceUploadFonts` | – | – | n/a: cloud service (UI-SPEC §11 #17, lead decision 9). Feedback goes through Help ▸ Submit Ideas and Feedback (row 1.5) |
| F-06 Add a font folder | `FontDS.OnAddFolder` (`CrlGenericUI` 0x1800C5330), `FolderTreeDS.AddFolder` / `RemoveFolder` (0x18005EC10, 0x1800C93C0), `OpenFoldersEnabled` | `Cmd::AddFolder` → `commands::add_folder` → `FontDatabase::add_folder`, `set_folder_recursive`; folder argument on the command line | `e2e_libraries::add_folder_from_the_menu_and_the_toolbar_with_its_refusals`; `operations::add_folder_opens_it_with_its_fonts`; `e2e_cmdline::a_folder_argument_opens_the_folder`, `the_same_folder_twice_is_not_added_twice`, `a_font_file_argument_adds_its_folder_and_selects_the_font`; `db::tests::folder_membership`, `folders_mru_panose_persist` | covered |
| F-07 Vendor URL, vendor, foundry | `FontDS.OnVendorURL`, `VendorURLString`, `SelectedVendor`, `SelectedFoundry`; `achVendID` | `vendors::vendor_url`, `vendor_name`, `foundry`; link row in `properties.rs` (Font info) → `dlg::open_url` | `vendors::tests::vendor_lookup_and_fallback`; `e2e_properties_options::font_info_tab_shows_the_core_values` (skips the Vendor URL row) | partial: clicking the link is not tested |

### 1.2 Font list, display and organisation (catalog §2)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| Group by family | `1eaf38ba`, `FontDS.GroupByFamily`, `AssetListDS.ShowGroupFlyout` | `Cmd::GroupByFamily`, `Catalog::query` | `catalog::tests::grouped_list_and_status`; `e2e_filters_list::view_toggles_group_recent_render_and_filter_list`, `family_header_click_selects_the_whole_family`; `operations::view_menu_toggles_grouping_and_properties` | covered |
| Recently used fonts | `53631843`, `AssetListDS.ShowMRUList` | `Cmd::ShowRecent`, `FontDatabase::touch_mru` / `mru` | `catalog::tests::libraries_folders_collections_recent_duplicates`; `integration::collections_mru_export_import_on_scanned_db`; `e2e_filters_list::view_toggles_group_recent_render_and_filter_list`; `e2e_filters_list::recently_used_fonts_after_a_double_click` | covered; **bug** BUG-302 (double-click does not record a font) |
| Use font to display font name | `c537dfdc`, `AssetListDS.RenderUsingAsset` | `Cmd::RenderInFont`, `preview.rs` | `e2e_filters_list::view_toggles_group_recent_render_and_filter_list`; `preview::tests::renders_a_system_font_when_available`, `keys_differ_by_content_and_size` | covered |
| Show Latin names | `a0face70`, `FontDS.ShowLatinName` | `Cmd::LatinNames`, `FontRecord::display_family` | `model::tests::latin_names_and_weight_bucket`; `e2e_filters_list::show_latin_names_in_japanese` | covered |
| Show duplicates (two caption variants) | `354411e6` / `57112ae6`, `FontDS.ShowDuplicates`, `ShowDuplicatesCaption` | `Cmd::ToggleDuplicates`, `duplicates::compute`, `Cmd::OpenLocation` | `duplicates::tests::*`; `e2e_filters_list::duplicate_fonts_hidden_shown_and_located`; `e2e_menus::view_menu_toggles_change_caption_and_check` | covered |
| Font details panel (`FontDS.Selected*`) | statusText items; family, sub-family, path, size, version, technology, code pages, italic, weight, width | `catalog::font_info`, `format::*`, `properties.rs` (Font info tab) | `format::tests::labels_match_screenshots`, `sizes_and_names`; `e2e_properties_options::font_info_tab_shows_the_core_values` | covered |
| Sample text / preview | `FontDS.SampleTextPreview`, `SampleTextInfoPreview`, `AssetEngineDS.AssetImages` | `fontlist.rs` sample combo, `settings::Settings::remember_sample`, `preview.rs` | `e2e_filters_list::sample_text_combo_presets_free_text_and_history`, `preview_size_buttons_and_slider_stay_within_8_and_86` | covered |
| Collections / groups (add to, remove from, rename, delete) | flyout `c25ceab5`, `AssetListDS.AddToGroupEnabled`, `OnAddNewGroup` 0x180047E60, `OnRemoveFromCurrentGroup` 0x180049860, `RenameCollectionDialogDS` | `commands::add_collection`, `commit_rename`, `Cmd::AddToCollection` / `RemoveFromOpenCollection` / `RemoveFromCollection`; `FontDatabase::create_collection`, `next_collection_name`, `rename_collection` | `db::tests::favorites_and_collection_names`; `e2e_libraries::add_collection_and_the_inline_name_editor`, `rename_with_f2_and_the_context_menu`, `dragging_fonts_onto_a_collection_adds_them`, `favorites_add_remove_and_remove_favorites_itself`; `e2e_menus::font_submenus_list_collections_and_files`; `operations::add_collection_rename_and_add_fonts`, `f2_renames_a_collection` | covered |
| Search / filter | `AssetListDS` filter properties, `OnClearFilters` 0x1800476A0, `AssetMgrDS.SourceList`; predicates 0x180113B50 … 0x180116770 | `filters::FilterSet`, `FilterOption::matches`, `filters::available_options`, `Catalog::query`; `fontlist.rs` search box, `libraries.rs` Filters | `filters::tests::*` (4); `catalog::tests::search_is_flat_and_filters_mark_filtered`; `e2e_filters_list::every_filter_option_lists_the_matching_fonts`, `filters_combine_or_within_and_across_groups`, `search_filters_live_flat_list_suggestions_and_escape`, `search_and_filters_combine`, `opening_another_library_clears_the_filters`; `operations::filters_and_search_narrow_the_list` | covered |
| Sort and view, selection | `AssetListDS` (`OnSelectAll` 0x180050EA0, `OnDeselectAll` 0x180051090, `ShowRemoveFromCurrentGroup`) | `commands::select_all` / `deselect_all`, `fontlist::click_row`, keyboard and type-ahead in `fontlist.rs` | `e2e_filters_list::selection_click_ctrl_shift_and_select_all`, `selection_with_the_keyboard`, `type_ahead_selects_by_name`, `double_click_selects_the_row` | covered |

### 1.3 Font matching (PANOSE) and Type 1 (catalog §3)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| Matching modes (Never show / Text / Text and styles) | `WPanosePreferencesDialogDS.ActiveIOFontMatchingType`, `OnExceptions` 0x1800CAE60 | `panose::PanoseMode`, `Matching::preferences_ui` (Options ▸ Font Matching) | `panose::tests::modes`; `e2e_getmore_matching::font_matching_modes_are_saved`, `never_show_imports_without_the_results_dialog`; `e2e_properties_options::options_size_reset_and_ok` | covered |
| Exception list | `WPanoseExceptionDialogDS.OnAdd` / `OnRemove` 0x1800C8D60 | `Matching::open_exceptions`, `matching::add_exception`, `exception_changes`; `FontDatabase::set_panose_exception` | `matching::tests::exceptions_add_replace_and_diff`; `e2e_getmore_matching::exceptions_add_and_remove`; `db::tests::folders_mru_panose_persist` | covered |
| Matching results (Panose / Exception, SameCodePage / ShowAll, Permanent / Temporary, OK, Stop) | `WFontMatchingResultsDS` (`OnStop` 0x1800B8E20, `OnOK`), ranking `WPanoseFontArray::GetFontMatchingForMissingFonts` 0x1801A6A40, `WPanoseFontMatching::FillFontListCallback` 0x1801A7840 | `panose::find_substitute`, `distance`; `Matching::open_results_for`; `FontDatabase::set_substitution` | `panose::tests::distance_ranks_similar_designs_first`, `panose_ranking_and_filters`, `stages_exceptions_substitutions_names_default`; `integration::panose_matching_on_real_fonts`; `matching::tests::results_ok_writes_exceptions_and_permanent_substitutions`; `e2e_getmore_matching::import_with_missing_fonts_saves_a_permanent_substitution_and_exception`, `results_cancel_saves_nothing`; `e2e_getmore_matching::import_with_missing_fonts_links_the_chosen_substitute` | covered; **bug** BUG-102 (OK never links the substitute into the imported collection: `Matching::take_resolved` has no caller) |
| Type 1 replacement | `Type1InfoMessageDS.ReplaceFonts` 0x1801EDE50, `Help` | `Matching::open_type1_message`, `panose::type1_replacements`, `TYPE1_MESSAGE`, `matching::type1_message_suppressed` | `panose::tests::type1_replacement_suggestion`; `matching::tests::type1_text_says_these_fonts`; `e2e_getmore_matching::type1_message_after_add_folder_and_replace_fonts`, `type1_message_do_not_show_again` | covered |

### 1.4 Text and glyph tooling (catalog §4)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| Glyph browser (`WGlyphDockerDS`): list, filters, resize | `OnDecreaseGlyphSize` / `OnIncreaseGlyphSize`, `CloseFilterList`, `Remove`, `RemoveAll` | `glyphs::enumerate_file`, `GlyphList::available_filters`, `filtered`; `properties.rs` `glyphs_tab` | `glyphs::tests::categories`, `type1_list_and_filters`; `integration::glyph_list_of_real_font`, `glyph_list_matches_the_captures`; `e2e_properties_options::glyphs_tab_count_filters_selection_and_size`; `e2e_properties_options::glyph_filter_names_keep_the_ampersand` | covered; **bug** BUG-305 (accessible name drops `&`) |
| Glyph Copy | `WGlyphDockerDS.Copy` | `properties.rs` `copy_glyph` (Ctrl+C, double-click) | – | gap |
| Glyph alternates pop-up | item `e4939644` | `properties.rs` `alternates_popup` | – (the list order is checked by `integration::glyph_list_matches_the_captures`) | gap |
| Variable fonts (`WTxtVariableFontDS`) | `FONTVARIABLEAXES`; Bahnschrift screenshots | `catalog::variable_axes`, `FontRecord::axes` / `instances`; `properties.rs` `variable_tab`; `fontlist::variations` | `integration::variable_font_instances_and_axes`, `bahnschrift_named_instances_when_available`; `e2e_properties_options::variable_font_tab_axes_and_values`, `variable_font_tab_named_instance_values` | covered |
| Font playground (`WFontPlaygroundDockerDS`), font sample export (`FontExportDS`), character/paragraph attributes (`WTxtAttributeDS`, justification 0x1800A37A0, Far-East rules 0x18007FCF0 …), text on path, columns, tabs, formatter, change case, statistics, find/replace, hyphenation, encoding, language | `CdrTxtUI.dll` | – | – | n/a: the suite's text dockers; the font manager window never shows them (Phase 01) |

### 1.5 Application shell (catalog §5)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| Welcome screen / "get more fonts" | `WWelcomeScreenDataSource.IsGetMoreVisible`, `InvokeGetMore*` (`WelcomeScreen` 0x180009CC0), `FontDS.InvokeGetMoreFonts` / `InvokeGetEmptyListMoreFonts` | `Cmd::GetMore`, `getmore::GetMore`, `fontlist.rs` empty states | `e2e_getmore_matching::get_more_*` (3); `e2e_menus::every_menu_and_the_toolbar_in_the_13_contexts` | covered |
| UI customisation, workspaces | `CustomizationDS` (31 handlers), `WQuickCustomizeDS`, `NewWorkspaceDS`, ... | – | – | n/a: no customisation in AnyFont |
| Preferences | `WPrefsApp`, `DocumentPrefsDS`, `WPrefs*` | `options.rs` (size, theme, window border, Font Matching), `settings.rs` | `e2e_properties_options::options_size_reset_and_ok`, `options_themes_and_window_border`; `settings::tests::round_trip_keeps_shell_and_core_fields`, `defaults_follow_the_spec`; `e2e_persistence::*` (2) | covered |
| About / System Info / Legal Notices | `WAboutBoxDS.OnSystemInfo` 0x1802FC880, `OnLegalNotices` 0x1802FD460, `OnClose` 0x1802FD4A0; `WSysInfoDS.SaveInfo` 0x1802FBB40 | `about.rs`, `sysinfo.rs`, `legal.rs` | `about::tests::about_lines`; `sysinfo::tests::every_category_has_rows`; `legal::tests::*`; `e2e_services::about_dialog_shows_version_buttons_and_closes`, `system_info_categories`, `system_info_save_writes_the_report`, `system_info_save_failure_is_reported`, `legal_notices_print_and_ok` | covered |
| Help | `HelpMenuDS`, `WebViewDialogDS` | `help.rs` (`help_url`, `open_product_help`), `Cmd::ProductHelp` | `help::tests::help_urls`; `e2e_services::product_help_from_menu_and_title_bar`, `product_help_through_the_real_start_up` | covered |
| Cloud dashboard | `WCustomCloudDashboardDS`, `WPasmDataSource` | – | – | n/a: no cloud |
| Crash report dialog | `CrashReportDialogDS.InvokeSubmitTicket` (`CRLFRMWK` 0x18000D290); Crashpad init (`CrlConfig` 0x18000F8A0) | `anyfont_sys::crash` (panic hook, `pending_reports`, `mark_handled`), `bugreport::BugReport::open_with_crash`, start-up offer in `AnyFontApp::new` | `crash::tests::report_files_lifecycle`, `redacts_profile`; `bugreport::tests::crash_report_prefills_a_bug`; `e2e-real` (no crash report after the run) | partial: the start-up offer of a pending report is not driven by an e2e test |
| Message boxes / errors | `MessageBoxDS` (10 handlers), `LowMemoryDialogDS`, `WCuiMonitorWarningDS` | `dialogs::Dialog::message`, `dlg::Message` | `e2e_libraries::add_folder_from_the_menu_and_the_toolbar_with_its_refusals` (refusal messages), `e2e_operations::failures_are_listed_read_only_file_and_invalid_font` | covered (low-memory and monitor warnings n/a) |
| Colour UI, asset manager docks | `WCuiColorSystemDS`, `colorUI::ColorSpecDS`, `AssetMgrDS`, `ContentDockerDS`, `TrayDockerDS`, ... | Libraries panel (`libraries.rs`) for the asset-manager part | see Collections and Libraries rows | n/a (colour UI); covered (libraries) |
| **AnyFont additions:** Updates button, Check for Updates | – (replaces the trial button) | `updater.rs`, `titlebar.rs` | `updater::tests::*` (6); `e2e_updates::*` (8); `e2e_menus::title_bar_buttons` | covered |
| **AnyFont additions:** Report a Bug, Submit Ideas and Feedback | – | `bugreport.rs`, `services/issue-relay` | `bugreport::tests::*` (5); `e2e_bugreport::*` (8); `services/issue-relay` `npm test`; `e2e_bugreport::non_bmp_title_is_cut_to_the_relay_limit` | covered; **bug** BUG-101 (fields cut by characters, the relay counts UTF-16 units) |

### 1.6 Parameters, defaults and limits (catalog §6)

| Item | Dossier value | AnyFont | Tests | Status |
|---|---|---|---|---|
| Minimum width of the fill container | 240 px (`FontManagerCore!sub_18001D100`, `@minItemWidth`) | `settings::LEFT_PANEL_MIN` = 240; window minimum 800×600 (`main.rs`) | `screens::restored_800x600_and_other_themes` | partial |
| Rasteriser options (`FontRasterizer`, `TTFOptimization`, `TextureMaxSize` in `corelapp.ini`) | `27\Config\corelapp.ini` | none: previews by `preview.rs` | – | n/a |
| Download gating | `IsUserAllowedToUseInternet` | no Internet option (lead decision 7); `eligibility` rule "a selected face is online-only" | `eligibility::tests::install_download_and_context_rules`, `toolbar_states_match_capture` | covered |
| Duplicates caption variants | items `354411e6`, `57112ae6` | `commands::menu_items` | `e2e_menus::view_menu_toggles_change_caption_and_check` | covered |
| PANOSE: 3 modes, permanent / temporary, exceptions | `WPanosePreferencesDialogDS`, `WFontMatchingResultsDS` | `panose.rs`, `matching.rs` | row 1.3 | covered |
| UI strings | 24 273 (1 365 used by the font manager) | `docs/strings-en.tsv`, 15 languages | `i18n::tests::*` (8); `e2e_i18n::every_language_is_covered` and the per-language tests | covered; **bug** BUG-201, BUG-202, BUG-203 |

### 1.7 Shortcuts, command line, toolbar states (dossier gaps 4, 9; `06-ui/screenshots/90-reports/toolbar-states.json`)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| Ctrl+A, Ctrl+Shift+A, Ctrl+Alt+I, Alt+F4, menu access keys | `FontManagerUI.xml` `<shortcutKeyTables>` (GUIDs 57453fa3, 0143b114, 64409843, 18c2f181) | `AnyFontApp::shortcuts`, `menus.rs` | `e2e_menus::shortcuts_and_keyboard_menus` | covered |
| Toolbar / menu enable states in the 13 captured contexts | `toolbar-states.json`; `FontDS.*Enabled` | `eligibility::evaluate`, `commands::menu_items`, `toolbar::draw` | `eligibility::tests::toolbar_states_match_capture`; `e2e_menus::every_menu_and_the_toolbar_in_the_13_contexts`, `toolbar_buttons_and_tooltips` | covered |
| Command-line grammar and the ten launcher switches | `CommandLine::ParseSwitch` (`CrlUtils` 0x180011C10), `SwitchSpecified` 0x180010D00, `BaseAppStartup::ParseCmdLine` (`CrlConfig` 0x180006670) | `anyfont_sys::cmdline::CommandLine` | `cmdline::tests::*` (12); `e2e_cmdline::the_command_line_is_parsed_like_the_original`, `no_splash_starts_with_the_main_window`, `lang_switch_sets_the_language_without_saving_it` | covered |
| Open in the running instance | DDE (`FontManager.exe` 0x1400029D0), `InterProcessEvent` | `anyfont_sys::single_instance` | `single_instance::tests::second_instance_forwards_files`, `ids_depend_on_app_name`; `e2e_cmdline::a_second_instance_forwards_its_files_to_the_first`, `a_forwarded_message_brings_the_window_forward`; `e2e-real` (second `anyfont.exe <font file>`) | covered |
| Splash | `WSplashScreen`, `DialogFunc` 0x140005a40 | `splash.rs` | `e2e_cmdline::the_splash_turns_into_the_main_window`, `no_splash_starts_with_the_main_window` | covered |

## 2. AnyFont feature list (Phase 09) → tests

The core rows of [`09-features.md`](09-features.md) already name their tests; all of them exist in `src/` or
`anyfont-core/tests/integration.rs`. The rows below add what Phase 09 does not list: the end-to-end tests written
since, and the Windows layer, which has no section there.

### 2.1 Core rows whose UI path is now also tested end to end

| Phase 09 core row | Unit / integration tests (from Phase 09) | End-to-end tests | Status |
|---|---|---|---|
| sfnt validation, formats (TTF/OTF/TTC/WOFF), WOFF2 reported unsupported, Type 1 | `sfnt::tests::*` (9), `parse::tests::*`, `type1::tests::*`, `woff::tests::woff_round_trip`, `integration::parses_fixture_fonts`, `collection_faces`, `woff_fixture_round_trip`, `parse_every_windows_font` (ignored: reads the PC's fonts, run by hand) | `e2e_operations::failures_are_listed_read_only_file_and_invalid_font` (invalid font) | covered |
| Technology rule, embedding rights, Font info fields | `integration::system_fonts_parse_when_available`, `model::tests::embedding_from_fs_type`, `format::tests::*` | `e2e_properties_options::font_info_tab_shows_the_core_values`, `e2e_filters_list::every_filter_option_lists_the_matching_fonts` | covered |
| Font database, incremental rescan, identical copies | `db::tests::*` (6), `integration::incremental_rescan_folders_duplicates_and_cancel`, `spawned_scan_reports_progress` | `startup::main_window_shows_the_installed_fonts`, `e2e_persistence::settings_library_and_window_survive_a_restart` | covered |
| Libraries, status text, grouping, labels | `catalog::tests::*` (3), `format::tests::labels_match_screenshots`, `integration::bahnschrift_named_instances_when_available` | `screens::main_window_states`, `e2e_libraries::section_headers_collapse_and_expand` | covered |
| Import / Export Folders and Collections | `exchange::tests::round_trip`, `imports_original_format_by_attributes`, `integration::collections_mru_export_import_on_scanned_db`, `commands::tests::import_attrs` | `e2e_libraries::export_and_import_folders_and_collections`, `operations::export_and_import_libraries`; `e2e_libraries::export_leaves_out_the_content_folder`, `import_does_not_report_fonts_of_the_imported_folders_as_missing` | covered; **bug** BUG-303, BUG-304 |
| Settings | `settings::tests::round_trip_and_bad_files` | `operations::settings_stay_in_the_test_data_folder`, `e2e_persistence::a_change_is_saved_about_800_ms_later_without_closing` | covered |
| Bad fonts never crash | `parse::tests::corrupt_bytes_never_panic`, `synthetic_font_is_rejected_by_ttf_parser_or_parsed` | – | covered (unit) |

### 2.2 UI rows of Phase 09 → current tests

Phase 09's UI table was written with the first smoke tests (`startup`, `operations`, `screens`). The current tests:

| Phase 09 UI row | Tests now | Status |
|---|---|---|
| Main window bands, title bar, splitters | `startup::main_window_shows_the_installed_fonts`, `screens::*` (2), `e2e_menus::title_bar_buttons` | covered (snap layouts and the DWM border colour only by hand) |
| Start-up (helper, single instance, logging, crash hook, splash, `/NoSplash`, `/Lang:`, Shift reset) | `e2e_cmdline::*` (9), `e2e_selftest::*` (3), `e2e_services::product_help_through_the_real_start_up`, `logging::tests::*` (3) | partial: Shift-held database reset untested (Phase 09 still says "manual") |
| Menus, access keys, check items, submenus | `e2e_menus::*` (8) | covered |
| Toolbar, tooltips, enable rules | `e2e_menus::toolbar_buttons_and_tooltips`, `every_menu_and_the_toolbar_in_the_13_contexts` | covered |
| Libraries (Add Folder refusals, Remove, rename, drag) | `e2e_libraries::*` (13) | covered; **bug** BUG-306 |
| Filters panel, Search, sample text, empty states | `e2e_filters_list::*` (18) | covered; **bug** BUG-301 (edit-box Cut/Copy/Delete) |
| Font list (markers, tooltip, selection, type-ahead, MRU) | `e2e_filters_list::selection_*`, `type_ahead_selects_by_name`, `e2e_libraries::*` (tooltips) | covered; **bug** BUG-302 |
| Preview size, View toggles | `e2e_filters_list::preview_size_buttons_and_slider_stay_within_8_and_86`, `view_toggles_group_recent_render_and_filter_list`, `e2e_menus::view_menu_toggles_change_caption_and_check` | covered |
| Font properties (Glyphs, Font info, Variable font) | `e2e_properties_options::*` (5 of 7) | covered except glyph Copy and the alternates pop-up (gap) |
| Install / Uninstall / Delete, reboot prompt, failures | `e2e_operations::*` (12) | covered |
| Import / Export, missing folders, PANOSE results | `e2e_libraries::export_*` / `import_*`, `e2e_getmore_matching::import_*` | covered; **bug** BUG-102, BUG-303, BUG-304 |
| Options, themes, window border, Font Matching | `e2e_properties_options::options_*` (2), `e2e_getmore_matching::font_matching_modes_are_saved` | covered |
| 15 languages, accessibility names | `i18n::tests::*`, `e2e_i18n::*` | covered; **bug** BUG-201, BUG-202, BUG-203, BUG-305 |
| Refresh information strip, folder watcher | `watcher.rs`, `fontlist.rs` `info_strip`, `Cmd::Refresh`; only the disabled "Refresh the Font List" item is checked (`e2e_menus::every_menu_and_the_toolbar_in_the_13_contexts`) | gap |

### 2.3 Windows layer (`anyfont-sys`)

| Feature | Dossier evidence | AnyFont code | Tests | Status |
|---|---|---|---|---|
| Installed-font enumeration (HKLM + HKCU `Fonts`) | 0x1802001D0 | `win/registry.rs` `list`, `FontSystem::installed_fonts` / `registrations` | `win::tests::real_listing_is_readable` (read-only), `sandbox::reopening_does_not_reseed`; `e2e-real` (`list_installed`) | covered |
| Registry value names (` (TrueType)`, ` & `, Type 1) | 0x180203780, 0x180202BC0 | `regname.rs` (`value_name`, `parse_value_name`, `data_for`, `type1_data`) | `regname::tests::*` (5), `fontfile::tests::*` (4) | covered |
| Type 1 registration (`Type 1 Installer\Type 1 Fonts`) | install worker 0x1802010F0 | `engine.rs`, `fontfile::type1_pair` | `sandbox::type1_pairs_register_in_the_type1_key_for_all_users_only` | covered |
| Unsupported / invalid files refused before registration | – | `fontfile::inspect` | `sandbox::unsupported_and_invalid_files`, `fontfile::tests::rejects_non_fonts` | covered |
| Progress and cancel | – | `install_with` / `uninstall_with` / `delete_with` callbacks | `sandbox::progress_callback_can_cancel`, `e2e_operations::progress_caption_and_cancel_of_an_install` | covered |
| Elevated helper (`CInterprocessProtocol`, `LaunchProtocolCallee` 0x18000A2F0) | `CrlInterprocess` 0x180013000 | `helper.rs` (`write_request`, `read_request`, `execute`, `run_elevated`) | `helper::tests::*` (2), `sandbox::helper_requests_run_against_the_backend`, `e2e_selftest::*`; `e2e-real` (`helper_install` / `helper_uninstall`, no UAC prompt on the runner) | partial: the real UAC prompt is never shown in a test |
| Logging | `OutputDebugStringW` only | `logging.rs` | `logging::tests::*` (3) | covered |
| Folders, sandbox backend, test guard | `GetUserProfileCorelDir` (`CrlConfig` 0x1800241C0) | `paths.rs`, `sandbox.rs`, `forbid-real-writes` | `paths::tests::*` (2), `env_sandbox::system_uses_the_sandbox_when_the_variable_is_set`, `guard::real_backend_refuses_writes_in_test_builds`, `win::tests::real_writes_are_refused_in_tests`, `real_engine_operations_are_refused_in_tests` | covered |
| Open font file location | `SHOpenFolderAndSelectItems` | `win/shell.rs` `open_location`, `Cmd::OpenLocation` | `sandbox::open_location_and_reboot_are_recorded_only`, `e2e_filters_list::duplicate_fonts_hidden_shown_and_located` | covered (the real Explorer call only by hand) |
| Real-system end to end | – | `tests/real-system/run-real.ps1`, `selftest.json` | `e2e-real` | covered on CI; **bug** BUG-401 (needs the Visual C++ runtime) |

## 3. Gaps in the evidence

| Item | What is missing | Where to add it |
|---|---|---|
| Glyph Copy (Ctrl+C, double-click) and the glyph alternates pop-up | no e2e test | `e2e_properties_options.rs` (the clipboard is egui's `copy_text`, readable from the harness output) |
| Vendor URL link | click not tested | `e2e_properties_options.rs` with `testing::urls()` |
| Refresh information strip / folder watcher | no test of the strip appearing after a change in a watched folder | `e2e_filters_list.rs` or a new file; `watcher::Watcher::changed` |
| Shift held at start-up (database reset) | no test (`main.rs` reads the real key state) | would need a seam in `main.rs`; manual for now |
| Crash report offered at the next start | unit tests only | `e2e_bugreport.rs` with `harness_start` and a pending report in the test folder |
| Real UAC prompt, real reboot, `MoveFileEx` delay-until-reboot on a user's PC | by rule never run outside a disposable machine (Phase 12 §7) | `e2e-real` covers the helper without a prompt |
| Eleven open QA bugs (BUG-101 … BUG-306) + BUG-401 | ignored tests / a failing real-system check | `docs/QA-BUGS.md`, Phase 14 §3 |

Dossier evidence for the claims used here is in `13-evidence/evidence-log.csv` (claims C-001 …) and
`rea-evidence-bundle.json`; the handler addresses come from `09-features/handler-coverage.csv`.
