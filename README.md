# CoWriting Editor

A desktop markdown editor with assist-only AI co-writing. You write; the AI gives feedback, asks
questions and explains. It has no way to change your text, everything it says is logged, and the
log becomes your disclosure of AI use.

It is the standalone sibling of the [CoWriting plugin for Claude Code](https://github.com/dozwa/claude-cowriting)
and writes the same log format.

**State: early.** What works: open and save a markdown file, ask, classify, log, disclosure. The
AI can be Claude (Anthropic API key), a model on your own computer (Ollama, LM Studio), or any
provider with an OpenAI-compatible API. A party of experts (Reviewer #2, the Statistics Nerd, …)
can be briefed and asked, and you can add experts of your own. References can be searched and
checked, and the document exports to PDF and Word. Not built yet: Claude Code as a backend. There
are no signed installers yet.

## Sources

Your bibliography is whatever your document's front matter names (`bibliography: refs.bib`, as
pandoc reads it) plus any file you add in the Sources tab: BibTeX, CSL JSON, RIS or a PubMed
export. The editor reads these files and never changes them.

- **Find a reference** searches your Zotero library when Zotero is running (through Better
  BibTeX, which knows the citekeys), otherwise your bibliography files. "Cite" puts `[@key]` where
  your cursor is.
- **Check the references** looks up every DOI at Crossref and compares title and year. A reference
  without a DOI is looked up by title; a DOI found that way is a suggestion for you to check, never
  a verification. No AI is involved.
- The Sources tab names keys your text cites that no bibliography file holds.
- The AI can search your bibliography files (`search_bibliography`). It cannot search Zotero or the
  web, and it cannot change a reference.

## Export to PDF and Word

"Export…" makes a PDF or a Word file of the document as it is in the editor, with
[pandoc](https://pandoc.org). The references behind your citekeys come from your Zotero library
(Better BibTeX) when it is running, then from your bibliography files. Keys found nowhere are named
after the export and stay in the text as they are. Your document is not changed.

The installer brings pandoc 3.14 and Typst 0.15.1 along, so both exports work without installing
anything (this adds about 60 MB to the download and 230 MB on disk). A PDF is typeset by Typst, on
A4 or US Letter. If a LaTeX is installed on your computer (XeLaTeX, LuaLaTeX, pdfLaTeX, Tectonic),
the export dialog offers it as well and remembers your choice. Page size, fonts and styles of a
Word file are set in Word. The reference list is in pandoc's default style (Chicago author-date);
choosing a style is not built. Licences of the two programs: `src-tauri/tools/`.

## Live review and scope

**Live review** is off until you turn it on in the Live tab. While it is on, the paragraphs you
change are sent to the AI once you pause for 20 seconds, at most once a minute and 20 times an
hour, and the experts you chose leave short hints with line numbers. Each review is logged like
any other AI input. What was in the document before you turned it on is not sent.

**Scope** is your plan in plain text (`scope.md` beside the log): a goal, targets, sections and
the points the text must make. The Scope tab shows how far the text is. "Check the points" sends
the points and the document to the AI and asks which points the text makes; that is logged too.
The AI cannot change the plan.

## AI text in your document

The editor compares what you paste, drop or type into the document with the answers in this
document's log. A passage that matches one (eight words or more in a row, or a paragraph that
is mostly an answer's wording) is logged and listed in the disclosure, with the question it came
from and whether it is still in the text. Nothing is blocked and nothing is marked in your text.
Entries are never removed; you can add a comment. Your own words that an answer quoted back,
code, references, links and citation keys are not compared. Text from other AI tools cannot be
detected, and the disclosure says so.

## What it does with your files

For `thesis.md` the editor keeps a folder `.cowriting/thesis.md/` beside it: the log
(`ai-log/YYYY-MM.jsonl`), the document as it was at each question (`snapshots/`), the AI's notes
(`notes/`, the only place it can write), the disclosure (`disclosure/`), the last reference check
(`verify.json`) and the references an export took from Zotero (`export/references.json`). What you tell the
experts is in `party.md`, experts you made in `party/<id>.md`: plain files you can read and edit. Nothing is created
until you first ask the AI, and nothing is written into the manuscript.

## What is sent where

When you ask, your question and your document go to the provider you set up. With a model on
your own computer nothing leaves it. A reference check sends each reference's DOI, or its title,
first author and year, to crossref.org, and nothing of your text. Zotero is asked on your own
computer only. Nothing is sent otherwise. An API key is kept in the system's
key store. The disclosure names the provider and the model that answered.

## Which models keep to the rules

No model can write to your text, and every answer is logged: that holds for any model. Whether a
model also *behaves* (declines to ghostwrite, marks AI text, invents no references) is tested per
model by `suites/behaviour`. Results ship in `tested-models.json`; **it is empty so far, so every
model is shown as "untested"**. A model that cannot call tools is sent the document with each
question and cannot write notes; the editor says so.

```
COWRITING_API_KEY=… cargo run -p cowriting-suites --bin behaviour -- \
    --backend openai-compatible --base-url http://localhost:11434/v1 --model qwen3.5:9b [--record]
```

## What it does not check

- Text from other AI tools that you type or paste in. The disclosure covers what went through this editor.
- Whether a model keeps to the assist-only rules. No model can write to the manuscript and every
  answer is logged, whatever it says; an answer that contains manuscript text is flagged when the model marks it.
- Personal data in your text or prompts.

## Building

Needs Rust, Node 22.15 or newer, and on Linux the packages Tauri lists for WebKitGTK.

```
cd ui && npm install && cd ..
python3 scripts/fetch-tools.py           # pandoc and Typst for this system, checked against pinned sums
cargo test                              # core, backends, the flow on a real folder
(cd ui && npm test)                     # the window, with the app replaced by a stand-in
ui/node_modules/.bin/tauri dev          # run it
ui/node_modules/.bin/tauri build        # unsigned installer for this system
```

To run the interface in an ordinary browser against the real Rust code (for development and for
browser automation; localhost only, one document, never shipped):

```
cargo run -p cowriting-suites --bin bridge -- --document /path/to/file.md \
    --backend openai-compatible --base-url http://localhost:11434/v1 --model gemma3:4b
(cd ui && npm run dev)                  # then open http://localhost:1420
```

To try it without a model: `COWRITING_BACKEND=fake:/path/to/script.json COWRITING_OPEN=/path/to/file.md`
(see `crates/backends/src/fake.rs` for the script's form).

## Layout

- `crates/core`: log, guard, stats, disclosure, the turn loop. No window, no network.
- `crates/backends`: the Claude API, OpenAI-compatible APIs, a scripted stand-in, and the lookups
  at Crossref and in Zotero.
- `crates/app`: one open document, as the window uses it; the export through pandoc.
- `src-tauri`: the window's commands, settings, key store.
- `ui`: editor and panels (TypeScript, React, CodeMirror). No file or network access.
- `suites`: the behaviour rules and the runner that asks real models.
- `fixtures/goldens`: the plugin's own output over `fixtures/`, which the core must reproduce
  byte for byte. `scripts/make-goldens.py --plugin <plugin repo> [--check]` regenerates or checks them.
