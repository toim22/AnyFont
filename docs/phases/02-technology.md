# Phase 02: Technology (original stack → AnyFont stack)

Dossier: `02-technology/stack.md`. This page sets the original's technology choices next to AnyFont's and says
why each replacement was picked. Library versions and licences are in [`11-third-party.md`](11-third-party.md).

## What the dossier says

- **Language and build:** C++ with MSVC 14.50 x64 (`PROD-X64-VC180` build configuration), MFC 14.0 Unicode as a
  shared DLL, the MSVC runtime DLLs. Legacy 32-bit parts only in setup and old filters.
- **Framework:** the vendor's common framework (`Crl*` modules) for strings, threads, config, resources, UI and
  interop; a declarative XML UI with a data-binding engine; COM objects with ATL class factories and type
  libraries; .NET interop assemblies for automation.
- **Graphics and text:** Skia 2.0 (with GDI/GDI+ fallback, GDI+ splash), DirectX 12 / D2D / DirectWrite in the
  drawing modules, ICU 66 and HarfBuzz for Unicode and shaping, LittleCMS for colour.
- **Font engine:** the vendor's own sfnt reader (`CdrFnt.dll`), no third-party font library.
- **Data:** SQLite font database accessed through a data layer, XML preferences, registry values.
- **Other:** Crashpad, V8 scripting, OpenCV / ONNX Runtime / DirectML, image codecs, CAD and Office converters
  (none of these reached by the font manager's own features).
- **Binary facts:** PE32+ GUI, ASLR + high-entropy VA + DEP, `/GS`, no CFG, no packing, Authenticode on every file.
- **Symbols:** no PDBs, RTTI present, COM type libraries, .NET metadata.

## AnyFont's stack

| Concern | Original | AnyFont | Why |
|---|---|---|---|
| Language, compiler | C++ / MSVC 14.50 x64 | **Rust** stable (1.99), MSVC x64 target, edition 2021; workspace `Cargo.toml` | memory safety for a program that parses untrusted font files; one toolchain |
| Executable layout | launcher EXE + application DLL + ~80 DLLs | **one executable** `anyfont.exe` built from three crates (`anyfont-core`, `anyfont-sys`, `anyfont`) | no DLL search path to protect, no `PATH` rewrite needed (Phase 03) |
| Application framework | MFC + vendor framework + data binding | **egui / eframe 0.37** (immediate mode), patched copy in `vendor/egui` for the global text translator; commands as the `commands::Cmd` enum | the UI is redrawn each frame from the state, so the binding engine's job (enable, check, caption) is a function call: `Catalog::eligibility`, `commands::menu_items` |
| UI definition | XML command bars / items / dialogs | Rust code following `docs/UI-SPEC.md`; colours and sizes in `docs/design-tokens.md` → `theme.rs` | the XML runtime is part of the vendor framework |
| GPU / windowing | Skia (GPU paths), GDI, DWM | **wgpu** renderer (`eframe::Renderer::Wgpu` in `main.rs`), undecorated window with our own title bar (`titlebar.rs`) and Win32 glue for hit testing and the DWM border (`platform.rs`) | egui's standard renderer; snap layouts kept through `WM_NCHITTEST` |
| Text shaping | HarfBuzz | **rustybuzz 0.20** (`preview.rs`) | Rust port of HarfBuzz, same shaping behaviour |
| Rasterising previews and glyphs | Skia | **tiny-skia 0.12** (`preview.rs`, `getmore::render_text`) | small software rasteriser; previews are drawn with the font file itself, on a worker thread |
| Icons | bitmaps mapped by `icons.map.xml` | own **SVG** icons rendered with **resvg/usvg 0.49** (`icons.rs`), tinted per state | no original assets may be copied; SVG scales with DPI |
| Font parsing | own sfnt reader in `CdrFnt.dll` | **ttf-parser 0.25** for tables + the original's validation rules reimplemented in `anyfont-core/src/sfnt.rs`; WOFF 1.0 in `woff.rs`, Type 1 in `type1.rs` | well-tested parser; the dossier's checks (Phase 04 §1) are kept |
| Unicode data | ICU 66 | Rust std, **unicode-general-category**, **unicode-script** (`glyphs.rs`) | only character classes and scripts are needed |
| Font database | SQLite through a data layer | **rusqlite 0.37** with bundled SQLite (`db.rs`) | same engine; schema in Phase 05 |
| XML (import/export) | `CrlXml` / tinyxml | **roxmltree** (`exchange.rs`) | read-only XML is enough; export is written directly |
| Settings | XML prefs + registry | **serde / serde_json** (`settings.json`) | one readable file; nothing in the registry except what the installer writes |
| Windows API | Win32 through MFC and the framework | **windows-rs 0.62** (`windows` crate) in `anyfont-sys` (registry, `AddFontResourceExW`, ACLs, shell, pipes, elevation) and in `anyfont` (`platform.rs`, `sysinfo.rs`) | official bindings |
| File dialogs | Windows shell dialogs | **rfd 0.18** (Windows' own dialogs) | |
| Clipboard | `CrlClip` | egui `copy_text`, **arboard** | |
| Network | WinINet / WinHTTP through the cloud modules, configured by `cloudconfig.json` | **ureq 3** (HTTPS + JSON) in `updater.rs`, `getmore.rs`, `bugreport.rs` | only user-started requests; no cloud configuration |
| Threads | `CrlUtils::CrlThread` | `std::thread` + `std::sync::mpsc` channels, `parking_lot` mutexes (Phase 03 §5) | |
| Hashing (duplicates) | SHA in `DBShaID` | **sha2** (`parse::sha256_hex`) | same idea: identical files are one font |
| Logging | `OutputDebugStringW` only | **log** + `anyfont_sys::logging` (rotating file, plus `OutputDebugStringW` in debug builds) | |
| Crash reporting | Google Crashpad (out-of-process handler) | Rust panic hook writing a text report (`anyfont_sys::crash`), offered as a bug report at the next start | no handler process; native crashes are not caught (Phase 14) |
| Localisation | `strings.xml` keyed by GUID, language from Windows | English strings in code, `assets/i18n/<lang>.tsv` keyed by the English text, language from Windows or `/Lang:` (`i18n.rs`) | AnyVector's translator pattern |
| Resources, manifest | VERSIONINFO, icons, manifest per module | `crates/anyfont/build.rs` + **winresource**: icon, version resource, `app.manifest` (Per-Monitor V2, common controls 6, `asInvoker`) | |
| Brand assets | skin PNGs | `xtask` (`cargo run -p xtask -- brand`): `.ico`, PNGs and installer wizard images generated from `assets/brand/anyfont-icon.svg` | |
| Installer | WiX burn + MSI packages | **Inno Setup** (`installer/AnyFont.iss`: x64, Windows 10 1809+, `PrivilegesRequired=lowest` with a per-user / all-users choice), `build.bat` | AnyVector pattern |
| Crash/feedback backend | vendor crash gateway | **Cloudflare Worker** `services/issue-relay` (TypeScript) that files GitHub issues | users need no GitHub account |
| CI | – | GitHub Actions: `.github/workflows/build.yml`, `e2e-real.yml` (real installer and real font system on a disposable runner) | |
| Tests | – | `cargo test` (unit + `egui_kittest` headless end-to-end with the sandbox font system), `forbid-real-writes` guard in `anyfont-sys` | `docs/TESTING.md` |

## Binary characteristics compared

| Item | Original | AnyFont |
|---|---|---|
| Architecture | x64 (plus x86 setup parts) | x64 only |
| Mitigations | ASLR, high-entropy VA, DEP, `/GS`; CFG not enforced | Rust's linker defaults on MSVC (ASLR, high-entropy VA, DEP); bounds-checked code instead of `/GS` cookies; `unsafe` only around Win32 calls |
| Packing / obfuscation | none | none (`strip = true`, thin LTO in the release profile) |
| Signing | Authenticode EV on every PE | not signed: there is no signing step in `build.bat`, `installer/AnyFont.iss` or the workflows (Phase 14) |
| Runtime dependencies | MSVC runtime DLLs, MFC DLL, the suite's DLLs | Windows only. Open: the build does not link the C runtime statically yet, so a Windows without the Visual C++ runtime cannot start AnyFont (QA BUG-401, Phase 14) |
| Symbols | none shipped (RTTI only) | none shipped |

## What AnyFont does not take over

- COM / ATL objects, type libraries, .NET interop and automation: AnyFont has no automation surface (Phase 03).
- V8 scripting, OpenCV / ONNX Runtime / DirectML, codecs, CAD/Office converters: not part of the font manager.
- DirectX 12 / D2D / DirectWrite drawing paths, GDI+ splash: replaced by wgpu and egui (`splash.rs`).
- LittleCMS: no colour management is needed for font previews.
