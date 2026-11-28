#!/usr/bin/env python3
"""The file an installed editor learns of a release from, and whether releases are built for updates.

    python3 scripts/latest-json.py key
        Says whether `src-tauri/tauri.updater.conf.json` holds the public key for updates
        ("yes" or "no"). Without it a release is built as before, and cannot update itself.

    python3 scripts/latest-json.py write --tag v0.1.2-alpha --signatures DIR --notes FILE [--out latest.json]
        Writes latest.json for the release `--tag`, in the form Tauri's updater reads: the version,
        what changed, the day, and for each system the file to fetch and its signature. DIR holds
        the release's `.sig` files under the names they have on GitHub; each stands for the file
        of the same name without `.sig`. The files are named where they will be once the release
        is published: https://github.com/<repository>/releases/download/<tag>/<name>.

The editor's server passes this file on as its answer (see `crates/app/src/update_check.rs`).
"""

import argparse
import datetime
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPOSITORY = "dozwa/cowriting-editor"

# What a file is for, by the end of its name: the system as the updater names it, and the kind of installer.
KINDS = [
    (".app.tar.gz", "darwin", ""),
    ("-setup.exe", "windows", ""),
    (".AppImage", "linux", ""),
    (".deb", "linux", "deb"),
    (".rpm", "linux", "rpm"),
]
ARCHITECTURES = [(r"aarch64|arm64", "aarch64"), (r"x86_64|amd64|x64", "x86_64")]
# The app bundle of macOS has no architecture in its name; the release builds it on Apple silicon.
STANDARD = {"darwin": "aarch64", "windows": "x86_64", "linux": "x86_64"}


def has_key():
    config = json.loads((ROOT / "src-tauri/tauri.updater.conf.json").read_text(encoding="utf-8"))
    return bool(config["plugins"]["updater"]["pubkey"].strip())


def platform(name):
    """The updater's name for what a file is for ("darwin-aarch64", "linux-x86_64-deb"), or None."""
    for ending, system, installer in KINDS:
        if name.endswith(ending):
            found = [arch for pattern, arch in ARCHITECTURES if re.search(rf"(?<![A-Za-z0-9])({pattern})(?![A-Za-z0-9])", name)]
            arch = found[0] if found else STANDARD[system]
            return f"{system}-{arch}" + (f"-{installer}" if installer else "")
    return None


def latest(tag, signatures, notes, now):
    platforms = {}
    for file in sorted(Path(signatures).glob("*.sig")):
        name = file.name.removesuffix(".sig")
        key = platform(name)
        if key is None:
            continue
        if key in platforms:
            sys.exit(f"two files for {key}: {name} and {platforms[key]['url'].rsplit('/', 1)[1]}")
        platforms[key] = {"signature": file.read_text(encoding="utf-8").strip(), "url": f"https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"}
    if not platforms:
        sys.exit(f"no signature of an installer in {signatures}")
    return {"version": tag.removeprefix("v"), "notes": notes.strip(), "pub_date": now.strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": platforms}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("key")
    write = commands.add_parser("write")
    write.add_argument("--tag", required=True)
    write.add_argument("--signatures", required=True)
    write.add_argument("--notes", required=True)
    write.add_argument("--out", default="latest.json")
    args = parser.parse_args()
    if args.command == "key":
        print("yes" if has_key() else "no")
        return
    made = latest(args.tag, args.signatures, Path(args.notes).read_text(encoding="utf-8"), datetime.datetime.now(datetime.timezone.utc))
    Path(args.out).write_text(json.dumps(made, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{args.out}: {made['version']} for {', '.join(made['platforms'])}")


if __name__ == "__main__":
    main()
