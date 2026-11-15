# AnyFont 0.1.0 – user interface specification

This spec describes AnyFont's user interface completely. A developer should be able to build the UI from this
document alone. Everything comes from the dossier's `06-ui` folder, mainly the screenshots of the original
program in `06-ui/screenshots`. We measured them pixel by pixel. Some things are AnyFont's own decisions, as the
team plan requires: the product name, Updates replacing the trial button, the Google Fonts catalog replacing
the store, and the Help additions. Each one is marked **[AnyFont]**. Where the dossier leaves a question open,
the question is marked **[decide]** and also listed in §12.

Related documents:

| Document | What it holds |
|---|---|
| `docs/design-tokens.md` | colours, fonts, radii and icon sizes, for every theme (written by the designer; this spec only names the tokens) |
| `docs/strings-en.tsv` | every English string AnyFont shows (§9) |
| `docs/phases/06-ui.md`, `07-shortcuts.md`, `08-localization.md` | per-phase record: what the dossier says, how AnyFont implements it |

Conventions:
- Sizes are logical pixels at 100 % Windows scaling (Options ▸ Size 100 %). Multiply them by the effective scale.
- "y 49–81" means an inclusive pixel range. Unless a section says otherwise, coordinates come from the
  maximized capture: a 1920×1080 client with the top-left at (0, 0).
- **UI font**: Segoe UI 9 pt, which is 12 px. **Bold** means Segoe UI Bold 9 pt. Text is ClearType-rendered in
  the screenshots; AnyFont uses egui's grey-scale anti-aliasing, so stems may look slightly thinner.
- Captions are written as in the dossier. `&` marks the access key (shown underlined). `…` is never used;
  ellipses are three dots, `...`.
- Colour values below are the **Dark** theme as captured. Code must use the named tokens from
  `design-tokens.md`, never these literals.

---

## 1. Main window layout

```
y 0   ┌───────────────────────────────────────────────────────────────────── title bar (32 px, 25 visible when maximized)
      │[icon] AnyFont                                    [?] [⚙] [⟳] [–] [□] [×]
y 25  ├─ menu bar (24 px) ─ File  Edit  Font  View  Help
y 49  ├─ toolbar (33 px) ─ [+F][+C][GM] | [Rm] | [In][Un][Dl] | [Del] | [Dup] | [Props]
y 82  ├──────────────────────────────────────────────────────────────── 1 px line
      │ Libraries (241 px)  │ [Enter sample text ▾] [Search        ]            │ font properties
      │  Folders ───── ︽   │ ───────────────────────────────────────────────── │ (271 px, hidden
      │  My fonts           │ font list                                       ▒ │  by default)
      │  My Collections ─ ︽│                                                 ▒ │
      │  Favorites          │                                                 ▒ │
      │ ─────── splitter ── │                                                 ▒ │
      │ Filters        [⊘]  │                                                 ▒ │
      │  Font Status ── ︽  │                                                 ▒ │
      │  ☐ Installed …      │                                                 ▒ │
y 1047├─────────────────────┴──────────────────────────────────────────────── 1 px line
      │                       My fonts: 681 fonts                 a ──●────── A
y 1079└───────────────────────────────────────────────────────────────────────
```

### 1.1 Vertical bands (maximized 1920×1080)

