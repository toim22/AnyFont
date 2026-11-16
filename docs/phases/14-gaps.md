# Phase 14: Gaps (dossier gaps → AnyFont, and AnyFont's own limitations)

Dossier: `14-gaps/unknowns.md` (gaps 1–10, residual items R-1…R-5), `coverage.md`, `failures.md`,
`consistency.md`. This page says how AnyFont resolved or worked around each open or partial item, then lists
AnyFont's own known limitations and the QA bugs that are still open. The traceability of features to tests is in
[`13-evidence.md`](13-evidence.md).

## 1. The dossier's gaps

The dossier closes 8 of its 10 gaps and leaves 2 partially closed. Closed gaps still matter: each is a fact AnyFont
had to decide how to use.

| # | Gap (dossier) | Dossier status | AnyFont |
|---|---|---|---|
| 1 | Binding `Path=` names → handler addresses | closed: 270 / 277 exact, 1 likely, 5 only in `VGCore.dll`, 1 framework default | AnyFont reimplements the handlers it needs from those addresses (Phases 04, 09, 10). The 5 `VGCore.dll` handlers (`HelpMenuDS::LegalMoreInfos`, `HelpMenuDS::ShowCloudConfig`, `WCustomCloudDashboardDS::QueryCloudFiles`, `WDocCommandsDS::OnExportProof`, `OnPrintProof`) are inert in the original and are **not reproduced**: Legal Notices has no "Show CloudConfig.json" (Phase 06), there is no cloud dashboard and no document proof. `OpenDesktopAppDS` (desktop-app link dialog) is not reproduced. The `WTxtAttributeDS` handlers recovered by the Ghidra pass belong to the text dockers, which AnyFont does not have |
| 2 | Registry key paths | closed: vendor key under `HKCU\Software\<vendor>\...\27.0` | AnyFont keeps **no settings in the registry**: `settings.json` in `%APPDATA%\AnyFont` (`settings::Paths`). The only registry values are the setup's language choice (`HKA\Software\AnyFont`: `Language`, `LanguageStamp`, read by `i18n.rs`) and Windows' own font registrations (`HKLM`/`HKCU ...\Fonts`, `Type 1 Fonts`; Phase 10 §2). No launch counters, no IPM/trial tracking |
| 3 | 64 caption GUIDs with no `caption.csv` row | closed: they are dynamic captions bound to data-source strings | AnyFont computes dynamic captions in code (`commands::menu_items`: "Hide filter list" / "Show filter list", the duplicates caption, "Add to collection" entries) and every static caption is a key in `docs/strings-en.tsv`. `e2e_menus::view_menu_toggles_change_caption_and_check` checks them |
| 4 | Command-line switch grammar | closed | reproduced in `anyfont_sys::cmdline::CommandLine` (same grammar, all ten switches); tested by `cmdline::tests::*` and `e2e_cmdline::the_command_line_is_parsed_like_the_original`. Modes AnyFont cannot honour are accepted and ignored (row "No DDE/COM" in §3) |
| 5 | Font database schema | closed: 20 tables, `FONTSDATA` 66 columns | AnyFont has its own SQLite schema (`anyfont_core::db`, Phase 05) mapping the columns it needs. Not reproduced by design: `KEYWORDS_*`, `FONTSAMPLES`, `FONTPREVIEWCACHE`, `FONTCURVES`, `Interprocess.sqlitedb`, the document-mapping columns |
| 6 | Crashpad integration | closed | **replaced**: `anyfont_sys::crash` (panic hook, local text report with the profile path redacted); at the next start the report is offered through Help ▸ Report a Bug (`bugreport::BugReport::open_with_crash`), sent through `services/issue-relay`. No handler process, nothing uploaded without the user |
| 7 | `M/104` resource format (licence feature tables) | **partially closed**: format documented, semantics deliberately not decoded | **not needed**: AnyFont has no licensing, trial or feature gating (Phase 00). Nothing is read from or modelled on these tables |
| 8 | Per-font download URL | **partially closed**: fixed parts and endpoint builders known; the assembled per-font URL (`FONTSDATA.DBAssetURL`) never observed | **worked around**: AnyFont does not use the vendor cloud. Download comes from the Google Fonts catalog: `getmore::CATALOG_URL` (`https://fonts.google.com/metadata/fonts`), `DOWNLOAD_LIST_URL` (per-family file list, `download_list_url`), `CSS_URL` (previews). An online font record keeps its URLs in `model::RemoteInfo` (`asset_url`, `metadata_url`, `provider`), the counterpart of `DBAssetURL` / `DBMetadataURL`. Files go to the AnyFont Content folder and are installed like any local file |
| 9 | Workspace shortcut set | closed: 4 shortcuts in the font manager UI + 622 in the drawing applications' workspaces | AnyFont implements the 4 (`AnyFontApp::shortcuts`: Ctrl+A, Ctrl+Shift+A, Ctrl+Alt+I, Alt+F4), the menu access keys and the per-control keys (Phase 07). The 622 workspace sequences belong to the drawing applications and are not needed |
| 10 | sfnt table requirements | closed (the residual note asks for an explicit per-flavour mandatory/optional statement) | `anyfont_core::sfnt::SfntFace::open` / `validate` implement every listed rule with one unit test each (`sfnt::tests::*`). AnyFont states the per-flavour matrix explicitly (Phase 04 §1): TrueType flavour needs `glyf`+`loca` (or bitmap-only `EBDT`/`CBDT`/`sbix`), CFF flavour needs `CFF ` or `CFF2`; `head`, `maxp`, `cmap` are mandatory for both; missing `hhea`/`hmtx`/`name` is a warning only. Failing files are listed as Corrupt with the reason instead of being dropped |

