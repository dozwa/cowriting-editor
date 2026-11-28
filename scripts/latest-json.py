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
        With `--no-files` in place of `--signatures` it names no files: a release built without
        the key for updates is still shown to editors as newer, and installed by hand.

    python3 scripts/latest-json.py newer --tag v0.1.2-alpha --new FILE [--old FILE]
        Says whether FILE, the latest.json of the release `--tag`, is to replace the one editors
        are told of now ("yes" or "no"): only a newer version does. It stops with an error when
        FILE is not that release's: another version, or a file from another place.

Editors read this file from the branch `updates` (see `crates/app/src/update_check.rs` and
`.github/workflows/updates.yml`).
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


def order(version):
    """A version as something to compare: 0.1.1-alpha is before 0.1.1 and before 0.1.2-alpha, as the editor reads it."""
    found = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+.*)?", version.strip())
    if not found:
        sys.exit(f"not a version: {version!r}")
    parts = [(0, int(part), "") if part.isdigit() else (1, 0, part) for part in found[4].split(".")] if found[4] else []
    return (int(found[1]), int(found[2]), int(found[3]), 0 if parts else 1, parts)


def is_newer(tag, new, old):
    version = tag.removeprefix("v")
    if new.get("version") != version:
        sys.exit(f"the file is for {new.get('version')!r}, not for {version}")
    files = f"https://github.com/{REPOSITORY}/releases/download/{tag}/"
    for name, file in (new.get("platforms") or {}).items():
        if not str(file.get("url", "")).startswith(files) or not file.get("signature"):
            sys.exit(f"{name}: not a signed file of the release {tag}")
    return old is None or order(version) > order(str(old.get("version", "")))


def latest(tag, signatures, notes, now):
    platforms = {}
    for file in sorted(Path(signatures).glob("*.sig")) if signatures else []:
        name = file.name.removesuffix(".sig")
        key = platform(name)
        if key is None:
            continue
        if key in platforms:
            sys.exit(f"two files for {key}: {name} and {platforms[key]['url'].rsplit('/', 1)[1]}")
        platforms[key] = {"signature": file.read_text(encoding="utf-8").strip(), "url": f"https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"}
    if not platforms and signatures:
        sys.exit(f"no signature of an installer in {signatures}")
    return {"version": tag.removeprefix("v"), "notes": notes.strip(), "pub_date": now.strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": platforms}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("key")
    write = commands.add_parser("write")
    write.add_argument("--tag", required=True)
    files = write.add_mutually_exclusive_group(required=True)
    files.add_argument("--signatures")
    files.add_argument("--no-files", action="store_true")
    write.add_argument("--notes", required=True)
    write.add_argument("--out", default="latest.json")
    newer = commands.add_parser("newer")
    newer.add_argument("--tag", required=True)
    newer.add_argument("--new", required=True)
    newer.add_argument("--old")
    args = parser.parse_args()
    if args.command == "key":
        print("yes" if has_key() else "no")
        return
    if args.command == "newer":
        read = lambda file: json.loads(Path(file).read_text(encoding="utf-8"))
        old = read(args.old) if args.old and Path(args.old).is_file() else None
        print("yes" if is_newer(args.tag, read(args.new), old) else "no")
        return
    made = latest(args.tag, args.signatures, Path(args.notes).read_text(encoding="utf-8"), datetime.datetime.now(datetime.timezone.utc))
    Path(args.out).write_text(json.dumps(made, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{args.out}: {made['version']} for {', '.join(made['platforms']) or 'no files (installed by hand)'}")


if __name__ == "__main__":
    main()
