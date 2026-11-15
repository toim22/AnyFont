# AnyFont architecture

AnyFont is one Windows program, `anyfont.exe`, built from three crates:

| Crate | Role |
|---|---|
| `crates/anyfont-core` | Platform-free model: font parsing, the SQLite font database, the scan, the catalog (libraries, filters, search, grouping, eligibility rules), glyph lists, PANOSE matching, import/export, the settings model |
| `crates/anyfont-sys` | Windows layer behind the `FontSystem` trait: installed fonts, install / uninstall / delete, protected system fonts, reboot handling, elevation helper, single instance, logging, crash reports, folders. A sandbox backend (`ANYFONT_SANDBOX_ROOT`) does the same operations on a private folder |
| `crates/anyfont` | The window (egui 0.37 / eframe with wgpu, patched `vendor/egui` for the global text translator), plus the services (updater, bug report, About, Get More, PANOSE dialogs, help) |

The UI reproduces the window described in `docs/UI-SPEC.md`; colours and sizes come from
`docs/design-tokens.md`.

## Start-up (`crates/anyfont/src/main.rs`)

1. `/InstallHelper:` on the command line: run the elevated install helper (`anyfont_sys::helper`) and exit.
2. Logging (`anyfont_sys::logging`) and the panic hook (`anyfont_sys::crash`), which writes a crash report.
3. Single instance: a second `anyfont.exe` forwards its files and switches to the first one, then exits. The
   first one listens on a background thread and hands the messages to the window through a channel.
4. The font system (`anyfont_sys::system()`: the real one, or the sandbox when `ANYFONT_SANDBOX_ROOT` is set),
   then `process_pending` (files queued for deletion at the last restart).
5. Paths (`settings::Paths`): `%APPDATA%\AnyFont\settings.json` and the font database, or everything under
   `ANYFONT_DATA_DIR` when that is set. Holding Shift at start-up deletes the database (it is rebuilt by the
   first scan).
6. The window: an undecorated eframe viewport, 640×360 for the splash (unless `/NoSplash`), which then grows
   to the saved size and position. `AnyFontApp::new` loads the language, the theme, the database and starts
   the first scan. A pending crash report offers the bug-report dialog.
7. After the window closes, `updater::run_setup_after_exit` runs a downloaded setup when the updater asked for
   it.

## The window (`crates/anyfont/src`)

| Module | What it draws or does | UI-SPEC |
|---|---|---|
| `lib.rs` | library root (the binary and the e2e tests use it); `tr` / `fmt` helpers for translated strings | |
| `app.rs` | `AnyFontApp`: the state, the frame layout (bands, splitters), background polling, global shortcuts, window placement, settings persistence, test hooks; `TestConfig` / `new_for_test` | §1, §8 |
| `titlebar.rs` | custom title bar: icon, title, ?, gear, Updates, minimize / maximize / close; hit regions for the native frame (snap layouts) | §4 |
| `platform.rs` | Win32 glue: the window subclass (`WM_NCHITTEST`, `WM_NCCALCSIZE`, maximize button hover), DWM border colour, system menu, bringing the window to the front | §4 |
| `menus.rs` | menu bar and all popup menus (main menus, context menus, edit-box menus), keyboard navigation, access keys | §2, §6 |
| `commands.rs` | the `Cmd` enum, the item lists of every menu, and `run`, which executes a command; Add Folder, collections, rename, import / export, edit-box commands | §2, §6, §7 |
| `toolbar.rs` | the toolbar; enable rules come from the core's eligibility (the same as the menus) | §3 |
| `libraries.rs` | Libraries (folders, collections, inline rename editor, drop target) and Filters | §5.1, §5.2 |
| `fontlist.rs` | sample text combo, Search (with suggestions), information strip, the virtualised font list (family headers, previews, markers, selection, type-ahead, drag to a collection), empty states | §1.3, §5.3 |
| `properties.rs` | font properties panel: Glyphs (filters, glyph grid, copy), Font info, Variable font (axis sliders driving the list preview) | §5.4 |
| `statusbar.rs` | status text and the preview-size control | §1.4 |
| `dialogs.rs` | the shell's own dialogs: messages, Remove library, Delete fonts, Reboot prompt, operation progress, missing folders, failures, crash offer | §7.2 |
| `options.rs` | Options (size, theme, window border, Font Matching) | §7.2.4 |
| `splash.rs` | start-up splash with loading status and version | |
| `preview.rs` | preview rasteriser: a worker thread renders text or glyphs with the font file itself (rustybuzz + tiny-skia) into egui textures, cached per request | §5.3 |
| `ops.rs` | runs Install / Uninstall / Delete on a thread through `FontSystem`, with progress and cancel | §7.2.10–12 |
| `watcher.rs` | notices changes in the watched font folders (refresh information strip) | §11 #5 |
| `theme.rs` | the four schemes (Light, Medium, Dark, Black) as design tokens; egui visuals; fonts | §10 |
| `i18n.rs` | 15 languages from `assets/i18n/*.tsv`, keyed by the English strings of `docs/strings-en.tsv`; installed as egui's global text translator | phase 08 |
| `icons.rs` | the SVG icon set, tinted per state | |
| `widgets.rs` | shared drawing helpers: captions with access keys, tooltips, check boxes, buttons, slider, scroll bar, accessibility info | |
| `settings.rs` | `AppSettings` (core settings plus shell-only state), paths, limits | |

