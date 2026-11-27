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
- **Updates from inside the app are not built.** They need a signing key for updates (below) and
  Tauri's updater plugin.
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

Tauri's updater checks a signature of its own on every update. That needs a key pair: the public
key goes into `tauri.conf.json`, the private key stays a GitHub secret
(`TAURI_SIGNING_PRIVATE_KEY`). Make it yourself, so the private key never passes through anyone
else's hands: `ui/node_modules/.bin/tauri signer generate`. Lose that key and installed copies
can no longer be updated. Guide: https://tauri.app/plugin/updater/

## The programs inside

pandoc (GPL-2.0-or-later) and Typst (Apache-2.0) ship unchanged; their licences and source links
are in `src-tauri/tools/`. To move to newer versions, change both together in
`scripts/fetch-tools.py` (URLs and SHA-256 sums) and run the export test: an older pandoc with a
newer Typst fails.
