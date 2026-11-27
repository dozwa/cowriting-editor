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

- **The count of a selection.** While text is selected, the top bar shows its words and
  characters in place of the document's, counted by the same rules; the document's numbers are
  in the tooltip. With several selections at once, the main one is counted. (#30)
- **Type and layout of a PDF.** The export dialog has a part "Type and layout": typeface of the
  text, size, margins, line spacing, indented paragraphs, ragged right, numbered headings. It
  works with Typst and with an installed LaTeX and is remembered for the next export. What the
  document's front matter sets comes first. The export now also passes the document's language
  on, for hyphenation, to PDF and Word. Not yet: typefaces for headings and code, page numbers,
  columns, named presets, and the layout of a Word file. (#13)
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

### Changed

- The MIT licence has its own file.
- The README points to the user wiki.
- The README says what the editor is for, and what it is not for or not yet. (#24)

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
- The Windows `.msi` and the Linux `.rpm`, which were never published, are no longer built.
- The release build leaves out signing secrets that are not set.
- The export test has a document with pictures, and the tests read the stylesheets.

## Releases

### 0.1.0, alpha (2026-11-24)

The first alpha: a markdown editor with assist-only AI co-writing. The AI comments and asks, it
does not write your text, and every exchange is logged for the disclosure.

- Installers for macOS (Apple Silicon, signed and notarised), Windows (unsigned) and Linux
  (AppImage and deb), with pandoc and Typst for the export to PDF and Word inside.
- The Windows and Linux installers were built by CI and not tried on a real machine.
- No updates from inside the app.
