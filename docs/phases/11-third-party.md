# Phase 11 – Third-party components

Dossier: `11-third-party/libraries.md`. This page lists what the original program uses from third parties, what
AnyFont uses instead, and how AnyFont's licence notices are produced and shipped.

## What the dossier says

- 98 third-party files ship with the original suite. The font manager's own **direct** third-party imports are only
  Skia, GDI+ (part of Windows) and the C++ runtime. Everything else arrives through the suite's shared modules.
- Its closure reaches Skia, ICU, HarfBuzz, OpenCV, a rebranded Intel IPP and the OpenCL loader. LittleCMS, codecs,
  CAD/Office converters, V8, ONNX Runtime, DirectML, Crashpad and VSTA belong to the rest of the suite.
- Font parsing uses **no** third-party font library: the suite's own font module reads the sfnt tables (`cmap`,
  `glyf`, `CFF `, `head`, `hhea`, `OS/2`, `post`, `name`, `GPOS`, `GSUB`, `kern`, `fvar`, `STAT`, `DSIG`).
- No licence texts ship in the install folder; the setup engine holds them.

## What AnyFont uses instead

AnyFont is one Rust executable. Nothing is a separate DLL, and none of the original's binaries, libraries or assets
is used.

| Original component | Used there for | AnyFont |
| --- | --- | --- |
| Skia (`skia.dll`, `skshaper`, `skunicode`) | drawing previews and the UI | **egui/eframe 0.37** (immediate-mode UI; patched copy in `vendor/egui`), **tiny-skia 0.12** (software rasterising of glyph previews), **resvg/usvg 0.49** (the SVG icons) |
| HarfBuzz | OpenType shaping in the font list and glyph view | **rustybuzz 0.20** (a Rust port of HarfBuzz) |
| ICU 66 | Unicode data, break iteration | Rust's own Unicode support, **unicode-general-category** (glyph classes) and the Unicode tables inside rustybuzz/egui. No ICU and no collation library. |
| The suite's sfnt parser | font metadata, validation | **ttf-parser 0.25** for the tables, plus AnyFont's own sfnt rules in `anyfont-core` (the dossier's checks, reimplemented) |
| The suite's font cache / asset database | font list cache, libraries | **rusqlite 0.37** with SQLite compiled in (`bundled`) |
| GDI+ (Windows) | bitmaps | **image 0.25** (PNG and ICO only) |
| tinyxml | XML | **roxmltree** (reading XML library and collection files) |
| zlib | DEFLATE | **miniz_oxide** (pure Rust) |
| WinINet / the suite's network layer | updates, online content | **ureq 3** (HTTPS, JSON): the updater and the online font catalog |
| Crashpad | crash reports | none. Help ▸ Report a Bug files a GitHub issue through `services/issue-relay` |
| MFC, the suite's common framework, data binding | application framework | egui plus AnyFont's own code |
| Windows shell dialogs | file and folder pickers | **rfd 0.18** (Windows' own dialogs) |
| – | clipboard, folders, file walking, hashing, settings | **arboard**, **dirs**, **walkdir**, **sha2** (duplicate detection), **serde/serde_json**, **log**, **parking_lot**, **thiserror** |
| – | build and tests only (not in the program) | **winresource** (embeds the icon and version), **tempfile**, **egui_kittest** |
| LittleCMS, OpenCV, Intel IPP, OpenCL, codecs, ODA SDK, Aspose, OpenCOLLADA, V8, ONNX Runtime, DirectML, VSTA | other suite programs | not needed: the font manager doesn't reach them for its own features |

Windows itself (registry, `AddFontResource`, `WM_FONTCHANGE`, Recycle Bin, elevation) is called from `anyfont-sys`.
The systems engineer records the Windows API crate it uses in `docs/sys-api.md`; it shows up in the notices like any
other crate.

The table lists the crates the team planned (`Cargo.toml`, `[workspace.dependencies]`). The authoritative list,
with every indirect crate and exact versions, is `THIRD-PARTY-NOTICES.txt`.

## Licence notices

- `scripts/licenses.ps1` reads `cargo metadata` for the Windows x64 target, walks every **normal** dependency of
  `anyfont.exe` (build and dev dependencies are left out, and so are AnyFont's own crates) and writes
  `THIRD-PARTY-NOTICES.txt`: the number of components, a count per licence expression, then each crate with its
  version, licence and repository. It produces the same list as AnyVector's `cargo run -p xtask -- licenses`.
- `build.bat` runs the script before each release build, so the file always matches the build.
- The installer copies it to `{app}\legal\THIRD-PARTY-NOTICES.txt`; the portable package has it in `legal\`.
- The program shows it in Help ▸ About AnyFont ▸ Legal Notices (see `docs/RELEASING.md`, *Legal notices*).
- Code copied from another project into AnyFont's own crates must be added to `$Bundled` in the script, with its
  licence and source, as AnyVector does for its MPL-2.0 palette tables. There is none today.
- Licences to watch: anything copyleft (GPL, LGPL, or MPL-2.0 for a file we change) needs the lead's approval before
  it is added. The fonts egui ships inside `epaint` are under the OFL-1.1 and the Ubuntu Font Licence; both allow
  embedding and are listed with the crate.

## Fonts

AnyFont ships no fonts of its own besides the UI fonts inside egui. Fonts from the online catalog are downloaded at
the user's request, under their own (open) licences, to `Documents\AnyFont\Content\Fonts`. They are not part of the
program and are not listed in the notices.
