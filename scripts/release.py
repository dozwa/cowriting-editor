#!/usr/bin/env python3
"""What the release workflow does to the repository when `dev` was merged into `main`.

    python3 scripts/release.py alpha    # on main: the dev version becomes the alpha's, the changelog
                                        # gets its heading, one commit and the tag are made
    python3 scripts/release.py dev      # on dev: takes main back and goes on under the next number

Neither pushes; the workflow does. `alpha` writes `tag`, `version` and `is_new` to $GITHUB_OUTPUT
and the release's notes to release-notes.md. Run by hand in a clone, they show what would happen.
"""

import datetime
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UNRELEASED = "## On `dev`, in no release"
RELEASES = "## Releases"
NOTHING = "Nothing yet."


def git(*args, check=True):
    done = subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True)
    if check and done.returncode != 0:
        sys.exit(f"git {' '.join(args)}:\n{done.stdout}{done.stderr}")
    return done


def version(*args):
    done = subprocess.run([sys.executable, str(ROOT / "scripts/set-version.py"), *args], cwd=ROOT, text=True, capture_output=True)
    if done.returncode != 0:
        sys.exit(done.stdout + done.stderr)
    return done.stdout.strip()


def out(**values):
    for name, value in values.items():
        print(f"{name}={value}")
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as file:
            file.writelines(f"{name}={value}\n" for name, value in values.items())


def move_changelog(released, today):
    """Moves what stands under the dev heading to a heading of the release. Returns what was moved."""
    path = ROOT / "CHANGELOG.md"
    text = path.read_text()
    if UNRELEASED not in text or RELEASES not in text:
        sys.exit(f"CHANGELOG.md needs the headings {UNRELEASED!r} and {RELEASES!r}")
    before, rest = text.split(UNRELEASED, 1)
    body, after = rest.split(RELEASES, 1)
    # the placeholder may still stand above the first entries
    body = body.strip().removeprefix(NOTHING).strip()
    if not body:
        return ""
    number, _, kind = released.partition("-")
    heading = f"### {number}, {kind} ({today})" if kind else f"### {number} ({today})"
    # one level down: "### Fixed" stands under the release's own "###" heading
    lower = re.sub(r"^(#{3,5}) ", r"#\1 ", body, flags=re.M)
    path.write_text(f"{before}{UNRELEASED}\n\n{NOTHING}\n\n{RELEASES}\n\n{heading}\n\n{lower}\n\n{after.lstrip()}")
    return body


def alpha():
    held = version("--check")
    released = held[: -len("-dev")] + "-alpha" if held.endswith("-dev") else held
    tag = f"v{released}"
    if git("rev-parse", "-q", "--verify", f"refs/tags/{tag}", check=False).returncode == 0:
        print(f"{tag} is there already: nothing to release.")
        out(tag=tag, version=released, is_new="false")
        return
    notes = ""
    if released != held:
        # The dev version came with a merge of dev. Only a merge commit (or a fast-forward) keeps dev's own commits
        # in main; after a squash or a rebase the two branches no longer know what they share, and taking main
        # back into dev would bring every entry of the changelog twice.
        parents = len(git("rev-list", "--parents", "-n", "1", "HEAD").stdout.split()) - 1
        is_forward = git("merge-base", "--is-ancestor", "HEAD", "origin/dev", check=False).returncode == 0
        if parents < 2 and not is_forward:
            sys.exit("dev was not merged into main with a merge commit (squashed or rebased?). Nothing was released. Merge dev into main again with \"Create a merge commit\".")
        version(released)
        notes = move_changelog(released, datetime.date.today().isoformat())
        git("commit", "-am", f"Alpha {released[: -len('-alpha')]}" if released.endswith("-alpha") else f"Release {released}")
    (ROOT / "release-notes.md").write_text((notes or "See CHANGELOG.md.") + "\n")
    git("tag", tag)
    out(tag=tag, version=released, is_new="true")


def next_dev(released):
    major, minor, patch = (int(part) for part in released.split("-")[0].split("."))
    return f"{major}.{minor}.{patch + 1}-dev"


def dev():
    """`dev` takes `origin/main` and gets the dev version after the one main holds."""
    released = subprocess.run(["git", "show", "origin/main:Cargo.toml"], cwd=ROOT, text=True, capture_output=True, check=True).stdout
    released = re.search(r'\[workspace\.package\]\nversion = "([^"]+)"', released).group(1)
    if released.endswith("-dev"):
        sys.exit(f"main holds {released}, a dev version: there is nothing to take back")
    merge = git("merge", "--no-edit", "origin/main", check=False)
    if merge.returncode != 0:
        # both branches changed the version since they last met (an alpha repaired on main): dev takes main's first
        git("merge", "--abort")
        version(released)
        git("commit", "-am", "dev takes the alpha's version for the merge")
        merge = git("merge", "--no-edit", "origin/main", check=False)
        if merge.returncode != 0:
            git("merge", "--abort")
            sys.exit(f"dev and main cannot be merged without a hand:\n{merge.stdout}{merge.stderr}\nMerge main into dev yourself and set the version to {next_dev(released)}.")
    following = next_dev(released)
    if version("--check") != following:
        version(following)
        git("commit", "-am", f"dev is {following}")
    out(version=following)


if __name__ == "__main__":
    {"alpha": alpha, "dev": dev}.get(sys.argv[1] if len(sys.argv) == 2 else "", lambda: sys.exit(__doc__))()
