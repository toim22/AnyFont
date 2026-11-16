# Phase 10 – Behaviour (command line, files, registry, logging, shell)

Dossier: `10-behavior/` (`command-line.md`, `environment-and-files.md`, `errors-logging-installer.md`, `network.md`,
`shell-clipboard-printing.md`), plus the font install/uninstall code in the dossier's font module (`CdrFnt`) and
utility modules (`CrlUtils`, `CrlInterprocess`, `CrlConfig`). Addresses are the dossier's IDA addresses.

AnyFont's implementation is `crates/anyfont-sys`; the API is in [`../sys-api.md`](../sys-api.md).

## 1. Command line

### What the dossier says

- **Grammar** (`CrlUtils::CommandLine::ParseSwitch` 0x180011C10, `SwitchSpecified` 0x180010D00):
  - a switch starts with `/` or `-`, and anything else is a file;
  - an optional value follows `:` and is trimmed of `"`;
  - lookup is case-insensitive.
- **Launcher switches** (`BaseAppStartup::ParseCmdLine`, `CrlConfig` 0x180006670): `DDE`, `Automation`, `Embedding`,
  `Print`, `P`, `NoSplash`, `NoStartupInitialAction`, `NoUI`, `Sdi`, `Mdi`. Only `/NoSplash` was exercised at
  runtime.

### AnyFont

`anyfont_sys::cmdline::CommandLine` uses the same grammar. All ten launcher switches are recognised (accessors in
`sys-api.md`). Unknown switches are accepted, logged and ignored, as the original ignores flags it does not use.

AnyFont adds:

| Switch | Use |
| --- | --- |
| `/Lang:<tag>` | UI language override (Phase 08) |
| `/InstallHelper:<request.json>` | Internal. The elevated helper for all-users operations (section 6). Not for users. |

Behaviour of the modes:

- `DDE`, `Automation` and `Embedding` have no COM/DDE server in AnyFont. They start normally.
- `Print`/`P` have no print feature (the original has none in this app either).
- `NoUI` is reserved.
- `NoSplash` skips the splash screen (`crates/anyfont/src/splash.rs`).

Files on the command line are font files or folders to show and install. They are forwarded to a running instance
(section 5).

## 2. Environment, files and folders

### What the dossier says

- `wWinMain` rewrites `PATH`. AnyFont doesn't need this and does not do it.
- Per-user data lives under `%APPDATA%\<product>`; shared data under `%ProgramData%`.
- Fonts are registered by the font module (`AddFontResourceExW`, `RemoveFontResourceExW`, `RegSetValueExW`).

### AnyFont folders

