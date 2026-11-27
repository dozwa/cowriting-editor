# Releasing

`git tag v0.2.0 && git push origin v0.2.0` runs `.github/workflows/release.yml`: it builds the
installers for macOS, Windows and Linux, with pandoc and Typst inside, and puts them into a
**draft** release on GitHub. Nothing is published until you publish the draft.

Set the version in `Cargo.toml` (`[workspace.package]`) and `src-tauri/tauri.conf.json` first.

In `CHANGELOG.md`, move what the release takes from "In the repository, in no release" to a new
heading under "Releases", with the version and the date. What stays behind is in no release.

## What is not done yet

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
