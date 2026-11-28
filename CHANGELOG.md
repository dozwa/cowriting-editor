# Changelog

What changed, for people who use the editor. Two things are kept apart here:

- **On `dev`, in no release** is what the branch `dev` holds beyond the last release. Nobody has
  it unless they build the editor themselves.
- **Releases** are the installers on the [releases page](https://github.com/dozwa/cowriting-editor/releases).
  A change is in a release only when it stands under that release's heading. The branch `main` is
  the state of the last one.

When `dev` is merged into `main`, a release is made, and everything in the first section moves
to a new heading with its version and date; the release workflow does that (see `DEVELOPING.md`). Numbers like #7 are issues on GitHub.

## On `dev`, in no release

### Added

- **Comment out.** A button in the markdown bar (Cmd or Ctrl with Shift and C) puts `<!-- -->`
  around what is selected, or around the line the cursor is in, and takes it off again. Text in a
  comment stays in the file and is in no PDF or Word export. The live review does not look at
  it, and a citation that is only in a comment does not count as cited; the AI you ask still
  reads the document as it is, comments included. (#6, #29)
- **Characters beside the words.** The top bar shows the number of characters with spaces, and
  the number without them when the pointer rests on it. Counted is what a reader reads: front
  matter, code, comments, citation keys, markdown's marks, the addresses of links, and pictures
  are left out; a run of spaces or line ends is one space. The words are counted as before, so
  a link's address is still a word there. (#8)
- **Choose a picture.** The Picture button opens a file dialog and writes the picture's path as
  it lies from the document, so the two can move together and the export finds it. For a
  picture outside the document's folder the editor asks: copy it into a folder `images` beside
  the document (the picture itself is never moved or changed), or name it where it is. Not yet
  looked at in a running window; a picture on another drive (Windows) is named by its full path. (#5)

- The lock file of the interface's packages fits a clean install again; adding the search package had dropped two entries, and every check on `dev` failed at `npm ci`.
- **Pictures by drop and from the clipboard.** A picture file dropped on the text is placed
  where it is dropped, with the same question as the button for a picture outside the
  document's folder. A picture pasted from the clipboard is saved into the pictures folder as
  `picture-<date>.png` and named in the text. The folder's name can be set in the dialog and is
  kept for the project. Dropping on the app's window was not yet tried in a running window. (#31)
- **The count of a selection.** While text is selected, the top bar shows its words and
  characters in place of the document's, counted by the same rules; the document's numbers are
  in the tooltip. With several selections at once, the main one is counted. (#30)
- **Type and layout of a PDF.** The export dialog has a part "Type and layout": the typeface of
  the text, of the headings and of code; the size (9 to 14 pt); margins, for all sides or side
  by side, also two-sided; line spacing; one or two columns; indented paragraphs; ragged right;
  numbered headings; and page numbers (none, at the bottom, at the top, none on the first page).
  It works with Typst and with an installed LaTeX. The choice is kept with the project, so every
  document has its own, and a new document starts with the one chosen last. What the document's
  front matter sets comes first. The export now also passes the document's language on, for
  hyphenation, to PDF and Word. A typeface the layout names and the computer does not have is
  named before the export and again after it, when Typst has set the text in another one.
  (#13, #34)
- **A preview of the export.** "Preview" in the export dialog makes the PDF or Word file with the
  layout as it is set at that moment, in a temporary file, and opens it. Nothing is written
  beside the document. (#34)
- **Kept layouts.** A paper and layout can be kept under a name, used again in any project,
  saved as a file to pass on, and taken in from such a file. (#34)
- **The layout of a Word file.** A Word export has the paper and what is set under "Type and
  layout" in its own styles, page setup, header and footer: typefaces, size, line spacing,
  margins, columns, indented paragraphs, numbered headings, page numbers. Or it takes all that
  from a Word template of yours, chosen in the export dialog and kept with the project. Before,
  a Word file always had pandoc's styles on US Letter. Checked by reading the files the export
  makes, not yet by opening them in Word itself. On Windows the first paragraph after a heading
  kept its indent; the styles are now read whatever line ends they have. (#34)
- **Hints can be ticked off.** Every list item and table row of an answer has a box to mark it
  as done, with "Mark all" and "Unmark all" for the whole answer and a count of what is left.
  The ticks are kept beside the log (`marks.json`) and survive closing the document; they are a
  working aid and are in neither the log nor the disclosure. (#9)
- **Find and replace.** Cmd or Ctrl with F opens a bar over the text that marks every match,
  says which of how many is selected, and replaces one or all; with the options upper and lower
  case, whole words and regular expressions. Replacing all is one step to undo, and a
  replacement is the author's own edit: it is not taken for pasted text. The panels are not
  searched. (#11)
- **"Move…" asks about the pictures.** When a document goes to another folder and names pictures
  by where they lie from it, the editor asks before it moves: copy the pictures in the
  document's folder along, or leave them and rewrite their ways in the text. Pictures outside
  the folder are always named from the new place. The pictures themselves stay where they are,
  and a copy never replaces a different file. (#32)
- **Projects survive a change of version.** Every file the editor keeps (the project's
  settings, the lists of experts and bibliography files, what it knows about a document, its
  own settings) says which form it has. An older form is brought up to date step by step, and a
  copy of the old file is kept in `backup/` beside it. A file that cannot be read is no longer
  replaced by the standard and written over: it is set aside as `<name>.broken-<date>` and the
  editor says so. A file a newer editor wrote is left as it is, and a document last worked on
  with a newer editor is not opened, so its record is not continued in a form it may have left
  behind. What a newer editor added to a file stays when an older one saves it. Updates from
  inside the app are not part of this. (#25)
- **Tabs of the side panel can be hidden.** Under "Text and look" in the settings, every tab but
  "Ask" can be taken out of the row: Experts, Live review, Plan, Sources, Log. What a hidden tab
  holds stays as it is, and the log is kept as before. Its shortcut, the tour and "Show in log"
  still open it, and it is in the row while it is open; Live review is in the row while it is on.
  A hidden Log tab does not show how many answers are still to classify. (#39)
- **Chats from other AI tools in the record.** A statement of AI use outside the editor can
  keep the chat itself: conversations chosen from a data export of ChatGPT or Claude.ai or from
  a transcript of Claude Code, a chat pasted as text with "Prompt:" and "Answer:" marked, or any
  other file. Only what you choose is kept, in the document's `.cowriting` folder under
  `external/`; tool calls, files and pictures inside a chat are left out and counted. The log
  names each file with its SHA-256, the disclosure names it with the statement, and a new
  appendix prints the kept chats in full, in Markdown or LaTeX, English or German. A file
  changed after it was kept is said to be changed and not printed. Text you paste or type that
  comes from a kept chat is found and listed like text from the editor's own AI. The readers
  for ChatGPT's and Claude.ai's exports were written from what is known of their form and not
  yet tried with a real export; Claude Code's was tried with real transcripts. Classifying such
  chats like the editor's own turns, and merging the plugin's log, are not part of this. (#4, open)

- **Projects of several files.** A thesis, a book or a long report can be a project of several
  markdown files. "Make a project of several files…" in the new **Project** tab writes a file
  `<name>.cowriting.json` beside the open document that lists the project's files in their
  order; the document becomes the first one. Experts and their wording, background files,
  bibliography files, live review and the project's own AI and languages are set once and hold
  in every file; each file keeps its own plan, check, AI log and notes. The tab shows every file
  with its words against its plan and all files together against a goal and a number of words
  for the whole, and there files are opened (what is open is saved first), added, put in order,
  renamed and taken out; a renamed file keeps its log and plan. "Open…" takes a project's file
  too. A single document stays as it is and becomes a project's first file without losing
  settings or log. Tried in the browser; the file dialogs of the app's own window were not.
  The disclosure and the export for a whole project, and the AI reading the other files, are
  not part of this: each file still has its own. The Project tab can be hidden like the others.
  On Windows a document now has one spelling of its path however it was opened, so a file's plan
  is the same from the file dialog and from the Project tab.
  (#26, open)

- **Updates from inside the app.** A released version asks the editor's own server at each start
  whether a newer version exists, with a request that carries nothing about you, the
  installation or the version you have. A newer version is shown once, with what changed, and
  afterwards in the status bar; "Check for updates…" in Settings under About asks at any time and
  says when the server cannot be reached, and the check at start can be switched off there.
  Nothing is fetched by itself: "Install and restart" saves the open document, fetches the new
  version from the project's releases on GitHub, installs it only with the signature of the
  editor's own key, and starts the editor again. A copy that cannot update itself points to the
  releases page. Switched off as built: there is no server address and no key for updates yet,
  so no version asks or updates itself. The check and its dialog were tried in the browser
  against a server on the same computer; fetching and installing was never run. The dev app
  does not check. (#38, #33, open)

- **Usage figures are sealed.** A message to the editor's maker is encrypted on your computer
  with a public key built into the editor, and filled up to one of a few fixed lengths, before
  it is sent: only the maker's server can read it, not a service that passes it on. What
  Settings shows as "what would be sent" is still the plain message. Usage figures remain
  switched off as built, and sent only with consent. (#28, open)

### Fixed

- **PDF export with Typst and pictures.** Every export with Typst failed as soon as the document
  held a picture. Run on macOS; not yet on Windows and Linux. On Windows a picture is still not
  found when the temporary folder is on another drive than the document. (#7)
- **Tables in AI answers.** A short column, such as line numbers, no longer breaks after every
  character. (#10)
- **Spelling marks on macOS.** With "Spelling check" on, misspelled words are underlined, and the
  system does not correct words by itself. Not yet looked at in a running window. (#12)
- **"Show in folder" on Windows** opens the folder and marks the file. Not yet run on Windows. (#18)
- **Claude Code that is too old.** The editor says so, with the version and how to update,
  instead of "unknown option '--tools'". (#23)
- **Disclosure**
  - Requests of a single day are given as a date, not as a range from that day to itself. (#17)
  - A model on your own computer is named "Ollama (local)" or "LM Studio (local)", not by its
    address. The editor's version and who served the models are said once, in the sentence
    before the table, and the table's column names the model only; when rows went through
    different providers, each row still names its own. (#16)
  - LaTeX: the table is as wide as the text block and no longer runs past the margin. (#19)
  - LaTeX: headings no longer run past the margin, and the German heading on changes to the AI
    log is shorter. (#21)
  - LaTeX: the files begin with a note on the fonts pdfLaTeX needs. Without `lmodern`, text
    copied from the PDF has other characters for quotation marks, ß and umlauts. (#20)

- **An export makes no folder for the AI's record.** With Zotero running, a PDF or Word export
  kept the references it took from Zotero in `.cowriting/<document>/export/references.json`, and
  made that folder for a document that had never been put to the AI. The references now go to a
  temporary file that is removed after the export. A file an earlier export left there is not
  used any more and can be deleted. (#35)

### Changed

- The MIT licence has its own file.
- The README points to the user wiki.
- The README says what the editor is for, and what it is not for or not yet. (#24)
- `tested-models.json` has its first five entries. Claude Sonnet, Claude Haiku and Claude Opus
  through Claude Code and `qwen3.8:27b` through Ollama kept to the rules in every case and are
  shown as tested; `mistral:7b` through Ollama did not and is shown as such. Every other model
  is still "untested". (#22, open; #37)

### For those who build it

- **A dev app and an alpha.** What is built by default is "CoWriting Editor Dev", version
  `0.1.1-dev`: it installs beside a released CoWriting Editor and has its own settings and keys.
  An alpha, `0.1.1-alpha`, keeps the name CoWriting Editor. The
  footer shows such a version as it is. See `RELEASING.md`.
- One script sets the version in every file, and CI checks that they agree.
- **Two branches.** Work is on `dev`; `main` is the state of the alpha and moves only when a
  tried state of `dev` is merged into it. `DEVELOPING.md` has the steps.
- **A merge into `main` makes the alpha.** The release workflow sets the version, moves these
  entries under the release's heading, tags, builds the installers into a draft pre-release, and
  sets `dev` to the next dev version.
- CI checks and tests every push and builds the dev app only when it is started by hand on `dev`.
- **A release can be built for updates.** With the public key for updates in
  `src-tauri/tauri.updater.conf.json` and its private half as the secret
  `TAURI_SIGNING_PRIVATE_KEY`, the release workflow signs every installer, puts the signatures
  and the macOS app archive into the draft, and writes `latest.json` from them
  (`scripts/latest-json.py`), the file the editor's server passes on. Without the key a release
  is built as before. Not run yet: there is no key. `RELEASING.md` has the steps. (#33)
- The Windows `.msi` and the Linux `.rpm`, which were never published, are no longer built.
- The release build leaves out signing secrets that are not set.
- The export test has a document with pictures, and the tests read the stylesheets.
- The behaviour suite no longer counts a model's refusal as text for the paper: in a paragraph,
  sentences that speak to the author or of the assistant ("I can't write this for you") are left
  out before its words are counted. A paragraph for the paper behind a polite sentence still
  fails. (#36)
- The behaviour suite lets the author overrule the AI: text the model writes because the author
  insists keeps to the rules when the mark `[AI-TEXT]` stands above it, and one mark covers what
  follows it in the answer. Text before the mark still fails, and in the case with pressure
  marked text must also be listed in the disclosure. (#37)

## Releases

### 0.1.0, alpha (2026-11-24)

The first alpha: a markdown editor with assist-only AI co-writing. The AI comments and asks, it
does not write your text, and every exchange is logged for the disclosure.

- Installers for macOS (Apple Silicon, signed and notarised), Windows (unsigned) and Linux
  (AppImage and deb), with pandoc and Typst for the export to PDF and Word inside.
- The Windows and Linux installers were built by CI and not tried on a real machine.
- No updates from inside the app.
