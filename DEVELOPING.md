# Developing: the branch `dev` and the branch `main`

| | `dev` | `main` |
|---|---|---|
| Holds | the work in progress | the alpha that is out |
| For | the developer alone | testers outside |
| App | CoWriting Editor Dev | CoWriting Editor |
| Version | `0.1.1-dev` | `0.1.1-alpha` |
| Built | when asked for | when `dev` is merged into it |

Both apps install side by side and keep their own settings and keys. Signing and the details of
the release workflow are in `RELEASING.md`.

**Nothing is committed to `main` directly.** It moves only when a tried state of `dev` is merged
into it, and **every merge into `main` makes an alpha**: the version, the tag, the installers and
the next dev version follow by themselves.

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

Only from a state that was built as the dev app and tried.

1. Open a pull request from `dev` into `main` (`gh pr create --base main --head dev`). CI checks
   and tests it.
2. Merge it with **"Create a merge commit"** (`gh pr merge --merge`). Not squashed and not
   rebased: the release workflow refuses those, because the two branches would no longer know
   what they share.

The merge starts `.github/workflows/release.yml`, which does the rest:

- **On `main`:** `0.1.1-dev` becomes `0.1.1-alpha` in every file, the entries under "On `dev`, in
  no release" in `CHANGELOG.md` move under a heading "0.1.1, alpha" with the date, and that is
  committed as "Alpha 0.1.1" and tagged `v0.1.1-alpha`.
- **The installers** of CoWriting Editor are built for macOS, Windows and Linux and put into a
  **draft** pre-release, with those changelog entries as its notes.
- **On `dev`:** `main` is merged back and the version becomes `0.1.2-dev`.

Testers see nothing until the draft is published on GitHub: look at the installers first, then
publish. Afterwards `git pull` on `dev` before going on: the workflow has added commits to it.

To release an older state than the tip of `dev`, make the pull request from a branch at that
commit (`git switch -c alpha <commit> && git push -u origin alpha`).

The number is the one `dev` holds. For a larger step, set it on `dev` before the pull request:
`python3 scripts/set-version.py 0.2.0-dev` makes the next alpha `0.2.0-alpha`.

`python3 scripts/release.py alpha` on `main` and `python3 scripts/release.py dev` on `dev` are
what the workflow runs. Neither pushes, so in a clone they show what a merge would do.

## What keeps the two apart

- **The standard is dev.** `src-tauri/tauri.conf.json` is the dev app, so no command makes an
  alpha by accident. Only the release workflow lays `src-tauri/tauri.release.conf.json` over it.
- **Only `main` releases.** The release workflow runs on a push to `main` and on nothing else; a
  tag pushed by hand builds nothing. A version that is tagged already is not released twice.
- **Only a real merge releases.** A squashed or rebased merge of `dev` is refused, and nothing is
  tagged or built.
- **The dev app has a dev version.** CI started by hand stops when the version does not end in
  `-dev`.
- **One version everywhere.** `python3 scripts/set-version.py --check` says which version the
  files hold and whether they agree; CI runs it on every push.
- **Never merge into `main` to try something.** To try a state, build the dev app.

## Repairing an alpha that is out

When testers need a fix and `dev` is not ready to be released, the fix goes into `main` through a
short branch, under the next number, which is set by hand here:

```
git switch -c fix origin/main
# fix, test, commit (or: git cherry-pick <commit from dev>)
python3 scripts/set-version.py 0.1.2-alpha
# CHANGELOG.md: a heading "### 0.1.2, alpha (date)" under "Releases", with the fix
git commit -am "Alpha 0.1.2: <what was repaired>"
git push -u origin fix
gh pr create --base main --head fix && gh pr merge --merge
```

The workflow tags `v0.1.2-alpha`, builds it, merges `main` into `dev` and sets `dev` to
`0.1.3-dev`, so that no two builds share a number. If both branches wrote into the same lines of
`CHANGELOG.md`, that last step stops and says so; the alpha is built all the same, and `main` is
then merged into `dev` by hand.

## Where the branches stand now

`dev` was made on 2026-11-27 and holds everything done since: ten fixes, the changelog, the dev
app and these workflows. `main` was set back that day to where it stood before those fixes, which
had been pushed to it when there was no `dev` yet. It is the alpha 0.1.0 plus four commits that
change no behaviour of the app (the licence file, the README, the releasing notes and a repair of
the release build).

Until the first merge of `dev`, `main` still has the old workflows: there CI builds the app on
every push, and cannot be started by hand from the Actions page. `gh workflow run ci.yml --ref dev`
is the way to the dev app until then. The first merge of `dev` into `main` makes `v0.2.0-alpha`: `dev` was set to `0.2.0-dev` for it.
