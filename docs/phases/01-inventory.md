# Phase 01: Inventory (original modules → AnyFont)

Dossier: `01-inventory/binaries.md` (Tier 2 table, 69 rows), `01-inventory/dependencies.md` (closure graph of
81 modules), `01-inventory/manifest.csv`. This page says, for every module in the closure of `FontManager.exe`,
which AnyFont crate or module covers its part of the font manager, or why AnyFont does not need it.

## What the dossier says

- The installation holds 4 382 files (513 PE files). The closure of `FontManager.exe` is **81 binaries**:
  **69 vendor modules** (Tier 2, all decompiled) and **12 third-party** modules (Skia, ICU, HarfBuzz, OpenCV, IPP,
  OpenCL).
- `FontManager.exe` is a launcher; the application is `FontManagerCore.dll`. The data sources the UI binds to
  (`FontDS`, `AssetListDS`, `FolderTreeDS`, `FontInteractionDS`, `WPanose*`, ...) live mostly in `CdrFnt.dll`,
  `CrlGenericUI.dll` and `CRLFRMWK.dll`, not in `FontManagerCore.dll` (`09-features/handler-map.md`).
- Many closure modules are there because the suite's shared libraries import each other, not because the font
  manager uses them: curves, raster processing, filters, print, PDF, OLE, comments, effects.
- LittleCMS (`lcms2.dll`), V8, ONNX Runtime, codecs, Crashpad's handler and the CAD/Office converters are in the
  installation but **not** in this closure.