### Residual items

| ID | Item | Dossier status | AnyFont |
|---|---|---|---|
| R-1 | Inter-process request protocol (`Interprocess.sqlitedb`) | closed | not reproduced: elevation uses request/response JSON files and `anyfont.exe /InstallHelper:` (`anyfont_sys::helper`, Phase 10 §6, Phase 12 §2) |
| R-2 | DDE service registration (`FontManager.exe` 0x1400029D0) | closed | no DDE. "Open in the running instance" uses a named mutex and pipe (`anyfont_sys::single_instance`); `/DDE` is accepted and ignored |
| R-3 | Anti-debug / anti-tamper | closed (none) | none in AnyFont either |
| R-4 | `license.xml` semantics | closed (as data) | not reproduced (no content-library licensing) |
| R-5 | "Evaluation Version" in the window title | closed | the title is "AnyFont" (`main.rs`, `titlebar.rs`), no suffix |

## 2. Coverage, failures and consistency of the dossier

| Dossier finding | Effect on AnyFont |
|---|---|
| 94.4 % of 158 757 functions decompiled; the rest are thunks and library functions (`coverage.md`) | no effect: every handler AnyFont reimplements has pseudocode or an address with a recovered registration |
| One non-thunk function without pseudocode each in `CdrCpr`, `CdrTxtUI`, `CRLCLR`, `CrlUtils` (`failures.md` table) | the `CdrTxtUI` one is the text-attribute registrar (`FillDataSourceMap@WTxtAttributeDS`, read from the Ghidra listing); compression, colour engine and the `CrlUtils` function are not on any path AnyFont reproduces. Note: `failures.md` also says "No non-thunk function was left without a body", which contradicts its own table, and `coverage.md` says no Ghidra session was needed while `unknowns.md` and `handler-map.md` describe a targeted Ghidra pass. AnyFont follows `unknowns.md` / `handler-map.md` |
| 243 of 247 menu command GUIDs resolve to an item definition; 12 menu items carry no binding (separators, flyouts, a macOS-only item); 18 menu items have no resolved caption (`consistency.md`) | AnyFont's menus come from the screenshots and `06-ui/menus.md` as written in `docs/UI-SPEC.md` §2, not from the raw XML: unbound items become separators or submenus, unresolved captions are the dynamic captions of gap 3. The 4 unresolved GUIDs are not on any captured menu and are not added |
| `used_by_fontmanager` flag in `strings-en.csv` is incomplete (Phase 08) | AnyFont's string list is built from what the window shows (`docs/strings-en.tsv`), checked by `scripts/i18n_check.py` and `e2e_i18n` |
| Features defined in the UI but never reached in the captured window | implemented in the least intrusive place (UI-SPEC §11, Phase 06), except Upload fonts and the toolbar overflow (§3) |

## 3. AnyFont's own known limitations

Collected from Phases 04–12, `docs/core-api.md`, `docs/sys-api.md`, `docs/UI-SPEC.md` and the code.

