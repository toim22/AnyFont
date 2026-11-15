# AnyFont Help

AnyFont is a font manager for Windows. Use it to browse every font on your PC, preview it with your own text,
sort fonts into collections, install and uninstall them, find duplicates, look inside a font at its glyphs and
details, and get free fonts from an online catalog.

This guide explains every part of the program. You can open it at any time from **Help ▸ Product Help...** or
with the **?** button in the title bar.

## Contents

- [Getting started](#getting-started)
  - [A tour of the main window](#a-tour-of-the-main-window)
  - [Opening a font file with AnyFont](#opening-a-font-file-with-anyfont)
- [Title bar buttons](#title-bar-buttons)
- [Menus](#menus)
  - [File menu](#file-menu)
  - [Edit menu](#edit-menu)
  - [Font menu](#font-menu)
  - [View menu](#view-menu)
  - [Help menu](#help-menu)
- [Toolbar](#toolbar)
- [Libraries](#libraries)
  - [My fonts](#my-fonts)
  - [Folders](#folders)
  - [Collections](#collections)
- [Filters](#filters)
- [Search, sample text and preview size](#search-sample-text-and-preview-size)
- [The font list](#the-font-list)
- [Font properties](#font-properties)
  - [Glyphs tab](#glyphs-tab)
  - [Font info tab](#font-info-tab)
  - [Variable font tab](#variable-font-tab)
- [Installing, uninstalling and deleting fonts](#installing-uninstalling-and-deleting-fonts)
  - [Installing fonts for all users or only for you](#installing-fonts-for-all-users-or-only-for-you)
  - [Uninstalling fonts](#uninstalling-fonts)
  - [Deleting font files](#deleting-font-files)
  - [Fonts that are in use: the restart prompt](#fonts-that-are-in-use-the-restart-prompt)
  - [Protected system fonts](#protected-system-fonts)
- [Duplicate fonts](#duplicate-fonts)
- [Corrupt and unsupported font files](#corrupt-and-unsupported-font-files)
- [Importing and exporting folders and collections](#importing-and-exporting-folders-and-collections)
- [Font matching with PANOSE](#font-matching-with-panose)
  - [Font Matching options](#font-matching-options)
  - [Font Matching Exceptions](#font-matching-exceptions)
  - [Font Matching Results](#font-matching-results)
  - [Type 1 fonts](#type-1-fonts)
- [Get More: the free online font catalog](#get-more-the-free-online-font-catalog)
- [Options](#options)
- [Updates](#updates)
- [Report a Bug and Submit Ideas and Feedback](#report-a-bug-and-submit-ideas-and-feedback)
- [About AnyFont](#about-anyfont)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Language](#language)
- [Where AnyFont keeps your data](#where-anyfont-keeps-your-data)
- [Troubleshooting and FAQ](#troubleshooting-and-faq)

---

## Getting started

When you start AnyFont for the first time, it scans the fonts installed on your PC and builds its font
database. This takes a moment on a PC with many fonts; later starts are quick, because AnyFont only looks at
fonts that changed since the last time.

AnyFont keeps watching your fonts while it runs. When you install a font with another program or copy font
files into a folder you added, the list updates by itself.

### A tour of the main window

From top to bottom, the AnyFont window has these parts:

| Part | What it is |
|---|---|
| **Title bar** | The program name, the [title bar buttons](#title-bar-buttons) **?** (Product Help), **⚙** (Options) and **Updates**, and the usual Minimize, Maximize and Close buttons. |
| **Menu bar** | The **File**, **Edit**, **Font**, **View** and **Help** [menus](#menus). |
| **Toolbar** | One-click buttons for the most common commands. See [Toolbar](#toolbar). |
| **Left panel, top: Libraries** | Where your fonts come from: **My fonts**, your **Folders** and **My Collections**. See [Libraries](#libraries). |
| **Left panel, bottom: Filters** | Check boxes that narrow the list down, for example to installed fonts or serif fonts. See [Filters](#filters). |
| **Sample text and Search boxes** | Above the font list. Type your own preview text, or search for a font by name. See [Search, sample text and preview size](#search-sample-text-and-preview-size). |
| **Font list** | Every font of the open library, each drawn in its own typeface. See [The font list](#the-font-list). |
| **Font properties panel** (right, hidden at first) | The glyphs, details and variable-font axes of the selected font. Show it with **View ▸ Show font properties** or <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>I</kbd>. See [Font properties](#font-properties). |
| **Status bar** | The open library and how many fonts it shows, for example `My fonts: 681 fonts`, and the preview size slider. |

You can resize the panels: drag the line between the left panel and the font list, the line between Libraries
and Filters, or the left edge of the font properties panel. AnyFont remembers the window size, the panel sizes
and most of your choices for the next time.

A typical first session:

1. Click **My fonts** in Libraries to see every font on your PC.
2. Type a word in **Enter sample text** to see it in every font.
3. Check a few boxes under **Filters**, for example **Installed** and **Serif**.
4. Right-click a font you like and choose **Add to collection ▸ Favorites**.
5. Click **Get More** on the toolbar to download free fonts, then select them and click **Install**.

### Opening a font file with AnyFont

If you chose **Open font files with AnyFont** when you installed AnyFont, you can right-click a `.ttf`, `.otf`
or `.ttc` file in File Explorer and choose **Open with ▸ AnyFont**. AnyFont starts (or comes to the front if it
is already running) and shows that font. This option never changes the program that opens font files when you
double-click them.

---

## Title bar buttons

AnyFont draws its own title bar. Next to the usual window buttons it has three buttons of its own:

| Button | Tooltip | What it does |
|---|---|---|
| **?** | Product Help | Opens this guide in your web browser. Same as **Help ▸ Product Help...**. |
| **⚙** (gear) | Options | Opens the [Options](#options) window: size of the user interface, theme, window border colour, font matching and product updates. |
| **Updates** (circular arrows) | Updates | Opens the [Updates](#updates) window. A small dot appears on the button while a new version of AnyFont is available. |
| **–** | Minimize | Minimizes the window. |
| **□** / **❐** | Maximize / Restore Down | Maximizes the window or restores it. Hover over it to use the Windows snap layouts. |
| **×** | Close | Closes AnyFont. |

You can also double-click the title bar to maximize or restore the window, drag it to move the window, and
right-click it for the standard Windows window menu.

---

## Menus

Press <kbd>Alt</kbd> or <kbd>F10</kbd> to move to the menu bar with the keyboard. <kbd>Alt</kbd>+<kbd>F</kbd>
opens the File menu and <kbd>Alt</kbd>+<kbd>E</kbd> the Edit menu. A command that can't be used right now is
shown greyed out.

### File menu

| Command | When you can use it | What it does |
|---|---|---|
| **Add Folder...** | Always | Asks you for a folder that contains fonts, adds it under **Libraries ▸ Folders** and opens it. See [Folders](#folders). |
| **Add Collection** | Always | Creates a new collection named "Collection 1" (or the next free number) under **My Collections** and lets you type its name right away. See [Collections](#collections). |
| **Get More...** | Always | Opens [Get More](#get-more-the-free-online-font-catalog), the free online font catalog. |
| **Remove Folder or Collection** | When a folder or a collection is open (Favorites too, but never My fonts) | Removes the open folder or collection from your Libraries, after asking you to confirm. Your font files are **not** touched. |
| **Import Folders and Collections...** | After AnyFont has finished its first scan of your fonts | Reads a folders-and-collections file (`.xml`) and adds what it contains. See [Importing and exporting](#importing-and-exporting-folders-and-collections). |
| **Export Folders and Collections...** | When you have at least one folder or collection | Saves your folders and collections to an `.xml` file. |
| **Exit** | Always (<kbd>Alt</kbd>+<kbd>F4</kbd>) | Closes AnyFont. |

### Edit menu

| Command | Shortcut | When you can use it | What it does |
|---|---|---|---|
| **Select All** | <kbd>Ctrl</kbd>+<kbd>A</kbd> | When the list shows at least one font | Selects every font the list shows (with the current filters and search). |
| **Deselect All** | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> | When at least one font is selected | Clears the selection. The font you worked with last stays highlighted, so you don't lose your place. |

While the cursor is in the Search or sample text box, <kbd>Ctrl</kbd>+<kbd>A</kbd> selects the text in that box
instead.

### Font menu

The Font menu works on the fonts you selected in the list. The same commands appear when you right-click a font
in the list. If you select a **family header** (the big family name above a group of styles), the commands work
on every style of that family.

| Command | When you can use it | What it does |
|---|---|---|
| **Install** | The selection contains a font that is not installed | Installs the selected fonts so that other programs can use them. See [Installing fonts](#installing-fonts-for-all-users-or-only-for-you). |
| **Uninstall** | The selection contains an installed font that is not a protected system font | Uninstalls the selected fonts. The files stay on your disk. See [Uninstalling fonts](#uninstalling-fonts). |
| **Download** | The selection contains an online catalog font that is not downloaded yet | Downloads the font to the AnyFont Content folder. In this version, catalog fonts reach your font list only after you download them in [Get More](#get-more-the-free-online-font-catalog), so this command is usually greyed out: use Get More to download fonts. |
| **Delete** | Same as Uninstall | Moves the selected font files to the Recycle Bin, after asking you to confirm. Installed fonts are uninstalled first. See [Deleting font files](#deleting-font-files). |
| **Add to collection ▸** | When a font is selected or highlighted | Opens a list of your collections (Favorites included), in alphabetical order. Click one to add the selected fonts to it. The collection that is open right now is not in the list. |
| **Remove from *collection*** | Only while a collection is open, for example **Remove from Favorites** | Removes the selected fonts from the open collection. The fonts themselves are not touched. |
| **Remove from collection ▸** | When no collection is open and a selected font belongs to a collection | Lists the collections that contain the selected fonts. Click one to remove the fonts from it. |
| **Show duplicate fonts** / **Hide duplicate fonts** | Always | Shows or hides extra copies of the same font. While duplicates are shown, the command reads **Hide duplicate fonts** and its icon is highlighted. See [Duplicate fonts](#duplicate-fonts). |
| **Open font file location ▸** | When exactly one font or one family header is selected | Lists the font's file (for a family, every style's file; with duplicates shown, every copy). Click a path to open File Explorer with that file selected. |

### View menu

| Command | When you can use it | What it does |
|---|---|---|
| **Refresh the Font List** | When AnyFont noticed changes it could not apply by itself | Scans your libraries again. An information strip with a **Refresh** button also appears above the font list at those times. |
| **Group by family** | Always (on at first) | Groups the styles of each family under a family header. When it's off, every style is listed on its own row. |
| **Show recently used fonts** | Always (off at first) | Shows the fonts you used most recently at the top of the list, above a thin line. |
| **Use font to display font name** | Always (on at first) | Draws each preview in its own font. When it's off, previews use AnyFont's normal interface font and the preview size slider is greyed out. This can make scrolling faster on a PC with thousands of fonts. |
| **Show Latin names** | Always (off at first) | Shows each family's English (Latin) name instead of its name in your language, for fonts that have both, for example Japanese or Chinese fonts. |
| **Hide filter list** / **Show filter list** | Always | Hides the Filters panel so that Libraries fills the whole left panel, or shows it again. |
| **Show font properties** / **Hide font properties** | Always (<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>I</kbd>) | Shows or hides the [font properties panel](#font-properties) on the right. |

The four options in the middle are check items: a highlighted icon means the option is on.

### Help menu

| Command | What it does |
|---|---|
| **Product Help...** | Opens this guide in your web browser. |
| **Check for Updates...** | Opens the [Updates](#updates) window and checks for a new version right away. |
| **Submit Ideas and Feedback...** | Opens a form to send an idea or feedback to the AnyFont team. See [Report a Bug and Submit Ideas and Feedback](#report-a-bug-and-submit-ideas-and-feedback). |
| **Report a Bug...** | Opens a form to report a problem. |
| **About AnyFont...** | Shows the version, the Source ID, system information and the legal notices. See [About AnyFont](#about-anyfont). |

---

## Toolbar

The toolbar buttons have no captions. Hover over a button to see its tooltip. Each button follows the same rules
as the matching menu command, so it is greyed out whenever that command is.

| Button | Tooltip | What it does |
|---|---|---|
| Add folder | **Add folder** – Browse to a folder that contains fonts to add it to your Libraries. | Same as **File ▸ Add Folder...** |
| Add collection | **Add collection** – Add a collection for easy access to fonts that reside in different folders. | Same as **File ▸ Add Collection** |
| Get More | **Get More** | Same as **File ▸ Get More...** |
| Remove | **Remove** – Remove a folder or collection from your Libraries. | Same as **File ▸ Remove Folder or Collection** |
| Install | **Install** – Install fonts and font families. | Same as **Font ▸ Install** |
| Uninstall | **Uninstall** – Uninstall fonts and font families. | Same as **Font ▸ Uninstall** |
| Download | **Download** – Download fonts to the AnyFont Content folder. | Same as **Font ▸ Download** |
| Delete | **Delete** – Delete font files from your system. | Same as **Font ▸ Delete** |
| Show duplicate fonts | **Show duplicate fonts** – Show duplicate font versions. (**Hide duplicate fonts** – Hide duplicate font versions. while on) | Same as **Font ▸ Show duplicate fonts**. The button is highlighted while duplicates are shown. |
| Show font properties | **Show font properties** – Show font details or glyph sets. | Same as **View ▸ Show font properties**. The button is highlighted while the panel is shown. |

When the window is too narrow for every button, the last ones move into a **»** button at the right end of the
toolbar. Click it to reach them.

---

## Libraries

The **Libraries** list at the top of the left panel decides which fonts the font list shows. Click a library to
open it. AnyFont selects its first font for you, and the status bar shows the library's name and font count.

**Folders** and **My Collections** are sections. Click a section header, or the double arrow at its right, to fold
it away or open it again.

Right-click a library for these commands:

| Command | What it does |
|---|---|
| **Install** | Installs every font of the library that is not installed yet. |
| **Uninstall** | Uninstalls every installed font of the library, except protected system fonts. |
| **Remove Folder or Collection** | Removes the folder or collection from your Libraries (not available for My fonts). |
| **Rename** | Lets you rename a collection you created. |

Install and Uninstall from this menu show the same progress window and prompts as the Font menu commands.

### My fonts

**My fonts** always comes first. Its tooltip says what it includes: "Includes all installed fonts, all fonts in
your added folders, and fonts downloaded from the Online library". In detail:

- every font installed on your PC, for you or for all users;
- the fonts in the folders you added;
- the fonts you downloaded with [Get More](#get-more-the-free-online-font-catalog). Until you install them, they
  are listed as **Not installed**.

My fonts can't be removed or renamed.

### Folders

Folders let you keep fonts anywhere on your disk, a USB drive or a network share, and browse them in AnyFont
without installing them.

- **Add a folder.** Choose **File ▸ Add Folder...** or the **Add folder** toolbar button, pick the folder and click
  **Select Folder**. The folder appears under **Folders**, named after the folder itself (hover over it to see the
  full path), and opens.
- **Subfolders.** AnyFont also looks for fonts in every subfolder of a folder you add. If you try to add a folder
  that is already covered, AnyFont tells you: "This folder has already been added." or "This folder has already
  been added as a subfolder of *folder*." If the folder doesn't exist any more, you see "Folder not found. Please
  choose another location."
- **On or off.** A folder can be turned off without removing it. Its fonts then leave My fonts, but the folder
  keeps its place in your Libraries and in exported files, and its fonts return when you turn it on again.
- **Remove a folder.** Open it and choose **File ▸ Remove Folder or Collection** (or the **Remove** toolbar button,
  or right-click the folder). AnyFont asks "Are you sure you want to remove this folder?". Click **Yes**. Only the
  entry in Libraries is removed; the folder and its font files stay where they are. Check **Do not show this
  message again** if you don't want to be asked next time.

Fonts in a folder are not installed just because you added the folder. Select them and click **Install** when
you want other programs to use them.

### Collections

A collection is a list of fonts you put together yourself, for a project, a client or a style, no matter where
the font files are. A font can be in as many collections as you like. Adding a font to a collection never moves
or copies its file.

- **Favorites** is a ready-made collection under **My Collections**.
- **Create a collection.** Choose **File ▸ Add Collection** or the **Add collection** toolbar button. A new
  collection named "Collection 1" (or the next free number) appears in alphabetical order, with its name selected
  in an edit box. Type a name and press <kbd>Enter</kbd> (or click somewhere else). Press <kbd>Esc</kbd> to cancel
  and remove the new collection. A collection's name can't be empty or the same as another collection's; if it is,
  the edit box stays open until you change it.
- **Rename a collection.** Select it and press <kbd>F2</kbd>, or right-click it and choose **Rename**. Type the new
  name and press <kbd>Enter</kbd>. Favorites can't be renamed.
- **Add fonts.** Select fonts in the list, then right-click and choose **Add to collection ▸** *name*, or use the
  Font menu. You can also **drag** fonts from the list onto a collection in Libraries.
- **Remove fonts from a collection.** Open the collection, select the fonts and choose **Remove from** *name* in
  the Font menu or the right-click menu. From any other library, use **Remove from collection ▸**.
- **Remove a collection.** Open it and choose **File ▸ Remove Folder or Collection**, the **Remove** toolbar button,
  or right-click it and choose **Remove Folder or Collection**. AnyFont asks "Are you sure you want to remove this
  collection?". The fonts in it are not touched. Favorites can be removed too.

An empty collection shows the hint "To populate this collection, drag fonts from an added folder or another
collection."

The **Font info** tab of the [font properties panel](#font-info-tab) shows which collections a font belongs to.

---

## Filters

The **Filters** panel, under Libraries, narrows the font list down. Filters apply to the library that is open.
While any filter is checked, the status bar adds "(filtered)", for example `My fonts (filtered): 10 fonts`.

Each group of filters has a blue header. Click a header, or the double arrow at its right, to fold the group or
open it. Only **Font Status** is open at first. A group lists only the options that at least one font in the open
library matches, so the lists can differ from library to library.

| Group | Options |
|---|---|
| **Font Status** | Installed, Not installed, Protected System Fonts, Duplicates, and Corrupt (only when the library contains damaged or unsupported font files) |
| **Embedding Rights** | Editable, Installable, Preview and Print (what the font's licence allows when you embed it in a document) |
| **Font Technology** | OpenType - TrueType, OpenType - PostScript, TrueType, Variable Fonts, and Type 1 when there are any |
| **Weight** | Light, Regular, Bold |
| **Width** | Condensed, Normal, Expanded |
| **Style** | Decorative, Display, Monospaced, Sans Serif, Script, Serif, Symbol |
| **Character Range** | Arabic, Chinese Simplified, Chinese Traditional, Cyrillic, Devanagari, Greek, Hebrew, Japanese, Korean, Latin, Turkish |
| **OpenType** | The OpenType layout features a font supports: Alternate Annotation Forms, Asian Forms, Asian Vertical Metrics, Asian Widths, Capital Spacing, Capitals, Case-Sensitive Forms, Contextual Alternates, Contextual Ligatures, Discretionary Ligatures, Fraction, Historical Forms, Horizontal Kana Alternates, Historical Ligatures, Number Styles, Ordinals, Ornaments, Position, Slashed Zero, Standard Ligatures, Stylistic Alternate, Stylistic Sets, Swash Variants, Vertical Alternates and Rotation |

**How filters combine:**

- Options **in the same group** combine with **OR**. Checking **Serif** and **Sans Serif** shows fonts that are
  serif *or* sans serif.
- **Different groups** combine with **AND**. Checking **Installed** (Font Status) and **Bold** (Weight) shows only
  fonts that are installed *and* bold.

Filters and the Search box work together: the list shows the fonts that match both.

**Clear filters.** The funnel button to the right of the **Filters** title unchecks every filter at once. Its X
turns red while filters are active, and the button is greyed out when nothing is checked.

If no font matches, the list says "No fonts found. Remove or change filters or explore the font packs available in
Get More." with buttons to clear the filters or open Get More.

To make more room for Libraries, hide the whole panel with **View ▸ Hide filter list**.

---

## Search, sample text and preview size

### Search

Type in the **Search** box above the list to find fonts by family or style name. The list updates as you type,
and a drop-down under the box suggests matching font names; click one to jump to it.

While you search, the list is flat: no family headers and no indenting. The closest matches come first. Press
<kbd>Esc</kbd> or delete the text to clear the search and go back to the full list.

If nothing matches, the list says "No fonts found. Clear the Search box, type a different keyword, or explore the
font packs available in Get More."

### Sample text

The **Enter sample text** box sets the text drawn for every font in the list. Type anything you like, or open the
drop-down and pick one of these:

- **Font Name** (the default: each font shows its own name)
- abcdefghijklmnopqrstuvwxyz
- ABCDEFGHIJKLMNOPQRSTUVWXYZ
- 1234567890
- The quick brown fox jumps over the lazy dog.

Clear the box to go back to **Font Name**. AnyFont remembers the text you used.

Right-click the Search or sample text box for the usual **Undo**, **Cut**, **Copy**, **Paste**, **Delete** and
**Select All** commands.

### Preview size

At the right end of the status bar, under the list:

- Drag the **slider** to make the previews bigger or smaller (from 8 to 86 pixels).
- Click the **small A** to make them one step smaller, or the **big A** to make them one step bigger.

The tooltip reads "Change the font size in the font list." AnyFont remembers the size.

---

## The font list

The font list shows the fonts of the open library, each drawn in its own typeface.

**Family headers.** With **View ▸ Group by family** on (the default), a family with two or more styles gets a
header row with the family name in large type, and its styles are listed under it, indented. A family with only
one style has no header. A thin line separates families.

**Font rows.** Each style shows a small label such as `Arial (Regular, Normal)` (family, style and width) and,
below it, the sample text drawn in that font. Each named style of a variable font, such as Bahnschrift Light
Condensed, has its own row.

**Markers at the right of each row:**

| Marker | Meaning |
|---|---|
| Green bar | The font is installed. |
| No bar | The font is not installed (for example, a font in an added folder or a font you downloaded with Get More). |
| Lock on the green bar | A [protected system font](#protected-system-fonts). Windows needs it, so it can't be uninstalled or deleted. |
| **O** | An OpenType font. |
| **TT** | A TrueType font. |

**Tooltips.** Hover over a font to see:

- its name;
- **Installed**, **Not installed** or **Online font (Install or download to use)**;
- **Protected System Font**, if it is one;
- **Duplicate(s):** followed by the paths of the other copies, if there are any.

**Selecting fonts:**

- Click a font to select it. <kbd>Ctrl</kbd>+click adds or removes a font, and <kbd>Shift</kbd>+click selects a
  range.
- Click a **family header** to select the whole family. The header row is highlighted, and Install, Uninstall,
  Delete and the other Font commands then work on every style of the family.
- <kbd>Ctrl</kbd>+<kbd>A</kbd> selects every font in the list; <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> clears
  the selection.
- Use the arrow keys, <kbd>Page Up</kbd>, <kbd>Page Down</kbd>, <kbd>Home</kbd> and <kbd>End</kbd> to move;
  hold <kbd>Shift</kbd> to extend the selection, or <kbd>Ctrl</kbd> to move without selecting, then
  <kbd>Ctrl</kbd>+<kbd>Space</kbd> to select or deselect the current font.
- Start typing a font name to jump to the next font that starts with those letters.

Right-click a font (or press <kbd>Shift</kbd>+<kbd>F10</kbd> or the Menu key) for the [Font menu](#font-menu)
commands. If you right-click a font that is not selected, it is selected first; right-clicking inside a selection
keeps it.

**Recently used fonts.** Turn on **View ▸ Show recently used fonts** to list the fonts you used most recently at the
top of the list, above a thin line, so you can get back to them quickly.

**When the list needs a refresh.** If your fonts changed in a way AnyFont could not pick up by itself, an
information strip with a **Refresh** button appears above the list, and **View ▸ Refresh the Font List** becomes
available. Click either to scan again.

---

## Font properties

The font properties panel on the right shows what is inside the selected font. Show or hide it with
**View ▸ Show font properties**, the **Show font properties** toolbar button, or
<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>I</kbd>. It is hidden at first, and you can make it wider by dragging its left
edge.

Three buttons at the top of the panel switch between its tabs:

| Tab | Tooltip |
|---|---|
| **Glyphs** ("g") | Preview and filter available glyphs for a selected font. |
| **Font info** ("A" with an "i") | Show detailed font information. |
| **Variable font** ("AA") | Adjust variable font properties. |

### Glyphs tab

The Glyphs tab shows every character and symbol (glyph) in the selected font. It has two parts.

**Glyph Filters** (top) narrows the grid down:

- **Common**: Currency, Mathematical Symbols, Numbers, Punctuation & Separators, Symbols, and Marks (when the font
  has accent marks that combine with letters).
- **Scripts**: the writing systems the font covers, such as Latin, Greek or Cyrillic.

Checked filters combine with OR: checking **Numbers** and **Currency** shows numbers and currency signs. The funnel
button next to **Glyph Filters** ("Remove all selected glyph filters.") unchecks them all. If the panel is too short
to show the filters, they open in a small pop-up list instead; click **Close** when you are done.

**Glyph Preview** (bottom) is a grid of the font's glyphs:

- The first cell is the space character, shown as a green ⎵ mark.
- Click a glyph to select it. If the glyph has **alternates** (other designs of the same character, such as swash
  or stylistic variants), a pop-up shows them.
- To **copy a glyph**, select it and press <kbd>Ctrl</kbd>+<kbd>C</kbd>, or double-click it. You can then paste the
  character into any other program.
- Drag the line between Glyph Filters and Glyph Preview to give either part more room.

At the bottom of the panel, `262 glyphs` (for example) tells you how many glyphs the grid shows after the filters.
The small A, slider and big A next to it change the size of the glyph cells ("Change the glyph size in the Glyph
Preview area.").

The grid shows a message instead of glyphs when:

- more than one font is selected: "Multiple fonts are selected. To view a glyph set, choose only one font.";
- the filters exclude every glyph: "No matching glyphs";
- the font is an online font that isn't on your PC yet: "Install or download the font to preview glyphs."

### Font info tab

The Font info tab lists the details of the selected font. When several fonts are selected, it shows the one you
worked with last.

| Row | What it shows |
|---|---|
| **Family:** | The family name, for example *Algerian*. |
| **Sub-family:** | The style, for example *Regular* or *Bold Italic*. |
| **File Path:** | Where the font file is. Hover over a long path to see all of it. |
| **File Size:** | The size of the file, for example *74.8 KB*. |
| **Version:** | The version written in the font, for example *Version 1.57*. |
| **Font technology:** | OpenType - TrueType, OpenType - PostScript or TrueType. |
| **Code Page:** | The Windows code pages (character sets) the font supports, one per line, for example *1252 (ANSI - Latin I)*. |
| **Italic:** | Yes or No. |
| **Weight:** | Light, Normal, Bold and so on. |
| **Width:** | Condensed, Normal, Expanded and so on. |
| **Embedding:** | What the font's licence allows when you embed it in a document: Editable, Installable, Preview and Print, or No Embedding. |
| **Manufacturer:** | The company that made the font. |
| **Designer:** | The designer, when the font names one. |
| **Copyright:** | The font's copyright notice. |
| **Collection:** | The collections that contain the font. |
| *Vendor link* | When the font names its vendor's website, a link below Collection. Click it to open the site in your browser. |

For a damaged font file, the tab also shows the reason the file could not be read. See
[Corrupt and unsupported font files](#corrupt-and-unsupported-font-files).

### Variable font tab

A variable font holds a whole range of styles in one file, along *axes* such as Weight or Width.

When exactly one variable font is selected, this tab shows one row for each of its axes, with:

- the axis name (Width, Weight and so on);
- a slider from the axis' smallest to its largest value;
- a box with the exact value, for example *300.0*.

Move a slider or type a value, and the font's preview in the list changes at once. This only changes the preview:
the font file itself is never modified.

If no variable font is selected, or several fonts are, the tab says "You don't have a variable font selected, or
you've selected multiple fonts. Choose one variable font to see and adjust its properties." Use the **Variable
Fonts** filter under Font Technology to find them.

---

## Installing, uninstalling and deleting fonts

Install, Uninstall and Delete show a progress window ("Installing *font*... 40%"). Click **Cancel** to stop after
the font that is being processed; the button reads **Close** when the work is done.

When an operation finishes, Windows and your other programs are told that the fonts changed. Most programs
notice new fonts right away; some only see them after you restart that program.

### Installing fonts for all users or only for you

Select one or more fonts that are not installed (for example, in an added folder or fonts downloaded with Get
More) and click **Install** on the toolbar, in the Font menu or in the right-click menu. To install every font of
a folder or collection, right-click it in Libraries and choose **Install**.

AnyFont installs fonts **for all users**, so everyone who signs in to this PC can use them. This changes the
Windows Fonts folder, so Windows shows its User Account Control prompt and asks for administrator permission, once
for the whole operation, not once per font. (When AnyFont already runs as administrator there is no prompt.)

If you choose **No** in the Windows prompt, nothing is installed and AnyFont offers to install the fonts **only for
you** instead. Click **Install for me** to do that, or **Cancel** to leave the fonts as they were.

| | For all users (Install) | Only for you (Install for me) |
|---|---|---|
| Who can use the font | Everyone who signs in to this PC | Only you, in your Windows account |
| Where the file goes | The Windows Fonts folder | Your personal fonts folder |
| Administrator permission | Needed: Windows shows its User Account Control prompt | Not needed |

Notes:

- If the same font is already installed, AnyFont doesn't install it twice. If an older file with the same font name
  is installed, the new one replaces it (protected system fonts are never replaced).
- Type 1 fonts (`.pfb` with `.pfm`) can only be installed for all users, and both files must be in the same folder;
  **Install for me** can't install them.
- Installed fonts show a green bar in the list and match the **Installed** filter.

### Uninstalling fonts

Select installed fonts and click **Uninstall** (toolbar, Font menu or right-click menu). To uninstall every
installed font of a library, right-click it in Libraries and choose **Uninstall**.

Uninstalling removes the font from Windows, so other programs can no longer use it, but the font file is not
deleted when it lives outside the Windows font folders. Fonts in an added folder simply show up as **Not
installed** afterwards, ready to be installed again.

Uninstalling a font that was installed for all users needs administrator permission. Protected system fonts can't
be uninstalled.

### Deleting font files

**Delete** removes font files from your PC. Select the fonts and click **Delete**. The **Delete fonts** window
asks "Are you sure you want to delete this font?" (or "these fonts") and lists the files, each with a check box:

- Uncheck the files you want to keep, or use **Select all** to check or uncheck all of them.
- Check **Do not show this message again.** if you don't want to be asked next time.
- Click **OK** to delete, or **Cancel**.

What happens:

- The files go to the **Recycle Bin**, so you can restore them from there if you change your mind.
- An installed font is uninstalled first.
- **Read-only fonts cannot be deleted.** Neither can [protected system fonts](#protected-system-fonts).
- Deleting fonts from the Windows Fonts folder needs administrator permission.

### Fonts that are in use: the restart prompt

Windows can't remove a font file while a program has it open. If that happens during Uninstall or Delete,
AnyFont removes all the other fonts and then shows the restart prompt: "The following fonts cannot be uninstalled
because they are in use by one or more applications. To uninstall these fonts, you must first restart your
computer. Do you want to restart your computer now?", with the list of fonts in use. (For Delete, the message says
"deleted".)

| Button | What it does |
|---|---|
| **Reboot now** | Marks the fonts for removal and restarts Windows right away. Save your work in other programs first. |
| **Reboot later** | Marks the fonts for removal. They are removed the next time you restart Windows. |
| **Ignore** | Leaves those fonts alone and continues. They stay installed. |
| **Cancel** | Stops without doing anything more to those fonts. |

Tip: closing the program that uses the font (for example, a word processor or design program) and trying again
often avoids the restart.

### Protected system fonts

Windows itself needs some fonts, such as Segoe UI, Arial, Tahoma and Marlett, and protects their files. AnyFont
marks these **protected system fonts** with a **lock** on the green bar, and their tooltip says "Protected System
Font".

You can preview them, look at their glyphs and details, add them to collections and open their file location, but
**Install, Uninstall, Download and Delete are greyed out** for them, and AnyFont never removes or replaces them,
even with administrator permission. To see them all, check **Protected System Fonts** under **Filters ▸ Font
Status**.

---

## Duplicate fonts

The same font often ends up on a PC more than once: installed and also in a downloads folder, or installed twice
in two versions. AnyFont recognises copies of the same font and, by default, shows only one of them in each
library. A copy of an installed font counts as installed.

- **See the copies.** Choose **Font ▸ Show duplicate fonts** or click the **Show duplicate fonts** toolbar button.
  Every copy then gets its own row. The command changes to **Hide duplicate fonts**, and the button stays
  highlighted until you turn it off.
- **Find fonts that have copies.** Check **Duplicates** under **Filters ▸ Font Status**.
- **See where the copies are.** Hover over a font: its tooltip lists the other copies under **Duplicate(s):**.
  With duplicates shown, **Font ▸ Open font file location ▸** lists every copy's file.

To clean up, show the duplicates, select the copies you don't need and **Delete** them (they go to the Recycle
Bin) or remove them from your collections.

---

## Corrupt and unsupported font files

AnyFont reads TrueType and OpenType fonts (`.ttf`, `.otf`, `.ttc`, `.otc`), WOFF web fonts (`.woff`) and Type 1
fonts (`.pfb`/`.pfa` with `.pfm` or `.afm`).

A file that can't be read never stops AnyFont. Instead:

- The file is left out of the normal list, and a **Corrupt** option appears under **Filters ▸ Font Status**.
- Check **Corrupt** to list these files. The **Font info** tab shows why each one could not be read, for example
  "WOFF2 files are not supported" or a Type 1 font whose outline file (`.pfb`) is missing.
- You can open the file's location or delete the file as usual.

Typical causes are a damaged or partly downloaded file, a file with the wrong extension, or a format AnyFont
doesn't support (WOFF2 web fonts and old `.fon` bitmap fonts).

If AnyFont reports "Your font database has been corrupted. To reset the database, you must restart AnyFont while
holding down Shift.", see [Rebuilding the font database](#rebuilding-the-font-database).

---

## Importing and exporting folders and collections

You can save your folders and collections to a file, to back them up or to move them to another PC.

**Export.** Choose **File ▸ Export Folders and Collections...**, pick a place and a name (the suggested name is
`AnyFont Folders and Collections.xml`) and click **Save**. The file lists your added folders (with their settings)
and every collection with its fonts. The command is available once you have at least one folder or collection.

**Import.** Choose **File ▸ Import Folders and Collections...**, pick an `.xml` file and click **Open**. AnyFont:

- adds the folders you don't have yet (folders you already have are skipped);
- creates the collections, or adds the fonts to a collection with the same name;
- finds each collection font among the fonts on this PC, even if the file is in a different place.

The command is available once AnyFont has finished scanning your fonts after it starts.

**"We can't find the following:"** If some folders or fonts in the file are not on this PC, AnyFont lists them in
this message. Click **OK**. If [font matching](#font-matching-with-panose) is turned on, the **Font Matching
Results** window then opens so that you can pick a replacement for each missing font.

---

## Font matching with PANOSE

PANOSE is a classification system that describes how a typeface looks: serif or sans serif, weight, proportions,
contrast and so on. AnyFont uses it to suggest the closest font you have when a collection refers to a font that
is not on your PC, for example after you import collections from another computer.

AnyFont looks for a replacement in this order: a substitution you saved as an exception, one you chose before, a
font with exactly the same name, another style of the same family, the nearest PANOSE match, and a font with a
similar name. If nothing fits, it uses a default font.

### Font Matching options

Open **Options** (the **⚙** button in the title bar). Under the **Font Matching** header, choose when AnyFont
offers font matching:

| Mode | What it does |
|---|---|
| **Text** (default) | Opens the Font Matching Results window for missing fonts, matched by family. |
| **Text and styles** | The same, and also matches each missing style (weight, width, italic) on its own. |
| **Never show** | Never opens the Font Matching Results window. |

The **Exceptions...** button opens the Font Matching Exceptions window.

### Font Matching Exceptions

An exception is a fixed rule: "whenever *this* font is missing, use *that* font". Exceptions always win over the
PANOSE suggestions.

1. Type the name of the missing font in **Missing Font:**.
2. Pick the font to use instead in **Substituted Font:**.
3. Click **Add Exception**. It appears under **List of current exceptions:**, with the columns **Missing Font** and
   **Substituted Font**.

To delete a rule, select it in the list and click **Remove Exception**. Click **OK** to keep your changes or
**Cancel** to discard them. The **?** button opens this guide.

### Font Matching Results

This window opens when fonts are missing (see [Importing](#importing-and-exporting-folders-and-collections)) and
lists each missing font with the match AnyFont proposes.

While AnyFont is still looking, the window shows "Working, please wait..." and a **Stop** button. Stopping ends
the search early: "Stopping will end the PANOSE matching process. All missing fonts will revert to a default
selection."

For each missing font in the list:

- **Use the PANOSE suggested match** keeps the font AnyFont proposes.
- **Substitute font with:** lets you pick another font from the list yourself.
  - **Show fonts with the same code page** limits that list to fonts that support the same character set as the
    missing font.
  - **Show all fonts** lists every font.
- **Save this exception for future font matching** stores your choice as an
  [exception](#font-matching-exceptions), so it is used automatically next time.
- **Make this substitution:**
  - **Permanent - keep these fonts** keeps the replacement fonts in your collections.
  - **Temporary - use original fonts** uses the replacement only for now; the collection keeps pointing at the
    original font, so it is used again as soon as it is on your PC.

Click **OK** to apply your choices or **Cancel** to close without changes. **?** opens this guide.

### Type 1 fonts

Type 1 (PostScript) fonts are an old font format. Windows and many programs are dropping support for it. When
AnyFont finds Type 1 font files, for example in a folder you just added, it shows the **Type 1 fonts detected**
message once for that folder. It explains that these fonts may not be supported in the future and suggests using
TrueType (TTF) or OpenType (OTF) fonts instead.

- **Replace fonts** opens the [Font Matching Results](#font-matching-results) window, where you can choose
  OpenType or TrueType replacements for the Type 1 fonts in your collections.
- **Help** opens this guide.
- Check **Do not show again** if you don't want to see the message any more.

---

## Get More: the free online font catalog

Choose **File ▸ Get More...** or click the **Get More** toolbar button to browse a free online font catalog:
Google Fonts. Every font in it is free and released under an open-source licence. You don't need an account, and
there is nothing to buy.

**Finding fonts:**

- Type in the **search box** at the top and click **Search**. The **×** in the box clears it.
- **Sort by:** **Featured** (the default), **Popular**, **Name** or **Newest**.
- **Filter by:** on the right, check one or more categories: **Serif**, **Sans Serif**, **Display**,
  **Handwriting**, **Monospace**. **Free** is always checked. Check **My Library** to see only the families you
  already downloaded.

**Each family in the list shows:**

- the family name drawn in that font (the font is fetched as the row comes into view; until then the name is
  drawn in AnyFont's normal font);
- its category, its designer and how many styles it has;
- **Free**, and a **Download** button. The button reads **Downloaded** when the family is already in your Content
  folder.

**Downloading.** Click **Download**. A progress window shows "Downloading *font*... 40%"; click **Cancel** to stop.
AnyFont saves the font files, together with their licence file, to:

```
Documents\AnyFont\Content\Fonts\<Family>\
```

**Using downloaded fonts.** Downloaded fonts appear in **My fonts** as **Not installed** (check the **Not
installed** filter to find them quickly). You can preview them and add them to collections right away. To use
them in other programs, select them and click **Install**.

**Offline.** AnyFont keeps a copy of the catalog list and refreshes it once a day. Without an Internet connection,
Get More shows the copy it already has; if there is none yet, it shows "Remote font catalog cannot connect to the
internet." Connect to the Internet and open Get More again.

---

## Options

Click the **⚙** button in the title bar to open **Options**. Changes apply right away; click **OK** to close the
window.

### Appearance

**Size** makes the whole AnyFont interface bigger or smaller, from **75 %** to **225 %**. Drag the slider (it
snaps to steps of 25 %), or type a value in the box. **Reset to Default** goes back to 100 %.

The size is relative to your Windows display scaling: 200 % makes AnyFont look twice as large as other Windows
programs. Very large sizes may crop parts of the window on a small screen.

### Color

- **Theme:** **Light**, **Medium**, **Dark** or **Black**. Dark is the default.
- **Window border:** the colour of the thin line around the AnyFont window. The first choice is the theme's own
  border colour; you can also choose your Windows accent colour or one of a few other colours.

### Font Matching

Choose **Text**, **Text and styles** or **Never show**, and open the exceptions with **Exceptions...**. See
[Font matching with PANOSE](#font-matching-with-panose).

### Product updates

The same **Allow product updates:** choice as in the [Updates](#updates) window.

---

## Updates

AnyFont can keep itself up to date. Open the **Updates** window with **Help ▸ Check for Updates...** or the
**Updates** button in the title bar.

The window shows "You are using AnyFont *x.y.z*." and checks for a new version at once ("Checking for
updates..."). Then it shows either "AnyFont is up to date." or "AnyFont *x.y.z* is available." together with the
new version's release notes.

| Button | What it does |
|---|---|
| **Install Now** | Downloads the new version (with progress), closes AnyFont, installs the update without asking anything more, and starts AnyFont again. Your settings, collections and fonts are kept. |
| **Release Notes** | Opens the new version's release page in your browser. |
| **Check Now** | Checks again. |
| **Close** | Closes the window. |

**Allow product updates:**

| Choice | What it does |
|---|---|
| **Keep the product up to date (recommended)** | AnyFont downloads updates by itself, but asks you before installing them. |
| **Check for updates and let me choose when to install them** | AnyFont tells you when a new version is available; you choose when to download and install it. |
| **Never check for updates** | AnyFont never looks for updates by itself. **Help ▸ Check for Updates...** still checks when you ask. |

**At start-up.** Unless you chose **Never check for updates**, AnyFont checks in the background every time it
starts, so it never makes the start slower. When a newer version exists, a message says "There is a new version
of AnyFont available." and shows the new version and yours:

- **OK** installs the new version now (AnyFont closes, updates and starts again);
- **Later** keeps your current version; AnyFont asks again the next time it starts.

While an update is available, the **Updates** button in the title bar shows a small dot.

Checking for updates and installing them needs an Internet connection. Without one, the window says "An Internet
connection is required. Please connect to the Internet and try again."

---

## Report a Bug and Submit Ideas and Feedback

Found a problem, or have an idea? Tell us from inside AnyFont. You don't need a GitHub account.

- **Help ▸ Report a Bug...** opens the bug form.
- **Help ▸ Submit Ideas and Feedback...** opens the idea form.

**The forms:**

| Report a Bug | Submit Ideas and Feedback |
|---|---|
| **Title (a short summary):** | **Title (your idea in a few words):** |
| **What happened?** | **Describe your idea or feedback:** |
| **Steps to reproduce (optional):** | **How would it help you? (optional)** |
| **What did you expect to happen? (optional)** | – |

Both forms also have:

- **Attach a file (optional):** click **Attach File...** to add one file, of the types GitHub accepts: images and
  videos up to 10 MB, other files (such as `.zip`, `.txt` or `.pdf`) up to 25 MB. **To send a font file, put it in a
  `.zip` file first.** A screenshot is often the most helpful attachment.
- **Include system information:** adds your AnyFont version, your Windows version, and the window size and scale.
  You see exactly what will be sent before you send it. Uncheck the box to leave it out.

Click **Send**. Your report is filed as a new issue on the AnyFont project on GitHub, and the window shows the link
to it so you can follow it.

**If the reporting service can't be reached**, AnyFont opens GitHub's new-issue page in your browser with your
report already filled in. Sign in to GitHub if asked, review the text and click **Create**. If you attached a file,
AnyFont opens the folder that contains it: drag the file from there into the description box on GitHub.

If you send several reports within a minute, you may be asked to wait a minute before trying again.

**See what others reported.** **View reported bugs** (in the bug form) and **View submitted ideas** (in the idea
form) open the list of issues on GitHub, so you can check whether your problem or idea is already there.

**After a crash.** If AnyFont closed unexpectedly, it offers to report the problem the next time it starts, with
the crash details already filled in. Nothing is sent unless you choose to send it.

---

## About AnyFont

**Help ▸ About AnyFont...** shows:

- the version and its release month, for example "Version 0.1.0 - November 2026 Release";
- the copyright notice;
- the **Source ID**: the exact build number of your copy. Please quote it when you report a bug.

Buttons:

| Button | What it does |
|---|---|
| **System Info...** | Opens **System Info**. Choose a category in **Choose a category:**: **System** (processor, memory and so on), **Display**, **AnyFont EXEs & DLLs** and **System DLLs**. **Save** writes all categories to a `.txt` file, which is handy to attach to a bug report. |
| **Legal Notices...** | Lists the open-source components AnyFont uses and their licences. **Print** prints the list; **OK** closes it. |
| **Close** | Closes the About window. |

---

## Keyboard shortcuts

**Main window**

| Keys | Action |
|---|---|
| <kbd>Ctrl</kbd>+<kbd>A</kbd> | Select all fonts in the list (in a text box: select all text) |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> | Deselect all |
| <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>I</kbd> | Show or hide the font properties panel |
| <kbd>Alt</kbd>+<kbd>F4</kbd> | Exit AnyFont |
| <kbd>Alt</kbd> or <kbd>F10</kbd> | Move to the menu bar |
| <kbd>Alt</kbd>+<kbd>F</kbd>, <kbd>Alt</kbd>+<kbd>E</kbd> | Open the File menu, the Edit menu |
| <kbd>Shift</kbd>+<kbd>F10</kbd> or the Menu key | Open the right-click menu of the focused font or library |
| <kbd>Tab</kbd> / <kbd>Shift</kbd>+<kbd>Tab</kbd> | Move between the sample text box, Search, the font list, Libraries, Filters and the properties panel |

**Font list**

| Keys | Action |
|---|---|
| <kbd>↑</kbd> <kbd>↓</kbd> <kbd>Page Up</kbd> <kbd>Page Down</kbd> <kbd>Home</kbd> <kbd>End</kbd> | Move through the list |
| <kbd>Shift</kbd> + a move key | Extend the selection |
| <kbd>Ctrl</kbd> + a move key | Move without changing the selection |
| <kbd>Ctrl</kbd>+<kbd>Space</kbd> | Select or deselect the current font |
| Letter keys | Jump to the next font whose name starts with what you type |

The <kbd>Delete</kbd> key does nothing in the font list, so you can't delete font files by accident. Use the
**Delete** command instead.

**Libraries**

| Keys | Action |
|---|---|
| <kbd>↑</kbd> <kbd>↓</kbd> | Move between libraries |
| <kbd>Enter</kbd> | Open the library |
| <kbd>←</kbd> <kbd>→</kbd> | Fold or open the Folders or My Collections section |
| <kbd>F2</kbd> | Rename the selected collection |
| <kbd>Enter</kbd> / <kbd>Esc</kbd> | While naming a collection: keep the name / cancel |

**Other**

| Keys | Action |
|---|---|
| <kbd>Esc</kbd> in the Search box | Clear the search |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> or double-click in Glyph Preview | Copy the selected glyph |
| <kbd>Enter</kbd> / <kbd>Esc</kbd> in a window | The highlighted (blue) button / Cancel or Close |

---

## Language

AnyFont speaks the language of Windows. If you picked a language while installing AnyFont, it uses that language
instead.

AnyFont is available in English, Chinese (Simplified), Chinese (Traditional), Czech, Dutch, French, German,
Italian, Japanese, Polish, Portuguese (Brazil), Russian, Spanish, Swedish and Turkish. If your Windows language is
not among them, AnyFont uses English. To change the language later, run the AnyFont setup again and choose another
language.

Font names follow the language too: a font that has a name in your language shows it. Turn on
**View ▸ Show Latin names** to see the English names instead.

---

## Where AnyFont keeps your data

| What | Where |
|---|---|
| Settings (window layout, options, view choices) | `%APPDATA%\AnyFont` |
| Font database (your fonts, folders, collections, font matching exceptions), logs and crash reports | `%LOCALAPPDATA%\AnyFont` |
| Fonts downloaded with Get More | `Documents\AnyFont\Content\Fonts`, one folder per family |

Type `%APPDATA%\AnyFont` or `%LOCALAPPDATA%\AnyFont` in the File Explorer address bar to open those folders.

**Uninstalling AnyFont** removes the program, its shortcuts and its **Open with** entries. It keeps your settings,
the font database, the Content folder with your downloaded fonts, and every font you installed, so nothing is lost
if you install AnyFont again later. Delete those folders yourself if you want to remove everything.

### Rebuilding the font database

If the font list looks wrong, or AnyFont tells you the font database has been corrupted, rebuild it:

1. Close AnyFont.
2. Hold down <kbd>Shift</kbd> and start AnyFont. Keep holding <kbd>Shift</kbd> until AnyFont asks whether to rebuild
   the font database.
3. Confirm. AnyFont scans your fonts again from scratch, which can take a minute on a PC with many fonts.

Tip: export your folders and collections first (**File ▸ Export Folders and Collections...**) so that you can import
them again if needed.

---

## Troubleshooting and FAQ

**I installed a font, but it doesn't show up in another program.**
Most programs only read the list of fonts when they start. Close the program completely and start it again. If you
installed the font **only for you** (you chose **Install for me** after cancelling the Windows prompt) and another
Windows user can't see it, uninstall it and install it again, this time answering **Yes** in the Windows prompt.

**I can't uninstall or delete a font: AnyFont says it is in use.**
A program has the font open. Close the programs that might use it (word processors, design and layout programs,
browsers) and try again. If it still doesn't work, choose **Reboot later** in the restart prompt; the font is
removed the next time you restart Windows. See [Fonts that are in use](#fonts-that-are-in-use-the-restart-prompt).

**Uninstall and Delete are greyed out for some fonts.**
These are probably [protected system fonts](#protected-system-fonts) (look for the lock). Windows needs them, and
AnyFont never removes them. Delete is also unavailable for read-only files. A selection that has no installed font
can't be uninstalled either.

**Windows asked me for administrator permission.**
Installing a font for all users, or uninstalling or deleting a font in the Windows Fonts folder, changes Windows for
everyone, so Windows asks first. If you choose **No** when installing, AnyFont offers **Install for me**, which
installs the fonts only for you and needs no permission. For Uninstall and Delete, **No** leaves those fonts as they
were.

**A font file doesn't appear in the list.**
It may be damaged or in a format AnyFont doesn't support. Check **Corrupt** under **Filters ▸ Font Status** and look
at the **Font info** tab for the reason. See [Corrupt and unsupported font files](#corrupt-and-unsupported-font-files).
If the file is in a folder you haven't added yet, add the folder with **File ▸ Add Folder...**.

**The list doesn't show a change I made to my fonts.**
Click **Refresh** in the information strip above the list, or choose **View ▸ Refresh the Font List** when it is
available. If the list still looks wrong, [rebuild the font database](#rebuilding-the-font-database).

**Get More says "Remote font catalog cannot connect to the internet."**
AnyFont can't reach the catalog. Check your Internet connection, and that a firewall or proxy isn't blocking
AnyFont, then open Get More again. Fonts you already downloaded stay available in My fonts. Update checks need the
Internet too.

**Where are the fonts I downloaded?**
In `Documents\AnyFont\Content\Fonts`, one folder per family, together with their licence files. They are listed in
**My fonts** as **Not installed** until you install them.

**The interface is too small or too large.**
Change **Size** in [Options](#options) (75 % to 225 %), or click **Reset to Default**.

**I removed a folder or collection by mistake.**
Removing never deletes font files. Add the folder again with **File ▸ Add Folder...**, or import a file you exported
earlier with **File ▸ Import Folders and Collections...**.

**How do I report a problem?**
Choose **Help ▸ Report a Bug...**, describe what happened and click **Send**. Include the **Source ID** from
**Help ▸ About AnyFont...** if you mention a version, and attach a screenshot or the **System Info** file if it
helps. See [Report a Bug and Submit Ideas and Feedback](#report-a-bug-and-submit-ideas-and-feedback).
