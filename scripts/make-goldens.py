#!/usr/bin/env python3
"""Writes fixtures/goldens/: what the CoWriting plugin's own code makes of the fixtures.

The Rust core is a port of the plugin's logic; these files are how the two are kept in step.
The disclosure and the prompts appendix come from the plugin's ailog.py, the rest from its
core.ts (run with Node). Developer and CI only: nothing here ships with the editor.

    scripts/make-goldens.py [--plugin ../Claude_CoWriting] [--check]

--check writes nothing and fails when the goldens on disk differ from what the plugin makes now.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
FIXTURES = REPO / "fixtures"
# a name that needs escaping in LaTeX and sits inside both kinds of quotes
PAPER_NAME = "Walking breaks & 100% focus_time"


def run(*args: str, cwd: Path | None = None) -> str:
    done = subprocess.run(args, cwd=cwd, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        sys.exit(f"{' '.join(args)} failed:\n{done.stderr}")
    return done.stdout


def plugin_output(plugin: Path) -> dict[str, str]:
    """Golden file (relative to fixtures/goldens) → content."""
    out: dict[str, str] = {}
    ailog = str(plugin / "plugin" / "tools" / "ailog.py")
    for log in sorted((FIXTURES / "logs").glob("*.jsonl")):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "paper.json").write_text(json.dumps({"name": PAPER_NAME}), encoding="utf-8")
            (root / "ai-log").mkdir()
            (root / "ai-log" / "2026-10.jsonl").write_text(log.read_text(encoding="utf-8"), encoding="utf-8")
            for lang in ("en", "de"):
                for fmt in ("md", "tex"):
                    run(sys.executable, "-I", ailog, "disclosure", "--lang", lang, "--format", fmt, cwd=root)
                    out[f"{log.stem}/ai-register.{lang}.{fmt}"] = (root / "disclosure" / f"ai-register.{lang}.{fmt}").read_text(encoding="utf-8")
                    for answers in (False, True):
                        run(sys.executable, "-I", ailog, "prompts", "--lang", lang, "--format", fmt, *(["--answers"] if answers else []), cwd=root)
                        name = f"ai-prompts{'-answers' if answers else ''}.{lang}.{fmt}"
                        out[f"{log.stem}/{name}"] = (root / "disclosure" / f"ai-prompts.{lang}.{fmt}").read_text(encoding="utf-8")
    out["core.json"] = run("node", str(REPO / "scripts" / "core-goldens.mjs"), str(plugin), str(FIXTURES))
    # the hooks module is TSX written for Claude Code: compiled to plain JavaScript, then run with a stand-in host
    with tempfile.TemporaryDirectory() as tmp:
        tsc = REPO / "ui" / "node_modules" / ".bin" / "tsc"
        if not tsc.exists():
            sys.exit("TypeScript is missing: run `npm ci` in ui/ first.")
        options = ["--jsx", "react", "--jsxFactory", "h", "--jsxFragmentFactory", "h", "--module", "esnext", "--target", "es2022", "--moduleResolution", "bundler", "--noCheck", "--skipLibCheck"]
        subprocess.run([str(tsc), str(plugin / "plugin" / "hooks" / "register.tsx"), "--outDir", tmp, *options], capture_output=True, text=True, check=False)
        if not (Path(tmp) / "register.js").exists():
            sys.exit("The plugin's hooks module did not compile.")
        out["scope.json"] = run("node", str(REPO / "scripts" / "scope-goldens.mjs"), tmp, str(FIXTURES))
    out["PLUGIN_COMMIT"] = run("git", "rev-parse", "HEAD", cwd=plugin)
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--plugin", type=Path, default=REPO.parent / "Claude_CoWriting", help="a checkout of the CoWriting plugin")
    parser.add_argument("--check", action="store_true", help="compare instead of writing")
    args = parser.parse_args()
    goldens = FIXTURES / "goldens"
    made = plugin_output(args.plugin.resolve())
    if args.check:
        made.pop("PLUGIN_COMMIT")  # a newer plugin may still make the same goldens
        differing = [name for name, text in made.items() if not (goldens / name).exists() or (goldens / name).read_text(encoding="utf-8") != text]
        if differing:
            sys.exit("The plugin no longer makes these goldens:\n  " + "\n  ".join(differing))
        print(f"{len(made)} goldens match the plugin at {args.plugin}.")
        return
    for name, text in made.items():
        (goldens / name).parent.mkdir(parents=True, exist_ok=True)
        (goldens / name).write_text(text, encoding="utf-8")
    print(f"Wrote {len(made)} goldens from the plugin at {made['PLUGIN_COMMIT'].strip()}.")


if __name__ == "__main__":
    main()