| Folder | Path | Content |
| --- | --- | --- |
| settings | `%APPDATA%\AnyFont` | settings (owned by `anyfont-core`) |
| data | `%LOCALAPPDATA%\AnyFont` | caches, `pending-deletes.json`, `helper\` (helper request files) |
| logs | `%LOCALAPPDATA%\AnyFont\logs` | `anyfont.log` plus four rotated files |
| crash reports | `%LOCALAPPDATA%\AnyFont\crash-reports` | `crash-*.txt`, and `handled\` |
| content | `Documents\AnyFont\Content\Fonts` | downloaded fonts (the original's content folder) |

AnyFont writes nothing to `%ProgramData%`.

Environment variables read:

- `ANYFONT_SANDBOX_ROOT`, `ANYFONT_SANDBOX_SEED`, `ANYFONT_SANDBOX_OPEN_EXPLORER`, `ANYFONT_LOG`
- `WINDIR`, `LOCALAPPDATA`, `APPDATA`, `USERPROFILE` (redaction), `USERNAME`/`USERDOMAIN` (single-instance id)

### Font registration (dossier findings)

| Item | Finding | Address |
| --- | --- | --- |
| Registry keys | `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts` (REG_SZ). Type 1: `HKLM\...\Type 1 Installer\Type 1 Fonts` (REG_MULTI_SZ `"T"`, pfm, pfb; created with access 0xF013F when missing). Writes use access 0x20106 (KEY_WRITE + WOW64_64KEY). | install worker 0x1802010F0 |
| Enumeration | Reads both HKLM and HKCU `Fonts` with 0x20119 (KEY_READ + WOW64_64KEY), skips `.fon`, joins relative data with the fonts folder | 0x1802001D0 |
| Value name | Full name + ` (TrueType)`. Collections: names joined with ` & ` + ` (TrueType)`. Type 1: `-` → space, no suffix. An existing value for the same file is reused. | 0x180203780, 0x180202BC0 |
| Copy | A file already in the fonts folder is registered in place. Otherwise it is copied; on a name clash the copy is called `"%s%s_%d%s"` (dir, stem, `_n`, extension). | 0x1802010F0 |
| Load / unload | `AddFontResourceExW` / `RemoveFontResourceExW` with flags 0 for installs. `FR_PRIVATE` (0x10) only on request; previews use 0x30. Uninstall calls `RemoveFontResource` in a loop until it fails. | 0x180201070, 0x1802010A0, 0x180202390 |
| Uninstall | Deletes the value from **both** HKLM and HKCU (`Fonts` or `Type 1 Fonts`) | 0x180202390 |
| Broadcast | `PostMessageW(HWND_BROADCAST, WM_FONTCHANGE, 0, 0)` | 0x1802010D0 |
| Temporary mode | Preference `TemporaryMode` (registered at 0x180083140, stored at 0x1803D2AC0): fonts are loaded for the session only, with no registry entry, no copy and no elevation | — |
| Install scope | The original installs per machine (HKLM) only. It has no Win32 per-user install. | — |

AnyFont implements the same, with these choices:

- **Three scopes.**
  - `AllUsers` reproduces the original.
  - `CurrentUser` uses Windows 10 1809+ per-user fonts: `%LOCALAPPDATA%\Microsoft\Windows\Fonts` and `HKCU\...\Fonts`
    with the full path, and needs no elevation. This is new.
  - `Session` is the original's TemporaryMode.
- **Value names.** CFF-flavoured OpenType gets ` (OpenType)`, which is what Windows itself writes. The original
  writes ` (TrueType)` for every sfnt. Names are sanitised: control characters are dropped, whitespace is collapsed, and names are cut to 200
  characters.
- **Registry data.** A bare file name for files directly in `%WINDIR%\Fonts`; a full path otherwise.
- **Type 1.** All users only (Windows reads `Type 1 Fonts` only from HKLM). The `.pfm`/`.pfb` pair is found by stem
  in the same folder. The PFM `dfDriverInfo` gives the PostScript name and `dfFace` the family.
- **Duplicates.** The same file, or a byte-identical file with the same value name, is `AlreadyInstalled`. A
  different file with the same value name replaces the old one, as `GetMasterDuplicate` does in the install
  handler (0x1800F1200). Replacing a protected font is refused.
- **Failure.** When `AddFontResourceExW` fails, the registry value and the copy are rolled back.

### Protected system fonts (dossier rule)

The rule is at `CdrFnt` 0x1802028E0:

1. `GetNamedSecurityInfoW(file, SE_FILE_OBJECT, DACL)`.
2. For each ACE whose SID `IsWellKnownSid(WinBuiltinAdministratorsSid)`:
   - an **allowed** ACE marks the font protected when `mask & 0x1F01FF != 0x1F01FF` (administrators lack full
     control);
   - a **denied** ACE marks it protected when `mask & 0x1F01FF != 0`.
3. The last matching ACE wins. An empty DACL means protected.

Shipped Windows fonts are owned by TrustedInstaller, and Administrators only get ReadAndExecute. That was checked
read-only on this PC: `C:\Windows\Fonts\arial.ttf`.

The handlers' filters use this as "vfunc56":

- Uninstall is offered only for `!protected` (0x1800F17B0).
- Delete only for `!protected` with local files (0x1800F26A0).
- The UI shows the "Protected System Font(s)" category (0x180113A60) and the tooltip `(%s)`.

AnyFont (`protect.rs`, `win/acl.rs`) marks a font protected when any of these holds:

- the dossier DACL rule;
- `SfcIsFileProtected` (Windows Resource Protection);
- the file is a critical shell font in `%WINDIR%\Fonts`: Marlett, Segoe UI family, Segoe UI Symbol/Emoji, Segoe MDL2
  Assets, Segoe Fluent Icons, Tahoma, Microsoft Sans Serif, Arial. This is a safety net for systems whose ACLs were
  changed.

Protected fonts are never uninstalled, deleted or replaced, even by the elevated helper.

### Fonts in use and the reboot rule (dossier)

| Item | Finding | Address |
| --- | --- | --- |
| In-use test | `SystemIO::File::IsOpenElsewhere`: `CreateFileW(GENERIC_READ, share 0)`; error 32 (sharing violation) means in use | `CrlUtils` 0x1800A2840 |
| Prompt | FontInteraction dialog (registrar 0x180182580): Reboot now → 0 (0x180183FF0), Reboot later → 1 (0x180184080), Ignore → 2 (default, 0x180184110), Cancel → 3 (0x1801841A0) = `EUsedFontAction` | — |
| Effect | `UninstallFontImpl` 0x18006DED0 / `InstallFontImpl` 0x18006D8E0 return at once on Cancel. Install ends with the reboot call when the action is 0. | — |
| Reboot | Enable `SeShutdownPrivilege` (`OpenProcessToken(0x28)`, `LookupPrivilegeValue`, `AdjustTokenPrivileges`, 0x1802043F0), then `ExitWindowsEx(EWX_REBOOT, 0x80040001)` (planned, application, maintenance) | 0x180202780 |
| Strings | "The following fonts cannot be uninstalled/deleted because they are in use…" (`30133dec`, `4eb5d55f`, `af92d0b6`, `c863e6d8`), "Reboot &now" `be6e2825`, "Reboot &later" `72c1a8a9` | strings-en |

Runtime check (read-only): a font loaded by Windows always fails the share-0 open. So AnyFont first removes the font
resource, then tests whether the file is still held, and reloads it if the operation does not go ahead.

`UsedFontAction` maps the dialog:

- `Ask` (AnyFont's first pass) returns the busy list.
- `RebootNow` / `RebootLater` unregister now and delete at restart. All-users files use
  `MoveFileEx(DELAY_UNTIL_REBOOT)`; per-user files use `pending-deletes.json` plus `process_pending()` at the next
  start, since `MoveFileEx` needs admin.
- `Ignore` skips.
- `Cancel` changes nothing if anything is busy.

`reboot_now()` performs the dossier sequence with the same reason code.

### Delete (Recycle Bin)

- The delete handler (0x1800F2190) uses `SystemIO::File::DeleteToRecycleBin` (`CrlUtils` 0x1800A2660):
  `SHFileOperationW(FO_DELETE)` with flags 0x4050 = `FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING`.
- Strings: "Read-only fonts cannot be deleted" (`8dc2d4c8`, `f7d23d0d`) and "This font appears in more than one
  location…" (`c19cb76b`).

AnyFont uses the same call and flags. It refuses protected, read-only, missing and non-font files. An installed font
is uninstalled first, then recycled. A Type 1 pair is recycled together.

## 3. Errors, logging, crash reports

### What the dossier says

- No log file: only `OutputDebugStringW`.
- Crash reports through Crashpad (`StabilityFramework::Crashpad::InitializeCrashpad`, `CrlConfig` 0x18000F8A0), with
  an uploader in the web-service module.
- Error dialogs through `MessageBoxDS`.

### AnyFont

- **Logging:** `anyfont_sys::logging`. A rotating file at `%LOCALAPPDATA%\AnyFont\logs\anyfont.log` (1 MiB, four
  old files kept), level from `ANYFONT_LOG`. Debug builds also send lines to `OutputDebugStringW`, like the
  original.
- **Crash reports:** `anyfont_sys::crash`. A panic hook writes a text report with version, OS build, thread, message,
  location, backtrace and the log tail. The profile path is redacted. Nothing is uploaded: at the next start the UI
  offers Help ▸ Report a Bug (Phase 11, `services/issue-relay`). There is no Crashpad and no out-of-process
  handler. Native crashes (access violations) are not caught; Rust code doesn't produce them outside `unsafe`.
- **Per-file errors:** every operation returns a per-file `FontOpError`. The UI maps them to the original's
  messages (table in `sys-api.md`). "Insufficient Permissions" (`c25f92f9`) is `AccessDenied`.

## 4. Network

The original fetches cloud fonts before installing (`FontDS::DownloadFonts` 0x1800F1AC0) from the record's
`DBAssetURL`. Its endpoints are the vendor's cloud and are not reproduced.

`anyfont-sys` itself makes no network calls. Downloads (the online catalog, `ureq`) are owned by `anyfont-core`/UI.
They save into `FontSystem::content_folder()` (`Documents\AnyFont\Content\Fonts`) and then call `install` like any
local file.

## 5. Shell integration, single instance, clipboard

| Original | AnyFont |
| --- | --- |
| DDE service, topic `System` (`FontManager.exe` 0x1400029D0), plus named events (`InterProcessEvent`) for "open in the running instance" | `single_instance`: a named mutex `Local\AnyFont.<hash>.SingleInstance` and a named pipe `\\.\pipe\AnyFont.<hash>` carrying `{files, switches}` as length-prefixed JSON with an ack. `<hash>` = user + session + sandbox root. The secondary calls `AllowSetForegroundWindow(ASFW_ANY)`. |
| `ShellUtils::CrlShellExecute::Execute` for URLs | the UI opens URLs (`open`/`ShellExecute`), not this crate |
| Open file location | `SHOpenFolderAndSelectItems` (fallback `explorer.exe /select,`) |
| Recycle Bin | `SHFileOperationW` 0x4050 (above) |
| File associations / context menus | none, in either the original binaries or AnyFont |
| Clipboard / drag-and-drop | UI crates (`arboard`, egui). Not in this crate. |

## 6. Elevation (all-users install/uninstall/delete)

### What the dossier says

- `CFontDatabase::InstallFont` / `UninstallFont` / `Deletefont` (0x18006D7A0, 0x18006E8D0, 0x18006F800) and
  `CFMDeleteFonts` (0x1801E1FD0) send requests through `CInstall`/`CUninstall`/`CDeleteFontProtocol`.
- `CInterprocessProtocol::CallInterprocessCallee` (`CrlInterprocess` 0x180013000) and `LaunchProtocolCallee`
  (0x18000A2F0) start the callee with `ShellExecuteEx`:
  - verb `runas` (`open` in TemporaryMode), `fMask` 0x440, `nShow` 0;
  - wait `INFINITE`;
  - parameters are row ids written with `"%i "` (0x180184EF0); the requests themselves travel through the
    interprocess SQLite database.

### AnyFont

The same shape, without SQLite:

1. The requester writes `%LOCALAPPDATA%\AnyFont\helper\request-<id>.json`.
2. It runs `anyfont.exe /InstallHelper:"<file>"` with `runas`, `SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI`,
   `SW_HIDE` and the main window as owner, and waits.
3. It reads `request-<id>.json.result.json`, then deletes both files.

One UAC prompt per operation. A declined prompt gives `ElevationCancelled`. `Session` scope never elevates, as in
the original's TemporaryMode. Validation of the request file is described in Phase 12.
