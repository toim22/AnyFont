# Phase documents

AnyFont follows the reverse-engineering dossier `C:\Projetos\engenharia-reversa\FontManager-ida`. Every top-level
folder of the dossier is one phase of the work and none is skipped (see [`../TEAM-PLAN.md`](../TEAM-PLAN.md)).
Each page below says what that dossier phase contains and how AnyFont implements it, with references to the code
and the tests.

| Phase | Dossier folder | Page | Status (AnyFont 0.1.0) |
|---|---|---|---|
| 00 | `00-summary` | [00-summary.md](00-summary.md) | product scope set: dossier features kept, vendor branding, licensing, store and cloud replaced by Updates, Get More (open-licence catalog) and bug report / feedback |
| 01 | `01-inventory` | [01-inventory.md](01-inventory.md) | all 69 vendor modules and 12 third-party modules of the closure mapped to an AnyFont crate/module or marked not needed |
| 02 | `02-technology` | [02-technology.md](02-technology.md) | stack chosen: Rust, egui/eframe + wgpu, ttf-parser, rustybuzz, tiny-skia, resvg, rusqlite, windows-rs, Inno Setup, Cloudflare Worker relay |
| 03 | `03-architecture` | [03-architecture.md](03-architecture.md) | three crates; every data source mapped to a module; start-up, threads and IPC compared |
| 04 | `04-pseudocode` | [04-pseudocode.md](04-pseudocode.md) | engine handlers (sfnt checks, parsing, scan, PANOSE, eligibility) reimplemented in `anyfont-core` |
| 05 | `05-data-models` | [05-data-models.md](05-data-models.md) | font database schema, settings and file formats defined; caches and keyword tables left out by design |
| 06 | `06-ui` | [06-ui.md](06-ui.md) | every captured screen, menu, toolbar, panel and dialog implemented (`docs/UI-SPEC.md`); uncaptured features placed per UI-SPEC §11 |
| 07 | `07-shortcuts` | [07-shortcuts.md](07-shortcuts.md) | the 4 application shortcuts, access keys and per-control keys implemented and tested |
| 08 | `08-localization` | [08-localization.md](08-localization.md) | English master strings + 14 translations, language from Windows; open bugs BUG-201/202/203 |
| 09 | `09-features` | [09-features.md](09-features.md) | feature catalog implemented in core and UI; the Windows-layer rows are in Phase 13 §2.3 |
| 10 | `10-behavior` | [10-behavior.md](10-behavior.md) | command line, folders, font registration, protected fonts, reboot rule, logging, single instance, elevation implemented in `anyfont-sys` |
| 11 | `11-third-party` | [11-third-party.md](11-third-party.md) | third-party crates listed with licence notices |
| 12 | `12-security` | [12-security.md](12-security.md) | `asInvoker`, per-operation elevated helper, request validation, sandbox and test guard; residual TOCTOU documented |
| 13 | `13-evidence` | [13-evidence.md](13-evidence.md) | traceability matrix feature → dossier evidence → code → tests; gaps: glyph copy/alternates, vendor link click, refresh strip, Shift reset, crash offer |
| 14 | `14-gaps` | [14-gaps.md](14-gaps.md) | all 10 dossier gaps and R-1…R-5 resolved or worked around; AnyFont's own limitations; 11 open QA bugs + BUG-401 |

Other references: [`../ARCHITECTURE.md`](../ARCHITECTURE.md), [`../core-api.md`](../core-api.md),
[`../sys-api.md`](../sys-api.md), [`../UI-SPEC.md`](../UI-SPEC.md), [`../TESTING.md`](../TESTING.md),
[`../QA-BUGS.md`](../QA-BUGS.md).
