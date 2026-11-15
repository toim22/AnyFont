# Phase 07 – Shortcuts and keys

Dossier: `07-shortcuts/shortcuts.md`. UI spec: [`docs/UI-SPEC.md` §8](../UI-SPEC.md#8-shortcuts-and-keys).

## What the dossier says

The program defines four keyboard accelerators:

| Shortcut | Command | Binding |
|---|---|---|
| Ctrl+A | Select All | `AssetListDS` select all |
| Ctrl+Shift+A | Deselect All | `AssetListDS` deselect all |
| Ctrl+Alt+I | Inspectors (command 64409843) | check state bound to `AssetListDS.PreviewShown` / `UIMgr.HideDockers` |
| Alt+F4 | Exit | window close |

Per-control key handling:
- The font list has `onKey -> AssetListDS.OnKey`. It also has `virtualKeyToIndex` / `virtualIndexToKey`,
  which provide keyboard navigation and type-ahead in the virtual list.
- The folder tree has `FolderTreeDS.OnKey`.
- `onDeleteKey` exists only on the palette editor, which is a suite dialog Font Manager never shows. The font
  list therefore has **no** Delete-key binding.

The menu captions show access keys only on &File, &Edit, E&xit, &Product Help... and &About... . Font, View and
Help have none.

## How AnyFont implements it

Shortcuts are part of the command table (`commands.rs`). A shortcut runs a command only when the command is
enabled, and its text appears right-aligned in the menu.

| Key | Where it works | AnyFont action |
|---|---|---|
| Ctrl+A | main window, focus not in a text box (in a text box it selects the text) | Select All |
| Ctrl+Shift+A | main window | Deselect All |
| Ctrl+Alt+I | main window | toggles the font properties panel (= View ▸ Show/Hide font properties). There is no menu item for it, as in the original |
| Alt+F4 | main window | Exit |
| Alt+F / Alt+E | main window | open File / Edit |
| Alt, F10 | main window | focus the menu bar; ← → between menus, ↓ opens one, Esc leaves |
| Shift+F10, Menu key | font list, Libraries | context menu for the focused row |
| ↑ ↓ PgUp PgDn Home End | font list | move the current row; Shift extends the selection, Ctrl moves without selecting |
| Ctrl+Space | font list | toggles the selection of the current row |
| letters / digits | font list | type-ahead: jumps to the next font whose name starts with the typed text (resets after 1 s) |
| ↑ ↓ | Libraries | move between rows. The original's tree selection opens the library at once, as a click does; Enter also opens it |
| ← → | Libraries, on a section header | collapse / expand |
| F2 | Libraries, user collection | rename inline (AnyFont addition, UI-SPEC §11 #9) |
| Enter / Esc | inline collection name editor | commit / cancel |
| Ctrl+C, double-click | Glyph Preview | copy the selected glyph's character (UI-SPEC §11 #11) |
| Esc | Search box | clears the search **[decide]** |
| Enter / Esc | dialogs | default button / Cancel (Close when there is no Cancel) |
| Tab / Shift+Tab | everywhere | focus order: menu bar is skipped. In the main window: sample text → search → font list → Libraries → Filters → properties panel. In dialogs: the order of the controls |
| Delete | font list | no binding (the dossier defines none, and Delete removes files) **[decide]** |

## Verification

`crates/anyfont/tests` gets a key test for each row above, using `egui_kittest` with the sandbox font system.
Example: Ctrl+A in C1 enables Uninstall; Ctrl+Shift+A then disables it; Ctrl+Alt+I shows and hides the panel.
