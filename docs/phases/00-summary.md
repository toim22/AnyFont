# Phase 00: Summary (product scope)

Dossier: `00-summary/executive-summary.md`, `00-summary/plan.md`. This page compares the scope of the analysed
program with the scope of AnyFont 0.1.0 and lists what AnyFont does differently on purpose. The phase table is in
[`../TEAM-PLAN.md`](../TEAM-PLAN.md); the index of all phase pages is [`README.md`](README.md).

## What the dossier says

- The analysed program is the font manager of a commercial graphics suite (version 27.2). It is not a standalone
  application: a 250 KB launcher (`FontManager.exe`) loads the application module (`FontManagerCore.dll`, export
  `StartApp`), which builds on the suite's shared framework, declarative XML UI, data-binding engine and font/text
  engine (`CdrFnt.dll`, `CrlCharacter`, `CdrTxt`, `CdrTxtUI`).
- Stack: C++ / MSVC 14.50 x64, MFC 14, the vendor framework (`Crl*`), Skia 2.0, ICU 66, HarfBuzz, a declarative UI
  of 72 command bars, 1 597 controls, 95 dialogs and 4 254 bindings to 148 data sources.
- The closure of `FontManager.exe` is 81 binaries: 69 vendor modules (all decompiled, 94.4 % of 158 757 functions)
  and 12 third-party ones.
- Main features (from the UI definition): Install / Uninstall / Download / Delete fonts and font libraries,
  Add Folder, Upload fonts, vendor links, reboot prompts; font list with group by family, recently used fonts,
  render in the font, Latin names, duplicates, font details; PANOSE missing-font matching (three modes,
  permanent/temporary substitutions, exceptions, results dialog); collections, search and filters; welcome
  screen "get more fonts"; the suite's text dockers, UI customisation, workspaces, preferences, About, help,
  crash-report dialog and cloud dashboard.
- Notable findings: the launcher rewrites `PATH`; the UI is data, not dialog templates; commands are GUIDs with no
  accelerator table; installing fonts is privileged; two resource-only DLLs carry licence feature tables; all
  cloud behaviour is configured in `cloudconfig.json`; the binaries are signed and not protected.
- Gap closure: 8 of 10 gaps closed, 2 partially closed (the licence feature-table format and the assembled
  per-font download URL). Phase 14 goes through them.

## AnyFont 0.1.0 scope

AnyFont is one Rust program, `anyfont.exe` (workspace version `0.1.0` in `Cargo.toml`), publisher "AnyFont Team",
repository `https://github.com/toim22/AnyFont`. Its window, menus, toolbar, panels and dialogs follow the dossier
(`docs/UI-SPEC.md`); its behaviour follows the decompiled handlers (Phases 04, 09, 10).

| Dossier feature | In AnyFont 0.1.0 | Where |
|---|---|---|
| Install / Uninstall / Delete fonts (F-01–F-03), reboot prompt, protected system fonts | yes | `anyfont-sys` (`FontSystem`, `engine.rs`, `protect.rs`), `anyfont/src/ops.rs`, `dialogs.rs` |
| Install / Uninstall a whole library | yes | `commands::Cmd::LibraryInstall` / `LibraryUninstall`, `Catalog::library_menu` |
| Download fonts (F-04) | yes, from an open-licence online catalog (below) | `getmore.rs`, `Cmd::Download`, `FontDatabase::add_remote_font` |
| Upload / share fonts (F-05) | no (cloud service) | Phase 06, UI-SPEC §11 #17 |
| Add Folder (F-06), folders, collections, Favorites | yes | `commands::add_folder`, `add_collection`, `FontDatabase::add_folder` |
| Vendor URL (F-07) | yes, a link in Font info | `vendors::vendor_url`, `properties.rs` |
| Font list: group by family, recent, render in font, Latin names, duplicates, filters, search | yes | `anyfont-core` `catalog`, `filters`, `duplicates`; `fontlist.rs`, `libraries.rs` |
| Font details (Font info), glyphs, variable fonts | yes | `catalog::font_info`, `glyphs`, `properties.rs` |
| PANOSE matching, exceptions, results, Type 1 replacement | yes | `anyfont-core/src/panose.rs`, `anyfont/src/matching.rs` |
| Import / Export folders and collections | yes | `exchange.rs`, `commands::import` / `export` |
| About, System Info, Legal Notices, Product Help | yes | `about.rs`, `sysinfo.rs`, `legal.rs`, `help.rs` |
| Command-line switches, single instance | yes | `anyfont-sys/src/cmdline.rs`, `single_instance.rs` |
| Splash screen | yes (`/NoSplash` skips it) | `splash.rs` |
| Localisation from the Windows UI language | yes, 15 languages | `i18n.rs`, `assets/i18n/*.tsv` |
| Crash-report dialog | replaced (below) | `anyfont-sys/src/crash.rs`, `bugreport.rs` |
| Text dockers (character/paragraph attributes, font playground, text on path, ...), colour UI, asset manager, UI customisation and workspaces, cloud dashboard | no: suite features the font manager window never shows (Phase 01) | – |