AnyFont is three crates in one executable: `anyfont-core` (platform-free engine), `anyfont-sys` (Windows layer)
and `anyfont` (the egui window and services). See [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

## Application and font modules

| Original module | Role in the font manager (dossier) | AnyFont |
|---|---|---|
| `FontManager.exe` | launcher: `wWinMain` 0x140001520, PATH fix-up, splash, `LoadAppDll`/`RunAppDll` | `crates/anyfont/src/main.rs` (`anyfont.exe`), `splash.rs` |
| `FontManagerCore.dll` | `StartApp` 0x180021770: app object, `FontManagerDS`/`Doc`/`View`, `WFontManagerStateManager`, key context, UI wiring | `anyfont/src/app.rs` (`AnyFontApp`, `AnyFontApp::new`, `main_ui`, `shortcuts`), `commands.rs` (`Cmd`, `run`) |
| `CdrFnt.dll` | font engine: sfnt reader (`CFontInformerOTF_STREAM::Initialize` 0x18017F470), font database, PANOSE, install / uninstall / delete workers, `FontDS` handlers (`OnInstallFonts` 0x1800F1010, `OnUninstallFonts` 0x1800F1610, `DownloadFonts` 0x1800F1AC0, `OnDeleteFonts` 0x1800F2190), `FontInteractionDS`, `WPanose*`, `WFontMatchingResultsDS`, `Type1InfoMessageDS`, `UploadFontsDS` | split three ways: **parsing and model** in `anyfont-core` (`sfnt`, `parse`, `woff`, `type1`, `db`, `scan`, `catalog`, `eligibility`, `panose`, `duplicates`, `vendors`); **system changes** in `anyfont-sys` (`engine.rs`, `protect.rs`, `regname.rs`, `win/registry.rs`, `win/acl.rs`, `win/shell.rs`); **dialogs** in `anyfont` (`dialogs.rs` reboot prompt / delete / progress, `matching.rs` PANOSE dialogs and Type 1 message). `UploadFontsDS` is not reproduced |
| `CrlCharacter.dll` | character sets, Unicode ranges | `anyfont-core/src/glyphs.rs` (`GlyphCategory`, `script_group`), `filters::CharRange`; Unicode data from `unicode-general-category` and `unicode-script` |
| `CdrTxt.dll` | text engine, shaping | `anyfont/src/preview.rs` (rustybuzz shaping, tiny-skia rasterising) |
| `CdrTxtUI.dll` | the suite's text dockers; the font manager uses the glyph docker (`WGlyphDockerDS`) and the variable-font UI (`WTxtVariableFontDS`) | `anyfont/src/properties.rs` (Glyphs tab, Variable font tab); `anyfont-core` `glyphs::enumerate_file`, `catalog::variable_axes`. The other text dockers (attributes, justification, Far-East rules, text on path, find/replace, ...) are not needed: the font manager window never shows them |
| `CrlGenericUI.dll` | `AssetListDS` (select all, clear filters, refresh, groups), `FolderTreeDS`, `FontDS.OnAddFolder` 0x1800C5330 | `anyfont/src/libraries.rs`, `fontlist.rs`, `commands.rs` (`select_all`, `deselect_all`, `add_folder`, `add_collection`, `remove_library`); queries in `anyfont-core` `Catalog::query`, `filters::FilterSet` |
| `WelcomeScreen.dll` | `WWelcomeScreenDataSource` ("get more" tabs, `IsGetMoreVisible`) | `anyfont/src/getmore.rs` (Get More = online font catalog), `Cmd::GetMore`, the empty states in `fontlist.rs` |
| `DRPCUNLR.dll`, `RMPCUNLR.dll` | resource-only licence feature tables (`M/104`) | **not reproduced**: no licensing in AnyFont |

## Framework modules

| Original module | Role (dossier) | AnyFont |
|---|---|---|
| `CRLFRMWK.dll` | UI runtime: XML command bars, containers, dialogs; `WAboutBoxDS`, `WSysInfoDS`, `CrashReportDialogDS`, `MessageBoxDS`, `HelpMenuDS` | egui/eframe plus `menus.rs`, `toolbar.rs`, `dlg.rs`, `dialogs.rs` (`Dialog::message`), `about.rs`, `sysinfo.rs`, `legal.rs`, `help.rs`, `bugreport.rs` |
| `DataBinding.dll` | `*Bind(DataSource=...;Path=...)` evaluation | not needed: commands are the `commands::Cmd` enum, enable/check states come from `anyfont_core::eligibility::evaluate` through `Catalog::eligibility` |
| `WindowPainter.dll` | skinning, non-client area | `titlebar.rs`, `platform.rs` (`WM_NCHITTEST`, `WM_NCCALCSIZE`, `set_border_color`), `theme.rs` |
| `CrlUtlUI.dll` | `CAppStateManager` (base of `WFontManagerStateManager`) | `AnyFontApp` state |
| `CRLCTL.dll`, `CrlWin.dll` | common controls, window helpers | `widgets.rs` (check boxes, buttons, slider, scroll bar, accessibility), `platform.rs` |
| `CustomizeUI.dll` | show/hide command bars, workspaces | not needed: AnyFont has no UI customisation or workspaces |
| `CrlTouch.dll` | touch input | not needed: egui handles touch and pen input |
| `CrlConfig.dll` | `BaseAppStartup` (`ParseCmdLine` 0x180006670), config directories, Crashpad init | `main.rs`; `anyfont-sys` `cmdline::CommandLine`, `paths.rs` (`AppPaths`), `crash.rs` |
| `CrlPref.dll` | preferences engine, cloud prefs | `anyfont-core/src/settings.rs` (`Settings::load` / `save`), `anyfont/src/settings.rs` (`AppSettings`, `Paths`); no cloud prefs |
| `CrlUtils.dll` | threads, events, files, `CommandLine::ParseSwitch` 0x180011C10, `File::IsOpenElsewhere` 0x1800A2840, `DeleteToRecycleBin` 0x1800A2660 | Rust std threads and channels; `anyfont-sys` `cmdline.rs`, `protect::is_open_elsewhere`, `win/shell.rs` (`recycle`) |
| `CrlPlatform.dll`, `CRLUTL.dll`, `CrlUtf.dll`, `CrlMath.dll` | strings, UTF conversions, utilities, maths | Rust std; `anyfont-sys/src/util.rs` |
| `CrlInterprocess.dll` | inter-process requests through `Interprocess.sqlitedb`, elevated callee (`LaunchProtocolCallee` 0x18000A2F0) | `anyfont-sys/src/helper.rs` (`/InstallHelper:` request files), `single_instance.rs` (named pipe) |
| `CrlResources.dll`, `LocalizedStrings.dll`, `CrlInternationalization.dll` | string table by GUID, UI language | `anyfont/src/i18n.rs`, `assets/i18n/*.tsv`, master list `docs/strings-en.tsv` |
| `CrlXml.dll` | XML | `roxmltree` in `anyfont-core/src/exchange.rs` |
| `CdrCpr.dll` | compression | `miniz_oxide` (WOFF tables, `woff.rs`) |
| `CdrImg.dll`, `Crl2D.dll` | image loading (`SkiaUtility::LoadSkImageFromFile`), 2-D helpers | `image` (PNG/ICO), `resvg`/`usvg` (`icons.rs`) |
| `CrlClip.dll` | clipboard | egui `copy_text` (glyph copy in `properties.rs`), `arboard` (edit-box Paste in `commands.rs`) |
| `CrlBoxInfo.dll` | product box / version identity (`AppBoxID.Xml`) | workspace version in `Cargo.toml`; `about::version_line` |
| `CrlCQL.dll` | query language of the asset manager | not needed: `Catalog::query` filters in memory |
| `CrlComponent.dll`, `FrameworkObjectModel.dll`, `CrlFrmCommon.dll`, `CrlAutomation.dll`, `ExtensionManager.dll` | component registry, object model, COM automation, extensions | not needed: AnyFont has no automation or plug-in surface |
| `CrlFeatureManagement.dll` | feature gating, `license.xml` | not reproduced (licensing) |
| `CRLWEBSERVICE.dll`, `CRLWEB.dll`, `CrlCloud.dll` | cloud endpoints, uploads, crash gateway | not reproduced. AnyFont's own network use (`ureq`): `getmore.rs`, `updater.rs`, `bugreport.rs` |

## Drawing-application modules (in the closure only through shared imports)

| Original modules | Why AnyFont does not need them |
|---|---|
| `CdrGfx.dll`, `cdrrip.dll`, `CDRCRV.dll`, `CRLFOM.dll`, `CRLCLR.dll`, `CRLCUI.dll`, `Styles.dll`, `CrlTransient.dll`, `CrlFXUtl.dll`, `CrlGfxView.dll`, `CDROPENCL.dll`, `CDRAUTOSENSE.dll` | graphics engine, raster processor, curves, fill/outline, colour engine and colour UI (palette handlers), styles, interactive tools, effects, document views, OpenCL. The font manager uses none of them for its own features; previews are rendered by `preview.rs` |
| `CDRFLT.dll`, `CdrFLTUI.dll`, `CdrContainer.dll`, `CdrOLE.dll`, `CdrPDF.dll`, `CdrPrn.dll`, `CrlPPD.dll`, `CrlComments.dll`, `CrlSIV.dll`, `CrlVcs.dll`, `CDRUTL.dll`, `CrlSCI.dll` | document filters, document containers, OLE, PDF, print, printer descriptions, comments, image viewer, version control, drawing utilities. The font manager has no documents and no print feature (Legal Notices ▸ Print hands a text file to the shell: `legal::print`). The two handler symbols the dossier found in these modules are shared names: `FolderTreeDS::AddFolder` (CdrContainer) is AnyFont's `commands::add_folder`; `WFontMatchingResultsDS::OnOK` resolved to a `CdrFLTUI` symbol of the same name and is AnyFont's `matching.rs` results dialog |

## Third-party modules in the closure (12)

| Original | AnyFont |
|---|---|
| `skia.dll`, `skshaper.dll`, `skunicode_core.dll`, `skunicode_icu.dll` | egui/eframe with wgpu for the UI; `tiny-skia` for glyph previews; `resvg`/`usvg` for icons |
| `harfbuzz.dll` | `rustybuzz` |
| `icuuc66.dll`, `icuin66.dll`, `icuio66.dll`, `icudt66.dll` | Rust Unicode support, `unicode-general-category`, `unicode-script`, the tables inside rustybuzz and egui |
| `opencv_world4100.dll`, `IPP.dll`, `OpenCL.dll` | not needed (imaging/compute for the drawing applications) |

Details and licences: [`11-third-party.md`](11-third-party.md).

## Data files

| Original | AnyFont |
|---|---|
| `27\FontManager\UIConfig\FontManagerUI*.xml` (command bars, items, containers, dialogs) | the UI in code, specified in `docs/UI-SPEC.md` (Phase 06) |
| `27\Languages\EN\Data\strings.xml`, `caption.csv`, tooltip CSVs | `docs/strings-en.tsv` + `crates/anyfont/assets/i18n/<lang>.tsv` (Phase 08) |
| `27\Resources\icons.map.xml` and icon bitmaps | own SVGs in `crates/anyfont/assets/icons/` (no original asset copied) |
| font database (4 SQLite files, 20 tables) | one SQLite file, `anyfont_core::db::FontDatabase` (Phase 05) |
| `PanosePreferences.xml`, `Workspace\_default.cdws`, registry under the vendor key | `settings.json` (`%APPDATA%\AnyFont`) and the database's PANOSE tables (Phase 05) |
| `cloudconfig.json`, `license.xml`, `AppBoxID.Xml` | none |
| Skin (`AboutBackground.png`, splash image) | `crates/anyfont/assets/brand/*.svg`, `splash.png` (own artwork) |
