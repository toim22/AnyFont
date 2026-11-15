# `anyfont-sys` API

`crates/anyfont-sys` is AnyFont's Windows layer. Everything that changes the operating system goes through it:
the list of installed fonts, the protected-system-font rule, install, uninstall, delete (Recycle Bin),
`WM_FONTCHANGE`, reboot support, "open file location" and elevation for all-users installs. It also has the
process helpers `main` needs: command line, logging, crash reports and single instance.

The dossier findings behind this behaviour are in [`phases/10-behavior.md`](phases/10-behavior.md). The security
design is in [`phases/12-security.md`](phases/12-security.md).

## Dependencies

| Crate | Licence | Used for |
| --- | --- | --- |
| `windows 0.62` | MIT OR Apache-2.0 | Win32 bindings: registry, GDI font resources, security/ACL, shell, pipes, shutdown |
| `ttf-parser`, `serde`, `serde_json`, `log`, `dirs` | MIT / Apache-2.0 | font names, JSON, logging, known folders (workspace versions) |
| `tempfile` (dev only) | MIT OR Apache-2.0 | test sandboxes |

## Start-up sequence (`anyfont` `main`)

The order matters. The helper must run before anything else, and single-instance forwarding comes before the UI.

```rust
use anyfont_sys::{cmdline::CommandLine, crash, logging, single_instance};

fn main() {
    let cl = CommandLine::from_env();

    // 1. Elevated helper mode: run the request and exit. No UI, no single-instance check.
    if let Some(request) = cl.install_helper() {
        let _ = logging::init();
        std::process::exit(anyfont_sys::helper::helper_main(&request));
    }

    // 2. Logging and the crash hook.
    let _ = logging::init();               // %LOCALAPPDATA%\AnyFont\logs\anyfont.log
    crash::install_panic_hook();           // writes %LOCALAPPDATA%\AnyFont\crash-reports\crash-*.txt

    // 3. Single instance: a second process hands over its files and exits.
    let guard = match single_instance::acquire("AnyFont") {
        Ok(single_instance::Instance::Secondary) => {
            let msg = single_instance::Message::from_command_line(&cl);
            let _ = single_instance::forward_to_primary("AnyFont", &msg, std::time::Duration::from_secs(5));
            return;
        }
        Ok(single_instance::Instance::Primary(g)) => Some(g),
        Err(e) => { log::warn!("single instance unavailable: {e}"); None }
    };
    // guard.listen(|msg| { /* send msg.files to the UI thread, request_repaint, focus the window */ })

    // 4. The font system and pending work from a previous run.
    let fs = anyfont_sys::system();        // Box<dyn FontSystem>; the sandbox when ANYFONT_SANDBOX_ROOT is set
    fs.process_pending();                  // deferred per-user deletes, stale helper files
    let crashes = crash::pending_reports(); // offer "Report a Bug" for them, then crash::mark_handled(path)

    // 5. Once the main window exists:
    // anyfont_sys::set_owner_window(hwnd as isize);  // owner of the UAC prompt and Explorer windows
}
```

Keep the `PrimaryGuard` alive for the whole run, because dropping it releases the mutex.

## Backends

`anyfont_sys::system()` returns:

- **the sandbox** when `ANYFONT_SANDBOX_ROOT` is set. It panics if the sandbox cannot be created and never falls
  back to the real system.
- **the real system** otherwise.

| Variable | Meaning |
| --- | --- |
| `ANYFONT_SANDBOX_ROOT` | Folder for the sandbox. It is created and seeded on first use and reused afterwards. |
| `ANYFONT_SANDBOX_SEED` | `;`-separated file names copied from `%WINDIR%\Fonts` into the sandbox's "Windows" fonts folder and registered as protected all-users fonts. `none` means no seed. The default is `arial.ttf;arialbd.ttf;times.ttf;cour.ttf;segoeui.ttf;cambria.ttc`. |
| `ANYFONT_SANDBOX_OPEN_EXPLORER` | When set, the sandbox's `open_location` really opens Explorer. Otherwise it only records the call. |
| `ANYFONT_SANDBOX_DECLINE_UAC` | When set, the sandbox acts as if the user cancelled every UAC prompt: an all-users install fails with `ElevationCancelled` (the shell then offers "Install for me"). `SandboxFontSystem::set_decline_elevation` does the same from a test. |
| `ANYFONT_LOG` | Log level: `error`, `warn`, `info` (default), `debug` or `trace`. |

