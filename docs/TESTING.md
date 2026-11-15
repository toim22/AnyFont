# Testing AnyFont

AnyFont is tested at three levels. Nothing that runs on your PC needs you to stop using it:

| Level | Where | What runs | Takes the mouse / keyboard / clipboard, opens windows? | Changes the PC's fonts or your profile? |
|---|---|---|---|---|
| Unit tests | your PC, CI | `cargo test` in each crate | no | no (`anyfont-sys` refuses real writes in tests) |
| Headless end-to-end suite | your PC, CI | the real AnyFont window, built in-process and driven through its accessibility tree (`egui_kittest`); `anyfont.exe /SelfTest:` on the sandbox backend | no: nothing is shown, input is synthetic | no: a sandbox font system and settings in a temporary folder |
| Real-system test | GitHub Actions only (`.github/workflows/e2e-real.yml`) | the real setup, the real program and the real Windows font system on a disposable `windows-latest` runner | n/a (a runner nobody uses) | only the runner's, which is thrown away |

## One command

```powershell
tests\run-e2e.ps1              # unit + headless e2e, translation check, issue-relay tests  (~4 min)
tests\run-e2e.ps1 -Ignored     # also run the tests of the known bugs (docs/QA-BUGS.md)
```

It runs, in order, and prints a summary table at the end:

1. `cargo test --workspace` – every unit test and the headless e2e suite (log: `target\e2e-logs\cargo-test.log`).
2. `python -I scripts\i18n_check.py` – the translation files against `docs/strings-en.tsv`. Reported, not counted
   as a failure (missing translations are expected while strings are still being added).
3. `npm.cmd test` in `services\issue-relay` – the issue relay's own tests (Node 22.6+ for `node test.ts`).

Exit code 0 when every step passed. The real-system test is not part of it: it never runs locally.
## The headless end-to-end suite (`crates/anyfont/tests/`)

Each `tests/*.rs` file is one test binary that builds the real window, `AnyFontApp`, with `egui_kittest` (wgpu
rendering, no OS window) and drives it like a user: it clicks menu items, toolbar buttons, Libraries rows, filter
check boxes and dialog buttons found by their accessibility label and role, types into the edit boxes, presses keys
and reads the result back from the accessibility tree and the app's test accessors (`status_text`, `row_labels`,
`face_states`, `library_names`, `settings`, ...).

Run one file: `cargo test -p anyfont --test e2e_menus`. Run one test: `cargo test -p anyfont --test e2e_menus
toolbar_buttons_and_tooltips`. Screenshots of some states go to `target/ui-compare/` (`screens.rs`; `e2e_i18n` with
`ANYFONT_E2E_SHOTS=1`).

### Isolation – why it is safe to run on your PC

- **Fonts:** every window works on `SandboxFontSystem` (`anyfont-sys`), a private copy of a font system in a
  temporary folder: its own "Windows\Fonts", per-user fonts folder, registry file and Recycle Bin, seeded with copies
  of a few `%WINDIR%\Fonts` files. Install, Uninstall, Delete, "Reboot now" and "Open font file location" only change
  or record things there (`fs.reboot_count()`, `fs.opened_locations()`); the sandbox never reboots or opens Explorer.
  As a safety net `anyfont-sys` is compiled with `forbid-real-writes` for the tests: the real backend panics on any
  write.
- **Profile:** settings and the font database live in the test's temporary folder (`TestConfig`, `Paths::in_dir`);
  `%APPDATA%\AnyFont` is never read or written.
- **Desktop:** `anyfont::testing` (see below) records browser, Explorer and printer requests instead of carrying them
  out, and answers native file dialogs with queued paths, so no window ever opens on your desktop. No real mouse,
  keyboard or clipboard input is used (Paste reads nothing in test mode).
- **Network:** `common::safe_env()` points the update feed, the issue relay, the font catalog and the help URL at
  `http://127.0.0.1:9` (nothing listens there) unless a test sets them; with interception on, the Get More download
  layer also refuses every host except 127.0.0.1. Tests that need a server start one on 127.0.0.1 (`common/http.rs`,
  a `TcpListener`). Nothing ever reaches GitHub, the real relay or Google Fonts.
- **Process-wide state:** environment variables, the UI language and the theme are global to a test binary; tests
  that change them hold `common::env_lock()`.

