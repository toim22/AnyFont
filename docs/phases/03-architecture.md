# Phase 03: Architecture (original structure → AnyFont crates and modules)

Dossier: `03-architecture/overview.md`, `classes.md`, `runtime.md` (and `03-architecture/modules/*.md`). This page
compares the original's process layout, class model, data sources, start-up, threads, IPC and command dispatch with
AnyFont's. AnyFont's own architecture is described in [`../ARCHITECTURE.md`](../ARCHITECTURE.md); the crate APIs in
[`../core-api.md`](../core-api.md) and [`../sys-api.md`](../sys-api.md).

## 1. Layering

| Original layer (dossier §4) | Modules | AnyFont |
|---|---|---|
| Launcher | `FontManager.exe` (`BaseAppStartup` shim) | `crates/anyfont/src/main.rs` |
| Application: model, data sources, UI wiring | `FontManagerCore.dll` (`FontManagerApp`, `FontManagerDS`, `FontManagerDoc`, `FontManagerView`, `WFontManagerStateManager`) | `crates/anyfont/src/app.rs` (`AnyFontApp`), `commands.rs` |
| Data binding | `DataBinding.dll` | none: direct calls (§4) |
| UI runtime | `CRLFRMWK`, `CRLCUI`, `CrlGenericUI`, `WindowPainter` | egui/eframe + `menus.rs`, `toolbar.rs`, `titlebar.rs`, `platform.rs`, `widgets.rs`, `dlg.rs`, `theme.rs` |
| Font and text engine | `CdrFnt`, `CrlCharacter`, `CdrTxt`, `CdrTxtUI` | `crates/anyfont-core` (model, parsing, database, queries, PANOSE) and `crates/anyfont-sys` (install/uninstall/delete on Windows) |
| Foundation | `CrlPlatform`, `CrlUtils`, `CrlConfig`, `CrlResources`, `CrlPref` | Rust std; `anyfont-sys` (`cmdline`, `paths`, `logging`, `crash`, `single_instance`); `anyfont` (`i18n.rs`, `settings.rs`) |

The original's dependency direction (launcher → application → binding → UI runtime / font engine → foundation)
is kept: `anyfont` depends on `anyfont-core` and `anyfont-sys`; `anyfont-core` depends on neither (no UI, no
Win32), and `anyfont-sys` does not depend on `anyfont-core` (it parses only what it needs for registry names, with
`ttf-parser`). The split mirrors the dossier's finding that the behaviour lives in the font engine (`CdrFnt.dll`),
not in the application module: AnyFont's engine is a crate the window only calls.

## 2. Class model → Rust types

| Original (dossier `classes.md`) | AnyFont |
|---|---|
| `FontManagerApp`, `IFontManagerApplication` | `AnyFontApp` (implements `eframe::App`) |
| `WFontManagerStateManager` (`CAppStateManager`) | the fields of `AnyFontApp` (open library, selection, panels, dialogs) and `AppSettings` |
| `FontManagerDoc` / `FontManagerView` | `anyfont_core::Catalog` (the in-memory snapshot) and `QueryResult` (what the list shows); `AnyFontApp::reload` / `requery` |
| `ComponentManager`, `DocumentManager`, `PostConstructible` | not needed (no documents, no component registry) |
| `AppEventListener` events `OnStartUp`, `OnFrameShow`, `OnPreShutDown`, `OnEndSession` | `AnyFontApp::new` (start-up), the first frame after the splash (`AnyFontApp::restore_window`), `eframe::App::on_exit` (saves the settings) |
| `IDataSource` / `DataSourceImpl` / `PartialDataSource<FontManagerDS>` | Rust modules with plain functions (§3) |
| `IUIPresenter`, `IFontSelectionTarget` (drag a font onto a document) | not needed; drag inside AnyFont only (fonts onto a collection, `libraries.rs`) |
| `CResetDialog` (reset preferences) | Options ▸ Reset to Default (`options.rs`); Shift at start-up rebuilds the database (`main.rs`) |
| COM objects with ATL class factories and type libraries | none: AnyFont exposes no COM objects |
| `AppStartupInit`, `BaseAppStartup`, `WAppStartup`, `WSplashScreen`, `StickyEvent` | `main()`, `StartOptions`, `splash::Splash` |

## 3. Data sources → AnyFont modules

The UI binds to data sources by name. AnyFont replaces each with the module that holds the same state and handlers.
As in the other phase pages, `commands::name` means the `AnyFontApp` method `name` defined in `commands.rs`.