| Band | y range | Height | Background token (Dark value) |
|---|---|---|---|
| Title bar (visible part) | 0–24 | 25 (32 when restored, see §4) | `titlebar.bg` (#262626) |
| Menu bar | 25–48 | 24 | `menubar.bg` (#404040) |
| Toolbar | 49–81 | 33 | `toolbar.bg` (#262626) |
| Toolbar bottom line | 82 | 1, full width | `line.strong` (#525252) |
| Body (panels) | 83–1046 | 964 | `panel.bg` (#262626) |
| Status line | 1047 | 1, full width; there is a gap at x 237–245 where the left divider meets it | `line.strong` (#525252) |
| Status bar | 1048–1079 | 32 | `panel.bg` |

The restored capture (800×600 window with a 7 px DWM shadow, client x 7–792) has the same bands, each
7 px lower: menu bar 32–55, toolbar 56–88, toolbar line 89. When maximized, the top 7 px of the 32 px
title bar are outside the screen, as with any Windows custom-frame window.

### 1.2 Horizontal columns (body)

| Column | x range | Width | Notes |
|---|---|---|---|
| Left panel (Libraries + Filters) | 0–240 | 241 | content rows are drawn x 0–237; their text starts at x 3–4 |
| Left divider | 241 | 1 | `line.divider` (#3c3c3c), from y 88 down to the status line |
| Gap | 242–244 | 3 | |
| Content area | 245–1919 | 1675 | search row, font list and list scrollbar |
| Font properties panel (when shown) | 1649–1919 | 271 | 1 px divider at x 1648 (`line.divider`); the list then ends at x 1644 |

The left panel is resizable at its divider (splitter). It has a minimum of **240 px**, from `@minItemWidth`
in `09-features/feature-catalog.md` §6. The font properties panel can also be resized at x 1648; its default
width is 271 px. **[decide]** Both maximum widths are unknown. Proposal: left ≤ 50 % of the window, properties
≤ 600 px.

Minimum window size **[decide]**: not captured. The restored capture at 800×600 lays out correctly, so 800×600 is
proposed as the minimum.

### 1.3 Content area: search row

| Element | Rect | Details |
|---|---|---|
| Sample text combo | x 245–494, y 89–112 (250×24) | editable combo box. Rounded 1 px border `input.border` (#525252), radius 2. Cue text "Enter sample text" in `text.cue`. Chevron at x 482–489. The open drop-down has a blue focus border (`accent`, #017dbc). Items: §5.3 |
| Gap | 8 px | |
| Search box | x 503–752, y 89–112 (250×24) | edit box with cue "Search". Same border. Typing filters live (§5.3) |
| Separator line | y 123, x 247–1901 | 1 px `line.strong` |
| Font list | x 245–1902, y 124–1038 | §5.3 |
| List scrollbar | x 1903–1919 (17 px), y 121–1038 | track `scroll.track` (#171717). The thumb is drawn as a thin 2–3 px line `scroll.thumb` (#959595) at x 1910–1912. **[decide]** Hover was not captured. Proposal: the thumb widens to 8 px while the pointer is over the track (overlay-style scrollbar, as in Windows 11) |
| Gap | y 1039–1046 | |

### 1.4 Status bar

| Element | Position | Details |
|---|---|---|
| Status text | x 245, text cap y ~1062–1072 (vertically centred) | `{library}: {count} fonts`, e.g. "My fonts: 681 fonts". When a filter or search is active, " (filtered)" follows the library name: "My fonts (filtered): 10 fonts". There is no singular form; the original shows "Collection 1: 1 fonts" (§12) |
| Small "A" button | x 1762–1772 | decreases the preview size by one step (1 px). Tooltip "Change the font size in the font list." |
| Preview size slider | track x 1798–1878 (81 px), centred vertically | blue fill `accent` (#017dbc) left of the thumb, `slider.rest` (#525252) right of it. The thumb is a hollow circle about 11 px across with a 2 px `accent` ring (default position x 1822–1832). Same tooltip |
| Big "A" button | x 1903–1910 | increases the preview size by one step |
| Left part (x 0–240) | | empty; the status line still runs across it |

When the font properties panel is shown, its own status strip sits at the same height inside the panel
(see §5.4.4). The list's A/slider/A group then moves left so that it stays right-aligned to the list.
**[decide]** In the capture the list slider stays where it was, under the panel (the original simply keeps it
in the content column). Proposal: right-align it to the list's column.

---

## 2. Menus

### 2.1 Menu bar

| Item | Text x (glyphs) | Highlight box | Access key |
|---|---|---|---|
| File | 14–31 | x 4–42 | **&File** (underlined F) |
| Edit | 55–74 | x 45–85 | **&Edit** |
| Font | 98–121 | x 88–132 | none in the original |
| View | 145–169 | x 135–180 | none in the original |
| Help | 193–217 | x 183–228 | none in the original |

The rule is: item box = text width + 10 px padding on each side, with a 3 px gap between boxes, and the first
box starts at x 4. Text is vertically centred (cap y 32–43). The hovered or open item gets a rounded rectangle
`menubar.hot` (#525252), radius 3, inset 2 px from the top and bottom of the bar.

Underlines show only while Alt is held or when the menu is entered with the keyboard, as in standard Windows.
**[decide]** Font, View and Help have no access key in the original. Proposal: keep them without one, as captured.
F10 or Alt plus the arrow keys still reach them.

### 2.2 Popup menu geometry (all menus, submenus and context menus)

| Property | Value |
|---|---|
| Border | 1 px `menu.border` (#525252) |
| Background | `menu.bg` (#262626) |
| Padding inside the border | 2 px top and bottom |
| Item height | 22 px |
| Separator height | 7 px; the 1 px line `menu.separator` (#525252) runs from the text column (x 32) to the right edge minus 2 |
| Icon column | 16×16 icon at x 9 (popup-relative), vertically centred; reserved even when no item has an icon |
| Text column | x ~36 |
| Shortcut text | right-aligned, 24 px from the right edge |
| Submenu arrow | `›` chevron, right edge minus 12 |
| Highlighted item | full-width `menu.hot` (#525252), inset 2 px horizontally |
| Disabled item | text and icon in `text.disabled`; never highlighted |
| Checked check-button item | the item's icon is drawn on a 20×20 `accent.checked` (#1d4470) box, radius 3 (no ✓ mark) |
| Width | widest text + shortcut + 60 px, at least ~196 px (File 257, Edit 214, Font 215, View 196, Help 236) |
| Position | top-level popup: left = item box left − 1, top = menu-bar bottom (y 48). Submenu: overlaps the parent's right edge by 3 px; its top equals the parent item's top |

### 2.3 Contexts used in the rules

There are 13 selection contexts. They are the folders of `06-ui/screenshots`.

| # | Context | Situation |
|---|---|---|
| C1 | deselect-all | My fonts after Ctrl+Shift+A; the current font (Arial Regular) is still highlighted |
| C2 | family-header-selected | a family header row (Agency FB) is clicked |
| C3 | font-selected | one installed, unprotected face is selected |
| C4 | system-font-selected | one protected system font is selected (lock icon) |
| C5 | all-selected | Ctrl+A in My fonts |
| C6 | favorites-library | Favorites (empty) is open |
| C7 | filter-no-results | the filters match nothing |
| C8 | font-properties-shown | C3 plus the properties panel |
| C9 | duplicates-shown | Show duplicate fonts is on, nothing explicitly selected |
| C10 | temp-folder-library | a user folder is open; its first font is auto-selected |
| C11 | temp-folder-font-selected | a face in a user folder is selected |
| C12 | user-collection-library | "Collection 1" is open; its first font is auto-selected |
| C13 | user-collection-font-selected | a face in Collection 1 is selected |

The enable rules below are written as predicates. Each predicate reproduces the captured state in all 13
contexts. "Selected" means *explicitly* selected rows. The "current font" (the last focused face, which stays
highlighted after Deselect All) does not count, except where a rule says so.

### 2.4 File

| # | Caption | Icon | Shortcut | Enable | Action |
|---|---|---|---|---|---|
| 1 | Add Folder... | add-folder | | always | native folder picker (§7.2.1). The folder is added under Libraries ▸ Folders and opened |
| 2 | Add Collection | add-collection | | always | creates "Collection {n}" under My Collections and opens the inline name editor (§5.1) |
| 3 | Get More... | get-more | | always | opens the Get More window, which is the Google Fonts catalog (§7.2.7) **[AnyFont]** |
| – | separator | | | | |
| 4 | Remove Folder or Collection | remove | | `current library is deletable`: any user folder or collection, **including Favorites**; never My fonts. True in C6, C10–C13 | remove confirmation (§7.2.5), then remove the library from the list. Font files are not touched |
| – | separator | | | | |
| 5 | Import Folders and Collections... | import | | `font database ready` (the first scan has finished) | native Open dialog, `*.xml` (§7.2.3) |
| 6 | Export Folders and Collections... | export | | `at least one user folder or user collection exists`. Favorites counts as a collection **[decide]** | native Save dialog, `*.xml` |
| – | separator | | | | |
| 7 | E&xit | exit | Alt+F4 | always | closes the window |

### 2.5 Edit (no icons)

| # | Caption | Shortcut | Enable | Action |
|---|---|---|---|---|
| 1 | Select All | Ctrl+A | the visible list is not empty (false in C6, C7) | selects every visible face |
| 2 | Deselect All | Ctrl+Shift+A | selected count > 0 | clears the selection; the current font keeps its highlight |

### 2.6 Font

The Font menu and the font-row context menu are the same command bar, with the same items, rules and order.

| # | Caption (dynamic) | Icon | Visible | Enable | Action |
|---|---|---|---|---|---|
| 1 | Install | install | always | `selection contains a font that is not installed` (no captured context: all fonts in the captures counted as installed) | install the selected not-installed fonts (progress §7.2.12; may lead to the reboot prompt §7.2.10) |
| 2 | Uninstall | uninstall | always | `selection (or the clicked family header) contains an installed, unprotected font`. True in C2 (menu), C3, C5, C8, C10–C13 | uninstall, with progress; if a font is in use, the reboot prompt follows |
| 3 | Download | download | `internet use allowed` (Options/network gate; hidden when offline use is forced) | `selection contains an online (catalog) font that is not yet downloaded` | download to the AnyFont Content folder (§7.2.7) **[AnyFont]** |
| – | separator | | | | |
| 4 | Delete | delete | always | same as Uninstall | delete-fonts confirmation (§7.2.11), then move the files to the Recycle Bin |
| – | separator | | | | |
| 5 | Add to collection ▸ | | always | true in every captured context; the rule is `there is a current font` | submenu: every collection (Favorites included) except the open one, sorted alphabetically, e.g. "Collection 1", "Favorites". Clicking one adds the selected faces (or the current font) to it |
| 6a | Remove from {collection} | | only when a collection is open (C6, C12, C13) | true in every captured collection context | removes the selected faces (or the current font) from the open collection. Example captions: "Remove from Favorites", "Remove from Collection 1" |
| 6b | Remove from collection ▸ | | when 6a is not visible | `some selected face (or the current font) belongs to a collection`. True only in C11 | submenu: the collections that contain the selection, sorted alphabetically |
| – | separator | | | | |
| 7 | Show duplicate fonts / Hide duplicate fonts | show-duplicates | always | always | toggle. Caption "Hide duplicate fonts" and the checked look (§2.2) while on (C9) |
| – | separator | | | | |
| 8 | Open font file location ▸ | | always | `exactly one face, or one family header, selected` (false in C5) | submenu: one item per font file. A family header lists every face's file; with duplicates shown, every copy is listed. Each item is the **full path in lower case**, middle-ellipsized to ≤ 400 px (e.g. `c:\windows\fonts\alger.ttf`). Clicking one opens Explorer with that file selected |

Family header (C2) **[decide]**: in the original, Uninstall and Delete are enabled in the menu but disabled
on the toolbar, although both use the same binding (`FontDS.UninstallEnabled`). AnyFont follows the menu: a
selected family header means "all faces of the family".

### 2.7 View

| # | Caption | Icon | Enable | Action |
|---|---|---|---|---|
| 1 | Refresh the Font List | refresh | `a manual refresh is needed`: the file system changed in a way the background watcher could not apply. Disabled in all 13 contexts | rescans the libraries |
| – | separator | | | |
| 2 | Show font properties / Hide font properties | font-properties | always | toggles the properties panel (§5.4). "Hide …" caption and the checked look while it is shown |

AnyFont adds four items here, between Refresh and Show font properties, because the dossier defines them but
the captured window never reaches them (§11).

### 2.8 Help

The original Help menu has only two items: &Product Help... and &About ... **[AnyFont]** AnyFont adds the
AnyVector items in the same places as AnyVector's Help menu (`AnyVector/crates/anyvector/src/ui/data.rs`):

| # | Caption | Icon | Action |
|---|---|---|---|
| 1 | &Product Help... | help (?) | opens AnyFont's local HTML help in the default browser. AnyVector's `help.rs` is the pattern |
| – | separator | | |
| 2 | Check for &Updates... | updates | Updates dialog (§7.2.15). The title-bar Updates button opens the same dialog |
| 3 | Submit Ideas and Feedback... | feedback | feedback form (§7.2.16) |
| 4 | Report a &Bug... | bug | bug report form (§7.2.16) |
| – | separator | | |
| 5 | &About AnyFont... | info | About box (§7.2.1) |

AnyVector uses "&Updates..." and a "Report a Bug ▸" flyout (Report a Bug... / View Reported Bugs...). The lead
asked for "Check for Updates..." and "Report a Bug..." as plain items, and this spec follows that.
"View reported bugs" stays available as the link in the bug-report dialog's footer, as in AnyVector
**[decide]**.

---

## 3. Toolbar

Band y 49–81 (33 px). Each button is a 24×24 hit rectangle with a 16×16 icon centred in it, vertically
centred in the band (y 53–76). Buttons have no captions; tooltips show instead (§3.2). Pitch is 32 px
(24 + 8 gap). A separator is a 1 px vertical line `line.strong`, about y 55–76, with 8 px space on each
side.

| # | x (hit rect) | Command | Icon (`crates/anyfont/assets/icons/`) | Kind | Enable (same as the menu) |
|---|---|---|---|---|---|
| 1 | 4–27 | Add Folder... | add-folder.svg | button | always |
| 2 | 36–59 | Add Collection | add-collection.svg | button | always |
| 3 | 68–91 | Get More... | get-more.svg | button | always |
| sep | ~100 | | | | |
| 4 | 109–132 | Remove Folder or Collection | remove.svg | button | library deletable (C6, C10–C13) |
| sep | ~141 | | | | |
| 5 | 150–173 | Install | install.svg | button | §2.6 #1 (off in all captures) |
| 6 | 182–205 | Uninstall | uninstall.svg | button | §2.6 #2. The capture shows it **off** in C2; AnyFont follows the menu (on) |
| 7 | 214–237 | Download | download.svg | button | §2.6 #3. Hidden when internet use is not allowed; the separator after it stays |
| sep | ~246 | | | | |
| 8 | 255–278 | Delete | delete.svg | button | same as Uninstall |
| sep | ~287 | | | | |
| 9 | 296–319 | Show duplicate fonts | show-duplicates.svg | toggle | always |
| sep | ~328 | | | | |
| 10 | 337–360 | Show font properties | font-properties.svg | toggle | always |

States:
- **Normal**: the icon only.
- **Hover**: rounded 24×24 `button.hot`, radius 3. Not captured **[decide]**; the proposal reuses `menubar.hot`.
- **Pressed**: `button.pressed`.
- **Toggle on**: rounded 24×24 `accent.checked` (#1d4470), radius 3, behind the icon.
- **Disabled**: the icon drawn with `icon.disabled` opacity.

Overflow: when the window is too narrow, the trailing buttons move into a `»` overflow button at the right end
(`crlfrmwk_overflowbar_horizontal`). It opens a popup menu with the hidden commands. At the 800 px minimum
everything fits.

### 3.1 Toolbar state matrix (as captured; ✓ = enabled)

| Command | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 | C9 | C10 | C11 | C12 | C13 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Add Folder / Add Collection / Get More | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Remove Folder or Collection | | | | | | ✓ | | | | ✓ | ✓ | ✓ | ✓ |
| Install | | | | | | | | | | | | | |
| Uninstall / Delete | | (✓ menu) | ✓ | | ✓ | | | ✓ | | ✓ | ✓ | ✓ | ✓ |
| Download | | | | | | | | | | | | | |
| Show duplicate fonts | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (on) | ✓ | ✓ | ✓ | ✓ |
| Show font properties | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (on) | ✓ | ✓ | ✓ | ✓ | ✓ |

### 3.2 Tooltips (header – body)

A tooltip has a bold header line, then a body line in normal text below it. If there is no body, only the
header is shown.

| Command | Header | Body |
|---|---|---|
| Add Folder | Add folder | Browse to a folder that contains fonts to add it to your Libraries. |
| Add Collection | Add collection | Add a collection for easy access to fonts that reside in different folders. |
| Get More | Get More | – |
| Remove | Remove | Remove a folder or collection from your Libraries. |
| Install | Install | Install fonts and font families. |
| Uninstall | Uninstall | Uninstall fonts and font families. |
| Download | Download | Download fonts to the AnyFont Content folder. **[AnyFont]**, the original names its own folder |
| Delete | Delete | Delete font files from your system. |
| Show duplicate fonts (off) | Show duplicate fonts | Show duplicate font versions. |
| Hide duplicate fonts (on) | Hide duplicate fonts | Hide duplicate font versions. |
| Show font properties | Show font properties | Show font details or glyph sets. |
| Hide font properties (on) | Hide font properties | Show font details or glyph sets. **[decide]** The hide-state body was not captured |

---

## 4. Title bar

AnyFont draws a custom title bar (no system caption), like AnyVector.

| Property | Restored | Maximized |
|---|---|---|
| Height | 32 | 25 visible (32 with 7 px off-screen) |
| App icon | 16×16 at client x 8–23, vertically centred | x 4–19 |
| Title text | starts 8 px right of the icon (client x 32 restored, 27 maximized), UI font, vertically centred | same |
| Window buttons (–, □/❐, ×) | 45 px wide each, full title-bar height, flush right | 39 px wide |
| Custom buttons (?, ⚙, Updates) | 50 px wide each, immediately left of minimize | 44 px wide |

Measured glyph centres, maximized at 1920: ? 1691, ⚙ 1735, third 1780, – 1823, □ 1862, × 1901. Restored
(window x 0–799 including the shadow): 529, 579, 630, 680, 725, 770.

| Element | Content | Action | Tooltip |
|---|---|---|---|
| Title | "AnyFont" **[AnyFont]** | double-click maximizes or restores; drag moves the window; right-click opens the standard system menu | |
| ? | question-mark glyph | Product Help (same as Help ▸ Product Help...) | Product Help |
| ⚙ | gear | Options (§7.2.4) | Options |
| Updates **[AnyFont]** | circular-arrow "updates" icon, which replaces the trial icon | Updates dialog (§7.2.15). Show a small `accent` dot on the icon when an update is available **[decide]** | Updates |
| – | minimize glyph | minimize | Minimize |
| □ / ❐ | maximize / restore glyph | toggle | Maximize / Restore Down |
| × | close glyph | close. Hover background `titlebar.close.hot`: the Windows red (#c42b1c) with a white glyph | Close |

Active and inactive looks. The capture was taken while the window was inactive: the icon was grey and the
glyphs dim (`titlebar.text.inactive`). The probe `pt3_active.png` shows the active look: the coloured app
icon and white text and glyphs (`titlebar.text`). Hovered custom and window buttons get a `titlebar.hot`
background over the full button rectangle.

The window resizes from the edges and corners with the standard 8 px invisible border. It supports Windows
snap layouts: hovering the maximize button asks for `HTMAXBUTTON`, as in AnyVector.

---

## 5. Panels

### 5.1 Libraries (top of the left panel)

| Element | Geometry | Details |
|---|---|---|
| Title "Libraries" | bold; text y 96–104 (cap), x 4 | |
| Section header "Folders" | text x 3, y 128–136, colour `accent` (#017dbc) | a 1 px `accent` line runs from 8 px right of the text to x 211, at the text's mid-height (y 132). Double chevron at x 222–230: ︽ when expanded, ︾ when collapsed. A click on the header or the chevron collapses or expands it |
| Row "My fonts" | y 144–168 (25 px), text x 4 | rows are 25 px apart. The selected row gets a rounded rect `row.selected` (#3c3c3c), x 0–237, radius 3 |
| User folder rows | below My fonts | caption = the folder's own name only (e.g. "fm-map-temp-folder"); tooltip = full path **[decide]** |
| Section header "My Collections" | text y 180–191, line at y 184 | same style as Folders |
| Collection rows | | "Favorites" is always present (text y 204–212). User collections follow, all sorted alphabetically (Collection 1, Favorites, …) |
| Splitter | y 292, x 0–238, 1 px `line.divider` | drag to resize the Libraries/Filters split. The Libraries part gets its own scrollbar when it overflows |

Tooltips:
- "My fonts": "Includes all installed fonts, all fonts in your added folders, and fonts downloaded from the
  Online library".

Behaviour:
- Clicking a row opens that library. Its first font is auto-selected, as in C10 and C12. Its name goes into
  the status text.
- Right-click selects the row and opens the Libraries context menu (§6.2).
- Fonts can be dragged from the list onto a collection row; that adds them to the collection. The original
  allows this: "To populate this collection, drag fonts from an added folder or another collection."
- Add Collection inserts the new row in alphabetical place and replaces it with an **inline name editor**
  (dialog 94dc2149). The editor is a 24 px tall edit box at x 2–235 with a 1 px `accent` border, containing
  "Collection {n}" fully selected. {n} is the lowest unused number. Enter or losing focus commits; Esc cancels
  and removes the new collection. An empty or duplicate name is refused: the editor stays open
  **[decide]**.

### 5.2 Filters (bottom of the left panel)

| Element | Geometry | Details |
|---|---|---|
| Title "Filters" | bold, text y 300–315 | |
| Clear filters button | funnel-with-X icon, x 218–234 | the X is grey (`icon.disabled`) when no filter is checked and red (`danger`) when filters are active. Tooltip header "Clear filters", body "Remove all selected font filters." Disabled when nothing is checked |
| Group header | text in `accent` plus a 1 px line and a ︽/︾ chevron, the same style as §5.1; 23 px apart | |
| Checkbox row | 26 px apart. A 12×12 box at x 4–15 (1 px border `check.border` #525252, fill `check.bg` #404040); label from x 19 | a checked row gets a rounded highlight `row.checked` (#525252) across the row and a white ✓ in the box |
| Scrollbar | shown when the groups overflow; same style as §1.3 | |

Groups and items, in order. Only Font Status is expanded by default; the others start collapsed **[decide]**,
since Filters (default).png shows Font Status open.

| Group | Items |
|---|---|
| Font Status | Installed, Not installed, Protected System Fonts, Duplicates |
| Embedding Rights | Editable, Installable, Preview and Print |
| Font Technology | OpenType - TrueType, TrueType, Variable Fonts |
| Weight | Light, Regular, Bold |
| Width | Condensed, Normal, Expanded |
| Style | Decorative, Display, Monospaced, Sans Serif, Script, Serif, Symbol |
| Character Range | Arabic, Chinese Simplified, Chinese Traditional, Cyrillic, Devanagari, Greek, Hebrew, Japanese, Korean, Latin, Turkish |
| OpenType | Alternate Annotation Forms, Asian Forms, Asian Vertical Metrics, Asian Widths, Capital Spacing, Capitals, Case-Sensitive Forms, Contextual Alternates, Contextual Ligatures, Discretionary Ligatures, Fraction, Historical Forms, Horizontal Kana Alternates, Historical Ligatures, Number Styles, Ordinals, Ornaments, Position, Slashed Zero, Standard Ligatures, Stylistic Alternate, Stylistic Sets, Swash Variants, Vertical Alternates and Rotation |

Semantics: items in the same group combine with OR, and groups combine with AND. The filtering itself belongs
to `anyfont-core`; this spec covers only how it looks. A group header shows no count. Filters apply to the open
library, which adds " (filtered)" to the status text.

### 5.3 Font list

Geometry: x 245–1902, y 124–1038. It scrolls vertically, and the scrollbar is the one in §1.3.

**Grouping.** "Group by family" is on by default:
- A family with **two or more** faces gets a **family header row**: the family name in Segoe UI Light about
  20 pt (27 px), text x 247, about 72–76 px tall, with no small label.
- Its faces follow, **indented**: the small label starts at x 285 instead of x 247.
- A family with **one** face has no header row; that face is not indented.
- A 1 px `line.strong` separator is drawn between families, e.g. at y 351, 427, 503.

**Face row** (default preview size):
- Pitch 76 px.
- Small label: UI font, cap top at row top + 12. Format `{family} ({style}, {width})`, e.g. "Arial (Regular, Normal)".
- Preview: the sample text rendered **in the face itself** at the preview size. Baseline area starts at row top
  + ~36; the default sample is the font's name. The default preview size has a cap height of ~25–26 px for
  Arial, i.e. a ~36 px font size.
- Selected row background: `row.selected.list` (#525252), across the whole list width x 245–1902.
- Hover **[decide]** (not captured): proposal `row.hot` (#333333).

Status markers at the right of each face row:

| Marker | Geometry | Meaning |
|---|---|---|
| Installed bar | x 1887–1900 (14 px wide), inset 4 px top and bottom, colour `status.installed` (#66bf83) | the font is installed |
| Lock | drawn over the bar, x 1890–1897 | protected system font |
| Technology icon | x 1860–1878 | italic "O" = OpenType (CFF or TrueType outlines); "TT" = TrueType |
| Not installed | **[decide]** not captured. Proposal: no bar | |
| Online (catalog) font | **[decide]** not captured. Proposal: a cloud glyph in place of the bar; tooltip "Online font (Install or download to use)" | |

**Preview size.**
- The small and big A buttons step the size by 1 px; the row pitch becomes 75/76/77.
- The slider maps linearly to the size. **[decide]** The range is unknown. The default thumb sits at 36 % of
  the track, which a range of 8–86 px fits.
- The value is remembered in the settings.

**Selection.**
- Click selects one row, Ctrl+click toggles a row, Shift+click selects a range.
- Clicking a family header selects only the header row: that row is highlighted, not its faces, as in C2. The
  commands then apply to the whole family.
- Ctrl+A selects everything, highlighted as one continuous band (C5).
- After Deselect All the current font keeps its highlight (C1).
- Double-clicking a family header did nothing visible in the capture. AnyFont does nothing either
  **[decide]**.

**Hover tooltip** (dialog 5a98a223), after the standard tooltip delay, lists:
1. the font name (bold);
2. a status icon plus "Installed", "Not installed" or "Online font (Install or download to use)";
3. "Protected System Font" if the font is protected;
4. a duplicate icon plus "Duplicate(s):" followed by the paths of the other copies, when duplicates exist.

**Search.**
- Typing in Search filters live by family and face name.
- While a search is active, the list is **flat**: no family headers and no indentation.
- Ordering is by relevance: exact or prefix matches of the family name first, then the rest. In the capture
  "Arial" listed Arial Narrow first, which suggests length or prefix ordering **[decide]**.
- Status: "My fonts (filtered): 10 fonts".

**Sample text combo items**, in order:
- Font Name
- abcdefghijklmnopqrstuvwxyz
- ABCDEFGHIJKLMNOPQRSTUVWXYZ
- 1234567890
- The quick brown fox jumps over the lazy dog.

Typing free text uses that text as the preview for every font. Clearing the box goes back to Font Name.

**Empty states.** Centred in the list, italic UI font, `text.secondary`:

| When | Text | Extras |
|---|---|---|
| Filters active, no match | "No fonts found. Remove or change filters or explore the font packs available in Get More." | two icon buttons below: Clear filters (funnel) and Get More (store) |
| Filters active, no match, no internet | "No fonts found. Remove or change filters to show fonts." | Clear filters icon |
| Search active, no match | "No fonts found. Clear the Search box, type a different keyword, or explore the font packs available in Get More." | Get More icon |
| Search active, no match, no internet | "No fonts found. Clear the Search box, or type a different keyword." | |
| Empty collection | "To populate this collection, drag fonts from an added folder or another collection." | |

**Information strip** (`f0f62460`, not captured; see §11 #5): a strip above the list. It shows an
information icon, the information string and a "Refresh" button. It is visible only when a manual refresh is
needed.

**Recently used list** (`daa659c7`, not captured; see §11 #2): when "Show recently used fonts" is on, the most
recently used fonts appear above the main list, followed by a 1 px separator.

### 5.4 Font properties panel

Toggled by View ▸ Show font properties, by the toolbar button or by Ctrl+Alt+I. Hidden by default. The panel
is x 1649–1919, 271 px wide (the panel PNGs are 276×993 including borders). The list shrinks: its installed
bar moves to x 1612–1625 and its scrollbar to x 1628–1644.

#### 5.4.1 Tab strip

Three 24×24 icon toggle buttons at y 89–112, x ~1652, ~1685, ~1718 (33 px pitch). The selected tab gets a
`accent.checked` (#1d4470) background with radius 3. Glyphs is the default tab.

| Tab | Icon | Tooltip |
|---|---|---|
| Glyphs | lowercase bold "g" | Preview and filter available glyphs for a selected font. |
| Font info | "A" with a small "i" badge | Show detailed font information. |
| Variable font | "AA" with an underline | Adjust variable font properties. |

#### 5.4.2 Glyphs tab

Top part, "Glyph Filters":
- A bold title at y ~148, with a clear-glyph-filters funnel at x ~1904. Tooltip "Remove all selected glyph
  filters."
- Group "Common", in the header style of §5.1. The capture truncates it to "Comm..."; AnyFont shows it in
  full **[decide]**. Its checkbox rows start at y 198, 26 px apart: Currency, Mathematical Symbols, Numbers,
  Punctuation & Separators, Symbols. "Marks" also appears when the font has combining marks; Bahnschrift shows
  it.
- Group "Scripts": the Unicode scripts present in the font (for example Latin, Greek, Cyrillic), collapsed.
- The filter area has its own scrollbar.

A splitter at y 322 separates it from the lower part.

Lower part, "Glyph Preview":
- A bold title at y ~337.
- A grid of the font's glyphs, rendered in the font, 6 columns of ~41 px square cells with no grid lines.
- The selected cell is marked by 1 px `accent` lines on its top and bottom edges. The first cell is the space,
  drawn as a green ⎵ marker.
- A down-chevron "more" affordance sits at the bottom.
- The grid has its own scrollbar.
- When more than one font is selected, the grid is replaced by the centred italic text "Multiple fonts are
  selected. To view a glyph set, choose only one font."
- Other empty states, in the same style: "No matching glyphs" (the glyph filters exclude everything) and
  "Install or download the font to preview glyphs." (an online catalog font that has not been downloaded).

Panel status strip, at the status-bar height:
- "{n} glyphs" at x 1652 (e.g. "262 glyphs", "986 glyphs"), counting the glyphs left after the glyph filters.
- Its own small A (x ~1780), slider (x 1816–1879) and big A (x ~1903) change the glyph cell size. Tooltip:
  "Change the glyph size in the Glyph Preview area."

#### 5.4.3 Font info tab

Two columns: labels right-aligned ending at panel x ~108 and followed by a colon, values left-aligned at panel
x ~118. Rows are ~34 px apart. A value wraps within the panel width. File Path is end-ellipsized; its tooltip
is the full path.

| Label | Example value |
|---|---|
| Family: | Algerian |
| Sub-family: | Regular |
| File Path: | c:\windows\fonts\alger.ttf (lower case) |
| File Size: | 74.8 KB / 353 KB (1 decimal under 100 KB, otherwise none **[decide]**) |
| Version: | Version 1.57 (the name-table string as is) |
| Font technology: | OpenType - TrueType / OpenType - PostScript / TrueType |
| Code Page: | one per line, e.g. "1252  (ANSI - Latin I)", "1250  (ANSI - Central Europe)" … "1258  (ANSI/OEM - Viet Nam)" (two spaces after the number) |
| Italic: | No / Yes |
| Weight: | Normal / Light / Bold / … |
| Width: | Extra-condensed / Condensed / Normal / … |
| Embedding: | Editable / Installable / Preview and Print / No Embedding |
| Manufacturer: | URW++ |
| Designer: | (empty when absent) |
| Copyright: | URW Software, Copyright 1993 by URW (wraps) |
| Collection: | the collections that contain the font, comma-separated |
| *(vendor link)* | the vendor URL as a hyperlink button below Collection, visible only when the font has one (§11 #8) |

When several fonts are selected, the tab shows the current font **[decide]**.

#### 5.4.4 Variable font tab

- No variable font selected, or several fonts selected: two centred italic lines. "You don't have a variable
  font selected, or you've selected multiple fonts." and, below it, "Choose one variable font to see and
  adjust its properties."
- One variable font: one row per axis. Each row has:
  - the axis name ("Width", "Weight", …),
  - a slider from the axis minimum to its maximum,
  - a spin box with 1 decimal ("75.0", "300.0").

  Changing a value re-renders that face's preview in the list. The value is not saved to the font. No named
  instances list is shown, matching the capture.

---

## 6. Context menus

The popup geometry is §2.2.

### 6.1 Font row (list)

The right-click target decides which item list the menu shows:
- a face row,
- a family header,
- a protected font,
- a multiple selection.

Whatever the target, the menu is the **Font menu (§2.6)**, with the same items, captions and enable rules.
Right-click on an unselected row selects it first; right-click inside the selection keeps it. Captured
variants:
- Family header: Uninstall and Delete enabled; Open font file location lists every face file.
- Single face: as §2.6.
- Protected system font: Install, Uninstall, Download and Delete disabled; Add to collection, Show duplicate
  fonts and Open font file location enabled.
- All selected: Open font file location disabled.
- In a user folder: Remove from collection ▸ is enabled when the face is in a collection.
- In a collection: "Remove from {collection}" replaces the submenu.

### 6.2 Libraries row (command bar 41be3b85)

| # | Caption | Icon | Enable |
|---|---|---|---|
| 1 | Install | none | the library contains at least one font that is not installed (My fonts: off; Favorites: off) |
| 2 | Uninstall | uninstall | the library contains at least one installed, unprotected font (My fonts: on; Favorites, empty: off; user folder or collection: on) |
| 3 | Remove Folder or Collection | trash | library deletable (My fonts: off; Favorites: **on**; user folder or collection: on) |
| 4 | Rename **[AnyFont, §11 #9]** | none | visible for user collections only; opens the inline editor |

Right-click selects the row first. The menu installs or uninstalls **every** font of that library, with the
progress dialog.

### 6.3 Text boxes

The sample text combo and the Search box use the standard edit context menu. In the original that is the
Windows one, drawn light. AnyFont draws it with the theme:
- Undo
- separator
- Cut, Copy, Paste, Delete
- separator
- Select All

The Windows-only items are omitted **[decide]**: Right to left Reading order, Show Unicode control characters,
Insert Unicode control character ▸, Open IME, Reconversion.

### 6.4 No context menu

These have no context menu in the original, and AnyFont adds none:
- section headers;
- the empty Libraries area;
- filter headers and rows;
- the status bar;
- the menu bar and the toolbar;
- the glyph grid and glyph filters.

The title bar gets the standard Windows system menu.

---

## 7. Dialogs

### 7.1 Common dialog frame

Every dialog has:
- a custom title bar in `dialog.titlebar.bg`, 32 px tall, with the title text at x 12 and a single × button;
- a background of `dialog.bg`;
- 1 px `dialog.border`;
- a body padding of 16 px.

Buttons:
- 24–25 px tall, at least 80 px wide, with radius 2.
- The **default button is blue**: `button_default_bg` fill (#1D4470) with a 1 px `button_default_border`
  (#017DBC) and `text` caption.
- Other buttons use `button.bg` with a 1 px `button.border`.

Esc and × mean Cancel, or Close when there is no Cancel. Enter triggers the default button. Dialogs are modal
to the main window unless noted. Sizes below include the title bar.

### 7.2 Dialogs AnyFont shows

#### 7.2.1 About AnyFont (5d13ff7a) – 579×386

- Title "About AnyFont".
- A black banner across the top with AnyFont's logo (brand SVG). Centred under it:
  - "Version {version} - {month year} Release", e.g. "Version 0.1.0 - November 2026 Release";
  - "(C) 2026 AnyFont Team.  All rights reserved." (two spaces, as in the original).
- A bordered box with "Source ID:" and the build id.
- Buttons along the bottom: "&System Info..." (left), "Legal &Notices...", and "Close" (right, default).

The geometry is AnyVector's `about.rs` copied as is (the same dialog size).

#### 7.2.2 System Info (7ed05b73) – 599×411

- Title "System Info".
- "C&hoose a category:" with a combo box 262 px wide.
- A two-column list (name, value) filling the dialog.
- Buttons "Close" (default) and "&Save".

Category "System" rows, as captured:
- Platform
- Processor
- Number of Processors
- Math Coprocessor
- Total physical memory
- Available physical memory
- Total virtual memory
- Available virtual memory
- Total memory used by app
- GDI Object Count
- …

**[decide]** The other categories were not captured. Proposal (AnyVector's list without Printing):
- System
- Display
- AnyFont EXEs & DLLs
- System DLLs

Save writes all categories to a .txt file chosen in a Save dialog. It then shows "System info was saved to
{}." or "Unable to save system info to {}." in a message box.

#### 7.2.3 Legal Notices (2d920b4a) – 514×739

- Title "Legal Notices".
- A white, scrolling, read-only text pane: the third-party licence notices (`docs/phases/11-third-party.md`).
- Buttons "&Print" (left; prints the text) and "OK" (right, default).

The original's "Show CloudConfig.json" button is omitted (cloud config).

#### 7.2.4 Options (ab1e1683) – 464×383 (title bar ⚙)

- Title "Options".
- Header **Appearance** (bold, larger).
- Label **Size** (bold). Below it, one row:
  - a small-square icon;
  - a slider with 7 ticks;
  - a large-square icon;
  - a spin box "100.0";
  - "%";
  - a "&Reset to Default" button.

  Slider and spin box are bound together to the UI scale. **[decide]** The range was not captured. Proposal:
  75–225 % in 25 % ticks; 100 % falls on the 2nd tick, as captured.
- The hint text, wrapped: "Hint: The scaling level is relative to the scaling level of the operating system.
  For example, scaling by 200% will make the application user interface appear twice as large as the UI of
  the operating system. Note that excessive scaling may cause certain workspace elements to appear cropped."
- Header **Color** (bold), then two rows:
  - "Theme:" with a combo of Light / Medium / Dark / Black. The default follows Windows' app mode: Dark when
    `AppsUseLightTheme` = 0, Light otherwise.
  - "Window border:" with a colour-swatch combo. **[decide]** Its values were not captured. Proposal: the
    first item is the theme's default border, then the Windows accent colour, then a small fixed palette.
- A separator, then "OK" (default, right).

Changes apply immediately, with no Cancel, as in the original. A scale change applies at once in egui; the
original's "You need to restart the application to apply these changes." message is used only if a setting
really needs a restart. Theme tokens: §10.

AnyFont additions (§11) go into this dialog as further headers below Color, in the same style.

#### 7.2.5 Remove confirmation – 421×199

- The title is the app title, "AnyFont".
- A warning icon at the left, with the text "Are you sure you want to remove this collection?" or "Are you
  sure you want to remove this folder?".
- A checkbox "Do not show this message again". When it is checked, Remove no longer asks.
- Buttons "&Yes" (default, blue) and "&No".

#### 7.2.6 Native file dialogs (`rfd`)

| Use | Kind | Title | Filter / fields |
|---|---|---|---|
| Add Folder... | folder picker | "Add Folder" | Windows shows "Folder:", "Select Folder" and "Cancel" itself |
| Import Folders and Collections... | open | "Open" (Windows default) | "XML - Extensible Markup Language (*.xml)" |
| Export Folders and Collections... | save | "Save As" (Windows default) | same filter; default name **[decide]**, proposal "AnyFont Folders and Collections.xml" |
| System Info ▸ Save | save | "Save System Info" | "Text (*.txt)" |

Errors after Add Folder:
- "Folder not found. Please choose another location."
- "This folder has already been added."

Both appear in a message box (§7.2.14).

#### 7.2.7 Get More – online font catalog [AnyFont] (736dc77a frame) – 1058×652, resizable, min width 1044

The original hosts a store page here; AnyFont reproduces the frame with Google Fonts content.

- Title "Get More".
- Top row: a search box with a clear × button, a "Search" button, then "Sort by:" with a combo (Featured /
  Name / Newest / Popular **[decide]**; "Featured" is the default caption).
- Left and centre, the result list. Each row is ~100 px tall and has:
  - a 165×78 thumbnail: the family name rendered in the family;
  - the family name in bold;
  - the category in grey, where the original shows "Font Pack" (e.g. "Sans Serif");
  - a one-line description: designer and number of styles;
  - on the right, where the original shows the price, "Free", plus a "Download" button. The button reads
    "Downloaded" (disabled) when the family is already in the Content folder.
- Right, a "Filter by:" panel of checkboxes:
  - the categories (Serif, Sans Serif, Display, Handwriting, Monospace);
  - "Free" (always checked, disabled);
  - "My Library" (families already downloaded).

The original's Filter by list was New, …, Fonts, …, Free, My Library.

Downloads:
- Files go to `%USERPROFILE%\Documents\AnyFont\Content\Fonts\<Family>\`.
- They show the operation progress dialog (§7.2.12) with "Downloading {}... {}%".
- Downloaded fonts appear in My fonts as **not installed**.
- With no connection, the list area shows "Remote font catalog cannot connect to the internet."

The Download toolbar command works on catalog fonts that are listed in the font list but not downloaded yet.
**[decide]** Whether undownloaded catalog fonts appear in My fonts at all is open. The original's My fonts
tooltip mentions "fonts downloaded from the Online library", and the status text "Online font (Install or
download to use)" exists. Proposal: no, only downloaded ones. Then Download stays disabled, as captured, and
Install on a catalog font is reached through Get More.

#### 7.2.8 Inline collection name editor (94dc2149)

See §5.1.

#### 7.2.9 Font tooltip (5a98a223)

See §5.3.

#### 7.2.10 Reboot prompt (8455a514) – min width 370

- The title is the app title.
- The prompt text (FontInteractionDS.UsedPrompt):
  - for several fonts: "The following fonts cannot be uninstalled because they are in use by one or more
    applications. To uninstall these fonts, you must first restart your computer.\n\nDo you want to restart
    your computer now?\n\nClick Ignore to continue without uninstalling the listed fonts.";
  - for one font, the singular variant: "The following font cannot be uninstalled because it is in use…",
    see the TSV.
- Below it, a list box of the fonts in use.
- Buttons:
  - "Reboot &now" (default): schedules the deletion with `MoveFileEx` and restarts Windows after confirming;
    sandbox backend only in tests;
  - "Reboot &later": schedules it, no restart;
  - "&Ignore": visible only in the simple layout; skips those fonts;
  - "Cancel".

#### 7.2.11 Delete fonts (a8b8f4b7) – min width 420

- Title "Delete fonts".
- Text: "Are you sure you want to delete this font?\n\nRead-only fonts cannot be deleted."
- A checked list of the files to delete (each item is the file path) and a "Select all" checkbox above or
  below it.
- A "Do not show this message again." checkbox.
- Buttons "OK" (default) and "Cancel".

The files go to the Recycle Bin. Installed fonts are uninstalled first.

#### 7.2.12 Operation progress (23e365df) – min width 400, modal, no title-bar ×

- One line of text, one of:
  - "Installing {}... {}%"
  - "Uninstalling {}... {}%"
  - "Downloading {}... {}%"

  where the first {} is the current font name.
- A progress bar.
- One button whose caption is "Cancel" while running and "Close" when done (or failed). Cancel stops after the
  current font.

#### 7.2.13 "We can't find the following:" (fdea122f) – min width 400

- The title is the app title.
- The text "We can't find the following:", a list box of the missing folders or font files from an import,
  and "OK".

#### 7.2.14 Message box (9da43edc)

The generic message box:
- the title (caption);
- an icon (information, warning or error);
- a bold main instruction (optional) and the message text;
- an optional "Do not show this message again" checkbox;
- buttons from &Abort, &Retry, &Ignore, &Yes, &No, &Try Again, &Continue, OK, Cancel, in the Windows order.

All AnyFont errors and confirmations use it. The texts are in the TSV, e.g.:
- "Your font database has been corrupted. To reset the database, you must restart AnyFont while holding down
  Shift."
- "Supported font formats: OTF, TTF" **[decide]**: the format list of AnyFont's parser.

#### 7.2.15 Updates [AnyFont] (AnyVector `updater.rs`)

This is the dialog behind Help ▸ Check for Updates... and the title-bar Updates button. Its layout, behaviour
and strings come from AnyVector, with "AnyVector" replaced by "AnyFont" and the "save your drawings" clauses
removed.

- Title "Updates".
- "You are using AnyFont {}."
- A status line: "Checking for updates...", "AnyFont is up to date.", "AnyFont {} is available.",
  "Downloading AnyFont {}..." or "AnyFont {} is ready to install.".
- "Allow product updates:" with a combo of:
  - Keep the product up to date (recommended)
  - Check for updates and let me choose when to install them
  - Never check for updates

  Each mode has a description line below the combo.
- Buttons "Release Notes", "Check Now", "Install Now" and "Close".

#### 7.2.16 Report a Bug / Submit Ideas and Feedback [AnyFont] (AnyVector `bugreport.rs`)

AnyVector's two forms, unchanged except for the product name. They send through the `anyfont-issues`
relay.

#### 7.2.17 PANOSE Font Matching Preferences (52600a8b), not captured, §11 #13

- Title "PANOSE Font Matching Preferences".
- Radio buttons "&Text", "Text and &styles" and "&Never show": the matching mode.
- "&Default font:" with a font combo of the installed fonts.
- An "&Exceptions..." button.
- Buttons "OK" and "Cancel".

#### 7.2.18 PANOSE Font Matching Exceptions (b3d42473), not captured

- Title "PANOSE Font Matching Exceptions".
- "&Missing Font:" with an edit box; "&Substituted Font:" with a font combo.
- Buttons "&Add Exception" and "&Remove Exception".
- "&List of current exceptions:" with a two-column list (Missing Font / Substituted Font).
- Buttons "?", "OK" and "Cancel".

#### 7.2.19 PANOSE Font Matching Results (b12fbfa5), not captured

- The title comes from the data source (DialogTitle). Proposal: "PANOSE Font Matching Results".
- The directions text, and a table of missing fonts with their proposed matches.
- While the matching runs:
  - "Working, please wait..." with a "Stop" button;
  - the note "Stopping will end the PANOSE matching process. All missing fonts will revert to a default
    selection.".
- Radio buttons "&Use the PANOSE suggested match" and "Substitute &font with:" with a font combo.
- Radio buttons "Show fonts with the same code page" and "Show &all fonts". **[decide]** The first caption is
  bound (`ShowSameCodePageString`) and its text was not found.
- A checkbox "&Save this exception for future font matching".
- "Make this substitution:" with "&Permanent - keep these fonts" and "Te&mporary - use original fonts".
- Buttons "?", "OK" and "Cancel".

#### 7.2.20 Type 1 fonts detected (3a3106a3), not captured, §11 #14

- A header "Type 1 fonts detected" and the dossier text, see the TSV.
- A "Help" button, a "Do not show again" checkbox and a "Replace fonts" button (which opens §7.2.19).

#### 7.2.21 Other non-captured pop-ups AnyFont implements

| Dialog | Use |
|---|---|
| Rename collection (c7ef0b94) | fallback for §5.1 when inline editing is not possible: "Name:" with an edit box, then OK and Cancel |
| Search suggestions (ab5f3dbb) | a drop-down under the Search box listing matching font names while typing. Min width 240 |
| Glyph alternates (e4939644) | a pop-up grid of the alternate glyphs of the clicked glyph (OpenType `aalt`, `salt`, `ssXX`) |
| Glyph filter list (5edfdeaa) | the glyph filter checkbox list with "Close", used when the panel is too short to show the filters inline |
| Variable font controls (c5ad4625) | the content host of §5.4.4 |
| Toolbar overflow (crlfrmwk_overflowbar_horizontal) | §3 |

### 7.3 Dialog map: every `06-ui/dialogs/*.md`

"Yes" means AnyFont implements the dialog, with the section that specifies it. "No" gives the reason. Almost
every "no" is a dialog of the shared text, colour, workspace or cloud framework that sits in the same
`FontManagerUI.dialogs.xml` but belongs to the suite's drawing application.

| File | Data source(s) | Used by AnyFont |
|---|---|---|
| 03bd785e-1553-5482-4022-ab498ccf277e.md | WTextColumnsSettingsDS | no: text-frame columns of the drawing app |
| 0885e5dd-9076-45cb-8c37-5ead6850608c.md | WEncodeTextDS | no: text encoding of document text |
| 0c524431-6b56-2cbe-451a-1ce1405a9416.md | WTextColumnsSettingsDS | no: text columns |
| 12148782-68fa-05b3-429b-0b54eefacf3a.md | (generic) | no: empty flyout host with a close button; egui pop-ups replace it |
| 1ad8588c-e5dd-6d91-449e-fcc68d8bb87e.md | WTxtAttributeDS | no: stylistic-set picker for text editing (AnyFont shows OpenType features only as filters) |
| 1d664666-017e-c998-4e4f-21e8943d0a95.md | WTextLanguageDS | no: language tagging of document text |
| 23e365df-cd58-0ab5-4287-bbdd84091ab1.md | FontDS | **yes**: operation progress, §7.2.12 |
| 25768fbb-e2a1-492c-8645-f4b0cf714213.md | WTxtAttributeDS | no: hyphenation settings |
| 2694c94d-b6f8-5a82-4fc5-e5a2d14c6163.md | PostScriptOptionsDS | no: PostScript halftone (printing) |
| 27880e9e-ef74-a080-4e2a-9fb6d0a2319f.md | CustomizationDS | no: suite table viewer (Export to CSV/Print) |
| 286ca4ae-4878-4987-93c5-b8ec474ea99a.md | WExportWorkspaceDS | no: workspace export; AnyFont has no workspaces |
| 2d48c59a-f302-87ac-4ea3-b6776e3b65ed.md | WCuiColorSystemDS | no: colour management |
| 2d54d047-09ec-53bd-46dd-f69978328508.md | WCuiPaletteEditorDS | no: palette editor |
| 2d920b4a-8a45-c790-43d5-b10f4e78f2c1.md | HelpMenuDS, LegalNoticeLicenseHtmlDS | **yes**: Legal Notices, §7.2.3 (without Show CloudConfig.json) |
| 2df4fb6b-e62d-8893-4569-dc539dc3468a.md | WTxtTabSettingsDS | no: text tabs |
| 2eabb345-8c80-4e03-b885-c415e6a3478c.md | WFontPlaygroundDockerDS | no: the drawing app's Font Playground docker; the font list's sample text covers previewing |
| 34a7f1f6-a327-4699-8db2-cce50202a097.md | AssetPropDialogDS, EffectPresetsDS | no: effect preset properties |
| 3a3106a3-b7f4-40c7-9747-9e7fe412dcc4.md | Type1InfoMessageDS | **yes (adapted)**: §7.2.20, §11 #14 |
| 3ac3cdf0-9725-43c1-8a8d-cf4fccf5509d.md | FontExportDS | no: "export drawing as a font" (creates a font from artwork) |
| 3ad5bb73-b4f1-7ba7-4555-a1a452df306b.md | WComboBoxExDS, WCuiPaletteEditorDS | no: palette editor |
| 42705e2c-1a04-e29c-45e4-845f17b68e84.md | WTextStatisticsDS | no: document text statistics |
| 444e48f7-082c-4435-bbea-7fbe886b0052.md | WQuickCustomizeDS | no: toolbar customisation; AnyFont's toolbar is fixed |
| 466623da-0570-2b87-4834-c465964aee78.md | WGlyphDockerDS | **yes**: Glyph Preview, §5.4.2 |
| 4da5ff35-85ed-47c8-9879-1325f0d9c559.md | HelpMenuDS | no: CloudConfig.json viewer (cloud) |
| 52600a8b-3a61-4351-a940-f65d08e2beec.md | WPanosePreferencesDialogDS | **yes**: §7.2.17 |
| 5374ecc3-e0da-4b77-8ff8-d48cc697086e.md | DownloadProgressDS | no: document download progress of the suite; AnyFont font downloads use 23e365df (§7.2.12) **[decide]** |
| 5a98a223-65b4-4a5c-9154-4d9f4b65b4ae.md | FontDS | **yes**: font row tooltip, §5.3 |
| 5c3ab2b3-9ea8-228b-449b-02fa326de81e.md | WTextColumnsSettingsDS | no: text columns |
| 5cb62f4c-eb42-9f88-4f59-faecbf7bd1dc.md | WCuiMonitorWarningDS | no: monitor colour-profile warning |
| 5d13ff7a-78a6-6dbe-437c-4325cea9c43e.md | WAboutBoxDS | **yes**: About, §7.2.1 |
| 5dacc24f-8b1d-4201-8c57-7873d75f5a66.md | WTxtAttributeDS | no: Far-East line-breaking rules |
| 5edfdeaa-e391-6e89-4ca0-6ba0e602f4ae.md | WGlyphDockerDS | **yes**: glyph filter list pop-up, §7.2.21 |
| 61d6e90c-a21e-7fb1-4221-9b0e7261e555.md | WCuiPresetNameDS | no: colour preset name |
| 61f63386-3177-f5b9-4d43-4a4d5235d8c9.md | (generic) | no: empty flyout host (min width 260) |
| 693c607c-89fa-48dc-9320-8d19e649470d.md | FontExportDS | no: export drawing as a font |
| 6ac5f0e4-7e03-4028-8938-e27e61f91a3b.md | WTxtAttributeDS | no: character spacing of text |
| 6b2fb73d-834a-401f-8b7f-64f07d3f27b3.md | WCuiColorSystemDS | no: colour management |
| 6bec897f-39f3-4f19-9d1a-41b3f275318c.md | ProjectTimerDS | no: project timer |
| 70217800-53a9-4867-a929-189352187758.md | ColorSpecDS, PaletteManagerDockerDS | no: palette manager |
| 736dc77a-b8b1-409b-a8e3-91e4a78bcedd.md | WWelcomeScreenDataSource | **yes (replaced)**: Get More frame, now the Google Fonts catalog, §7.2.7 |
| 7446df46-0b6c-480c-99ae-a45dc7c14e0b.md | NewWorkspaceDS | no: workspaces |
| 76eca627-b86e-4432-912d-fdda314e9da9.md | SpotColorPaletteMapping | no: spot colours |
| 7849430a-5de0-e5a6-44f7-b1513fde74bb.md | WTxtAttributeDS | no: stylistic sets of text |
| 7892ab4b-120c-4726-8219-fc0e3b1ac356.md | DownloadProgressDS | no: modal document download (see 5374ecc3) **[decide]** |
| 78c60577-9afa-44b3-94ad-e66e1faa9454.md | WImportWorkspaceDS | no: workspaces |
| 798a8920-5de0-4015-8600-f28fba525fdd.md | ViewHostDS | no: docker list; the only "docker" (font properties) has its own toggle |
| 7a71a4bd-c6cd-43cc-8819-026d35ddf1f9.md | WPasmDataSource | no: cloud notification settings |
| 7ab12eef-97ad-4348-8360-261090fb8786.md | WOpenTypeSuggestionsDS | no: on-canvas OpenType suggestions |
| 7ed05b73-d6f2-7182-4b12-24d93a6e7e67.md | WSysInfoDS | **yes**: System Info, §7.2.2 |
| 804db361-6f57-d4a3-429c-b3d03ef2e06a.md | WChangeCaseDS | no: change case of text |
| 8455a514-0997-419d-8da4-127d5a045d74.md | FontInteractionDS, AppDS | **yes**: reboot prompt, §7.2.10 |
| 8512ff59-b0c6-4b16-8603-8e76c0c41fe3.md | AssetPropDialogDS | no: asset properties (title, tags) of the content manager |
| 86172d10-7ed8-4996-8197-ce58b2ce8014.md | WTxtAttributeDS | no: underline settings |
| 8bca1676-d15b-cfba-4957-aa310d6c598b.md | WTxtTabSettingsDS | no: text tabs |
| 900f19cd-4647-475c-a190-b173e4b0c057.md | CrashReportDialogDS | no: Crashpad crash report; replaced by AnyFont's Report a Bug (§7.2.16) |
| 94078ed0-1d04-44ce-8753-d433fa617e65.md | LowMemoryDialogDS | no: low-memory warning of the drawing app (unsaved work) |
| 94dc2149-751e-e0b3-475b-a7fa76f19ca8.md | AssetListDS | **yes**: inline collection name editor, §5.1 |
| 9541a75a-0b51-0794-4a08-5de6f40b946e.md | (generic) | no: empty flyout host (min width 235) |
| 95486fb3-e4c7-48c5-8765-2f547e60ee2c.md | PaletteMgr | no: "more palettes" |
| 9da43edc-3d0e-04ab-4e25-f6bb51a28ad0.md | MessageBoxDS | **yes**: message box, §7.2.14 |
| a7073fc7-1b18-e7ab-4ff0-91d4159909bc.md | WTxtTabSettingsDS | no: text tabs |
| a8b8f4b7-1089-44b7-af52-3d5ceeaa70f9.md | FontInteractionDS | **yes**: Delete fonts, §7.2.11 |
| ab1e1683-2394-92bd-4126-2b48e5f917d9.md | CustomizationDS | **yes**: Options, §7.2.4 |
| ab5f3dbb-4459-46d2-8511-83d49ab22e8d.md | AssetListDS | **yes**: search suggestions, §7.2.21 |
| ab9314a5-151f-05bc-44f7-545acc145137.md | WCuiColorByNameDS | no: colour by name |
| b12fbfa5-1c52-4c07-9581-d1f1d33e99fa.md | WFontMatchingResultsDS | **yes**: §7.2.19, §11 #13 **[decide]** on the entry point |
| b2b57a74-db68-4cad-82af-e18838ca5079.md | ProgressBarDialogDS | no: generic suite progress; 23e365df covers AnyFont |
| b36dc3a7-f7dd-4dcc-8959-07c214d16fd7.md | WCustomHyphenationDialogDS | no: custom hyphenation |
| b3d42473-3552-4f0c-8590-d7fdf81e7681.md | WPanoseExceptionDialogDS | **yes**: §7.2.18 |
| b6de2e23-9311-447e-8064-53c2fbb8d93b.md | WTxtAttributeDS | no: drop cap |
| b8517de3-ee2e-49fc-a4f7-315632ea3268.md | UploadFontsDS | no: uploads fonts to the vendor's cloud; AnyFont has no cloud account **[decide]** |
| be8c7788-2152-4b4b-be9e-d7d678f78c95.md | WTxtAttributeDS | no: justification settings |
| c5ad4625-866c-4ad1-91a3-ba2fca720bc0.md | WTxtVariableFontDS | **yes**: variable font controls, §5.4.4 |
| c7ef0b94-6c11-43ff-8e64-bd83ca230c0d.md | RenameCollectionDialogDS | **yes**: rename collection, §7.2.21, §11 #9 |
| cfe05aaf-7f35-79b1-40e2-519cb7850e71.md | WTxtTabSettingsDS | no: text tabs |
| crlfrmwk_breadcrumbs_popup.md | BreadcrumbsContDS | no: breadcrumb bar of the content manager |
| crlfrmwk_overflowbar_horizontal.md | WQuickCustomizeDS | **yes**: toolbar overflow, §3 |
| crlfrmwk_overflowbar_vertical.md | WQuickCustomizeDS | no: AnyFont has no vertical toolbar |
| d2ed3e3d-a96a-4d02-ba06-d691535b177f.md | FontExportDS | no: export drawing as a font |
| d4a45a03-0341-22aa-4fc5-287d2103da2c.md | FrameWndDataSource, WWelcomeScreenDataSource | no: suite application launcher |
| d50e3054-3bc5-e6a2-4ea1-1916bf2a47df.md | ColorSpecDS | no: colour specification |
| daa659c7-1d97-4e65-a0e5-b39a5f5aa47f.md | AssetListDS, AssetEngineDS | **yes**: font list pane (recently used list + list + filter-list toggle), §5.3, §11 |
| de163d96-aea3-4173-bedd-1f3c237a0076.md | WebViewDialogDS | no: "WebView2 not installed"; AnyFont uses no WebView |
| e36af7eb-0876-ba91-4380-715c27cc22c3.md | ModalResultDS | no: generic OK/Cancel wrapper; the egui modal helper replaces it |
| e4939644-a854-1283-4e3e-71f1eadd278c.md | WGlyphDockerDS | **yes**: glyph alternates pop-up, §7.2.21 |
| e4a0fcfa-8830-70bf-4d8d-f9e89ff606f7.md | WPaletteGridDS | no: palette grid |
| ed7a60e8-6ff3-4e34-8a0b-a4f6074c4c3.md | WCuiEyedropperPreviewDS | no: eyedropper |
| f05b499e-a15f-4505-a0c5-43278d826f1b.md | WTxtAttributeDS | no: character variants of text |
| f05b499e-a15f-4505-a0c5-43278d826f1b__2.md | WTxtAttributeDS | no: duplicate definition of the above |
| f0f62460-a4fc-4171-b306-88fe46873065.md | AssetListDS, AssetEngineDS | **yes**: font list pane with Refresh, Clear filters, resize slider and information strip, §5.3, §1.4 |
| f38388d8-a49b-4a99-8daf-c7008b3d837a.md | WTxtAttributeDS | no: bullets and numbering |
| fb2c5c78-c061-4b85-bcfa-e5e8ab2ebbba.md | OpenDesktopAppDS | no: web-app "open in desktop" prompt |
| fcda3765-18b3-c58b-4268-ed822a1b49f3.md | WDeleteWorkspaceConfirmDS | no: workspaces |
| fdea122f-80ec-4d2f-b0ec-566a852e5fe7.md | FontDS, AppDS | **yes**: "We can't find the following:", §7.2.13 |
| ff94a27b-87df-4cf3-a20f-72a2f339c7ee.md | AssetMgrDS, EffectPresetsDS | no: effect preset filter |

There are 95 files: 25 yes and 70 no. Other windows the dossier captured but AnyFont does not reproduce:
- the trial window shown on exit and the trial/purchase window (licensing rule of the team plan);
- the original's store page content, replaced by §7.2.7.

---

## 8. Shortcuts and keys

Full detail is in `docs/phases/07-shortcuts.md`. Summary:

| Key | Scope | Action |
|---|---|---|
| Ctrl+A | main window (not while a text box has focus, where it selects text) | Edit ▸ Select All |
| Ctrl+Shift+A | main window | Edit ▸ Deselect All |
| Ctrl+Alt+I | main window | "Inspectors": toggles the font properties panel (bound to the same state as View ▸ Show font properties) |
| Alt+F4 | main window | File ▸ Exit |
| Alt+F, Alt+E | main window | open File, Edit |
| F10 / Alt | main window | focus the menu bar |
| Shift+F10, Menu key | font list, Libraries | context menu of the focused row |
| ↑ ↓ PgUp PgDn Home End | font list (AssetListDS.OnKey) | move the current row; with Shift, extend the selection; with Ctrl, move without selecting |
| Space | font list | toggle the current row's selection (with Ctrl) |
| letter keys | font list | type-ahead: jump to the next font whose name starts with the typed prefix (virtualKeyToIndex) |
| ↑ ↓ Enter ← → | Libraries (FolderTreeDS.OnKey) | move between rows; Enter opens; ← and → collapse and expand a section |
| Enter / Esc | inline collection editor | commit / cancel |
| Enter / Esc | dialogs | default button / Cancel (or Close) |
| Esc | Search box | clears the search **[decide]** |
| Delete | font list | **[decide]** not defined in the dossier (`onDeleteKey` exists only on the palette editor). Proposal: no binding, because Delete removes files |

Menu captions show only the shortcuts above: Select All Ctrl+A, Deselect All Ctrl+Shift+A, Exit Alt+F4.

---

## 9. Strings

`docs/strings-en.tsv` holds every English string AnyFont shows, one per line:
- deduplicated;
- in the dossier's wording, with "AnyFont" in place of the original product name;
- with access keys (`&`) as in the menus;
- with `{}` as the placeholder (format strings);
- with `\n` for line breaks.

`#` lines are comments that group the strings by area. The TSV is the key list for the translations in
`crates/anyfont/assets/i18n/<code>.tsv` (§8 of `docs/phases/08-localization.md`).

Strings the program builds, which the TSV holds as formats:
- `{}: {} fonts` and `{} (filtered): {} fonts`;
- `{} glyphs`;
- `Collection {}`;
- `Remove from {}`;
- `{} ({}, {})` is not translated, because it is font data.

---

## 10. Themes

Options ▸ Color ▸ Theme offers four themes. The names come from the original's window painter schemes 01–04:

| Theme | Scheme | Notes |
|---|---|---|
| Light | LightestGrey | light window, dark text |
| Medium | MediumGrey | |
| Dark | DarkGrey | every capture in the dossier uses this theme; the hex values in this spec are this theme's |
| Black | Black | |

The default is Dark when Windows' `AppsUseLightTheme` is 0, otherwise Light. "Window border" sets the 1 px
window frame colour, with the theme default first.

All colours, fonts, radii and icon sizes are **tokens defined in `docs/design-tokens.md`**. That file is the
authority for their values. This spec uses short descriptive names (e.g. `accent`, `row.selected`); the table
below maps each one to the designer's token. Tokens marked *missing* are not in `design-tokens.md` yet,
because the screenshots do not show those states; the designer should add them.

| Name in this spec | `design-tokens.md` token |
|---|---|
| `titlebar.bg` | `title_bar_bg` |
| `titlebar.text` / `titlebar.text.inactive` | `text_title_bar` (#969696, the inactive capture); active look *missing* (the probe showed white text) |
| title-bar glyphs | `icon_title_bar` (?, gear, Updates), `icon_window_buttons` (–, □, ×) |
| `titlebar.hot`, `titlebar.close.hot` | *missing* (hover not captured) |
| `menubar.bg`, `menubar.hot` | `menu_bar_bg`, `menu_bar_item_open_bg` |
| `menu.bg`, `menu.border`, `menu.separator`, `menu.hot` | `menu_bg`, `menu_border`, `menu_separator`, `menu_item_hot_bg`; arrow `menu_submenu_arrow` |
| `toolbar.bg` | `toolbar_bg`; separators `toolbar_separator` |
| `button.hot`, `button.pressed` | *missing* |
| `button.bg`, `button.border` | `button_bg`; border *missing* |
| `accent` | `accent` (#017DBC): section headers and their lines (`section_header_text`), slider fill and thumb ring, focus border (`edit_border_focus`) |
| `accent.checked` | `selection_blue` = `toolbar_button_checked_bg` = `tab_selected_bg` (#1D4470); chevrons `section_chevron` |
| `line.strong` | `toolbar_bottom_line`, `status_bar_top_line`, `list_row_separator`, `tab_strip_bottom_line` (all #525252) |
| `line.divider` | `panel_splitter`, `properties_left_border` (#3C3C3C) |
| `panel.bg`, `dialog.bg` | `panel_bg`, `dialog_bg`; dialog button line `dialog_separator` |
| `row.selected` (Libraries) | `library_item_selected_bg` |
| `row.selected.list` | `list_row_selected_bg` |
| `row.checked`, `row.hot` | *missing* (checked filter row #525252 measured in this spec; hover not captured) |
| `check.border`, `check.bg` | `checkbox_border`, `checkbox_bg` |
| `input.border` | `edit_border`; combo boxes in dialogs `combo_bg` |
| `text`, `text.disabled`, `text.cue` | `text`, `text_disabled`, `text_placeholder` |
| `text.secondary` | *missing* (empty-state italics; proposal: `text`) |
| `icon.disabled` | `icon_disabled` |
| `scroll.track`, `scroll.thumb` | `scrollbar_track`, `scrollbar_thumb` |
| `slider.rest` | `slider_track`; thumb `slider_thumb` |
| `status.installed` | `installed_bar`; lock `lock` |
| `danger` | `badge_remove` (#FF6250) |
| `dialog.titlebar.bg`, `dialog.border` | `title_bar_bg`; frame: Options ▸ Window border (captured #000000) |

**[decide]** `design-tokens.md` offers only **Dark** and **Light**, and the Light palette is derived, not
measured. The Theme combo in the original binds to `CustomizationDS.ColorSchemeItems`; the four scheme names
above come from the original's window-painter strings. The lead decides whether AnyFont offers 2 or 4 themes.
If four, the designer adds Medium and Black palettes.

---

## 11. Features defined in the dossier but not reachable in the captured window

Each feature below is implemented, in the least intrusive place the dossier's UI definition suggests. Each
one is also listed in `docs/phases/06-ui.md`.

| # | Feature (dossier) | Where AnyFont exposes it | Default |
|---|---|---|---|
| 1 | Group by family: checkButton `1eaf38ba`, `FontDS.GroupByFamily`, enabled by `AssetListDS.ShowGroupFlyout`; bar `afafdcb9`, a flyout with no captured trigger | View menu, first item of a new group after Refresh the Font List | on (the captured list is grouped) |
| 2 | Show recently used fonts (`53631843`, `AssetListDS.ShowMRUList`, MRU list in `daa659c7`) | View menu, same group | off |
| 3 | Use font to display font name (`c537dfdc`, `AssetListDS.RenderUsingAsset`) | View menu, same group. When off, previews use the UI font and the size slider is disabled | on (captured previews are drawn in each font) |
| 4 | Show Latin names (`a0face70`, `FontDS.ShowLatinName`) | View menu, same group. Shows the English (Latin) family name instead of the localised one | off **[decide]** |
| 5 | Refresh information strip and "Refresh" button (`f0f62460`, `InformationString`, `InformationIconTooltip`, `AssetListDS.OnManualRefresh`) | strip above the font list when a manual refresh is needed; View ▸ Refresh the Font List becomes enabled at the same time. Tooltip "Refresh the font list." | hidden |
| 6 | Hide filter list / Show filter list (`daa659c7`/`f0f62460`, `AssetListDS.FilterListShown`) | View menu, item "Hide filter list" / "Show filter list" (dynamic caption, check look). When hidden, Libraries fills the left panel | shown |
| 7 | Hide preview / Show preview and Inspectors Ctrl+Alt+I (`AssetListDS.PreviewShown`, `UIMgr.HideDockers`) | the same state as View ▸ Show font properties. Ctrl+Alt+I toggles it; no extra item | hidden |
| 8 | Vendor URL (`FontDS.OnVendorURL`, `VendorURLString`, `VendorURLVisible`, `SelectedVendorURL`) | Font info tab, a hyperlink row below Collection. It opens the font's vendor URL (name ID 11) in the browser | visible when the font has one |
| 9 | Rename collection (`c7ef0b94`, `RenameCollectionDialogDS`; `Rename` items in the asset manager bars) | Libraries context menu ▸ Rename, for user collections only, using the inline editor (§5.1). F2 on a selected collection row does the same | – |
| 10 | Search suggestions (`ab5f3dbb`, `AssetListDS.SuggestedCount`) | drop-down under the Search box | – |
| 11 | Glyph alternates (`e4939644`) and glyph Copy (`WGlyphDockerDS.Copy`) | Glyph Preview: a click on a glyph with alternates opens the alternates pop-up. Ctrl+C or a double-click copies the selected glyph's character to the clipboard | – |
| 12 | Font list options pop-up (`afafdcb9` as a flyout) | covered by #1–#4 in the View menu | – |
| 13 | PANOSE font matching (52600a8b, b3d42473, b12fbfa5) | Options dialog, a new header **Font Matching** with a button "PANOSE Font Matching Preferences...". It opens 52600a8b, whose Exceptions... button opens b3d42473. b12fbfa5 opens when an import (§7.2.13) or a collection references fonts that are no longer present and the matching mode is not "Never show" **[decide]** | mode Text |
| 14 | Type 1 replacement (3a3106a3) | AnyFont lists only OTF/TTF. When Add Folder or a rescan finds Type 1 files (.pfb/.pfm), the dialog appears once per folder; "Replace fonts" opens the PANOSE results (b12fbfa5) to choose OpenType replacements in collections. The dossier text speaks of "This document", so the wording needs a decision **[decide]** | – |
| 15 | Reboot prompt (8455a514), Delete fonts (a8b8f4b7), operation progress (23e365df), "We can't find…" (fdea122f) | the normal Uninstall, Delete, Install/Download and Import flows (§7.2.10–13) | – |
| 16 | Download (`FontDS.OnDownloadFonts`, gated by `IsUserAllowedToUseInternet`) | toolbar, Font menu and Get More (Google Fonts) **[AnyFont]**. Options gets a **Network** header with the checkbox "Allow AnyFont to use the Internet" (new string) **[decide]** | allowed |
| 17 | Upload fonts (b8517de3) | not implemented: a cloud service with no AnyFont counterpart **[decide]** | – |
| 18 | Library Install / Uninstall (`FontDS.OnInstallLibrary`, `OnUninstallLibrary`) | Libraries context menu (already captured) | – |
| 19 | Toolbar overflow | §3 | – |
| 20 | Font database reset (string "…restart AnyFont while holding down Shift.") | holding Shift while AnyFont starts asks to rebuild the font cache | – |

New View menu with #1–#4 and #6 (AnyFont):

```
Refresh the Font List
───
Group by family                    (check)
Show recently used fonts           (check)
Use font to display font name      (check)
Show Latin names                   (check)
───
Hide filter list / Show filter list
Show font properties / Hide font properties
```

---

## 12. Open questions for the lead

1. **Family header and Uninstall/Delete.** The menu has them enabled, the toolbar disabled. The spec follows
   the menu (§2.6).
2. **Favorites is removable.** In the original, Remove Folder or Collection is enabled for Favorites. Keep that,
   or protect Favorites? The spec keeps it.
3. **Pluralisation.** "Collection 1: 1 fonts" is the original's wording. Keep it, or add "{}: {} font"?
4. **Help menu.** The spec uses plain items "Check for Updates..." and "Report a Bug..." instead of
   AnyVector's "&Updates..." and the "Report a Bug ▸" flyout.
5. **Values not captured.** Theme combo values (taken from the scheme names), Window border swatches, System Info
   categories, the Options scale range, the preview-size range, the Get More sort options and filters.
6. **Search ordering.** The capture shows Arial Narrow before Arial, which suggests prefix and length ordering.
7. **"Comm…".** The glyph group header is truncated in the capture. The spec shows "Common" in full.
8. **Download, Get More and online fonts.** Download is disabled in every capture. Should undownloaded catalog
   fonts appear in My fonts (§7.2.7)? Proposal: no. Is an "Allow AnyFont to use the Internet" option needed
   (§11 #16)?
9. **Placement of unreachable features.** The View-menu additions (§11 #1–#4, #6), the Rename item, the
   PANOSE entry point (Options), the Type 1 wording and the trigger for the PANOSE results dialog.
10. **Upload fonts.** It is omitted (cloud only).
11. **Download progress dialogs.** 5374ecc3 and 7892ab4b are unused; font downloads use 23e365df.
12. **Delete key.** It is unbound in the font list.
13. **Not captured.** Hover looks, the not-installed and online status markers, the list scrollbar on hover,
    the minimum window size, the maximum panel widths, and how collapsed the filter groups start.
14. **Family header double-click.** It has no effect, as captured.
15. **Edit-box context menu.** The spec draws it with the theme and without the IME/Unicode items.
16. **Themes.** Two themes (`design-tokens.md`: Dark and Light) or four (Light, Medium, Dark, Black,
    from the original's scheme names)? See §10.

## 13. Lead decisions (binding for the UI team)

1. Family header: Uninstall/Delete follow the **menu** (enabled when the family has eligible faces); the toolbar uses the same rule.
2. Favorites can be removed, as in the original; it is not re-created automatically.
3. Status text keeps the original wording "{}: {} fonts" (no singular form), translations may add plural forms.
4. Help menu: plain items "Check for Updates...", "Submit Ideas and Feedback...", "Report a Bug..." after Product Help and before the separator + About AnyFont.
5. Themes: the original's four schemes **Light, Medium, Dark, Black** (Dark is measured in `design-tokens.md`; derive the others: Black = darker panels #1A1A1A / list #111, Medium = #535353 family, Light = #F0F0F0 family with dark text). Default Dark.
6. Unshown values: accept the proposals (System Info list without Printing, Options scale 75–225 %, preview size 8–86 px, Get More sort/filters).
7. "My fonts" lists installed fonts **plus** the fonts in the AnyFont Content folder (downloaded with Get More), which appear as *Not installed* (so the Not installed filter and Install are meaningful). Catalog fonts that are not downloaded are not listed; Download stays enabled only for online placeholders (none in 0.1.0 → disabled, as captured). No new Internet option.
8. Placement of unreachable features: as proposed (View menu toggles, Rename collection in the Libraries context menu + F2, Options ▸ "Font Matching" header with the three PANOSE modes + Exceptions..., PANOSE results after importing collections that reference missing fonts). Type 1 dialog text: replace "This document" with "These fonts".
9. Upload fonts omitted; downloads use the operation-progress dialog.
10. Delete key unbound in the font list.
11. Small points: as proposed ("Common", no double-click action on headers, themed edit menu without IME items, proposals for unshown states).