### Shared helpers

| File | What |
|---|---|
| `tests/common/mod.rs` | `Env` (temp folder, `sandbox()`, `config_with`, `font_folder` = read-only copies of `%WINDIR%\Fonts` files, `fixture_folder` = fixtures of `anyfont-core/tests/fixtures`, `epaint_default_fonts` and `%WINDIR%`), `harness` (`AnyFontApp::new_for_test`), `harness_start` + `Start` (the real start-up path `AnyFontApp::new`: settings from disk, `/Lang:`, splash, files, single-instance messages, `updater_out`), `settle`, `wait_until`, `finish_operation`, menu helpers (`click_menu`, `menu_item`, `open_menu_items` = label/enabled/checked of the open menus), `viewport_commands` / `click_for_commands` (title bar: the `ViewportCommand`s are asserted, the window is never really minimized or closed), `EnvVar`, `env_lock`, `screenshot` |
| `tests/common/http.rs` | the local HTTP server (routes, recorded requests incl. JSON / multipart bodies) |
| `tests/svc/mod.rs` | helpers of the services tests (relay payload check mirroring `services/issue-relay/src/report.ts`, catalog/feed builders) |

### Test inventory

| File | Tests | Covers |
|---|---|---|
| `startup.rs`, `screens.rs`, `operations.rs` | 13 | the first smoke tests: start-up, screenshots of the main states and themes, basic operations |
| `e2e_menus.rs` | 8 | every menu (File/Edit/Font/View/Help + submenus) and every toolbar button in the 13 selection contexts of UI-SPEC §2.3, checked item by item (caption, enabled, checked) against UI-SPEC §2.4–§2.8 / §13 and `toolbar-states.json`; toolbar tooltips; title bar (?, Options, Updates, Minimize/Maximize/Close as viewport commands); Libraries and font-row context menus; Ctrl+Alt+I, Alt+F4, File ▸ Exit, F10/Alt+F/Alt+E, arrows, Esc |
| `e2e_libraries.rs` | 13 | Add Folder (menu, toolbar; not found, already added, subfolder, recursion), Add Collection and the inline editor, F2 / context-menu Rename, Remove with confirmation and "Do not show this message again" (also after a restart), Favorites, library Install/Uninstall, section headers, drag fonts onto a collection, Export/Import round trip, "We can't find the following:" |
| `e2e_filters_list.rs` | 18 | every Filters option against the core's own matching on fixture fonts (OR within a group, AND across groups), Clear filters, Search (live, flat, suggestions, Esc), sample text presets/free text/history, preview size slider and small/big A (8–86), View toggles (group by family, recently used, use font to display name, Latin names in ja-JP, filter list), duplicates (filter, rows, location submenu), selection (click, Ctrl, Shift, Ctrl+A, Ctrl+Shift+A, header, keyboard, type-ahead), the edit-box context menu |
| `e2e_operations.rs` | 12 | Install/Uninstall/Delete from the toolbar, Font menu and context menu; family header; progress captions and Cancel; Delete fonts dialog; failures list (read-only, invalid font); fonts in use: Reboot now (reboot *requested* from the sandbox, never performed), Reboot later + simulated reboot, Ignore, Cancel; protected system fonts refused |
| `e2e_properties_options.rs` | 7 | Font properties: Glyphs (count, Common/Scripts filters, clear, multiple selection, glyph size), Font info (values against the core), Variable font (axes of `colr_1_variable.ttf` and a Bahnschrift copy, slider, named instance); Options: size slider/spin 75–225, Reset to Default, the 4 themes, window border, Font Matching radios, OK saves |
| `e2e_services.rs` | 7 | Product Help (menu, title-bar ?, real start-up; URL asserted, no browser), About, System Info categories and Save (written / failure), Legal Notices + Print (recorded, nothing printed) |
| `e2e_updates.rs` | 8 | Check for Updates with `ANYFONT_UPDATE_FEED` (local JSON / local server): newer version → message, Later not shown again this session, Install Now downloads from the local server → `UpdaterRequest::ExitAndInstall` (the setup is never run), up to date, offline, local setup, update-mode combo saved |
| `e2e_bugreport.rs` | 8 | Report a Bug / Submit Ideas and Feedback to a fake relay (`ANYFONT_ISSUE_RELAY`): exact JSON and multipart payload as the relay validates it, attachment, success link, relay down / 500 → browser fallback URL + Explorer request (both recorded), 429 → try later, field limits |
| `e2e_getmore_matching.rs` | 11 | Get More with `ANYFONT_CATALOG_URL` + a local server: search/filters/sort, Download → Not installed in My fonts → Install (sandbox), offline + Retry, failed download; PANOSE: modes saved, Exceptions add/remove, import with missing fonts → Font Matching Results → substitute → OK, Never show, Cancel; Type 1 message |
| `e2e_i18n.rs` | 46 | each of the 15 languages: every menu and the main dialogs; no untranslated `strings-en.tsv` key shown where the language has a translation, no caption overflowing its control, translated accessibility labels |
| `e2e_persistence.rs` | 2 | two windows in sequence on the same data folder: settings, window placement, last library, folders and collections survive; the delayed (800 ms) save |
| `e2e_selftest.rs` | 3 | `anyfont.exe /SelfTest:` (the real binary and its `/InstallHelper:` child) runs the real-system script `tests/real-system/selftest.json` on the sandbox backend; the self-test refuses the real backend outside a GitHub-hosted runner, and a script that does not say it is disposable |
| `e2e_cmdline.rs` | 9 | command-line grammar (unknown switches ignored), font file and folder arguments, splash → main window, `/NoSplash`, `/Lang:`, single instance: a second instance forwards its files to the first one's pipe server (started in-process under a unique name) |

