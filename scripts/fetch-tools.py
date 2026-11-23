#!/usr/bin/env python3
"""Downloads the pinned pandoc and Typst for one system into src-tauri/tools/, where the build
picks them up for the installer. Every file is checked against its SHA-256 sum before it is used.

    python3 scripts/fetch-tools.py [--system macos-arm64|macos-x64|windows-x64|linux-x64|linux-arm64]
"""

import argparse
import hashlib
import io
import platform
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parent.parent / "src-tauri" / "tools"
PANDOC = "https://github.com/jgm/pandoc/releases/download/3.14/"
TYPST = "https://github.com/typst/typst/releases/download/v0.15.1/"

# system -> program -> (url, sha256)
PINNED = {
    "macos-arm64": {
        "pandoc": (PANDOC + "pandoc-3.14-arm64-macOS.zip", "58d6593884fec9d4424b641b92503c316d9c36decb2891ff8d73ae74dd3b6304"),
        "typst": (TYPST + "typst-aarch64-apple-darwin.tar.xz", "48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a"),
    },
    "macos-x64": {
        "pandoc": (PANDOC + "pandoc-3.14-x86_64-macOS.zip", "891708579061e7b1c1a6ce0df30ebb3cb2b6d0ca045ed3c8554ab830c1e0c270"),
        "typst": (TYPST + "typst-x86_64-apple-darwin.tar.xz", "7f9fdd9584866245de9a79e0add8f9236fae6f40a8a45e2c4771ccc14db4e0fa"),
    },
    "windows-x64": {
        "pandoc": (PANDOC + "pandoc-3.14-windows-x86_64.zip", "0144b96e8c7b023fd382ec5ef5bdc475e873f1019d6fa72502492df2c9a94a3b"),
        "typst": (TYPST + "typst-x86_64-pc-windows-msvc.zip", "19ce3551153c2fe7ee9fa2f95208310c8f4d3209fedb699e0333faf8913f6736"),
    },
    "linux-x64": {
        "pandoc": (PANDOC + "pandoc-3.14-linux-amd64.tar.gz", "c5f4720cdefb3f7bdc5d07202ef579feef0d1370773c873804dd8fdaa8c4f1b8"),
        "typst": (TYPST + "typst-x86_64-unknown-linux-musl.tar.xz", "a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c"),
    },
    "linux-arm64": {
        "pandoc": (PANDOC + "pandoc-3.14-linux-arm64.tar.gz", "c692f3d78a415741bbbac4cdbbbe26fa7edca1f43d2301b982fbf4009cc8cf7f"),
        "typst": (TYPST + "typst-aarch64-unknown-linux-musl.tar.xz", "5aa8d74a3d906e60ea12a66ac2f37f8eef1b14cbad7182a745e393a10c23dcee"),
    },
}


def this_system() -> str:
    arm = platform.machine().lower() in ("arm64", "aarch64")
    if sys.platform == "darwin":
        return "macos-arm64" if arm else "macos-x64"
    if sys.platform == "win32":
        return "windows-x64"
    return "linux-arm64" if arm else "linux-x64"


def program_in(archive: bytes, url: str, name: str) -> bytes:
    """The program's own file in a release archive: the regular file called <name> or <name>.exe."""
    wanted = (name, name + ".exe")
    if url.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(archive)) as z:
            found = [i for i in z.infolist() if i.filename.rsplit("/", 1)[-1] in wanted and i.file_size > 1_000_000]
            return z.read(found[0])
    with tarfile.open(fileobj=io.BytesIO(archive)) as t:
        found = [m for m in t.getmembers() if m.isfile() and m.name.rsplit("/", 1)[-1] in wanted]
        return t.extractfile(found[0]).read()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--system", choices=sorted(PINNED), default=this_system())
    system = parser.parse_args().system
    TOOLS.mkdir(parents=True, exist_ok=True)
    for name, (url, sha256) in PINNED[system].items():
        target = TOOLS / (name + (".exe" if system.startswith("windows") else ""))
        mark = TOOLS / f".{name}.sha256"
        if target.exists() and mark.exists() and mark.read_text().strip() == sha256:
            print(f"{target.name}: already here")
            continue
        print(f"{target.name}: downloading {url}")
        with urllib.request.urlopen(url, timeout=300) as response:
            archive = response.read()
        got = hashlib.sha256(archive).hexdigest()
        if got != sha256:
            print(f"{url}: SHA-256 is {got}, expected {sha256}. Nothing was unpacked.", file=sys.stderr)
            return 1
        target.write_bytes(program_in(archive, url, name))
        target.chmod(0o755)
        mark.write_text(sha256 + "\n")
        print(f"{target.name}: {target.stat().st_size // 1_048_576} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
