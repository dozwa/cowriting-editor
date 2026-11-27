# CoWriting Editor

A desktop markdown editor with assist-only AI co-writing. You write; the AI gives feedback, asks
questions and explains. It has no way to change your text, everything it says is logged, and the
log becomes your disclosure of AI use.

It is the standalone sibling of the [CoWriting plugin for Claude Code](https://github.com/dozwa/claude-cowriting)
and writes the same log format.

## What it is for

Long texts that are yours and for which you must be able to say how AI was used:

- **A thesis, a paper, a report or a proposal**, written over weeks, where the institution or the
  journal expects a disclosure of AI use. The log and the disclosure made from it are what the
  editor is built around; the rest serves them.
- **Revising your own draft with critical readers.** The experts, the plan and the check of its
  points are there for the author to look at their own text from other sides before somebody
  else does. They comment; they do not rewrite.
- **Working with a model on your own computer** when the text may not leave it.

What it is not for, or not yet:

- **Quick notes and everyday writing.** Markdown, a log beside every document and a live review
  are heavy for a shopping list or a letter.
- **Having text written for you.** The AI declines, and what it formulates anyway is marked and
  ends up in the disclosure. That is the point of the editor, not a gap.
- **Reviewing or screening other people's manuscripts.** The experts read the document you are
  writing; there is no workflow for a reviewer or an editor of a journal.
- **Writing together.** One author, one document, one computer. Projects of several files (#26)
  and working from a shared folder (#1) are open questions, not features.
- **LaTeX documents** (#2). The editor reads and writes markdown; LaTeX is an export.

**State: early.** What works: open and save a markdown file, ask, classify, log, disclosure. The
AI can be Claude (Anthropic API key), a model on your own computer (Ollama, LM Studio), or any
provider with an OpenAI-compatible API. A party of experts (Reviewer #2, the Statistics Nerd, …)
can be briefed and asked, and you can add experts of your own. References can be searched and
checked, and the document exports to PDF and Word. The AI can also be your own installed Claude
Code. The macOS installer is signed and notarised; the Windows one is not signed yet, and the app
does not update itself (see `RELEASING.md`).

**How to use it:** the [user guide in the wiki](https://github.com/dozwa/cowriting-editor/wiki)
explains every feature with screenshots, and how to tailor the experts, files and settings.

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
Word file are set in Word.

**Reference style.** Without a choice, citations and the reference list are in pandoc's default
style (Chicago author-date). In the Sources tab you can choose a style file (`.csl`, as
zotero.org/styles has them) for this document; like a bibliography file it is read where it lies
and never changed. A style the document names itself (`csl: apa.csl` in the front matter) comes
first. Licences of the two programs: `src-tauri/tools/`.

## Writing markdown

Above the text is a row of buttons that write markdown for you: title and headings, bold, italic,
lists, quotation, link, picture, table, citation, footnote and comment. Each puts the marks around what you
selected, or writes an example to type over; a button's tooltip shows the markdown it writes. At
the bottom of the window you can hide the buttons, turn on **line numbers** (the AI's answers name
lines), and switch to **Source**, which shows the text exactly as it is saved, every mark in
place. The line between text and side panel can be dragged; a double-click puts it back.

**Find and replace** (Cmd or Ctrl with F) opens a bar over the text: every match is marked, the
bar says which of how many is selected, Enter and Shift+Enter go on and back (Cmd or Ctrl with G
does the same from the text), and Escape closes it. A word that is selected is what is looked
for. The options are upper and lower case, whole words, and regular expressions. "Replace all" is
one step to undo. A match in marks the preview hides, such as a link's address, shows them.

The **Picture** button lets you choose the file (PNG, JPEG, GIF, SVG, WebP or PDF) and writes its
path as it lies from your document. A picture that is not in the document's folder can be copied
into a folder `images` beside the document, so that both move together, or named where it is;
the picture itself is never moved or changed.

**Comment out** puts `<!-- -->` around what you selected, or around the line the cursor is in: a
sentence you are not sure about, a note to yourself. It stays in the file, is not counted as words
and is in no export. The live review passes over it, and a citation that stands only in a comment
is not counted as cited (the export would not have it either). When you ask the AI, it reads the
document as it is, comments included: you may be asking about just that passage, and the log
records what the AI was given.

The top bar counts the **words and characters** of your text (with spaces; the count without them
is shown when the pointer rests on it). Front matter, code, comments, citation keys and markdown's
marks are not counted, and of a link only its text is a character.

"How markdown works", at the end of the row, shows the marks worth knowing. Its two links open
commonmark.org/help and pandoc's manual in your browser. With zotero.org/styles (from the Sources
tab) these are the only web pages the editor ever opens, and their addresses are fixed in the app.

## Settings

"Settings…" (Cmd or Ctrl with the comma) has seven pages. Everything is kept in one file,
`settings.json` in the editor's configuration folder; the About page shows where that is.

- **The AI**: the provider and model, and the longest answer you allow, in tokens. An answer that
  was cut short by the limit is marked as such. Claude Code decides its answers' length itself.
- **Live review**: the pause before a review (5 to 600 seconds), the time between two reviews
  (15 seconds to an hour) and the most reviews in an hour (1 to 120).
- **Text and look**: light, dark or as the system; the typeface, size and line length of your text;
  the typeface of the interface and the size of its buttons and panels (small to larger; your text
  keeps its own size); spelling marks. You see a change while you choose it.
- **Files**: the folder "Open" and "New" start in, a folder for exports, saving by itself after a
  pause in typing, and opening the last document again at start.
- **Language**: four choices that need not agree. The editor's own language (English or German, or
  as the computer is set); the language you write in (for the spelling check, and the AI is told);
  the language the AI answers in (as you ask, or always English, or always German: a German
  interface, an English text and German feedback go together); the language of the disclosure.
  The experts that come with the editor have German names in a German window; the log and the
  disclosure keep their English ones. What the app reports about files and providers is English for now.
- **Shortcuts**: keys for saving, opening, the views, the tabs of the side panel (Cmd or Ctrl with
  1 to 6) and the markdown buttons, each of which you can change or remove. Copy, paste and undo cannot be reassigned.
- **About**: version, where the editor keeps what, what is sent to whom, licences, and feedback:
  a report you write is opened as a new GitHub issue in your browser, or copied, and you send it.
  You see every character first; the version, system and model are added only if you leave that
  ticked, and nothing of your document or log ever is.

`COWRITING_SETTINGS=/some/folder` makes the editor keep its settings there instead (for tests and trials).

## Projects

A project is one markdown file. Its AI log, the AI's notes and the snapshots stay in the hidden
`.cowriting` folder beside it, so the record of AI use travels with the text. Its settings are
kept in the editor's own folder (`projects/<file name>-<fingerprint>/` beside `settings.json`):
the experts and what you told them, the plan, the bibliography files, live review, and

- **Background files** (Experts tab): files of yours the AI should know, such as wording rules, a
  glossary or the methods. Each is for everyone or for the experts you tick; "Files" on an
  expert's card links a file for that expert alone and shows what it gets. Only the path is
  kept; the file is read when the AI is asked, sent along (at most 24,000 characters of a file),
  and named in the log. Text files as they are; of a Word file (.docx, .odt, .rtf) or a PDF the
  editor reads the text once, keeps that copy in the project's folder and reads the file again
  when it has changed. A PDF without text in it (a scan) cannot be linked.
- **An expert's prompt** (Experts tab, "Prompt"): the whole description of an expert can be
  reworded for the project, and put back. The assist-only rules are added to it and cannot be edited.
- **The AI, the languages and the pace of live review** for this document alone: tick "Only for …"
  on those pages of the settings. A document's own provider and model are kept with the project;
  a key is in the system's key store, one for each provider. Everything else in the settings is
  the same for all projects.

Settings an earlier version kept beside the document are moved over when it is opened. Moving or
renaming a document inside the editor takes its settings along. So does moving it outside the
editor together with its `.cowriting` folder (a whole folder renamed or moved, say): the log's
folder notes where its document was, and the settings follow when the document is opened at the
new place and is gone from the old one. A file moved without that folder starts with fresh
settings (a log left in the same folder is offered as before).

## AI use outside the editor

The editor sees only what happens in it. In the Log tab, "State AI use elsewhere…" takes what you
used another AI tool for: the tool and the purpose are needed; model, date, what it worked on,
what came of it and how you checked it are optional. A statement can be corrected or taken back;
the log keeps every line (entries of type `external`), and the disclosure lists what holds now in
a section of its own, marked as your own statement.

## The tour

At the first start a tour goes through the window in twelve short steps: the AI, opening a
document, writing, each tab of the side panel, export and disclosure. It is a card at the lower
left, not a dialog: the part it speaks about is marked, and everything stays usable on the way.
"Skip the tour" or Escape ends it. It does not start by itself again; "Take the tour" on the
first page and "Show the tour" in Settings under About bring it back.

## Alpha, bugs and support

The editor is an alpha, and the footer says which. "Report a bug" in the top bar opens a form in
three parts (what you did, what happened, what you expected) and opens it as a mail or a new
GitHub issue, or copies it, for you to send; nothing is sent by the editor.

So that a bug can be found, the editor keeps a technical log (`diagnostics.log` in the settings
folder, at most about 400 KB): which commands ran, how long they took, what failed, which tab or
dialog was shown, and the measures of a document (size, line ends, counts of headings, tables,
citations and the like). It never holds your text, your questions, the AI's answers, file names,
paths, addresses, mail addresses or keys: every message passes one filter that takes these out,
and anything in quotation marks with them. The log stays on your computer. In a bug report you
can tick "add the technical log of the last 10 minutes" and see every line before it goes, or save
a file to attach; your document's text goes into that file only if you tick that too. The log can
be turned off and deleted in Settings under About.

The editor can tell its maker that it is used, and only with your consent: a sign of use at each
start (a random number made for the installation, the version, the kind of system) and, agreed to
separately, a session's figures when the editor closes (minutes open, requests to an AI by kind of
provider and model, documents worked on, words added). It asks once, with nothing ticked; Settings
under About shows the exact messages, the privacy statement (`docs/PRIVACY.md`,
`docs/DATENSCHUTZ.md`) and lets you withdraw. As built now this is switched off altogether: there
is no server address (`SERVER` in `crates/app/src/usage.rs`) and the statement is a draft with
parts to fill in, and while either is so the editor neither asks nor sends.

Every tenth start it asks for a coffee, feedback or a star on
GitHub: twenty starts of quiet after a coffee or feedback, and never again for a star once you
followed that link. "Calm" in the status bar hides the explanations and hints.

## Ready-made requests

Under the question field of the Ask tab are six buttons: **Feedback** (what works, the three
biggest problems, one next step), **Socratic** (questions, one at a time), **Structure** (reverse
outline and argument map), **Language** (grammar and spelling with the corrected form and the
rule), **Explain** (a concept, for your own understanding) and **Consistency** (terms,
abbreviations, citation keys). What you typed narrows the request; with nothing typed the button
asks about the whole document. Each is logged under its purpose, so the disclosure lists language
corrections apart from feedback. They are the plugin's commands of the same names.

An answer often holds many hints at once. Every item of a list and every row of a table in an
answer has a box to **tick it off as done**, and "Mark all" does it for the whole answer, so you
see what is left while you work through it. The ticks stay when you close the document. They are
a working aid of yours: they say that you have dealt with a hint, not what you did with it, and
they are in neither the log nor the disclosure. What became of the answer as a whole is still
the choice under it (adopted, adapted, rejected, information only).

## Live review and plan

**Live review** is off until you turn it on in the Live review tab. While it is on, the paragraphs you
change are sent to the AI once you pause for 20 seconds, at most once a minute and 20 times an
hour (the settings change these numbers), and the experts you chose leave short hints with line numbers. Each review is logged like
any other AI input. What was in the document before you turned it on is not sent.

**Plan** is your plan in plain text (`scope.md` in the project's folder): a goal, targets, sections and
the points the text must make. The Plan tab shows how far the text is. "Check the points" sends
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
(`verify.json`), the hints you ticked off (`marks.json`) and the references an export took from Zotero (`export/references.json`). What you tell the
experts is in `party.md`, experts you made in `party/<id>.md`, both in the project's folder (see Projects): plain files you can read and edit. Nothing is created
until you first ask the AI, and nothing is written into the manuscript.

The one other folder the editor makes is `images/` beside the document, and only when you have a
picture copied there, or when "Move…" copies a document's pictures along (see below).

## Renaming and moving

"Move…" renames or moves the open document and takes its folder of logs and notes along. When the
document goes to another folder and names pictures by where they lie from it (`![…](images/plot.png)`),
the editor asks first. Either the pictures in the document's folder are copied to the same place
beside the new file, and the text stays as it is; or they stay, and the editor rewrites those ways
in your text so that they lead from the new folder to the pictures. That is the one change the
editor ever makes to your text, and only when you chose it for this move. The pictures themselves
are never moved, so another document that names them still finds them. Pictures given as HTML
(`<img>`) or by reference (`![…][ref]`) are not seen by this. If you
rename a file outside the editor, its log stays behind under the old name. When you then open the
file, the editor says so and asks whether this is that document; it never attaches a log on its
own. A log is offered only from the same folder, so a file moved to another folder outside the
editor has to be moved back, or its `.cowriting/<name>/` folder moved along by hand.

## What is sent where

When you ask, your question and your document go to the provider you set up. With a model on
your own computer nothing leaves it. A reference check sends each reference's DOI, or its title,
first author and year, to crossref.org, and nothing of your text. Zotero is asked on your own
computer only. Nothing is sent otherwise. An API key is kept in the system's
key store. The disclosure names the provider and the model that answered.

## Claude Code as the AI

If Claude Code is installed and signed in on your computer, the editor can ask it instead of an
API. For each question the editor starts your own `claude` program and nothing else of yours:

- every tool Claude Code has (reading and writing files, the shell, the web) is switched off;
  it gets only the editor's tools, which cannot write your document. If Claude Code reports any
  other tool, the editor stops the call before anything is answered;
- your Claude Code settings, memory, plugins, skills and other MCP servers are not loaded, and the
  conversation is not kept by Claude Code;
- the editor never reads or stores Claude Code's sign-in.

Claude Code adds a short note of its own to each call (the system, a temporary folder). Questions
and document go to Anthropic under your Claude Code sign-in.

**Before you rely on this:** Anthropic's terms do not clearly say whether another application may
start your Claude Code under a Pro or Max subscription. Their legal page allows signing in to the
unmodified Claude Code with your own subscription and forbids applications from handling Claude
sign-ins or routing requests through subscriptions for their users. This editor does neither with
your credentials, but the question is not settled; with an Anthropic API key there is no doubt.

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
ui/node_modules/.bin/tauri dev          # run it: the dev app, with settings of its own
ui/node_modules/.bin/tauri build        # unsigned installer of the dev app for this system
```

What you build is "CoWriting Editor Dev": it installs beside a released CoWriting Editor and keeps
its own settings and keys. Work is on the branch `dev`; `main` is the state of the alpha that is out. How the two
are steered is in `DEVELOPING.md`, signing and the release workflow in `RELEASING.md`, and what changed since the
last release in `CHANGELOG.md`.

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
- `crates/backends`: the Claude API, OpenAI-compatible APIs, Claude Code, a scripted stand-in, and the lookups
  at Crossref and in Zotero.
- `crates/app`: one open document, as the window uses it; the export through pandoc.
- `src-tauri`: the window's commands, settings, key store.
- `ui`: editor and panels (TypeScript, React, CodeMirror). No file or network access.
- `suites`: the behaviour rules and the runner that asks real models.
- `fixtures/goldens`: the plugin's own output over `fixtures/`, which the core must reproduce
  byte for byte. `scripts/make-goldens.py --plugin <plugin repo> [--check]` regenerates or checks them.

## Licence

CoWriting Editor is free software under the MIT licence: see [LICENSE](LICENSE).

pandoc and Typst, which the installer brings along, keep their own licences: `src-tauri/tools/`.
