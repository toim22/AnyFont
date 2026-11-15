# AnyFont – bugs found by the end-to-end tests

Each bug has a test that reproduces it, marked `#[ignore = "BUG-n: ..."]`, so `cargo test` stays green until it is
fixed. To check a fix, run the ignored test, for example
`cargo test -p anyfont --test e2e_libraries -- --ignored export_leaves_out_the_content_folder`. When it passes,
remove its `#[ignore]` and close the entry below (or delete it).

IDs are grouped by area: 1xx services dialogs, 2xx window chrome / i18n, 3xx main window content, 4xx real system
(found in a Windows Sandbox run; now tested on GitHub Actions, `e2e-real.yml`). Line numbers are those of the code when the bug was found.

| ID | Area | Title | Test |
|---|---|---|---|
| BUG-101 | Report a Bug | Fields are cut by Unicode characters, but the relay counts UTF-16 units | `e2e_bugreport::non_bmp_title_is_cut_to_the_relay_limit` |
| BUG-102 | Font matching | OK in Font Matching Results never puts the substitute into the imported collection | `e2e_getmore_matching::import_with_missing_fonts_links_the_chosen_substitute` |
| BUG-201 | i18n | Captions with an `&` access key are shown in English in every other language | `e2e_i18n::untranslated::<lang>` (14 tests) |
| BUG-202 | i18n | Updates dialog: the update-mode choice and its note are cut off | `e2e_i18n::overflow::<lang>` (11 tests) |
| BUG-203 | i18n / accessibility | Plain egui buttons give screen readers the English text | `e2e_i18n::a11y::<lang>` (14 tests) |
| BUG-301 | Edit boxes | Right-clicking a text box clears its selection: Cut, Copy and Delete are always disabled | `e2e_filters_list::edit_box_menu_cut_copy_delete_with_a_selection` |
| BUG-302 | Font list | Double-clicking a font row never records it as recently used | `e2e_filters_list::recently_used_fonts_after_a_double_click` |
| BUG-303 | Export | Export includes AnyFont's hidden content folder | `e2e_libraries::export_leaves_out_the_content_folder` |
| BUG-304 | Import | Import looks for a collection's fonts before the imported folders are scanned | `e2e_libraries::import_does_not_report_fonts_of_the_imported_folders_as_missing` |
| BUG-305 | Accessibility | A check box's accessible name loses every `&` of its caption | `e2e_properties_options::glyph_filter_names_keep_the_ampersand` |
| BUG-306 | Libraries | Removing Favorites only empties it; Favorites stays in Libraries | `e2e_libraries::removing_favorites_removes_it_for_good` |
| BUG-401 | Setup / start-up | AnyFont does not start on a Windows without the Visual C++ runtime (VCRUNTIME140.dll) | `e2e-real.yml` check "runs on a Windows without the Visual C++ runtime" (`tests/real-system/run-real.ps1`) |

---

## BUG-101 – bug report fields are cut by Unicode characters, the relay counts UTF-16 units

- **Steps:** Help ▸ Report a Bug..., type a title of 150 × 😀 (or any text with characters outside the BMP), fill the
  rest, Send.
- **Expected:** the title is cut to the relay's limit (200 UTF-16 code units, `services/issue-relay/src/report.ts`),
  so the relay accepts the report.
- **Actual:** the title is cut to 200 *characters* = 300 UTF-16 units; the real relay answers 400 and AnyFont falls
  back to the browser.
- **Suspected:** `crates/anyfont/src/bugreport.rs:288` `clip()` counts with `.chars()`; used by `relay_body` (~293)
  for every field.

## BUG-102 – Font Matching Results: OK never links the substitute into the imported collection

- **Steps:** Options ▸ Font Matching = Text. File ▸ Import a libraries XML whose collection references a face that
  is not on this PC. In PANOSE Font Matching Results choose "Substitute font with:" Arial, OK.
- **Expected:** the collection contains Arial (status "Imported: 1 fonts" / the collection lists 1 font).
- **Actual:** the collection stays empty ("0 fonts"); `Matching::take_resolved()` is never called by the shell.
- **Suspected:** `crates/anyfont/src/app.rs` ~958 `services()` (the result of `matching.ui` only triggers a reload) and
  `crates/anyfont/src/commands.rs` ~556 `import` (the missing fonts are handed to the dialog but the chosen
  substitutes are never added to the collection).

## BUG-201 – captions with an `&` access key are shown untranslated (all 14 non-English languages)

- **Steps:** start with `/Lang:pt-BR` (or any other language); look at the menu bar, File ▸ Exit, Help ▸ Check for
  Updates..., Options ▸ Reset to Default, Options ▸ PANOSE ▸ Exceptions.