Services (owned by the services developer, used through their interfaces): `about.rs`, `legal.rs`,
`sysinfo.rs`, `bugreport.rs`, `getmore.rs`, `matching.rs`, `help.rs`, `dlg.rs`, `updater.rs`.

### Frame loop

Each frame `AnyFontApp::ui`:
1. applies the appearance (scheme, UI scale, border colour, language);
2. polls background work: incoming single-instance messages, the scan, the running operation, the glyph
   loader, the folder watcher;
3. draws the splash, or the main window: title bar, menu bar, toolbar, left panel, content, properties panel,
   status bar, then open menus, dialogs, Options and the services;
4. saves the settings 800 ms after the last change (and on exit).

Strings are written in English in the code. Text drawn through egui (`WidgetText`, `painter.text`) is translated
by the global translator; font data (family names, file paths, previews) is drawn with `widgets::data_text` /
`data_galley`, which bypass it.

### Data flow

The database (`anyfont_core::FontDatabase`) is the source of truth. After a change (scan, Add Folder, a
collection edit, an operation) the shell rebuilds the `Catalog` snapshot (`AnyFontApp::reload`) and runs the
query for the open library, filters, search and View toggles (`requery`). The font list, the status text and
the eligibility of every command come from that `QueryResult` and `Catalog::eligibility`.

Long work never blocks the UI: the scan (`anyfont_core::scan::spawn_scan`), previews (`preview.rs`), glyph lists
(`properties.rs`) and font operations (`ops.rs`) run on threads and report back through channels; the UI polls
them each frame and asks for a repaint while they run.

## Safety of tests and development runs

- `anyfont-sys` is built with `forbid-real-writes` for the tests: the real backend refuses to change the
  system, so a test can only work on a sandbox.
- `ANYFONT_SANDBOX_ROOT=<dir>` makes `anyfont.exe` use the sandbox: installed fonts, the registry, the Windows
  Fonts folder and AnyFont's own folders all live under `<dir>`. `ANYFONT_DATA_DIR=<dir>` redirects the settings
  and the database only.
- The end-to-end tests (`crates/anyfont/tests/*.rs`) build the real window with
  `AnyFontApp::new_for_test(cc, TestConfig { .. })`: a sandbox under a temporary folder, settings and database
  under the same folder, language en-US, the scan on the UI thread. They drive it with `egui_kittest` through the
  accessibility tree (menus, toolbar, Libraries, Filters, Search, dialogs), and `tests/screens.rs` renders the
  main states to `target/ui-compare/` for comparison with the captures of the original.

## Build

`crates/anyfont/build.rs` embeds the icon, the version resource and `app.manifest` (Per-Monitor V2 DPI awareness,
common controls 6, `asInvoker`). `cargo build -p anyfont` builds the program; `cargo test -p anyfont` runs the
unit and end-to-end tests.