| Data source (dossier) | Handlers / properties | AnyFont |
|---|---|---|
| `FontDS` (`FontManagerDS`; handlers in `CdrFnt.dll`) | `OnInstallFonts` 0x1800F1010, `OnUninstallFonts` 0x1800F1610, `DownloadFonts` 0x1800F1AC0, `OnDeleteFonts` 0x1800F2190; `InstallEnabled`, `UninstallEnabled`, `DeleteEnabled`, `DownloadEnabled` | `AnyFontApp::run` (`Cmd::Install`, `Uninstall`, `Delete`, `Download`) → `dialogs::start_fonts_op`, `dialogs::ask_delete` → `ops::Running` → `anyfont_sys::FontSystem`; enable states from `anyfont_core::eligibility::evaluate` via `Catalog::eligibility` |
| `FontDS` | `OnInstallLibrary` 0x1800EF6D0, `OnUninstallLibrary` 0x1800EFA20 | `Cmd::LibraryInstall` / `LibraryUninstall`, `Catalog::library_menu` |
| `FontDS` | `OnAddFolder` (`CrlGenericUI` 0x1800C5330), `OnExportGroups`, `OnImportGroups` | `commands::add_folder`, `commands::export` / `import` → `anyfont_core::exchange` |
| `FontDS` | `GroupByFamily`, `ShowLatinName`, `ShowDuplicates` (+ captions) | `Cmd::GroupByFamily`, `LatinNames`, `ToggleDuplicates`; `FontQuery` fields; `duplicates::compute` |
| `FontDS` | `Selected*` (family, sub-family, path, size, version, technology, code pages, ...), `OnVendorURL`, `SelectedVendor`, `SelectedFoundry` | `catalog::font_info`, `format::*`, `vendors::vendor_url` / `foundry`; drawn by `properties.rs` (Font info tab) |
| `FontDS` | `SampleTextPreview`, `InvokeGetMoreFonts`, `InvokeGetEmptyListMoreFonts` | `fontlist.rs` (sample text combo, empty states), `Cmd::GetMore` → `getmore.rs` |
| `AssetListDS` (`CrlGenericUI`) | `OnSelectAll` 0x180050EA0, `OnDeselectAll` 0x180051090, `OnClearFilters` 0x1800476A0, `OnManualRefresh` 0x180040820, `OnAddNewGroup` 0x180047E60, `OnDeleteGroup` 0x1800481E0, `OnRemoveFromCurrentGroup` 0x180049860, `Increment/DecrementAssetImageSize`, `ShowMRUList`, `RenderUsingAsset`, `FilterListShown`, `PreviewShown` | `commands::select_all` / `deselect_all`; Clear filters in `libraries.rs` (Filters panel); `Cmd::Refresh` + `watcher.rs` (refresh strip); `commands::add_collection`, `remove_library`, `Cmd::RemoveFromOpenCollection`; `statusbar::size_control`; `Cmd::ShowRecent`, `RenderInFont`, `ToggleFilterList`, `ToggleProperties`; queries by `Catalog::query` with `filters::FilterSet` |
| `AssetMgrDS`, `FolderTreeDS`, `RenameCollectionDialogDS` | libraries, folders, collections, rename | `libraries.rs` (`draw_left_panel`, inline rename editor), `commands::commit_rename`; storage in `FontDatabase` (`add_folder`, `remove_folder`, `create_collection`, `rename_collection`, `remove_collection`, `add_to_collection`) |
| `FontInteractionDS` (registrar 0x180182580) | `OnRebootNow`, `OnRebootLater`, `OnIgnore` (+ Cancel) = `EUsedFontAction` | reboot prompt in `dialogs.rs`; `anyfont_sys::UsedFontAction`; `FontSystem::reboot_now` |
| `DownloadProgressDS` and the operation-progress dialog | progress, `OnCloseCancelProgress` | `ops::Running::progress_text` / `cancel`, drawn by `dialogs::show` |
| `WPanosePreferencesDialogDS` | `ActiveIOFontMatchingType`, `OnExceptions` (0x1800CAE60) | `Matching::preferences_ui` (Options ▸ Font Matching), `panose::PanoseMode` |
| `WPanoseExceptionDialogDS` | `OnAdd`, `OnRemove` (0x1800C8D60) | `Matching::open_exceptions`, `matching::add_exception`, `exception_changes`; `FontDatabase::set_panose_exception` / `remove_panose_exception` |
| `WFontMatchingResultsDS` | `SubstitutionType`, `FontFilterType`, `SubstitutionStatus`, `OnOK`, `OnStop` | `Matching::open_results` / `open_results_for`, `panose::find_substitute`; `FontDatabase::set_substitution` |
| `WPanoseFontArray` / `WPanoseFontMatching` (0x1801A6A40, 0x1801A7840) | ranking | `panose::distance`, `find_substitute` |
| `Type1InfoMessageDS` | `ReplaceFonts` 0x1801EDE50, `Help` | `Matching::open_type1_message`, `panose::type1_replacements`, `TYPE1_MESSAGE` |
| `WGlyphDockerDS`, `WTxtVariableFontDS` (`CdrTxtUI`) | glyph list, size, copy; variable axes | `properties.rs` (`PropsState`, Glyphs and Variable font tabs), `glyphs::enumerate_file`, `catalog::variable_axes` |
| `WWelcomeScreenDataSource` | `IsGetMoreVisible`, `InvokeGetMore*` | `getmore::GetMore` |
| `WAboutBoxDS`, `WSysInfoDS` (`CRLFRMWK`) | `OnSystemInfo`, `OnLegalNotices`, `SaveInfo` | `about::About`, `sysinfo::collect` / `report`, `legal.rs` |
| `HelpMenuDS`, `WebViewDialogDS` | help, links | `help::open_product_help`, `dlg::open_url` |
| `MessageBoxDS` | OK / Yes / No / Cancel | `dialogs::Dialog::message`, `dlg::Message` |
| `CrashReportDialogDS` | `InvokeSubmitTicket` | `bugreport::BugReport::open_with_crash` |
| `WPasmDataSource` | `IsUserAllowedToUseInternet` (Download gate) | none: lead decision 7 (no Internet option; Download is enabled only for online placeholders) |
| `UploadFontsDS`, `CustomizationDS`, `WCustomCloudDashboardDS`, `WTxtAttributeDS` and the other text dockers, colour UI | – | not reproduced (Phase 00, Phase 09) |