Tests that reproduce a known bug are marked `#[ignore = "BUG-n: ..."]`; the list is `docs/QA-BUGS.md`. Run them
with `cargo test -p anyfont --test <file> -- --ignored`.

### Test seams in the app (`crates/anyfont/src/testing.rs`)

The app has a few one-line hooks so the tests can see requests to the outside world without anything opening:

| Seam | Where it is called | Effect in tests |
|---|---|---|
| `testing::intercept(External::Url)` | `dlg::open_url` (every browser link: help, release notes, bug-report fallback, vendor URL), Help ▸ Product Help in test mode | the URL is recorded (`testing::urls()`), no browser |
| `testing::intercept(External::Explorer)` | bug report: "show the attachment in Explorer" | recorded, no Explorer |
| `testing::intercept(External::Print)` | Legal Notices ▸ Print | recorded, nothing printed |
| `testing::pick(...)` | Add Folder, Import, Export, System Info ▸ Save, bug report ▸ Attach File | answers with `testing::queue_pick(Some(path))`; with nothing queued, "cancelled" – no dialog |
| `testing::redirect(url)` | Get More downloads (`getmore.rs`) | URLs mapped with `redirect_prefix` go to the local server; any other non-local host is refused |

They do nothing unless `testing::enable()` was called (the test helpers do) or `ANYFONT_TEST_INTERCEPT` is set.
"Open font file location" needs no seam: the sandbox font system records the path instead of opening Explorer.
`main.rs` also hands `/SelfTest:<script>` to `testing::selftest::run` (see the real-system test below).

## Real-system test (GitHub Actions only)

The headless suite cannot test the real installer, the real Windows font system (registry, `C:\Windows\Fonts`, the
per-user fonts folder, GDI, the Recycle Bin, the elevated helper) or the real uninstaller. That test changes the
fonts of the Windows it runs on, so it runs **only** on a disposable GitHub-hosted runner: the job `e2e-real` of
`.github/workflows/e2e-real.yml` (on pushes to `main`, pull requests and by hand). Nothing of it runs on your PC.
`tests/real-system/run-real.ps1` and `/SelfTest:` with the real backend both refuse to run anywhere else.

The job builds the program and the setup with `build.bat`, then `tests\real-system\run-real.ps1`:

1. installs the setup silently (`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER`) and checks the program
   folder, the legal notices, the Start-menu shortcut and the uninstall entry;
2. checks that the program does not need the Visual C++ runtime, which a fresh Windows lacks (BUG-401): it must
   either link the CRT statically or ship the DLLs next to `anyfont.exe`;
