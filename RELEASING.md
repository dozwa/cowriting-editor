# Releasing

There are two apps, built from the same code:

| | Dev app (branch `dev`) | Alpha (branch `main`) |
|---|---|---|
| For | the developer alone | testers outside |
| Name | CoWriting Editor Dev | CoWriting Editor |
| Identifier | `io.github.dozwa.cowriting-editor.dev` | `io.github.dozwa.cowriting-editor` |
| Version | `0.1.1-dev` | `0.1.1-alpha` |
| Built by | CI started by hand on `dev`, or `tauri build` | a merge of `dev` into `main` |
| Found | on the CI run, for 90 days, with access to the repository | on the releases page, as a pre-release |

Both can be installed side by side. Each has its own settings, its own keys in the system's key
store and its own defaults; a document's `.cowriting` folder is the same for both.

**The dev app is the standard.** `src-tauri/tauri.conf.json` describes it, so `tauri dev`, a plain
`tauri build` and CI all make the dev app. Only the release workflow lays
`src-tauri/tauri.release.conf.json` over it, which gives the released name and identifier.

The commands for both, day to day, are in `DEVELOPING.md`.

## Versions

`dev` holds a dev version: the next alpha's number with `-dev` behind it; `main` holds the
version of the alpha that is out. After `0.1.1-alpha`
comes `0.1.2-dev`, then `0.1.2-alpha`, and so on. The release workflow sets both by itself.
`python3 scripts/set-version.py <version>` writes a version into every file that holds it, for a
larger step; `--check` (run by CI) says whether they agree.

## From the dev app to an alpha

Work is on the branch `dev`. A state of it that was built as the dev app and tried is merged into
`main` by a pull request, with a merge commit; the steps are in `DEVELOPING.md`.

The merge runs `.github/workflows/release.yml`. It gives `main` the alpha's version, moves the
changelog's entries under a heading of that version, tags the commit, builds the installers for
macOS, Windows and Linux, with pandoc and Typst inside, and puts them into a **draft**
pre-release on GitHub. Then it merges `main` back into `dev` and sets the next dev version there.
Nothing is published until you publish the draft. A version without `-alpha` on `main` is built
the same way as a full release.

The workflow commits and pushes to `main` and `dev` as `github-actions[bot]`. If either branch is
protected one day, that bot has to be allowed to push, or the workflow needs a token of its own.

## What is not done yet

- **The release workflow as it is now has never run on GitHub.** What it does to the two
  branches was run through in a clone (two alphas in a row, a repair, a squashed merge that is
  refused); that it may push to `main` and `dev`, opens the draft and hands the tag to the build
  shows on the first merge.
- **The dev app and the alpha as two apps have not been built yet.** The configuration is read
  and checked, but no installer of either was made since: the first run of each workflow will show
  whether the bundlers take a version like `0.1.1-dev`. The Windows `.msi` and the Linux `.rpm`
  were never published and are no longer built; an `.msi` cannot carry such a version.
- **The release workflow ran once, for v0.1.0.** Windows and Linux built; the macOS job failed on
  the empty signing secrets, which is repaired but not run again. The macOS image of v0.1.0 was
  built, signed and notarised by hand.
- **Signing is prepared, not tried.** Without the secrets below the installers are unsigned:
  macOS says the app is from an unidentified developer (right-click → Open gets past it), Windows
  shows a SmartScreen warning.
- **Updates from inside the app are built and switched off.** The check for a newer version, its
  dialog, the updater and the release workflow's part are there; what is missing is the key for
  updates and the server's address (below). Until both are set, no version asks or updates
  itself. Fetching and installing an update was never run: that needs two signed releases.
- **The Windows certificate is not wired in**, because how depends on what you buy (see below).

## macOS: Developer ID and notarisation

Needs an Apple Developer membership (99 USD a year). In the repository's settings, under
Secrets and variables → Actions, add:

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | Your "Developer ID Application" certificate, exported as .p12 and base64-encoded |
| `APPLE_CERTIFICATE_PASSWORD` | The password you gave the .p12 |
| `APPLE_SIGNING_IDENTITY` | Its name, e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Your Apple ID's email address |
| `APPLE_PASSWORD` | An app-specific password from appleid.apple.com |
| `APPLE_TEAM_ID` | Your team id |

To check on the first signed build: Apple notarises an app only if **every** program inside it is
signed, and this app carries pandoc and Typst in `Contents/Resources/tools/`. If notarisation
rejects them, they have to be signed in the workflow before bundling, or moved to Tauri's
`externalBin`, which signs them.

## Windows: code signing

Certificates for individuals now come on a hardware token or as a cloud service (Azure Trusted
Signing, SSL.com eSigner, DigiCert KeyLocker); a plain certificate file is no longer sold. Which
one you choose decides the workflow step, so it is not written yet. Tauri's guide:
https://tauri.app/distribute/sign/windows/

## Updates from inside the app

How it works, in the order things happen:

1. **A release is built with signatures.** With the key for updates, `tauri build` writes a
   signature (`.sig`) beside every installer and, on macOS, the app as an archive
   (`.app.tar.gz`), which is what an update there installs. The signature names the version it
   was made for. The workflow puts all of that into the draft release, and the job `updates`
   writes `latest.json` from the signatures (`scripts/latest-json.py`): the version, the
   changelog's entries, and for each system the file on GitHub and its signature.