## 4. Command dispatch

| Original (`runtime.md` §6) | AnyFont |
|---|---|
| A command is a GUID (`itemData guid`) | a `commands::Cmd` variant; `menus::Item` carries it |
| `onInvoke`/`enable`/`check` binding expressions evaluated by `DataBinding.dll` | `commands::menu_items` builds each menu with its enabled and checked state from `AnyFontApp::eligibility()` and the settings; `toolbar::draw` uses the same `Eligibility`, so menus and toolbar always agree |
| Handler = data-source member function | `AnyFontApp::run(ctx, cmd)` in `commands.rs` |
| Keys through `Framework::UI::IKeyContext` (Ctrl+A, Ctrl+Shift+A, Ctrl+Alt+I, Alt+F4 + per-control keys) | `AnyFontApp::shortcuts` (global keys) and the per-control handlers (`fontlist.rs` `keyboard`, `libraries.rs` `libraries_keyboard`, `menus.rs` access keys); Phase 07 |
| Dynamic captions (`*CT(*Bind(...))`) | `commands::menu_items` computes the caption (e.g. "Hide filter list" / "Show filter list", the duplicates caption) |

## 5. Start-up sequence

| Step | Original `FontManager.exe!wWinMain` (0x140001520) and `StartApp` (0x180021770) | AnyFont `main()` (`crates/anyfont/src/main.rs`) |
|---|---|---|
| 1 | `AppStartupInit` / `BaseAppStartup` objects | `CommandLine::from_env()` (`anyfont-sys/src/cmdline.rs`, same grammar as `CommandLine::ParseSwitch`) |
| 2 | `SetDllDirectoryW` (DLL search restricted to the app folder) | not needed: one executable, no DLLs of our own to load |
| 3 | `PATH` rewrite: expand variables for the logged-on user, keep only absolute entries, `SetEnvironmentVariableW` | **not done**: AnyFont loads nothing through `PATH`, and child processes (the elevated helper, the setup) are started by absolute path |
| 4 | `CoInitialize(nullptr)` | no process-wide COM initialisation. COM is initialised where it is used: `anyfont-sys/src/win/shell.rs` (`CoInitializeEx(COINIT_APARTMENTTHREADED)` for `SHOpenFolderAndSelectItems`); `rfd` does the same for the file dialogs |
| 5 | read `cloudconfig.json`, apply cloud preferences | none |
| 6 | `AppStartupTime` telemetry | none |
| 7 | `AfxWinInit`, `BaseAppStartup::Initialize(lpCmdLine)` | `/InstallHelper:` → `anyfont_sys::helper::helper_main` and exit; `/SelfTest:` → `testing::selftest::run` and exit; then `logging::init`, `crash::install_panic_hook`, unknown switches logged |
| 8 | DDE service when `/DDE` (0x1400029D0); named events for the running instance | `single_instance::acquire`: a later instance sends its files with `forward_to_primary` and exits; the first one `listen`s on a named pipe (Phase 10 §5) |
| 9 | splash on its own thread (`WSplashScreen`, `DialogFunc` 0x140005a40), private messages 0x3E0–0x3E8 report progress | the window itself starts as a 640×360 splash (`splash::show`), unless `/NoSplash`; the scan reports progress through its channel, then the window grows to the saved size (`AnyFontApp::restore_window`) |
| 10 | `LoadAppDll("FontManagerCore")` + `RunAppDll` → `StartApp` | `anyfont_sys::system()` (real or sandbox font system), `process_pending()` (deletes queued at the last restart), `Paths::resolve`, Shift held → database deleted, `AppSettings::load`, `crash::pending_reports`, then `eframe::run_native` → `AnyFontApp::new` |
| 11 | `StartApp`: state manager, UI language (`IGLB_UILanguage`), data sources, UI from XML, key context, main frame | `AnyFontApp::new`: language (`i18n::init`, `Lang::resolve`), theme, database, first scan (`AnyFontApp::start_scan`), crash-report offer |
| 12 | failure: `AfxMessageBox("Unable to load %s...")` | `log::error!("the window failed: ...")` |
| 13 | shutdown: `DeleteAllSingletons`, `CoUninitialize` | `on_exit` saves the settings; after the window closes `updater::run_setup_after_exit` starts a downloaded setup when the user chose Install Now |

