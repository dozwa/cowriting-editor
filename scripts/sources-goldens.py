#!/usr/bin/env python3
"""Runs the plugin's own bibliography and reference-check functions over fixtures/literature and
fixtures/sources-cases.json and prints what they return as JSON. Developer and CI only.

    python3 -I scripts/sources-goldens.py <plugin repo> <fixtures folder>
"""

import difflib
import json
import os
import sys
from pathlib import Path

plugin, fixtures = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
sys.path.insert(0, str(plugin / "plugin" / "tools"))
import bibliography  # noqa: E402
import verify_refs  # noqa: E402
import zotero_search  # noqa: E402

cases = json.loads((fixtures / "sources-cases.json").read_text(encoding="utf-8"))
files = bibliography.find_files(fixtures / "literature")


def searched(query: str) -> list[dict]:
    # the plugin searches literature/ of the folder it runs in
    here = os.getcwd()
    os.chdir(fixtures)
    try:
        return zotero_search.from_bib(query, 15)
    finally:
        os.chdir(here)


out = {
    "files": [p.name for p in files],
    "formats": {p.name: bibliography.format_name(p) for p in files},
    "entries": {p.name: bibliography.load(p) for p in files},
    "dois": [bibliography.clean_doi(d) for d in cases["dois"]],
    "norms": [bibliography.norm(t) for t in cases["titles"]],
    "families": [bibliography.family(n) for n in cases["families"]],
    "ratios": [difflib.SequenceMatcher(None, bibliography.norm(a), bibliography.norm(b)).ratio() for a, b in cases["pairs"]],
    "similar": [verify_refs.similar(a, b) for a, b in cases["pairs"]],
    "searches": [searched(q) for q in cases["searches"]],
}
sys.stdout.write(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