2. **The editor asks the server**, at each start and when the author asks: one GET of the
   address in `SERVER` (`crates/app/src/update_check.rs`), with nothing in it about the editor or
   the author. The server answers with the `latest.json` of the newest published release and
   holds no files itself (#15).
3. **The author starts the update** in the dialog. The updater takes the answer the check got,
   fetches the file from the release on GitHub, and installs it only if the signature fits the
   public key built into the app and names exactly the announced version
   (`requireSignedVersion`). A file from anywhere but this project's releases is refused before
   anything is fetched. So a server that answers wrongly can at worst offer nothing.

What the maintainer has to do once:

- **Make the key pair**, yourself, so the private key never passes through anyone else's hands:
  `ui/node_modules/.bin/tauri signer generate -w ~/.tauri/cowriting-editor.key`. Give it a
  password.
- **The public key** (the content of the `.pub` file, one line) goes into `pubkey` in
  `src-tauri/tauri.updater.conf.json`. That file is laid over a release's configuration only
  while it holds a key; the dev app never has it.
- **The private key** (the content of the key file) becomes the secret
  `TAURI_SIGNING_PRIVATE_KEY`, its password `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. With a public
  key in the file and no secret, the release build stops and says so.
- **Keep the private key and its password safe, outside GitHub too.** Lose them and installed
  copies can no longer be updated: every author would have to install a release by hand once.
- **The server's address** goes into `SERVER` in `crates/app/src/update_check.rs`, and the
  privacy statement's part on the check is filled in (`docs/PRIVACY.md`, `docs/DATENSCHUTZ.md`).
  The first release with both the key and the address is the first one that can be updated
  *from*; everyone on an older one installs it by hand.

What the server has to answer: the newest published release's `latest.json`, unchanged, with
`content-type: application/json`. Alphas are pre-releases, so GitHub's "latest release" does not
name them; the server has to take the newest release from the list of releases, or be told of a
new one. A draft's `latest.json` already names the files where they will be once the release is
published, so nothing has to be rewritten on publishing.

To try the check without a server: `COWRITING_UPDATE_SERVER=http://127.0.0.1:<port>/latest.json`
makes the dev app and the bridge ask a server on this computer (only such an address is
accepted). They show the notice and cannot install.

Still to find out on the first signed releases:

- **macOS** installs an update only from a signed and notarised build, and only where the app
  may be replaced (not from the disk image, not from a folder the user cannot write to).
- **Windows** unsigned: the installer of an update shows SmartScreen again each time.
- **Linux**: an AppImage replaces itself; whether the `.deb` gets a signature and updates through
  the updater (it needs the rights to install a package) was not tried.
- If one system's build fails, the job `updates` does not run and the draft has no
  `latest.json`: run the failed job again before publishing.

Guide: https://tauri.app/plugin/updater/

## Changing the terms of use or the privacy statement

The four texts are `docs/TERMS.md`, `docs/NUTZUNGSBEDINGUNGEN.md`, `docs/PRIVACY.md` and
`docs/DATENSCHUTZ.md`; English and German say the same. While one of them has a part in double
square brackets, all of them are drafts: nobody is asked to agree, and no usage figures are asked
for or sent.

When a text changes in what it says, set `VERSION` in `crates/app/src/terms.rs` to the day of the
change and write the same day at the top of both terms. Every author is then asked to agree
again at the first start of the release that carries the change. A corrected typo needs no new
version. `COWRITING_TERMS_TRIAL=1` shows the dialog in the dev app and the bridge while the texts
are drafts.

The texts were written without a lawyer. Have them looked at before the brackets are filled in.

## Changing a file the editor keeps

The rules are at the top of `crates/core/src/stored.rs`; every stored JSON file is read and
written through that module and has a `Kind` there. Version 0.1.0 wrote the first form of each.

- **A new field** needs no new form. Give it a standard (`#[serde(default)]`), so an old file
  reads. An older editor keeps the field when it saves the file (`extra` in the types).
- **A field renamed, retyped or removed, or a meaning changed,** is a new form:
  1. Write the step: a function on the file's JSON that takes the last form to the new one, and
     add it to the end of the kind's `steps`. The form's number follows from their count. Steps
     are never changed or removed afterwards: a file may be any number of forms behind.
  2. Add a fixture: the file as the last released version wrote it, under
     `fixtures/stored/<version>/` (`fixtures/stored/v0.1.0` was made by running v0.1.0), and a
     test in `crates/app/tests/update.rs` that opens it and finds everything still there.
  3. Say it in `CHANGELOG.md`: a released editor of the form before will leave such a file as it
     is and ask for an update, and will not open a document whose `doc.json` is newer.
- **The log** (`ai-log/*.jsonl`) has no form number. Lines are only added; none is rewritten.
  A new kind of line or a new field is fine. The reader (`LogEntry::from_value`) must go on
  understanding every form a line ever had; `update.rs` reads the log of v0.1.0 for that.

## The programs inside

pandoc (GPL-2.0-or-later) and Typst (Apache-2.0) ship unchanged; their licences and source links
are in `src-tauri/tools/`. To move to newer versions, change both together in
`scripts/fetch-tools.py` (URLs and SHA-256 sums) and run the export test: an older pandoc with a
newer Typst fails.