## Intentional differences

| Original | AnyFont | Reason / where |
|---|---|---|
| Vendor product name, logo, splash art, About text, copyright, "(Evaluation Version)" in the title | "AnyFont", its own icon (`assets/brand/anyfont-icon.svg`) and redrawn SVG icons (`assets/icons`), "AnyFont Team" | team plan: no third-party branding or assets. Functional captions ("Add Folder...") are kept |
| Trial button in the title bar, trial window on exit, purchase and licence windows, licence feature tables (`DRPCUNLR`/`RMPCUNLR`, `license.xml`) | none. The third title-bar button is **Updates** | `titlebar.rs`, `updater.rs` (GitHub releases feed, Check for Updates, Install Now runs the setup after exit: `updater::run_setup_after_exit`) |
| "Get More" store / welcome screen, cloud downloads (`DBAssetURL`, `cloudconfig.json` endpoints) | **Get More** = the Google Fonts catalog (open licences): search, filters, sort, Download into the AnyFont Content folder `%USERPROFILE%\Documents\AnyFont\Content\Fonts`. Downloaded fonts show in My fonts as *Not installed* and can then be installed | `getmore.rs` (`CATALOG_URL`, `DOWNLOAD_LIST_URL`), `AnyFontApp::ensure_content_folder`, UI-SPEC §7.2.7, lead decision 7 |
| No cloud account, analytics, IPM messages, launch counters | none: AnyFont sends nothing except the requests the user starts (update check, catalog, bug report) | Phase 10 §4, Phase 12 |
| Crashpad + crash-report upload dialog | a panic hook writes a local text report; at the next start AnyFont offers **Help ▸ Report a Bug...** with it | `anyfont_sys::crash::install_panic_hook`, `pending_reports`; `bugreport::BugReport::open_with_crash` |
| Help: Product Help, About | adds **Check for Updates...**, **Submit Ideas and Feedback...**, **Report a Bug...** | `Cmd::CheckUpdates`, `Cmd::Feedback`, `Cmd::ReportBug`; lead decision 4 |
| Bug reports / feedback | sent as GitHub issues through the Cloudflare Worker `services/issue-relay` (no GitHub account needed); falls back to a pre-filled GitHub issue in the browser | `bugreport.rs` (`relay_url`, `relay_body`, `new_issue_url`) |
| Per-machine install only | all users through an elevated helper; "Install for me" (per user, no admin) when the UAC prompt is declined | `ops::install_scope`, `dialogs.rs`, `anyfont-sys` `Scope::CurrentUser` |
| `PATH` rewrite, DDE/COM automation server, OLE embedding, printing | not done | Phase 03, Phase 10 §1 |
| Two colour schemes in the captures | four schemes (Light, Medium, Dark, Black), Dark by default | `theme.rs`, lead decision 5 |

## Rules that come from this phase

- No phase is skipped: every dossier folder `00-summary` … `14-gaps` has a page in `docs/phases/`.
- Development and tests never change this PC's fonts: Install / Uninstall / Delete run against the sandbox backend
  (`ANYFONT_SANDBOX_ROOT`) or on a disposable GitHub runner (`docs/TESTING.md`).
- No AI-tool attribution in commits, docs or code.
