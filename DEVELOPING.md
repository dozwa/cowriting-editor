# Developing: the dev app and the alpha

The editor is built in two lines. They are **not two git branches**: all work is on `main`, and
what makes an alpha is a tag on one of its commits.

| | Dev | Alpha |
|---|---|---|
| For | the developer alone | testers outside |
| App | CoWriting Editor Dev | CoWriting Editor |
| Version | `0.1.1-dev` | `0.1.1-alpha` |
| In git | every commit on `main` | the commit tagged `v0.1.1-alpha` |
| Built | when asked for | when the tag is pushed |

Both apps install side by side and keep their own settings and keys. The details of signing and
of the release workflow are in `RELEASING.md`.

## Day to day: dev

Work on `main`. A push is checked and tested by CI (types, formatting, clippy, the tests); **no
app is built**.

```
cargo test && (cd ui && npm test)       # before a commit
git push                                # CI checks and tests, nothing more
```

Every change gets a line in `CHANGELOG.md` under "In the repository, in no release".

To have the dev app:

```
ui/node_modules/.bin/tauri dev          # run it from the source, on this computer
ui/node_modules/.bin/tauri build        # an installer of the dev app for this system

gh workflow run ci.yml                  # or: installers for macOS, Windows and Linux, built by CI
gh run watch                            # wait for it
gh run download <run id>                # fetch them; they are named cowriting-editor-dev-<commit>-<system>
```

CI builds the commit `main` is at. For another one: `gh workflow run ci.yml --ref <branch or tag>`.
The installers stay on the run for 90 days and are seen only by those with access to the
repository.

## Making an alpha

Only from a state that was built as the dev app and tried. Note its commit; then, on that commit:

```
python3 scripts/set-version.py 0.1.1-alpha
# CHANGELOG.md: move what the alpha takes to a new heading under "Releases", with the date
git commit -am "Alpha 0.1.1"
git tag v0.1.1-alpha
git push origin main v0.1.1-alpha       # the tag starts the release workflow

python3 scripts/set-version.py 0.1.2-dev
git commit -am "main is 0.1.2-dev"
git push
```

The release workflow builds the installers and puts them into a **draft** pre-release. Testers
see nothing until the draft is published on GitHub: look at the installers first, write the
notes, then publish.

If `main` has moved on since the tried state, make the alpha from that state and not from the
tip: `git switch -c alpha-0.1.1 <commit>`, do the three steps up to the tag there, push the
branch and the tag, and set `main` to the next dev version separately.

## What keeps the two apart

- **The standard is dev.** `src-tauri/tauri.conf.json` is the dev app, so no command makes an
  alpha by accident. Only the release workflow lays `src-tauri/tauri.release.conf.json` over it.
- **A tag has to match.** The release workflow stops when the tag is not `v` plus the version in
  the files, or when that version ends in `-dev`.
- **One version everywhere.** `python3 scripts/set-version.py --check` says which version the
  files hold and whether they agree; CI runs it on every push.
- **Never tag to try something.** A tag that starts with `v` builds a release. To try a state,
  build the dev app.

## Repairing an alpha that is out

When testers need a fix and `main` is not ready to be released:

```
git switch -c fix-0.1.1 v0.1.1-alpha    # a branch from the alpha
# fix, test, commit (or: git cherry-pick <commit from main>)
python3 scripts/set-version.py 0.1.2-alpha
git commit -am "Alpha 0.1.2" && git tag v0.1.2-alpha
git push origin fix-0.1.1 v0.1.2-alpha
```

Then bring the fix to `main` if it is not there, and set `main` to the dev version after the
number just used (`0.1.3-dev`), so that no two builds share a number.
