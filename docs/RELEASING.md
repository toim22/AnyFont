# Releasing AnyFont

How a version goes from `main` to the users' PCs, and the names and formats the program depends on. The in-app
updater finds a new version only through the GitHub release described here, so follow the names exactly.

## Cut a release

1. **Version.** Set `version` in the root `Cargo.toml`, section `[workspace.package]` (every crate inherits it).
   Run `cargo build` once so `Cargo.lock` follows. The installer, `build.bat` and the release workflow all read the
   version from there.
2. **Changelog.** In `CHANGELOG.md`, turn `## <version> – unreleased` into `## <version> – <date>` (YYYY-MM-DD) and
   write what changed. The release workflow copies this section, without HTML comments, into the GitHub release,
   and AnyFont shows that text in its update prompt. Keep it short Markdown: headings, lists, bold.
3. **README.** Update the version badge if it names the version.
4. **Legal notices.** `build.bat` regenerates `THIRD-PARTY-NOTICES.txt` (`scripts\licenses.ps1`). Commit the file if
   it changed (a new or updated crate).
5. **Build and test locally.**

   ```bat
   build.bat test
   build.bat
   ```

   This writes `dist\AnyFont-<version>-setup.exe`, `dist\AnyFont-<version>-win-x64.zip`, `dist\AnyFont\` and
   `dist\symbols\`. Try the setup in Windows Sandbox (see `docs/TESTING.md`): a fresh install, an update over the
   previous version, the silent update below, and an uninstall. Never test font installation on the development PC.
6. **Commit and tag.**

   ```bat
   git commit -am "AnyFont <version>"
   git tag v<version>
   git push origin main v<version>
   ```

7. **Publish.** Pick one of the two paths below.

### Path A: let CI publish (recommended)

Pushing the tag `v<version>` runs `.github/workflows/build.yml`. It checks that the tag matches `Cargo.toml`, runs
the tests, runs `build.bat` on `windows-latest`, and creates the release **AnyFont \<version\>** with:

- `AnyFont-<version>-setup.exe`
- `AnyFont-<version>-win-x64.zip`
- `SHA256SUMS.txt`

The notes come from the `CHANGELOG.md` section.

### Path B: publish the local build

Create the release yourself before pushing the tag, or instead of it. `gh` creates the tag on GitHub if it doesn't
exist yet:

```bat
gh release create v<version> dist\AnyFont-<version>-setup.exe dist\AnyFont-<version>-win-x64.zip --repo toim22/AnyFont --title "AnyFont <version>" --notes-file notes.md
```

`notes.md` is the version's section of `CHANGELOG.md`. When the workflow then runs for the tag and finds the release
already there, it leaves it untouched. Its own build stays in the run's artifacts.

### After publishing

- Open <https://github.com/toim22/AnyFont/releases/latest>. It must show the new version with both files.
- Start the previous version of AnyFont, choose the Updates button (or wait for the background check) and let it
  update. AnyFont should close, update and start again on its own.

## Rules the updater depends on

- **Publish a full release.** Drafts and pre-releases are invisible to the updater, which asks GitHub for
  `releases/latest`. Use a pre-release only for a version that users should *not* get automatically.
- **Tag `v<version>`** with a plain `major.minor.patch` version (`v0.1.0`). The updater strips the `v` and compares
  the numbers with its own version.
- **Exactly one asset whose name ends in `-setup.exe`**, named `AnyFont-<version>-setup.exe`. Don't rename, re-zip
  or replace it after publishing: the updater reuses a setup it already downloaded when the asset size matches.
- **Never change the installer's `AppId`** (`{80A727BB-0AF4-4B46-A41E-269BD512170B}` in `installer/AnyFont.iss`). It
  is how a new setup finds and replaces the installed version, per user or for all users.

## Contract with the program

The UI team ports `updater.rs` and `bugreport.rs` from AnyVector. The values below are what this repository's
installer, workflow and relay provide; the program must use the same ones.

### Updater (`crates/anyfont/src/updater.rs`)

| What | Value |
| --- | --- |
| Repository (`REPO`, shared with `bugreport.rs`) | `toim22/AnyFont` |
| Latest release | `GET https://api.github.com/repos/toim22/AnyFont/releases/latest`, header `Accept: application/vnd.github+json`, `User-Agent: AnyFont/<version>`; fallback `gh api repos/toim22/AnyFont/releases/latest` |
| Fields read | `tag_name` (`v0.1.0`), `body` (the notes, Markdown), `html_url`, and from `assets[]` the first whose `name` ends with `-setup.exe` (case-insensitive): `name`, `browser_download_url`, `size` |
| Setup file | `AnyFont-<version>-setup.exe` |
| Fallback asset name in code | `AnyFont-setup.exe` |
| Silent update command | `AnyFont-<version>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RELAUNCH=1` (AnyVector's `/SILENT /SP-` variant works too: any silent mode plus `/RELAUNCH=1` restarts the program) |
| What the setup does on a silent update | closes a running `anyfont.exe` (Restart Manager, `CloseApplications=yes`), installs where the previous version is (same per-user or all-users mode), keeps the tasks chosen before, leaves the language alone, and starts `anyfont.exe` again as the original user when `/RELAUNCH=1` |

### Language chosen in the setup (`crates/anyfont/src/i18n.rs`)

- The interactive setup writes `Language` (`en-US`, `zh-CN`, `zh-TW`, `cs-CZ`, `nl-NL`, `fr-FR`, `de-DE`, `it-IT`,
  `ja-JP`, `pl-PL`, `pt-BR`, `ru-RU`, `es-ES`, `sv-SE`, `tr-TR`) and `LanguageStamp` (`yyyymmddhhnnss`) to
  `Software\AnyFont`, under `HKEY_CURRENT_USER` for a per-user install and `HKEY_LOCAL_MACHINE` for an all-users one.
- AnyFont reads HKCU first, then HKLM (as AnyVector's `installer_choice()`), and applies the language once for each
  new stamp. A silent setup writes neither value.
- The setup's language list is `Lang::ALL` in AnyVector's order. If AnyFont's list changes, update `InitializeWizard`
  and `LangOfId` in `installer/AnyFont.iss`.

### Command line and file types

- The optional task "Open font files with AnyFont" registers the ProgID `AnyFont.Font` in `OpenWithProgids` of
  `.ttf`, `.otf` and `.ttc` and `Applications\anyfont.exe`. Both run `"<install folder>\anyfont.exe" "<file>"`, so
  AnyFont must accept a font file path as its first argument and show that font. The task is unchecked by default
  and never changes the default program of those types.
- The setup installs `anyfont.exe`, `anyfont.ico` and `legal\THIRD-PARTY-NOTICES.txt`.

### Uninstall

- Removes only the program files, the shortcuts, the `Open with` entries and the two language values.
- Keeps the settings, the font database, the AnyFont Content folder (`Documents\AnyFont\Content`) and every installed
  font. AnyFont must keep its data outside the install folder (`%APPDATA%`, `%LOCALAPPDATA%`, `Documents`).

### Bug reports and ideas (`crates/anyfont/src/bugreport.rs`)

| What | Value |
| --- | --- |
| Labels | `bug` (Report a Bug), `enhancement` (Submit Ideas and Feedback) |
| Relay address (`RELAY_URL`) | `https://anyfont-issues.<subdomain>.workers.dev`, known after the first deploy (see `services/issue-relay/README.md`); probably the same Cloudflare subdomain as AnyVector's relay |
| Override for tests | environment variable `ANYFONT_ISSUE_RELAY` |
| Request | `POST <relay>/issues`, `User-Agent: AnyFont/<version>`, `Content-Type: application/json`, or `multipart/form-data` with the JSON in the part `report` and one file in the part `file` |
| JSON | `{"kind":"bug"\|"idea","title","text","more","expected","version","program":"AnyFont","system"}`. Limits in characters: title 200, text 8000, more 4000, expected 2000, version 32, program 40, system 600. `version` must be `x.y.z` (optionally with a suffix). `program` must be exactly `AnyFont`. |
| Answers | `201 {"url": "https://github.com/toim22/AnyFont/issues/<n>"}`; `400` refused, `413` too large, `429` too many (2 a minute per address), `502` GitHub failed (the program then opens the prefilled page) |
| Browser fallback | `https://github.com/toim22/AnyFont/issues/new?labels=<bug\|enhancement>&title=…&body=…` |
| Attachments | same types and sizes as AnyVector (`ATTACH_*` = `src/attachment.ts`): images and videos up to 10 MB, the other GitHub types up to 25 MB. Font files are not among them and must be zipped. |
| Issue footer | `Sent from AnyFont (Help ▸ Report a Bug)` / `(Help ▸ Submit Ideas and Feedback)`. If AnyFont puts these commands elsewhere, change `menu` in `services/issue-relay/src/report.ts`. |

### Legal notices

`THIRD-PARTY-NOTICES.txt` at the repository root lists every crate `anyfont.exe` links, with its licence. The
installer copies it to `{app}\legal`, and `build.bat` puts it in `dist\AnyFont\legal`. For Help ▸ About AnyFont ▸
Legal Notices, the program can embed it with `include_str!("../../../THIRD-PARTY-NOTICES.txt")`, or run
`scripts\licenses.ps1 -AssetCopy` to also write AnyVector's path, `crates/anyfont/assets/legal/third-party-licenses.txt`.