The process is DPI-aware Per-Monitor V2 and `asInvoker` through `crates/anyfont/app.manifest` (embedded by
`build.rs`), where the original relies on MFC defaults.

## 6. Threads

| Original (`runtime.md` §4) | AnyFont thread | Code |
|---|---|---|
| Main UI thread + MFC message pump | the eframe/winit UI thread; `AnyFontApp::ui` polls every background job each frame (`poll_background`) | `app.rs` |
| Splash-screen thread | none: the splash is drawn by the UI thread | `splash.rs` |
| Font loading / asset scanning workers (`CrlThread`) | the scan worker with its own SQLite connection, progress and cancel | `anyfont_core::scan::spawn_scan` (`ScanHandle`) |
| Framework workers (rendering, thumbnails) | the preview rasteriser | `preview.rs` (`Previews`) |
| – | the glyph-list loader `anyfont-glyphs` | `properties.rs` |
| – | the font operation worker (Install / Uninstall / Delete) with progress and cancel | `ops::Running::start` |
| – | the folder watcher `anyfont-folder-watch` | `watcher::Watcher::start` |
| – | the single-instance pipe server `anyfont-instance-pipe` | `anyfont_sys::single_instance` |
| – | short-lived network threads: update check and download, catalog and font downloads, bug-report send | `updater.rs`, `getmore.rs`, `bugreport.rs` |

Workers report back through `std::sync::mpsc` channels or shared state behind `parking_lot::Mutex`; the UI asks for
a repaint while any of them runs. `FontDatabase` is `Send` but not `Sync`: the UI keeps one connection for edits and
answers queries from the `Catalog` snapshot (`docs/core-api.md` §1).

## 7. IPC and processes

| Original | AnyFont |
|---|---|
| DDE service (`System` topic) and `InterProcessEvent` named events for "open in the running instance" | named mutex + named pipe (`single_instance.rs`), JSON message `{files, switches}` |
| `Interprocess.sqlitedb` requests + elevated callee started with `ShellExecuteEx("runas")` (`CrlInterprocess` 0x180013000, 0x18000A2F0) | request/response JSON files + `anyfont.exe /InstallHelper:` started with `runas` (`helper::run_elevated`, `win/shell.rs` `run_elevated_and_wait`); Phase 10 §6, Phase 12 §2 |
| Crashpad handler processes | none |
| COM server / automation | none |

## 8. Where AnyFont differs on purpose

- **No binding engine, no XML UI.** The dossier's data sources become modules; their `Enabled` properties become one
  rule set (`eligibility.rs`) used by menus, toolbar and context menus alike.
- **No documents.** The original's document/view pair serves the drawing application that hosts it
  (`IFontSelectionTarget`, document-mapping columns in `FONTSDATA`); AnyFont has none.
- **Engine and system separated.** `anyfont-core` never touches Windows; every system change goes through the
  `FontSystem` trait, so the whole program runs against `SandboxFontSystem` in tests (`docs/TESTING.md`).