3. starts AnyFont (`/NoSplash` and an unknown switch) on the real backend, waits for the window and the first scan of
   the installed fonts, takes a screenshot, starts a second `anyfont.exe <font file>` (it must forward the file to the
   first instance and exit), closes the window with `WM_CLOSE`, and checks there was no panic or crash report. The
   window checks are *soft* (reported as warnings), since the runner's desktop session is not guaranteed;
4. runs `anyfont.exe /SelfTest:<script>` (below), checks that the Recycle Bin holds the deleted copy, and that no
   test font is left in the fonts folders or the registry;
5. runs the uninstaller silently and checks the leftovers: the program folder, the uninstall entry, the shortcut,
   `Software\Classes\Applications\anyfont.exe` and `Software\AnyFont` must be gone. The settings, database, logs
   and Content folder are kept on purpose (`[UninstallDelete]` is empty);
6. writes `summary.json`. The job uploads everything as the artifact **AnyFont-e2e-real**: `summary.json`,
   `selftest.json`, `run-real.log`, `setup.log`, `uninstall.log`, `anyfont.log` and the screenshots.

### `anyfont.exe /SelfTest:<script.json>`

A test-only mode (`crates/anyfont/src/testing/selftest.rs`). It runs a script of font operations through the
program's own layers, with no UI input:
- `ops::Running`, the worker behind Install / Uninstall / Delete;
- the elevated helper protocol: `/InstallHelper:` request and response files;
- the core's scan and catalog, which is what My fonts lists.

The steps are in `tests/real-system/selftest.json`; `run-real.ps1` fills in the folders:

| Op | Checks |
|---|---|
| `environment` | user, elevation, backend, fonts folders |
| `list_installed` | at least N registrations; the given Windows fonts are listed and protected; My fonts is not empty |
| `install` / `uninstall` | the operation succeeds, per user and for all users. If Windows reports the font in use, the step names the holder (`holders.ps1`, Restart Manager), waits 1 s and 3 s and tries again. It then answers the prompt with "Reboot later" and checks that the registration is gone and the file is queued for deletion at restart (`PendingFileRenameOperations`). It fails if AnyFont itself holds the font |
| `check_installed` | the file is in the scope's fonts folder and registered there (`reg.exe query` HKCU/HKLM; on the sandbox backend, the sandbox's registry), and My fonts lists it with its family as installed |
| `check_not_installed` | no registration and no file left (or the file queued for the restart) |
| `helper_install` / `helper_uninstall` | the same, including the in-use handling, through a child `anyfont.exe /InstallHelper:<request>`. This is the path a non-elevated AnyFont takes after the UAC prompt; the runner's user is already an administrator, so there is no prompt |
| `delete_copy` | a copy of a fixture is deleted through the Delete worker and goes to the Recycle Bin |
| `protected_refused` | `arial.ttf` / `segoeui.ttf` in the all-users fonts folder: protected, Uninstall and Delete refused with `Protected`, file and registration intact |

Where it runs:
- With the **real backend**, the script must say `"i_am_a_disposable_machine": true`, and the machine must be a
  GitHub-hosted runner (`GITHUB_ACTIONS=true`, `RUNNER_ENVIRONMENT=github-hosted`) or Windows Sandbox. Anywhere
  else it refuses and exits with 2 before it writes anything, not even a log line.
- With the **sandbox backend** (`ANYFONT_SANDBOX_ROOT`), it runs anywhere. The headless test `e2e_selftest.rs` uses
  that to check the same script on every `cargo test`.

Windows Sandbox is not used by any test any more: its window got in the way of whoever used the PC.
## Adding a test

- Use `common::harness(env.config(), size)` for most tests; `env.sandbox()` + `env.config_with(&fs)` when the test
  needs the sandbox's controls (`set_in_use`, `reboot_count`, `opened_locations`, ...); `harness_start` for the real
  start-up path.
- Find controls by role and label (`h.get_by_role_and_label(Role::Button, "Install")`), call `settle(&mut h)` after an
  input, `finish_operation` after Install/Uninstall/Delete, `wait_until` for background work.
- File dialogs: `anyfont::testing::queue_pick(Some(path))` before the click.
- Network: start `common::http::Server`, set the variable with `EnvVar::set` while holding `env_lock()`.
- When a test finds a bug, don't weaken it: mark it `#[ignore = "BUG-n: ..."]` and add the bug to
  `docs/QA-BUGS.md`.
