# Phase 08 – Localization

Dossier: `08-localization/languages.md`, `08-localization/strings-en.csv`. Master list:
[`docs/strings-en.tsv`](../strings-en.tsv).

## What the dossier says

- The installed program has one language folder, `EN`. Its master table is `strings.xml`, with
  `lang="en" master="true"` and 24 273 strings.
- About 1 365 of those strings are referenced by the Font Manager UI. The `used_by_fontmanager` flag in
  `strings-en.csv` is incomplete: many strings the window shows (empty states, status formats, tooltips) are
  not flagged.
- How text resolves: UI item guid → `caption.csv`, `tooltipHeader.csv` or `tooltipBody.csv` → string guid →
  `strings.xml`.
- The UI language comes from Windows: `IGLB_UILanguage` and `CrlLocale::GetUserLocaleID`. If the selected
  language is missing, the program falls back to EN.
- There is no language selector in the Font Manager UI.
- Format strings use printf syntax (`%s`, `%d`, `%%`).

## How AnyFont implements it

AnyFont uses AnyVector's translator, copied as a pattern from `AnyVector/crates/anyvector/src/i18n.rs`:

- **English is the source.** Every string the UI shows is written in English in the code. The keys are listed,
  deduplicated, in `docs/strings-en.tsv`:
  - dossier wording;
  - "AnyFont" in place of the original product name (the vendor name never appears);
  - `&` access keys;
  - `{}` placeholders instead of `%s`/`%d`;
  - `\n` for line breaks.
- **Translations** live in `crates/anyfont/assets/i18n/<code>.tsv`. Each line is one `English<TAB>translation`
  pair, with `#` comments allowed. The files are embedded in the executable.
- **The translator.** egui's text-translator hook (the AnyVector patch in `vendor/egui`) passes every label,
  button, menu item, tooltip and painted caption through `i18n::translator`. It translates in this order:
  1. an exact match;
  2. a `{}` template match: `"{} (filtered): {} fonts"` matches "My fonts (filtered): 10 fonts", and the
     translation receives the values in order (`{}`) or by position (`{0}`, `{1}`).

  Text that users type is never translated, and neither is font data: font names, the "{family} ({style},
  {width})" label, Font info values and file paths.
- **Language choice.**
  - The default follows Windows' display language (`GetUserDefaultUILanguage`), as in the original.
  - AnyFont adds no language selector, matching the dossier. **[decide]** AnyVector has one in Options. If the
    lead wants it, it goes in Options under Appearance, as "Language:" with a combo.
  - The settings value `language` (empty means follow Windows) is kept, so a selector can be added later
    without a migration.
- **Languages for 0.1.0.** English, plus the languages AnyVector already ships (pt-BR is the reference;
  zh-CN, zh-TW, cs-CZ, nl-NL, fr-FR, de-DE, it-IT, ja-JP, pl-PL, ru-RU, es-ES, sv-SE, tr-TR), as far as
  translations exist. A missing key falls back to English.
- **Fonts for scripts.** CJK UI text needs a CJK UI font, chosen per language as in `i18n::add_fonts`:
  Microsoft YaHei UI, JhengHei UI, Yu Gothic UI or Malgun Gothic.
- **Tools.** Port AnyVector's `xtask/i18n.py` (lists keys not translated yet) and `xtask/i18n_check.py` (checks a
  language against the keys) to read `docs/strings-en.tsv`. CI fails when a string shown by the UI is missing
  from `strings-en.tsv`.

## Rules for developers

1. Never build a sentence from fragments. Use one format string with `{}`, and add it to `strings-en.tsv`.
2. Captions keep the dossier's capitalisation exactly ("Add to collection", "Refresh the Font List").
3. There is no plural machinery. The original says "1 fonts", and AnyFont keeps that unless the lead decides
   otherwise (UI-SPEC §12 #3).
4. New AnyFont strings (Updates, bug report, catalog, the "Allow AnyFont to use the Internet" option) go in
   the TSV in their own `#` section.
