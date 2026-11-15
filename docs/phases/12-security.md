# Phase 12 – Security

Scope: `crates/anyfont-sys`, the part of AnyFont that changes the system: font registration, elevation, files
outside the app's folders, the inter-process pipe and crash files. Dossier background is in
[`10-behavior.md`](10-behavior.md); the API is in [`../sys-api.md`](../sys-api.md).

## 1. Privilege model

- **AnyFont runs unelevated.** The app manifest should stay `asInvoker`; that file is owned by the app crate. Per-user install, session fonts, browsing, previews, the content
  folder, logs and settings need no admin rights.
- **Only all-users operations elevate:** install for all users; uninstall or delete of fonts in `%WINDIR%\Fonts` or
  registered in HKLM.
  - Elevation happens **per operation**, through a short-lived helper process (`anyfont.exe /InstallHelper:<file>`)
    started with `ShellExecuteEx("runas")`. That is the original's `CInterprocessProtocol` design.
  - The main process never becomes elevated, and the helper has no UI and exits when the request is done.
- **Privileges:** `SeShutdownPrivilege` is enabled only inside `reboot_now()`. That runs only after the user chose
  "Reboot now" in the in-use prompt. No other privilege is enabled. The original enables `SeSecurityPrivilege` in
  its font worker; AnyFont doesn't need it, because reading a DACL needs only `READ_CONTROL`.
- **Session fonts** (`Scope::Session`, the original's TemporaryMode) never elevate.

## 2. The elevated helper

The request travels in a JSON file because the helper has no other channel: an elevated process can't reliably
receive a pipe from a medium-integrity one without extra ACL work. The file is
`%LOCALAPPDATA%\AnyFont\helper\request-<id>.json`, inside the user's profile.

`helper::read_request` checks, before anything runs:

- the file name is `request-*.json` and the path is absolute;
- it is a regular file (`symlink_metadata`, so a link is not followed) of at most 4 MiB;
- the envelope has the magic `AnyFont.InstallHelper` and version 1;
- an install request has scope `AllUsers` (the helper is not a general install service);
- there are 1 to 100 000 paths, all absolute, with no NUL characters.

The operation then runs through the same engine as the unelevated path, so the same rules apply:

- Protected system fonts are never uninstalled, deleted or replaced.
- `delete` only touches font files (`.ttf .otf .ttc .otc .pfm .pfb .fon .fnt`), refuses read-only files, and only
  moves files to the **Recycle Bin**. Nothing is hard-deleted.
- `uninstall` only removes files that a registry value points at and that live directly in a fonts folder. A
  registered font elsewhere is unregistered and its file is kept.
- `install` only copies **into** `%WINDIR%\Fonts`. Source files are only read.

The response is written next to the request (`.result.json`). The requester reads it, then deletes both. Files older
than 24 h are removed by `process_pending()`.

### Residual risk (TOCTOU)

The request file is writable by the user who asked for the operation. Malware already running as that user could
swap the file between the UAC consent and the helper reading it. The worst it could do is install, unregister or
recycle **non-protected font files** with admin rights:

- protected fonts are refused;
- non-font files are refused;
- deletes are recoverable from the Recycle Bin;
- installs only add fonts.

The same malware could already show its own UAC prompt, so this does not raise its power meaningfully. A future
hardening would be a per-request DACL on `helper\`, or passing the request over an anonymous pipe handle. Both were
judged not worth the complexity for this risk.

## 3. Font files as untrusted input

- Font files come from users and the internet. `anyfont-sys` only parses them with `ttf-parser` (pure Rust, no
  `unsafe` parsing in our code) to read names. The PFM reader bounds-checks every offset.
- Rasterising and validation belong to `anyfont-core` (Phase 04/05).
- **Install doesn't load a font into the system before it parses.** A file `ttf-parser` rejects is never copied or
  registered (`InvalidFont`).
- Copies into the fonts folders get a fresh, unique name in that folder (`stem_<n>.ext`). Names come only from the
  source's file name, never from data inside the font, so a crafted name table cannot write outside the fonts
  folder.
- Registry value names come from the font's name table. They are sanitised (control characters removed, at most 200
  characters). Registry value names may contain any other character, so there is no injection risk.

## 4. Inter-process channel (single instance)

- **Pipe:** `\\.\pipe\AnyFont.<hash>`, created with `FILE_FLAG_FIRST_PIPE_INSTANCE` and
  `PIPE_REJECT_REMOTE_CLIENTS`. The first flag means another process can't squat the name before the app starts.
  The default security descriptor is used, which gives the creating user, SYSTEM and Administrators access.
- **Messages:** one per connection, length-prefixed JSON, at most 1 MiB. A malformed message is logged and dropped.
- **Content:** file paths only. The primary treats them like files dropped on the window: they are shown and nothing
  is installed without the user acting.
- **Mutex:** `Local\` (per session), so another session or user can't block the app.
- `AllowSetForegroundWindow(ASFW_ANY)` lets the primary take focus. It is the standard pattern, granted only for the
  moment the secondary runs.

## 5. Secrets and personal data

- **No secrets in the app.** `anyfont-sys` holds no keys, tokens or credentials, and makes no network calls.
- **Logs** contain file paths and operation results, never font contents or user documents. They stay local.
- **Crash reports:**
  - stay local, and are only shared when the user picks Help ▸ Report a Bug;
  - before writing, `%USERPROFILE%` (in both slash styles, case-insensitive) is replaced with the literal
    `%USERPROFILE%`, so user names in profile paths don't leak;
  - the UI should show the report text before anything is sent (Phase 06).

## 6. Protecting the developer's machine (tests)

The team rule: tests never install, uninstall or delete fonts on the development PC.

- **Sandbox backend.** `system()` returns the sandbox when `ANYFONT_SANDBOX_ROOT` is set, and **panics** rather than
  falling back to the real system if the sandbox can't be opened. The sandbox:
  - copies a few system fonts as seed (read-only use of `%WINDIR%\Fonts`);
  - keeps its "registry" in a JSON file;
  - never calls `AddFontResource`, never broadcasts, never elevates and never reboots.
- **Guard (`forbid_in_tests!`).** Every real write panics in unit tests and in builds with the `forbid-real-writes`
  feature. That covers registry values, `AddFontResource`/`RemoveFontResource`, `WM_FONTCHANGE`, file copies and
  removals in the fonts folders, `MoveFileEx`, the Recycle Bin and reboot. The crate's integration tests get the
  feature through a self dev-dependency, and `tests/guard.rs` proves it. Those guard tests use only non-existent
  paths, so they would be harmless even if the guard failed.
- **Read-only real-system tests** (allowed): enumerating the installed fonts, and the Arial DACL check.
- Manual testing of real installs belongs in Windows Sandbox or a VM, never on a development machine.

## 7. What was not verified

By rule, none of these real write paths ran on a real system during development:

- install/uninstall/delete on the real registry;
- `AddFontResource`/`RemoveFontResource`, including on fonts Windows loaded at sign-in;
- the elevated helper under real UAC;
- `MoveFileEx` delay-until-reboot;
- `reboot_now`.

They are covered by the sandbox, which runs the same engine, and by code review. They need a pass in Windows Sandbox
or a VM before release (`TESTING.md`).
