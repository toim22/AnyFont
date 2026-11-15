# anyfont-issues: bug reports and ideas without a GitHub account

AnyFont's *Help › Report a Bug* and *Help › Submit Ideas and Feedback* send their form here. This Cloudflare Worker:
- checks the report
- allows at most 2 reports a minute from one address
- creates the issue on `toim22/AnyFont` as the **AnyFont Reports** GitHub App, labeled `bug` or `enhancement`
- keeps the file the user attached, if any, and links it in the issue (an image shows in it)

## Attachments

GitHub has no API for the files people drag into an issue, so the Worker keeps each attached file as an asset of the
release **Report attachments** (tag `report-attachments`) in a repository of its own,
[`toim22/AnyFont-attachments`](https://github.com/toim22/AnyFont-attachments) (`ATTACHMENTS_REPO` in
`cloudflare.config.ts`). That keeps AnyFont's Releases page for versions only. The Worker creates the release the
first time. The issue gets an *Attachment* section with a link to the file, and the picture itself for an image.

The Worker takes the same types and sizes GitHub takes in an issue (`src/attachment.ts`, the same lists as
`ATTACH_*` in `crates/anyfont/src/bugreport.rs`):
- images (.png, .gif, .jpg, .jpeg, .svg) and videos (.mp4, .mov, .webm): up to 10 MB
- the other types GitHub lists (.pdf, .zip, .txt, .log, Office files and so on): up to 25 MB

A font file (`.ttf`, `.otf`, `.ttc`, `.woff`…) has to go in a `.zip`, as on GitHub. If the issue can't be created, the uploaded file is
deleted. To remove a file later, delete its asset on the release's page.

The person reporting needs no GitHub account. No key ships inside AnyFont: the app's private key lives only in the
Worker's secrets.

When the Worker can't be reached, AnyFont falls back to opening GitHub's new-issue page in the browser, filled in.
That path needs a GitHub account.

It is the same service as AnyVector's and AnyCapture's (`services/issue-relay` in those repositories), with its own
name, GitHub App, repositories and rate-limit namespace (`4203`; AnyCapture uses `4201`, AnyVector `4202`).

## Files

- `src/index.ts`: `POST /issues` (JSON, or multipart with the JSON in the part `report` and the file in the part
  `file`) → `201 {"url": …}`. It answers 400 when it refuses a report, 413 when the
  report is too large, 429 when there were too many, and 502 when GitHub failed. `GET /` answers `ok`.
- `src/report.ts`: the checks (kind, required title and description, lengths, version, program) and the issue's
  text. `@name` mentions and "fixes #1" are defused, so a report can't ping people or close issues.
- `src/attachment.ts`: the attachment rules, the asset name and the issue's *Attachment* section.
- `src/github.ts`: the GitHub App sign-in, the issue creation and the attachment upload. A JWT signed with the private key gets a one-hour
  installation token, which is kept while valid.
- `test.ts`: offline tests. Run them with `npm test`; every deploy runs them first.

## Set up (once)

Nothing here has been done yet: the Worker is not published, and the GitHub App and the attachments repository don't
exist. Until then, AnyFont's reports open GitHub's new-issue page in the browser.

### 0. The repositories

1. `toim22/AnyFont` (public) receives the issues. New GitHub repositories already have the labels `bug` and
   `enhancement`; if they were deleted, create them again (the Worker drops a missing label rather than failing).
2. Create the public repository `toim22/AnyFont-attachments` with **Add a README file** ticked. It only keeps the
   release with the attached files, and the README gives it the first commit the release's tag needs.

### 1. Create the GitHub App

1. Open <https://github.com/settings/apps/new> (signed in as `toim22`).
2. **GitHub App name**: `AnyFont Reports`. Any free name works; it shows on the issues as `<name>[bot]`.
3. **Homepage URL**: `https://github.com/toim22/AnyFont`.
4. **Webhook**: untick **Active** (the app receives nothing).
5. **Permissions › Repository permissions**: **Issues: Read and write** and **Contents: Read and write** (for the
   attachments release). Leave everything else as it is (*Metadata: Read-only* is added by itself).
6. **Where can this GitHub App be installed?** **Only on this account**.
7. **Create GitHub App**.

### 2. Get its ID and private key

1. On the app's page, copy the **App ID** (a number, under *About*).
2. At the bottom, **Private keys › Generate a private key**. GitHub downloads a `.pem` file.
3. Save it in this folder as `github-app.pem`. It never goes into Git (`.gitignore`).

### 3. Install it on the repository

1. In the app's settings, left menu, **Install App** → **Install** next to `toim22`.
2. **Only select repositories** → `AnyFont` and `AnyFont-attachments` → **Install**. The attachments repository
   is a public repository with only a README; it needs a first commit so the release can have a tag.

### 4. Publish the Worker

In this folder (`services/issue-relay`):

1. Copy `secrets.example.env` to `secrets.env` and put the App ID after `GITHUB_APP_ID=`.
2. Run:

```bash
npm.cmd install
```

```bash
npm.cmd run first-deploy
```

`first-deploy` does three things:
- runs the tests
- adds the private key to `secrets.env` (`secrets.mjs`)
- publishes with `cf deploy --secrets-file secrets.env`

If `cf` isn't signed in to Cloudflare yet, it asks you to sign in in the browser. At the end it prints the address,
`https://anyfont-issues.<your subdomain>.workers.dev`.

3. Check it: opening that address in the browser shows `ok`.
4. Delete `secrets.env` and `github-app.pem`, or keep the `.pem` somewhere safe outside the repository. The keys are
   stored at Cloudflare now.

### 5. Point AnyFont at it

`RELAY_URL` in `crates/anyfont/src/bugreport.rs` holds the address. To try a Worker somewhere else without
rebuilding, set the environment variable `ANYFONT_ISSUE_RELAY` to its address before starting AnyFont.

## Every day

- **Changed the code?** Run `npm.cmd run deploy` (tests first).
- **Logs:** Cloudflare dashboard › Workers & Pages › `anyfont-issues` › Logs. They show each created issue and every
  GitHub error.
- **A new private key** (the old one leaked or was deleted): generate it on the app's page and save it as
  `github-app.pem`. Put the ID in `secrets.env` again and run `npm.cmd run first-deploy`.
- **Spam:** close or delete the issue on GitHub. To stop all reports at once, uninstall the app from the repository
  (GitHub › Settings › Applications › AnyFont Reports › Configure › Uninstall). AnyFont then opens the browser
  form instead.

## Cost

Free. The Workers free plan covers 100,000 requests a day, and the rate limiting binding is included.
