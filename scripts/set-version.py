#!/usr/bin/env python3
"""Sets the editor's version in every file that holds it, or checks that they agree.

    python3 scripts/set-version.py 0.1.2-dev        # main, between releases: the dev app
    python3 scripts/set-version.py 0.1.1-alpha      # the commit that is tagged v0.1.1-alpha
    python3 scripts/set-version.py --check          # all files hold the same version
    python3 scripts/set-version.py --check v0.1.1-alpha   # ... and it is this tag's, and not a dev version

A version is MAJOR.MINOR.PATCH with "-dev" or "-alpha" behind it, or nothing for a full release.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION = re.compile(r"^\d+\.\d+\.\d+(-(dev|alpha))?$")
# the workspace's own packages in Cargo.lock
PACKAGES = ["cowriting-app", "cowriting-backends", "cowriting-core", "cowriting-editor", "cowriting-suites"]


def held():
    """Where the version stands: file -> version."""
    cargo = re.search(r'\[workspace\.package\]\nversion = "([^"]+)"', (ROOT / "Cargo.toml").read_text())
    lock = (ROOT / "Cargo.lock").read_text()
    found = {
        "Cargo.toml": cargo.group(1) if cargo else None,
        "src-tauri/tauri.conf.json": json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())["version"],
        "ui/package.json": json.loads((ROOT / "ui/package.json").read_text())["version"],
        "ui/package-lock.json": json.loads((ROOT / "ui/package-lock.json").read_text())["packages"][""]["version"],
    }
    for name in PACKAGES:
        entry = re.search(rf'name = "{name}"\nversion = "([^"]+)"', lock)
        found[f"Cargo.lock ({name})"] = entry.group(1) if entry else None
    return found


def replace(path, pattern, replacement, count=1):
    text = (ROOT / path).read_text()
    changed, n = re.subn(pattern, replacement, text, count=count, flags=re.M)
    if n != count:
        sys.exit(f"{path}: expected {count} place(s) for the version, found {n}")
    (ROOT / path).write_text(changed)


def main():
    args = sys.argv[1:]
    if args[:1] == ["--check"]:
        found = held()
        versions = set(found.values())
        if len(versions) != 1 or None in versions:
            sys.exit("The files do not agree on the version:\n" + "\n".join(f"  {where}: {version}" for where, version in found.items()))
        version = versions.pop()
        if not VERSION.match(version):
            sys.exit(f"{version} is not MAJOR.MINOR.PATCH, with -dev or -alpha behind it or nothing")
        if len(args) > 1:
            if args[1] != f"v{version}":
                sys.exit(f"The tag is {args[1]}, the files hold {version}: a release is tagged v{version}")
            if version.endswith("-dev"):
                sys.exit(f"{version} is a dev version: it is not released. Set the alpha's version first.")
        print(version)
        return
    if len(args) != 1 or not VERSION.match(args[0]):
        sys.exit(__doc__)
    version = args[0]
    replace("Cargo.toml", r'(\[workspace\.package\]\nversion = )"[^"]+"', rf'\1"{version}"')
    replace("src-tauri/tauri.conf.json", r'^(  "version": )"[^"]+"', rf'\1"{version}"')
    replace("ui/package.json", r'^(  "version": )"[^"]+"', rf'\1"{version}"')
    replace("ui/package-lock.json", r'\A(\{\n  "name": "cowriting-editor-ui",\n  "version": )"[^"]+"', rf'\1"{version}"')
    replace("ui/package-lock.json", r'(    "": \{\n      "name": "cowriting-editor-ui",\n      "version": )"[^"]+"', rf'\1"{version}"')
    for name in PACKAGES:
        replace("Cargo.lock", rf'(name = "{name}"\nversion = )"[^"]+"', rf'\1"{version}"')
    print(f"The version is {version} in: " + ", ".join(held()))


main()