- **Expected:** Arquivo, Editar, Sair, Verificar atualizações..., Redefinir para o padrão, Texto e estilos, Nunca mostrar,
  Exceções..., Remover exceção, Adicionar exceção, Lista de exceções atuais:
- **Actual:** File, Edit, Exit, Check for Updates..., Reset to Default, Text and styles, Never show, Exceptions...,
  Remove Exception, Add Exception, List of current exceptions: (the screen-reader label is translated, the drawn
  text is not; the menu-bar boxes are sized for the English words).
- **Suspected:** `crates/anyfont/src/i18n.rs:542-556` `caption()` strips the `&` before the lookup, but the
  translation files keep it in their keys (`&File	&Arquivo`), so the lookup misses. Menus, push buttons and radio
  buttons all go through it.

## BUG-202 – Updates dialog text is cut off (cs, nl, fr, de, it, pl, ru, es, sv, tr, pt-BR)

- **Steps:** language pt-BR, Help ▸ Check for Updates...
- **Expected:** the "Allow product updates:" combo and the note under it fit inside the dialog.
- **Actual:** "Verificar se há atualizações e deixar que eu escolha quando instalá-las" runs past the dialog's right
  edge (text x 538–938, dialog body ends at 845); the note below is clipped. Screenshot with
  `ANYFONT_E2E_SHOTS=1`: `target/ui-compare/i18n-pt-BR-Updates.png`.
- **Suspected:** `crates/anyfont/src/updater.rs:108-120`: a combo of fixed width 330 and a one-line note that does not
  wrap.

## BUG-203 – plain egui buttons expose the English text to screen readers (all 14 non-English languages)

- **Steps:** any language; open About, Report a Bug, Get More or PANOSE Exceptions; read the accessibility tree
  (Narrator / UI Automation).
- **Expected:** the button drawn as "Fechar" is announced "Fechar".
- **Actual:** it is announced "Close". Same for System Info..., Legal Notices..., Search, OK, Cancel, Attach File...,
  Send, ...
- **Suspected:** `crates/anyfont/src/dlg.rs:147-157` (`button_at`, `button`, `default_button`, built on
  `egui::Button::new(text)`) and other plain egui widgets: the global translator is applied when the text is drawn,
  the AccessKit label keeps the key.

## BUG-301 – right-clicking a text box clears its selection: Cut, Copy and Delete always disabled

- **Steps:** type "georgia" in Search, right-click ▸ Select All, then right-click Search again.
- **Expected:** Cut, Copy and Delete enabled.
- **Actual:** all three disabled (and Copy would copy nothing).
- **Cause:** egui collapses the text selection on any pointer press (the secondary button too) before the edit menu
  reads the remembered selection.
- **Suspected:** the right-click handling of the Search box and sample combo in `crates/anyfont/src/fontlist.rs`, and
  `crates/anyfont/src/commands.rs` `edit_box_menu` (~250), `selection_of` (~623), `remember_selection` (~633): the
  selection should be captured before the press reaches the TextEdit.

## BUG-302 – double-clicking a font row never records it as recently used

- **Steps:** double-click a font row; View ▸ Show recently used fonts.
- **Expected:** the font appears in the recently used list.
- **Actual:** the database's MRU list stays empty.
- **Suspected:** `crates/anyfont/src/fontlist.rs` `draw_list` (~547): the double-click branch comes after
  `if resp.clicked()`, and egui reports a double-click as a click too, so `touch_mru` (~550) never runs. The list is
  also not re-queried after `touch_mru`.

## BUG-303 – Export includes the hidden AnyFont Content folder

