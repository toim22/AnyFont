# AnyFont design tokens (Dark theme)

Colours measured pixel by pixel in the reference screenshots
(`C:\Projetos\engenharia-reversa\FontManager-ida\06-ui\screenshots`, "the screenshots" below), with
Python + Pillow on the PNGs (exact RGB values, not estimates). Paths in the "Sampled in" column are
relative to that folder. The UI team builds the egui theme from this table; the icons in
`crates/anyfont/assets/icons/` use the same values (see that folder's `README.md`).

Text in the screenshots is ClearType-rendered (coloured fringes), so text colours were taken from the
solid centre of stems; they match the plain glyph colour `#CACED4` everywhere except where noted.

## Surfaces

| Token | Hex | Where it is used | Sampled in |
|---|---|---|---|
| `title_bar_bg` | `#262626` | custom title bar | `08-title-bar/Title bar.png` (any empty pixel), `02-main-window/Main window - restored size.png` y 0–31 |
| `menu_bar_bg` | `#404040` | menu bar strip (File Edit Font View Help) | restored main window, x 400, y 32–55 |
| `menu_bar_item_open_bg` | `#525252` | menu bar title while its menu is open | `03-menus/font-selected/Font.png`, "Font" x 88–132 |
| `toolbar_bg` | `#262626` | toolbar strip | `04-toolbar/Toolbar.png` |
| `toolbar_bottom_line` | `#525252` | 1 px line under the toolbar | restored main window, y 89 |
| `toolbar_separator` | `#525252` | 1 px vertical separators, 22 px tall (y 6–27 of the 34 px strip) | `04-toolbar/Toolbar.png`, x 101/142/247/288/329 |
| `toolbar_button_checked_bg` | `#1D4470` | toggled toolbar button (Show font properties while the panel is open) | `06-panels/font-properties/full-window/Variable font selected (Bahnschrift).png`, x 334–344, y 66 |
| `panel_bg` | `#262626` | Libraries, Filters, font list, font properties, status bar, dialogs | everywhere |
| `panel_splitter` | `#3C3C3C` | 1 px splitter between the left panels and the list, and between Libraries and Filters | maximized main window x 241; restored main window y 299 |
| `library_item_selected_bg` | `#3C3C3C` | selected entry in Libraries ("My fonts") | restored main window, x 120, y 151–175 |
| `list_row_selected_bg` | `#525252` | selected font row / family header row in the font list | restored main window, x 400, y 128–203 |
| `list_row_separator` | `#525252` | 1 px line between font families | `06-panels/font-list/Family header double-clicked.png`, x 800, y 270/346/422 |
| `installed_bar` | `#66BF83` | green bar at the right of every installed font row (14 px wide; the lock is drawn on it) | restored main window, x 759–772 |
| `status_bar_top_line` | `#525252` | 1 px line above the status bar | restored main window, y 559 |
| `scrollbar_track` | `#171717` | list / filter / glyph scroll bars | restored main window, x 778–790 |
| `scrollbar_thumb` | `#959595` | scroll bar thumb (thin, 2–3 px) | restored main window, x 783 |
| `tab_selected_bg` | `#1D4470` | selected font properties tab (Glyphs / Font info / Variable font), rounded ~3 px | `06-panels/font-properties/Tab Glyphs.png`, x 6–30, y 0–23 |
| `tab_strip_bottom_line` | `#525252` | line under the properties tab strip | `06-panels/font-properties/Tab Glyphs.png`, x 200, y 32 |
| `properties_left_border` | `#3C3C3C` | left edge of the font properties panel | `06-panels/font-properties/Tab Glyphs.png`, x 2 |

## Text

| Token | Hex | Where it is used | Sampled in |
|---|---|---|---|
| `text` | `#CACED4` | all normal text: menus, list labels and previews, panel titles, dialogs | restored main window, menu bar / "My fonts" / previews |
| `text_title_bar` | `#969696` | window title "AnyFont" | restored main window, x 38–250, y 8–24 |
| `text_disabled` | `#808080` | disabled menu items ("Install", "Download", "Remove from collection") | `03-menus/font-selected/Font.png`, "Install" |
| `text_placeholder` | `#575757` | "Enter sample text", "Search" placeholders | restored main window, x 258–350, y 100–116 |
| `section_header_text` | `#017DBC` | "Folders", "My Collections", filter group names ("Font Status"…), "Comm…" in Glyph Filters | `06-panels/filters/Filters (default).png` (ClearType; stem centre = `#017DBC`) |
| `glyph_metric_lines` | `#95CAFF`, `#C6E2FF`, `#00DC32` (baseline), `#0080FF` | the coloured metric lines drawn in an empty glyph cell (space) of Glyph Preview | `06-panels/font-properties/Tab Glyphs.png`, x 27, y 274/282/296/306 |

## Accent and section headers

| Token | Hex | Where it is used | Sampled in |
|---|---|---|---|
| `accent` (focus blue) | `#017DBC` | section header line (1 px, after the name), slider fill and thumb ring, default-button border, focused edit border, inline name editor border | `06-panels/filters/Filters (default).png` x 150 (every header line); About "Close" border; Options slider |
| `section_chevron` | `#1D4470` | the double chevrons (collapse ▲▲ / expand ▼▼) at the right end of each section header | `06-panels/filters/Filters - Font Technology.png`, x 222–230, y 48–55 / 94–101; `06-panels/libraries/*` x 226 |
| `selection_blue` | `#1D4470` | default button fill, selected tab, toggled toolbar button (same value as the chevrons) | About dialog "Close" fill; tab strip |
| `selection_blue_hot` | `#1D4499` / `#1D5CAD` | text-row highlight inside the default button (anti-aliasing of the caption) | About dialog, x 520, y 358–361 |
| `os_accent` | `#0078D4` | Windows accent: text selection inside the inline name editor (New collection) | `06-panels/libraries/New collection - inline name editor.png`, x 60, y 150–156 |

## Menus and context menus (app menus)

| Token | Hex | Where it is used | Sampled in |
|---|---|---|---|
| `menu_bg` | `#262626` | popup background | `03-menus/font-selected/Font.png` |
| `menu_border` | `#525252` | 1 px popup border | `07-context-menus/font row - single font (Algerian) ▸ Add to collection.png`, x 70 |
| `menu_separator` | `#525252` | 1 px separators (inset from the icon column) | `03-menus/font-selected/Font.png`, x 250, y 101/130/181/210 |
| `menu_item_hot_bg` | `#525252` | highlighted row (open submenu "Add to collection") | same context menu, y 146–167 |
| `menu_text` / `menu_text_disabled` | `#CACED4` / `#808080` | item captions | `03-menus/font-selected/Font.png` |
| `menu_submenu_arrow` | `#969696` | "›" submenu arrow (4×8 px) | `03-menus/font-selected/Font.png`, x 288–291, y 141–148 |

The right-click menu of the sample text and search boxes is the Windows native edit menu, light:
background `#F9F9F9`, separators `#D7D7D7` (`07-context-menus/Sample text box.png`). That is the OS
menu, not an app theme; AnyFont may draw its own dark edit menu instead.

## Controls

| Token | Hex | Where it is used | Sampled in |
|---|---|---|---|
| `edit_bg` / `edit_border` | `#262626` / `#525252` | sample text combo box, Search box (1 px border) | restored main window, x 255–760, y 96–119 |
| `edit_border_focus` | `#017DBC` | border of the focused/hovered edit box | `06-panels/font-list/Search typed (Arial).png`, x 253 |
| `combo_bg` | `#3C3C3C` | combo boxes in dialogs (Options › Theme, Window border) | `05-dialogs/Title bar ▸ Options (gear).png`, x 160, y 253–276 |
| `button_bg` | `#3C3C3C` | push buttons ("System Info…", "Legal Notices…", "Reset to Default") | `05-dialogs/Help ▸ About Corel Font Manager.png`, x 60, y 347–370 |
| `button_default_bg` / `button_default_border` | `#1D4470` / `#017DBC` | default button ("Close", "OK") | About dialog x 520 y 347–370; Options x 400 y 344–367 |
| `checkbox_bg` / `checkbox_border` | `#404040` / `#525252` | filter check boxes (12×12, corners rounded ~2 px) | `06-panels/filters/Filters - Font Technology.png`, x 4–15, y 115–126 |
| `slider_track` | `#525252` | unfilled part of a slider (4 px tall, round ends) | restored main window status bar; Options › Size |
| `slider_fill` | `#017DBC` | filled part of a slider | same |
| `slider_thumb` | ring `#017DBC` (≈1.5 px), centre `#262626`, Ø ≈ 11 px | preview size slider, Options › Size, Variable font axes | restored main window x 691–702, y 573–585 |
| `dialog_bg` | `#262626` | dialogs | About, Options |
| `dialog_separator` | `#525252` | 1 px line above the dialog buttons | About x 60 y 299/329; Options x 400 y 336 |
| `about_banner_bg` / `about_banner_border` | `#000000` / `#808080` | About banner (548×206 inside a 1 px border) | About dialog, x 60, y 40–246 |
| `info_box_border` | `#525252` | "Source ID" box in About | About dialog, y 299–329 |

## Icon colours

| Token | Hex | Where it is used | Sampled in |
|---|---|---|---|
| `icon` | `#CACED4` | every light glyph (toolbar, menus, list row technology icons, tabs, status bar) | `04-toolbar/Toolbar.png`, enabled icons |
| `icon_disabled` | `#5D5D5D` | disabled toolbar/menu glyphs; the badge becomes `#434343`, its cut-out stays the background | `04-toolbar/Toolbar.png`, Install/Download |
| `icon_title_bar` | `#5D5D5D` | "?", gear and Updates buttons in the title bar (their normal, non-hover look) | `08-title-bar/Title bar buttons.png` |
| `icon_window_buttons` | `#969696` | minimize / maximize / restore / close glyphs | `08-title-bar/Title bar buttons.png` |
| `icon_filter_clear` | `#5D5D5D` | the funnel in the Filters / Glyph Filters headers (no filter set = disabled look) | `06-panels/filters/Filters - Font Technology.png` |
| `badge_add` | `#1BB474` | green "+" badge (Add Folder, Add Collection) and AnyFont's Install badge | `04-toolbar/Toolbar.png` |
| `badge_remove` | `#FF6250` | red badge of Uninstall (↑) and red × of Delete | `04-toolbar/states/user-collection-font-selected.png` |
| `lock` | `#000000` | padlock on the green installed bar | restored main window, x 762–769 |

## Light theme

No screenshot shows a light theme. Options › Color › Theme shows **Dark** (the captured setting); its
list was not opened in the capture (`05-dialogs/inside/Title bar ▸ Options (gear)/list 1.png` and
`list 2.png` show the closed combo box). The dossier's English strings contain both "Light" and "Dark"
(`08-localization/strings-en.csv`), and the Options dialog binds the combo box to
`CustomizationDS.ColorSchemeItems` / `SelectedColorSchemeItem` (`06-ui/dialogs/ab1e1683-…md`).
So AnyFont offers **Dark** (this table, the default) and **Light**. The light palette is not measured;
until one is, the UI team derives it from egui's light visuals with the same accent (`#017DBC`),
installed bar (`#66BF83`) and badge colours, and draws the icons with a dark tint (see the icon README).
The "Window border" combo box (a colour swatch, black in the capture) sets the 1 px window frame
colour; the captured frame is `#000000`.