| Area | Limitation | Why / where |
|---|---|---|
| Font formats | **WOFF2 is not decoded**: such files are listed as Corrupt with "WOFF2 files are not supported" | needs Brotli plus the `glyf`/`loca` transform; the original reads no web fonts at all (`parse.rs`, Phase 09) |
| PANOSE | the distance weights are **AnyFont's own** (`panose::DIGIT_WEIGHTS`) with name similarity and code-page filters on top | the original's weights are not recoverable from the dossier (Phase 04 §8). Rankings can differ from the original's |
| Database | no keyword tables, sample/preview/curve caches or document-mapping columns | search runs in memory; previews are rendered from the font file (Phase 05) |
| Automation | **no DDE, COM, Automation or OLE embedding**; `/DDE`, `/Automation`, `/Embedding` start normally, `/Print` and `/P` print nothing, `/NoUI` is reserved | AnyFont exposes no automation surface (Phase 10 §1) |
| Start-up | no `PATH` rewrite, no `SetDllDirectoryW`, no process-wide `CoInitialize` | single executable; COM initialised where used (Phase 03 §5) |
| Crash reports | **Crashpad replaced** by a Rust panic hook: native crashes (access violations, stack overflows) are not caught | Rust code does not produce them outside `unsafe` (Phase 10 §3) |
| Install scope | installs go to **all users** through the elevated helper (one UAC prompt per operation); "Install for me" (per user) is offered only when the prompt is declined. The original's TemporaryMode exists as `Scope::Session` in `anyfont-sys` but the UI never uses it. Type 1 fonts install for all users only | `ops::install_scope`, `dialogs.rs`; Windows reads `Type 1 Fonts` only from HKLM (Phase 10 §2) |
| Registry value names | CFF-flavoured OpenType gets ` (OpenType)` (what Windows writes) where the original writes ` (TrueType)` for every sfnt | `regname.rs`, Phase 10 §2 |
| Elevated helper | residual TOCTOU: malware running as the same user could swap the request file between UAC consent and the helper reading it; impact limited to non-protected font files | Phase 12 §2 |
| Real system | install / uninstall / delete on the real registry, `AddFontResource`, the helper under a real UAC prompt, `MoveFileEx` delay-until-reboot and `reboot_now` never run on a development machine | by rule; covered by the sandbox (same engine) and by `e2e-real` on a disposable runner (without a UAC prompt), Phase 12 §7 |
| Network | no "Allow AnyFont to use the Internet" option; Download in My fonts stays disabled (there are no online placeholders in 0.1.0) — fonts are downloaded from Get More | lead decision 7, UI-SPEC §11 #16 |
| Features left out | Upload fonts, text dockers, UI customisation / workspaces, cloud dashboard, colour UI, font-sample export (`FontExportDS`), low-memory / monitor warnings | cloud or suite features (Phases 00, 01, 13) |
| UI | no toolbar overflow `»` (UI-SPEC §11 #19); the edit-box context menu is themed and has no IME items; the Delete key is unbound in the font list | the toolbar fits at the 800 px minimum width; lead decisions 10, 11 |
| Distribution | `anyfont.exe` and the setup are **not code-signed**; the C runtime is not linked statically (BUG-401) | no signing step in `build.bat`, `installer/AnyFont.iss` or the workflows |

### Documentation drift found while writing this page

| Page | Says | Code now |
|---|---|---|
| `10-behavior.md` §1 | "`NoSplash` is accepted. AnyFont has no splash screen." | there is a splash (`splash.rs`); `/NoSplash` skips it (`main.rs`, `e2e_cmdline::no_splash_starts_with_the_main_window`) |
| `09-features.md` "Not implemented in the shell" | the glyph alternates pop-up is left for later | implemented: `properties.rs` `alternates_popup` (not yet tested, Phase 13 §3) |
| `09-features.md` UI table | start-up is tested "manual (sandbox run)"; tests are the first smoke tests | `e2e_cmdline`, `e2e_selftest` and the other `e2e_*` files (Phase 13 §2.2) |

## 4. Open QA bugs

From `docs/QA-BUGS.md` and the `#[ignore = "BUG-n: ..."]` markers in `crates/anyfont/tests` at the time of
writing. Each has a test that fails until it is fixed (`cargo test -p anyfont --test <file> -- --ignored`).

| ID | Area | Bug | Test |
|---|---|---|---|
| BUG-101 | Report a Bug | fields are cut by characters, but the relay counts UTF-16 units (emoji title refused) | `e2e_bugreport::non_bmp_title_is_cut_to_the_relay_limit` |
| BUG-102 | Font matching | OK in Font Matching Results never links the substitute into the imported collection (`Matching::take_resolved` has no caller) | `e2e_getmore_matching::import_with_missing_fonts_links_the_chosen_substitute` |
| BUG-201 | i18n | captions with an `&` access key are shown in English in the 14 other languages | `e2e_i18n` (per language) |
| BUG-202 | i18n | Updates dialog: the update-mode choice and its note are cut off (11 languages) | `e2e_i18n` (per language) |
| BUG-203 | i18n / accessibility | plain egui buttons give screen readers the English text | `e2e_i18n` (per language) |
| BUG-301 | Edit boxes | right-clicking a text box clears its selection, so Cut / Copy / Delete are always disabled | `e2e_filters_list::edit_box_menu_cut_copy_delete_with_a_selection` |
| BUG-302 | Font list | double-clicking a font row never records it as recently used | `e2e_filters_list::recently_used_fonts_after_a_double_click` |
| BUG-303 | Export | Export includes AnyFont's hidden Content folder | `e2e_libraries::export_leaves_out_the_content_folder` |
| BUG-304 | Import | Import looks for a collection's fonts before the imported folders are scanned | `e2e_libraries::import_does_not_report_fonts_of_the_imported_folders_as_missing` |
| BUG-305 | Accessibility | a check box's accessible name loses every `&` of its caption | `e2e_properties_options::glyph_filter_names_keep_the_ampersand` |
| BUG-306 | Libraries | removing Favorites only empties it; Favorites stays in Libraries | `e2e_libraries::removing_favorites_removes_it_for_good` |
| BUG-401 | Setup / start-up | AnyFont does not start on a Windows without the Visual C++ runtime (`VCRUNTIME140.dll`) | `e2e-real.yml` check in `tests/real-system/run-real.ps1` |

When a bug is fixed, its `#[ignore]` is removed and its row is deleted from `docs/QA-BUGS.md`; update this table and
the status column of [`13-evidence.md`](13-evidence.md) at the same time.