- **Steps:** add a folder, File ▸ Export Folders and Collections...; import the file in another profile.
- **Expected:** only the user's folders and collections are exported.
- **Actual:** `...\Documents\AnyFont\Content\Fonts` is exported too, and Import adds it as a user folder named
  "Fonts". On a fresh profile File ▸ Export is enabled although the user has nothing to export (UI-SPEC §2.4 #6).
- **Suspected:** `crates/anyfont-core/src/exchange.rs` `export_xml` (~118) and `has_exportable` (~88) do not skip the
  content folder; `crates/anyfont/src/commands.rs` `export` (~564) / `file_menu` (~120).

## BUG-304 – Import resolves a collection's fonts before the imported folders are scanned

- **Steps:** export a folder plus a collection of fonts from that folder; import the file into a fresh profile.
- **Expected:** the collection gets its fonts; nothing is reported missing.
- **Actual:** PANOSE Font Matching Results lists the fonts (e.g. "Tuffy Regular") as missing and the collection
  arrives empty; the results window also sits on top of "We can't find the following:".
- **Suspected:** `crates/anyfont/src/commands.rs` `import` (~544): `import_file` and `import_missing` run before the
  new folders have been scanned (the scan starts afterwards).

## BUG-305 – a check box's accessible name drops every `&` of its caption

- **Steps:** select Tuffy, Font properties ▸ Glyphs; read the accessible names of the Common filter rows.
- **Expected:** "Punctuation & Separators".
- **Actual:** "Punctuation  Separators" (double space). The drawn text is right.
- **Suspected:** `crates/anyfont/src/widgets.rs:85` `a11y_checked` (and `a11y`, ~79) remove every `&` as an
  access-key marker, also for data captions and for `&&`. Same for the Filters rows (`libraries.rs:346`) and any font
  or script name containing `&`.

## BUG-306 – removing Favorites only empties it; it stays in Libraries

- **Steps:** add a font to Favorites (Font ▸ Add to collection ▸ Favorites); right-click Favorites in Libraries ▸
  Remove Folder or Collection; Yes. Close and start AnyFont again.
- **Expected:** Favorites is removed from Libraries and is not re-created at the next start (UI-SPEC §13 lead
  decision 2: "Favorites can be removed, as in the original; it is not re-created automatically"; §2.4 #4: the
  command is enabled for Favorites).
- **Actual:** Favorites is only emptied and stays in Libraries.
- **Suspected:** `crates/anyfont-core/src/db.rs:476` `remove_collection`: the `is_favorites` branch only clears
  `collection_fonts` instead of deleting the collection. Deleting the row alone is not enough: opening the database
  (`db.rs:265`, `INSERT ... WHERE NOT EXISTS (... is_favorites = 1)`) re-creates Favorites on every start, so a
  "Favorites was removed" marker is needed.

## BUG-401 – AnyFont does not start on a Windows without the Visual C++ runtime

- **Steps:** on a Windows where no program has installed the Visual C++ 2015–2022 redistributable (a fresh
  Windows, or Windows Sandbox), run `AnyFont-0.1.0-setup.exe`, then start AnyFont from the Start menu.
- **Expected:** AnyFont opens.
- **Actual:** nothing opens. Windows shows "The code execution cannot proceed because VCRUNTIME140.dll was not found"
  and the process ends with 0xC0000135 (STATUS_DLL_NOT_FOUND). This also breaks `/InstallHelper:`, the elevated
  install helper. Found by a Windows Sandbox run of the real-system test (since replaced by GitHub Actions): in the sandbox
  `C:\Windows\System32\vcruntime140.dll` does not exist. With the DLL copied next to `anyfont.exe`, everything works.
- **Suspected:** the release build links the CRT dynamically (`anyfont.exe` imports `VCRUNTIME140.dll`; there is no
  `.cargo/config.toml`), and `installer/AnyFont.iss` neither bundles nor installs the redistributable. Fixes:
  - link the CRT statically: `[target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"]` in
    `.cargo/config.toml`, which needs no admin rights and suits the per-user setup;
  - or ship `vcruntime140.dll` (+ `vcruntime140_1.dll`) app-locally in the setup.
- **Test:** `tests/real-system/run-real.ps1` (job `e2e-real`): "runs on a Windows without the Visual C++ runtime (static CRT or runtime shipped)" checks the imports of the installed `anyfont.exe`. The runner has the runtime installed, so the program itself starts there.

---

## Observations (not filed as bugs)

- **Test mode zoom:** with `TestConfig`, a UI scale of 100 % is never applied to the zoom (`app.rs` ~741), so after
  Options ▸ Reset to Default the zoom keeps its previous value in the headless tests only.
- **`new_for_test` and the updater:** the test window sets updates to Never in its settings but does not pass the
  mode to the updater, so the Updates dialog shows "let me choose" there; the real start-up path is right.
- **Reboot prompt lists a font twice** when an installed font that is in use is deleted (once per file: the
  installed copy and the source).
- **Get More cache** lives in the real `%LOCALAPPDATA%\AnyFont` (`getmore.rs` ~314) even when `ANYFONT_DATA_DIR`
  redirects the settings; the tests write nothing there but may read cached previews.
- **Legal Notices ▸ Print** writes `%TEMP%\AnyFont Legal Notices.txt` before printing, and an up-to-date update
  check deletes `%TEMP%\AnyFont Updates`; the tests point `TMP`/`TEMP` at their own folder.
- **A font installed for all users is "in use" for about a second.** In Windows Sandbox, an all-users font
  uninstalled right after its install (the self-test, and through the helper) is reported in use. Retried after
  1 s, it uninstalls fine; by the time Restart Manager is asked, nobody holds it, so Windows (the font cache) holds
  it briefly. A user who installs and at once uninstalls would see the reboot prompt for nothing. A short retry in
  the uninstall's in-use check (`anyfont-sys` `engine.rs` ~432, `protect::is_open_elsewhere`) would avoid that.
- By design (UI-SPEC): Open font file location is disabled for more than one selected font.
