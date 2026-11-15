# AnyFont 0.1.0 – team plan

AnyFont is a Windows font manager written in Rust. Its behaviour, screens and data model follow the
reverse-engineering dossier in `C:\Projetos\engenharia-reversa\FontManager-ida` (the "dossier"). Every
top-level folder of the dossier is one phase of the work and none is skipped; `docs/phases/` records,
per phase, what the dossier says and how AnyFont implements it.

## Ground rules (every team member)

- **Look and behaviour come from the dossier**, above all `06-ui/screenshots` (the screenshots of the real
  program). Do not redesign the UI and do not drop features. A feature the dossier defines but the captured
  window never reaches is still implemented, in the least intrusive place the dossier's UI definition suggests,
  and listed in `docs/phases/`.
- **No third-party branding or assets.** Nothing from the original program is copied: no icon bitmaps, logos,
  names ("Corel"), store or licence screens. Icons are redrawn as our own SVGs that look as close as possible
  to the screenshots. Functional captions ("Add Folder...", "Show duplicate fonts") are kept as they are.
- Product name **AnyFont**; executable `anyfont.exe`; publisher "AnyFont Team"; repository
  `https://github.com/toim22/AnyFont`. Version `0.1.0` (workspace `Cargo.toml`).
- Licensing/trial/purchase windows of the original are **not** reproduced. The title bar's third button
  (the trial icon in the original) becomes **Updates** (check for updates / install).
- The "Get More" store and "Download" become an online font catalog (Google Fonts, open licences): fonts are
  downloaded to the **AnyFont Content folder** (`%USERPROFILE%\Documents\AnyFont\Content\Fonts`) as
  not-installed fonts, which the user can then Install.
- **Never change this PC's fonts while developing or testing.** Install/Uninstall/Delete are only ever run
  against the sandbox backend (`ANYFONT_SANDBOX_ROOT`, see `anyfont-sys`) or inside Windows Sandbox.
- Never add Claude/Anthropic attribution anywhere (commits, docs, code comments). Do not commit or push –
  the lead integrates and commits.
- Rust stable (1.99), MSVC x64. `cargo build`/`cargo test` must stay green for your crate. Prefer crates
  already in `~/.cargo/registry` (egui/eframe 0.37 – patched copy in `vendor/egui` – ttf-parser 0.25,
  rustybuzz 0.20, tiny-skia 0.12, resvg/usvg 0.49, image 0.25, rfd 0.18, ureq 3, walkdir, sha2).
- Sibling apps built the same way, to copy patterns from (read-only!):
  `C:\Users\anton\Documents\GitHub\AnyVector` (closest: egui 0.37, i18n translator, updater, bug reports,
  splash, About/System Info, xtask brand, Inno Setup installer, issue relay) and
  `C:\Users\anton\Documents\GitHub\AnyCapture` (Windows Sandbox e2e, CI, issue templates).

## Workspace

| Path | Owner | What |
|---|---|---|
| `crates/anyfont-core` | core engineer | font file parsing and validation (sfnt rules), font metadata (the `FONTSDATA` model), the font database (SQLite cache), libraries (folders, collections, Favorites), filters, search, grouping, duplicates, PANOSE matching (modes, exceptions, substitutions), import/export of folders and collections, recently used fonts, settings model. No UI, no Win32. |
| `crates/anyfont-sys` | systems engineer | Windows integration behind a `FontSystem` trait: installed-font enumeration (registry), protected system fonts, install / uninstall (per user or all users), delete (Recycle Bin), WM_FONTCHANGE, reboot prompt support, open file location, elevation. A sandbox backend for tests. |
| `crates/anyfont` | UI team | the egui application: custom title bar, menus, toolbar, Libraries, Filters, font list, font properties (Glyphs / Font info / Variable font), status bar, context menus, all dialogs, splash, updater, bug report/feedback, About/System Info/Legal Notices, Options, i18n. |
| `crates/anyfont/assets` | design | brand (`brand/anyfont-icon.svg` = the app icon, given by the product owner), `icons/*.svg` (all UI icons), generated `anyfont.ico`. |
| `xtask` | build engineer | `cargo run -p xtask -- brand` (ico, PNGs, installer wizard images from the brand SVG), other helpers. |
| `installer/AnyFont.iss`, `build.bat`, `.github/` | build engineer | Inno Setup installer, build script, CI, issue templates. |
| `services/issue-relay` | build engineer | Cloudflare Worker `anyfont-issues` (same as AnyVector's) that files bug reports/feedback as GitHub issues without a GitHub account. |
| `tests/`, `crates/anyfont/tests` | QA | headless end-to-end tests (egui_kittest, sandbox font system) + a Windows Sandbox run of the real installer. |
| `docs/` | everyone | `ARCHITECTURE.md`, `phases/NN-*.md`, `core-api.md`, `sys-api.md`, `UI-SPEC.md`, `TESTING.md`. |

## Phases (dossier folder → AnyFont)

| Dossier | AnyFont |
|---|---|
| 00-summary | product scope, this plan |
| 01-inventory | which original modules map to which AnyFont module (`docs/phases/01-inventory.md`) |
| 02-technology | Rust/egui stack choices replacing MFC/Corel framework/Skia/ICU/HarfBuzz |
| 03-architecture | crate layout mirroring FontManagerCore / CdrFnt / framework split |
| 04-pseudocode | behaviour of the handlers (FontDS, AssetListDS, CdrFnt sfnt checks) reimplemented |
| 05-data-models | font database schema, settings, file formats |
| 06-ui | every screen, menu, toolbar, panel, context menu, dialog |
| 07-shortcuts | Ctrl+A, Ctrl+Shift+A, Ctrl+Alt+I, Alt+F4 + mnemonics + per-control keys |
| 08-localization | English master strings + translations, language chosen from Windows |
| 09-features | every feature of the catalog / handler map |
| 10-behavior | command line switches, files/registry, errors/logging, network, shell/clipboard |
| 11-third-party | our third-party crates + licence notices |
| 12-security | hardening, privilege handling for install, no secrets in the app |
| 13-evidence | traceability: feature → dossier evidence → AnyFont code → test |
| 14-gaps | how each open gap of the dossier was resolved in AnyFont |
