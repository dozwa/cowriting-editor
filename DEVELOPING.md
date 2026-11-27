# Developing: the branch `dev` and the branch `main`

| | `dev` | `main` |
|---|---|---|
| Holds | the work in progress | the alpha that is out |
| For | the developer alone | testers outside |
| App | CoWriting Editor Dev | CoWriting Editor |
| Version | `0.1.1-dev` | `0.1.1-alpha` |
| Built | when asked for | when its tag is pushed |

Both apps install side by side and keep their own settings and keys. Signing and the details of
the release workflow are in `RELEASING.md`.

**Nothing is committed to `main` directly.** It moves only when a tried state of `dev` is merged
into it and becomes an alpha.

## Day to day: on `dev`

```
git switch dev
cargo test && (cd ui && npm test)       # before a commit
git push                                # CI checks and tests, nothing more
```

Every change gets a line in `CHANGELOG.md` under "On `dev`, in no release".

A push is checked and tested by CI (types, formatting, clippy, the tests); **no app is built**.
To have the dev app:

```
ui/node_modules/.bin/tauri dev          # run it from the source, on this computer
ui/node_modules/.bin/tauri build        # an installer of the dev app for this system

gh workflow run ci.yml --ref dev        # or: installers for macOS, Windows and Linux, built by CI
gh run watch                            # wait for it
gh run download <run id>                # fetch them; they are named cowriting-editor-dev-<commit>-<system>
```

`--ref dev` is needed: without it CI takes `main`, and refuses to build the dev app from an
alpha's version. The installers stay on the run for 90 days and are seen only by those with
access to the repository.

## From `dev` to `main`: making an alpha

Only from a state that was built as the dev app and tried. With `<tried>` its commit (or `dev`
itself, when nothing came after):

```
git switch main
git merge --no-ff --no-commit <tried>
python3 scripts/set-version.py 0.1.1-alpha
# CHANGELOG.md: move what the alpha takes to a new heading under "Releases", with the date
git commit -am "Alpha 0.1.1"
git tag v0.1.1-alpha
git push origin main v0.1.1-alpha       # the tag starts the release workflow
```

The release workflow builds the installers and puts them into a **draft** pre-release. Testers
see nothing until the draft is published on GitHub: look at the installers first, write the
notes, then publish.

Then `dev` takes the alpha back and goes on under the next number:

```
git switch dev
git merge main
python3 scripts/set-version.py 0.1.2-dev
git commit -am "dev is 0.1.2-dev"
git push
```

Merging `main` back each time is what keeps the next merge into `main` free of conflicts in the
version and the changelog.

## What keeps the two apart

- **The standard is dev.** `src-tauri/tauri.conf.json` is the dev app, so no command makes an
  alpha by accident. Only the release workflow lays `src-tauri/tauri.release.conf.json` over it.
- **A tag has to match.** The release workflow stops when the tag is not `v` plus the version in
  the files, when that version ends in `-dev`, or when the tagged commit is not on `main`.
- **The dev app has a dev version.** CI started by hand stops when the version does not end in
  `-dev`.
- **One version everywhere.** `python3 scripts/set-version.py --check` says which version the
  files hold and whether they agree; CI runs it on every push.
- **Never tag to try something.** A tag that starts with `v` builds a release. To try a state,
  build the dev app.

## Repairing an alpha that is out

When testers need a fix and `dev` is not ready to be released, the fix goes onto `main` through a
short branch, as the next alpha:

```
git switch -c fix main
# fix, test, commit (or: git cherry-pick <commit from dev>)
git switch main && git merge --no-ff --no-commit fix
python3 scripts/set-version.py 0.1.2-alpha
# CHANGELOG.md: a heading for 0.1.2 with the fix
git commit -am "Alpha 0.1.2" && git tag v0.1.2-alpha
git push origin main v0.1.2-alpha
```

Then `dev` takes the fix. Both branches have changed the version since they last met, which
would be a conflict in five files; giving `dev` the alpha's version first makes the two agree:

```
git switch dev
python3 scripts/set-version.py 0.1.2-alpha
git commit -am "dev takes the alpha's version for the merge"
git merge main
python3 scripts/set-version.py 0.1.3-dev    # the number after the one just used
git commit -am "dev is 0.1.3-dev"
git push
```

If both wrote into `CHANGELOG.md`, that one conflict is left to settle by hand.

## Where the branches stand now

`dev` was made on 2026-11-27. At that time `main` already held the fixes made since the alpha
0.1.0 (they were pushed before there was a `dev`), so until the next alpha `main` is ahead of
what testers have. From `v0.1.1-alpha` on, `main` is the alpha.
