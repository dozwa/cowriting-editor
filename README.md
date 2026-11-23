# CoWriting Editor

A desktop markdown editor with assist-only AI co-writing. You write; the AI gives feedback, asks
questions and explains. It has no way to change your text, everything it says is logged, and the
log becomes your disclosure of AI use.

It is the standalone sibling of the [CoWriting plugin for Claude Code](https://github.com/dozwa/claude-cowriting)
and writes the same log format.

**State: early.** The first slice works: open and save a markdown file, ask, classify, log,
disclosure, with the Claude API as the only backend. Not built yet: other providers, Claude Code as
a backend, experts, live review, scope, sources, and the detection of AI text pasted into the
manuscript. There are no signed installers yet.

## What it does with your files

For `thesis.md` the editor keeps a folder `.cowriting/thesis.md/` beside it: the log
(`ai-log/YYYY-MM.jsonl`), the document as it was at each question (`snapshots/`), the AI's notes
(`notes/`, the only place it can write) and the disclosure (`disclosure/`). Nothing is created
until you first ask the AI, and nothing is written into the manuscript.

## What is sent where

When you ask, your question and your document go to the provider you set up (Anthropic, or a
gateway you name). Nothing is sent otherwise. The API key is kept in the system's key store.

## What it does not check

- Text from other AI tools that you type or paste in. The disclosure covers what went through this editor.
- Whether a model keeps to the assist-only rules. No model can write to the manuscript and every
  answer is logged, whatever it says; an answer that contains manuscript text is flagged when the model marks it.
- Personal data in your text or prompts.

## Building

Needs Rust, Node 22.15 or newer, and on Linux the packages Tauri lists for WebKitGTK.

```
cd ui && npm install && cd ..
cargo test                              # core, backends, the flow on a real folder
(cd ui && npm test)                     # the window, with the app replaced by a stand-in
ui/node_modules/.bin/tauri dev          # run it
ui/node_modules/.bin/tauri build        # unsigned installer for this system
```

To try it without a model: `COWRITING_BACKEND=fake:/path/to/script.json COWRITING_OPEN=/path/to/file.md`
(see `crates/backends/src/fake.rs` for the script's form).

## Layout

- `crates/core`: log, guard, stats, disclosure, the turn loop. No window, no network.
- `crates/backends`: the Claude API and a scripted stand-in.
- `crates/app`: one open document, as the window uses it.
- `src-tauri`: the window's commands, settings, key store.
- `ui`: editor and panels (TypeScript, React, CodeMirror). No file or network access.
- `fixtures/goldens`: the plugin's own output over `fixtures/`, which the core must reproduce
  byte for byte. `scripts/make-goldens.py --plugin <plugin repo> [--check]` regenerates or checks them.