The sandbox layout under `<root>` is:

- `Windows\Fonts` (all users)
- `LocalAppData\Microsoft\Windows\Fonts` (current user)
- `AppData\Roaming` and `LocalAppData` (AnyFont's own folders)
- `Documents` (content folder)
- `RecycleBin`
- `sandbox-registry.json` (the `HKLM`/`HKCU` `Fonts` and `Type 1 Fonts` keys, plus counters)

It never calls `AddFontResource`, never broadcasts and never needs elevation. `SandboxFontSystem::open(root, config)`
is public for tests, along with these test controls:

- `set_in_use`, `set_protected`
- `pending_reboot`, `simulate_reboot`
- `reboot_count`, `font_change_count`
- `recycle_bin`, `opened_locations`

Run the whole app against a sandbox with `set ANYFONT_SANDBOX_ROOT=%TEMP%\anyfont-sb` followed by `anyfont.exe`.
Single-instance names include the sandbox root, so a sandboxed instance never talks to a normal one.

## The `FontSystem` trait

`FontSystem: Send + Sync`. Share it as `Arc<dyn FontSystem>` (or keep the `Box`) and call the mutating methods
**on a worker thread**. They do file I/O, can wait for a UAC prompt, and the elevated helper blocks until it
finishes.

| Method | Purpose |
| --- | --- |
| `backend() -> Backend` | `Real` or `Sandbox { root }`, e.g. for a "sandbox" badge in the title bar. |
| `font_dirs() -> FontDirs` | `all_users` (`%WINDIR%\Fonts`) and `current_user` (`%LOCALAPPDATA%\Microsoft\Windows\Fonts`). |
| `paths() -> &AppPaths` | `settings_dir` (`%APPDATA%\AnyFont`), `data_dir` (`%LOCALAPPDATA%\AnyFont`), `logs_dir`, `crash_dir`, `content_fonts_dir` (`Documents\AnyFont\Content\Fonts`). |
| `content_folder() -> PathBuf` | Creates and returns the content folder where downloaded fonts go. |
| `installed_fonts() -> io::Result<Vec<InstalledFont>>` | All registrations: HKLM and HKCU `...\Fonts`, `Type 1 Fonts`, plus this session's temporary fonts. `.fon` values are skipped, as in the original. |
| `is_protected(path) -> bool` | Protected system font (see below). Protected fonts can't be uninstalled or deleted. |
| `protected_fonts() -> Vec<PathBuf>` | Files of the protected installed fonts (the "Protected System Fonts" category). |
| `registrations(path) -> Vec<InstalledFont>` | The registrations that point at this file. An empty list means it is not installed. |
| `needs_elevation(scope) -> bool` | True for `AllUsers` when the process is not elevated. Use it to show the shield icon on the command. |
| `install[_with](files, scope, used[, progress]) -> InstallReport` | Install font files. |
| `uninstall[_with](paths, used[, progress]) -> UninstallReport` | Unregister installed fonts. |
| `delete[_with](paths, used[, progress]) -> DeleteReport` | Move font files to the Recycle Bin, uninstalling them first when installed. |
| `open_location(path)` | Explorer with the file selected. |
| `reboot_now()` | Restart Windows. Call it only after the user picked "Reboot now". |
| `process_pending() -> usize` | Run deferred per-user deletes left from the previous session. Call at start-up. |

Free functions:

- `is_elevated()`
- `set_owner_window(hwnd)`
- `paths::app_paths()`: the folders without building a font system; honours the sandbox variable.

### `InstalledFont`

| Field | Meaning |
| --- | --- |
| `value_name` | The registry value name, e.g. `Arial (TrueType)`. |
| `path` | The main file (`.pfm` for Type 1). |
| `extra_paths` | The `.pfb` for Type 1, otherwise empty. |
| `scope` | `AllUsers` (HKLM), `CurrentUser` (HKCU) or `Session`. |
| `kind` | `Fonts`, `Type1` or `None` (session). |
| `exists` | Whether the file is on disk. |
| `protected` | Whether this is a protected system font. |

### Scopes

| `Scope` | Where the file goes | Registry | Elevation |
| --- | --- | --- | --- |
| `CurrentUser` | `%LOCALAPPDATA%\Microsoft\Windows\Fonts` | `HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts`, full path | none |
| `AllUsers` | `%WINDIR%\Fonts` | `HKLM\...\Fonts`, bare file name (Type 1: `HKLM\...\Type 1 Installer\Type 1 Fonts`) | UAC, via the helper |
| `Session` | stays where it is | none: `AddFontResourceEx` only, gone at sign-out | none |

`Session` is the original's "TemporaryMode" preference.

Type 1 fonts (`.pfm` + `.pfb` with the same stem, in the same folder) can only be installed for `AllUsers`, because
Windows reads the `Type 1 Fonts` key only under HKLM. Give either file. The pair is installed once, and both inputs
report the same outcome.

### Install outcomes and errors

Each input file gets a `FileResult { path, result }`. `results` is in input order.

- `Ok(Installed { value_name, installed_path, replaced })`. `replaced` is the older file that had the same registry
  name and was removed; show "Replaced …" if you want.
- `Ok(AlreadyInstalled { value_name, installed_path })`. The same file, or a byte-identical copy, is already
  registered.

`FontOpError` values. The UI strings are taken from `docs/strings-en.tsv`.

| Error | When | Suggested message |
| --- | --- | --- |
| `Protected` | uninstall, delete or replace of a protected system font | "Protected System Font" / tooltip `(%s)` |
| `NotFound` | file missing | — |
| `UnsupportedFormat(ext)` | not `.ttf/.otf/.ttc/.otc/.pfm/.pfb` (`.fon/.fnt` can be deleted but not installed) | — |
| `InvalidFont(why)` | the parser rejects it | — |
| `ReadOnly` | delete of a read-only file | "Read-only fonts cannot be deleted" |
| `InUse` | the file is held open by another program (see the next section) | the "fonts in use" prompt |
| `NotInstalled` | uninstall of a file that is not registered | — |
| `Type1NeedsAllUsers` | Type 1 with `CurrentUser`/`Session` | — |
| `MissingCompanion(path)` | `.pfm` without `.pfb`, or the reverse | — |
| `ElevationCancelled` | the user said No to UAC | nothing, or a status-bar note |
| `AccessDenied` | Windows refused the write | "Insufficient Permissions" |
| `Cancelled` | progress callback returned `false`, or `UsedFontAction::Cancel` | — |
| `Io(text)` | anything else | show the text |

`Report` helpers:

- `succeeded()`, `failed()`
- `elevation_cancelled()`
- `in_use` (paths)
- `reboot_required`

### Progress

`progress(done, total, current_path) -> bool` is called before each file. Return `false` to cancel the rest; those
files get `Cancelled`. Use it for "Installing %s... %d%%" and "Uninstalling %s... %d%%". The elevated helper path
has no per-file progress: show an indeterminate bar while it runs.

## Fonts in use: the two-pass flow

A font file can be held open by another program. The original then shows the FontInteraction dialog with
"Reboot now", "Reboot later", "Ignore" and "Cancel" (`EUsedFontAction` 0–3). AnyFont does the same in two passes:

1. Call `uninstall(paths, UsedFontAction::Ask)` (or `delete`). Free fonts are processed. Fonts in use are left
   untouched, get `Err(InUse)`, and are listed in `report.in_use`.
2. If `in_use` is not empty, show the prompt: "The following fonts cannot be uninstalled because they are in
   use…" with the list.
   - **Reboot now** → `uninstall(&report.in_use, UsedFontAction::RebootNow)`, then `fs.reboot_now()`.
   - **Reboot later** → `uninstall(&report.in_use, UsedFontAction::RebootLater)`. The fonts are unregistered now
     and the files are removed at the next restart.
   - **Ignore** → nothing more; the fonts stay.
   - **Cancel** → nothing more. Pass `UsedFontAction::Cancel` up front if the whole operation must be
     all-or-nothing.

With `RebootNow`/`RebootLater`, `report.reboot_required` is `true` and the outcome's `file` is
`FileRemoval::PendingReboot`.

How the deferral is done:

- **All-users files** use `MoveFileEx(MOVEFILE_DELAY_UNTIL_REBOOT)`.
- **Per-user files** are listed in `%LOCALAPPDATA%\AnyFont\pending-deletes.json`, because that call needs admin
  rights. `process_pending()` deletes them at the next start.

Install uses `used` too: when a different file with the same registry name is replaced and the old one is in use.

`delete` follows the original:

- It refuses protected, read-only and missing files.
- It uninstalls an installed (non-protected) font first, then sends the file to the Recycle Bin. A Type 1 pair goes
  together.
- A file held open that is not installed gives `InUse`.

## Elevation (all-users operations)

When the process is not elevated:

- `install(.., Scope::AllUsers, ..)` and the all-users part of `uninstall`/`delete` run in a second copy of
  `anyfont.exe`, started with `ShellExecuteEx("runas", "/InstallHelper:<request file>")`. The caller waits.
- The paths are split. Files in `%WINDIR%\Fonts` or registered in HKLM go to the helper; the rest is done in
  process. Results are merged back in input order.
- One operation causes at most one UAC prompt.
- If the user declines, those files report `ElevationCancelled`.

`main` must handle `/InstallHelper:` first (see the start-up sequence). Call `set_owner_window` so the UAC prompt
has the main window as its owner.

## Command line (`cmdline`)

`CommandLine::from_env()` / `parse(args)` / `parse_str(line)`:

- Switches start with `/` or `-` (`--x` is accepted).
- An optional value follows `:`; quotes are trimmed.
- Switch names are case-insensitive.
- Anything else is a file.

Accessors:

- `no_splash()`, `no_startup_initial_action()`, `no_ui()`
- `dde()`, `automation()`, `embedding()`, `print()` (`/Print` or `/P`)
- `sdi() -> Option<bool>` (`/Sdi` or `/Mdi`)
- `lang()` (`/Lang:pt-BR`, AnyFont's own)
- `install_helper()`
- `has(name)`, `value(name)`
- `unknown_switches()` (accepted and ignored; log them)
- `forwardable_files()` (absolute paths)

`split_command_line` follows the `CommandLineToArgvW` rules, and `quote_arg` quotes one argument.

## Logging (`logging`)

`logging::init()` installs the `log` backend.

- **File:** `%LOCALAPPDATA%\AnyFont\logs\anyfont.log`.
- **Rotation:** at 1 MiB the file becomes `anyfont.1.log`, and so on; four old files are kept.
- **Line format:** `2026-11-15 13:04:05.123Z INFO  [thread] target: message`.
- **Debug builds** also send each line to `OutputDebugStringW`.
- `logging::tail(n)` returns the last lines, for "Report a Bug" and crash reports.

Use the `log` macros everywhere. Don't log font file contents or user documents. Paths are fine.

## Crash reports (`crash`)

`crash::install_panic_hook()` chains the previous hook and writes `crash-<yyyymmdd-hhmmss>.txt` to `crash_dir`.

- **Contents:** version, OS build, thread, message, location, backtrace and the last 60 log lines.
- **Redaction:** the user profile path is replaced with `%USERPROFILE%`.
- **At start-up:** `crash::pending_reports()` lists unhandled reports. The UI offers "Report a Bug" (the GitHub
  issue relay), then calls `crash::mark_handled(path)`, which moves the file to `crash-reports\handled\`.

Nothing is uploaded automatically.

## Single instance (`single_instance`)

- `acquire(app) -> Instance::{Primary(guard), Secondary}`.
- `guard.listen(callback)` starts a pipe-server thread. The callback runs on that thread with a
  `Message { files, switches }`: forward it to the UI thread and bring the window to the front.
- `forward_to_primary(app, &msg, timeout)` sends from a second process. It calls `AllowSetForegroundWindow` first
  so the primary may take focus.

## Tests

`cargo test -p anyfont-sys` covers:

- The command-line grammar.
- Registry name formatting.
- PFM parsing.
- The ACL rule.
- Sandbox round trips: install, uninstall, delete, in-use flows, Type 1, session, replace, helper requests.
- Logging rotation, crash files and pipe forwarding.

### The guard

Every write the real backend can make goes through `forbid_in_tests!`. It panics with "… use the sandbox backend"
in two cases: in this crate's unit tests, and in any build with the `forbid-real-writes` feature. The covered
writes are:

- registry values
- `AddFontResource` / `RemoveFontResource`
- `WM_FONTCHANGE`
- copies into and removals from the fonts folders
- `MoveFileEx`
- the Recycle Bin
- reboot

The crate's own integration tests get the feature through a dev-dependency on itself (`tests/guard.rs` checks it).
**Other crates** whose tests might reach `anyfont-sys` should add it the same way:

```toml
[dev-dependencies]
anyfont-sys = { path = "../anyfont-sys", features = ["forbid-real-writes"] }
```

They should still use `SandboxFontSystem` or `ANYFONT_SANDBOX_ROOT`. The guard is the safety net, not the test
strategy. Reading the real registry and fonts folder is allowed.
