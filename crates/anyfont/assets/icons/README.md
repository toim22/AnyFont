# AnyFont UI icons

Our own vector drawings of every icon the reference screenshots show
(`C:\Projetos\engenharia-reversa\FontManager-ida\06-ui\screenshots`). They were drawn from scratch on a
pixel grid to match the shapes, proportions, stroke weights and colours of the reference; no bitmap of
the original program was copied or traced.

- **Grid.** 16×16 viewBox for toolbar, menu, title bar, panel and status bar icons; 24×24 for the font
  technology icons of the font list rows (the reference draws those about 20 px tall). Strokes are 1 px
  on half-pixel coordinates, so the icons are crisp at 100 % and 200 %.
- **Colours.** Light on dark like the screenshots: glyphs `#CACED4`, green badge `#1BB474`, red badge
  `#FF6250`, section chevrons `#1D4470`, padlock `#000000` (drawn on the green installed bar `#66BF83`).
  Badge symbols (+, ↑, ↓) are cut out of the badge, so the background shows through, as in the
  reference. All values are in `docs/design-tokens.md`.
- **One SVG per icon.** States are produced by the UI:
  - *Disabled* (toolbar, menus): greyscale, then 33 % opacity over the background — the glyph lands on
    `#5D5D5D` and a badge on about `#434343`, which is what the reference shows. In egui: rasterize a
    greyscale copy once and draw it with `tint(Color32::from_white_alpha(84))`; for single-colour icons a
    plain `tint(Color32::from_rgb(117, 115, 112))` gives the same `#5D5D5D`.
  - *Title bar buttons* (`title-*`): drawn `#5D5D5D` normally in the reference → same tint as disabled;
    use the untinted icon (`#CACED4`) on hover.
  - *Window buttons* (`window-*`) and the submenu arrow: `#969696` in the reference →
    `tint(Color32::from_rgb(190, 186, 181))`.
  - *Filter clear* (`filter-clear`): disabled look (`#5D5D5D`) while no filter is set.
  - *Light theme*: tint the single-colour glyphs dark (e.g. `#3A3A3A`).
- **App icon.** The title bar's top-left icon and the window icon are the brand icon
  (`../brand/anyfont-icon.svg`, rendered into `../anyfont.ico` and `../icon-256.png` by
  `cargo run -p xtask -- brand`); there is no separate UI icon for it.

`contact-sheet.png` shows every icon at 1×, 2× and 4× next to a crop of the same spot in the reference
(enlarged without smoothing), and the whole toolbar rebuilt from these SVGs under the reference
toolbar. Rebuild it after changing an icon: `cargo run -p xtask -- icons` (set `ANYFONT_REFERENCE` if
the screenshots are elsewhere; without them the sheet shows our icons only).

## Icons

| Icon | Used for | Matches (reference screenshot) |
|---|---|---|
| `add-folder.svg` | toolbar 1 and File › Add Folder… | `04-toolbar/buttons/01 Add Folder (enabled).png`, `03-menus/*/File.png` |
| `add-collection.svg` | toolbar 2 and File › Add Collection | `04-toolbar/buttons/02 Add Collection (enabled).png`, `03-menus/*/File.png` |
| `get-more.svg` | toolbar 3 and File › Get More… (AnyFont: online font catalog) | `04-toolbar/buttons/03 Get More (enabled).png` |
| `remove.svg` | toolbar 4, File › Remove Folder or Collection, Libraries right-click › Remove Folder or Collection | `04-toolbar/buttons/04 Remove Folder or Collection (enabled).png`, `07-context-menus/Libraries - user collection (Collection 1).png` |
| `install.svg` | toolbar 5, Font › Install, font row right-click › Install (badge green in AnyFont; disabled in every reference state) | `04-toolbar/buttons/05 Install (disabled).png`, `03-menus/font-selected/Font.png` |
| `uninstall.svg` | toolbar 6, Font › Uninstall, font row right-click › Uninstall | `04-toolbar/buttons/06 Uninstall (enabled).png`, `07-context-menus/font row - single font (Algerian).png` |
| `download.svg` | toolbar 7, Font › Download, font row right-click › Download | `04-toolbar/buttons/07 Download (disabled).png` |
| `delete.svg` | toolbar 8, Font › Delete, font row right-click › Delete | `04-toolbar/buttons/08 Delete (enabled).png` |
| `show-duplicates.svg` | toolbar 9, Font › Show duplicate fonts, font row right-click › Show duplicate fonts | `04-toolbar/buttons/09 Show duplicate fonts (enabled).png` |
| `font-properties.svg` | toolbar 10 and View › Show font properties (toggle; checked = `#1D4470` button background) | `04-toolbar/buttons/10 Show font properties (enabled).png`, `03-menus/deselect-all/View.png` |
| `import.svg` | File › Import Folders and Collections… | `03-menus/deselect-all/File.png` |
| `export.svg` | File › Export Folders and Collections… | `03-menus/deselect-all/File.png` (disabled there) |
| `exit.svg` | File › Exit | `03-menus/deselect-all/File.png` |
| `refresh.svg` | View › Refresh the Font List | `03-menus/deselect-all/View.png` (disabled there) |
| `help.svg` | Help › Product Help… | `03-menus/deselect-all/Help.png` |
| `about.svg` | Help › About AnyFont… | `03-menus/deselect-all/Help.png` |
| `chevron-right.svg` | submenu arrow in menus ("Add to collection ›", "Open font file location ›") | `03-menus/font-selected/Font.png` |
| `chevron-down.svg` | sample text drop-down button, combo boxes, spin box down, glyph list scroll-down mark | `02-main-window/Main window - restored size.png` (sample text box), `06-panels/font-properties/Tab Variable font - variable font (Bahnschrift).png` |
| `chevron-up.svg` | spin box up (Variable font axis values) | `06-panels/font-properties/Tab Variable font - variable font (Bahnschrift).png` |
| `title-help.svg` | title bar "?" (Product Help) | `08-title-bar/Title bar buttons.png` |
| `title-options.svg` | title bar gear (Options) | `08-title-bar/Title bar buttons.png` |
| `title-updates.svg` | title bar third button = **Updates** (check for / install updates); same style as the reference's trial button, new symbol: a box with a down arrow and a circular-arrow badge | `08-title-bar/Title bar buttons.png` (third button) |
| `window-minimize.svg` | title bar minimize | `08-title-bar/Title bar buttons.png` |
| `window-maximize.svg` | title bar maximize (window not maximized) | `02-main-window/Main window - restored size.png` |
| `window-restore.svg` | title bar restore (window maximized) | `08-title-bar/Title bar buttons.png` |
| `window-close.svg` | title bar close, dialog close "×" | `08-title-bar/Title bar buttons.png`, `05-dialogs/*` |
| `section-expand.svg` | Libraries / Filters / Glyph Filters section header, collapsed section (▼▼) | `06-panels/libraries/Libraries - Folders collapsed.png`, `06-panels/filters/Filters (all collapsed).png` |
| `section-collapse.svg` | same headers, expanded section (▲▲) | `06-panels/libraries/Libraries (default).png`, `06-panels/filters/Filters - Font Technology.png` |
| `filter-clear.svg` | Filters header and Glyph Filters header: clear all filters | `06-panels/filters/Filters (default).png`, `06-panels/font-properties/Tab Glyphs.png` |
| `lock.svg` | font list row: protected system font (on the green installed bar) | `02-main-window/Main window - restored size.png` (Arial rows) |
| `tech-truetype.svg` (24) | font list row: TrueType | `02-main-window/Main window - restored size.png` (Arial) |
| `tech-opentype.svg` (24) | font list row: OpenType (also OpenType – TrueType) | `02-main-window/Main window - restored size.png` (Arial Black), `06-panels/font-list/Family header double-clicked.png` |
| `tech-variable.svg` (24) | font list row: variable font | `06-panels/font-properties/full-window/Variable font selected (Bahnschrift).png` |
| `tech-type1.svg` (24) | font list row: PostScript Type 1 (not in the reference; same style) | — |
| `tech-other.svg` (24) | font list row: other technologies (raster / unknown; not in the reference; same style) | — |
| `preview-smaller.svg` | status bar, left of the preview size slider (smaller previews) | `06-panels/font-list/Preview size smaller (small A clicked once from default).png` |
| `preview-larger.svg` | status bar, right of the preview size slider (larger previews); also under Glyph Preview | `06-panels/font-list/Preview size larger (big A clicked once).png`, `06-panels/font-properties/Tab Glyphs.png` |
| `tab-glyphs.svg` | font properties tab Glyphs | `06-panels/font-properties/Tab Glyphs.png` |
| `tab-font-info.svg` | font properties tab Font info | `06-panels/font-properties/Tab Font info.png` |
| `tab-variable.svg` | font properties tab Variable font | `06-panels/font-properties/Tab Variable font.png` |

Not icons (drawn by the UI from `docs/design-tokens.md`): check boxes, sliders, scroll bars, the green
installed bar, separators, the section header line, the selected-tab and checked-button backgrounds.
No search icon appears in the reference (the Search box is a plain edit box with a placeholder).
