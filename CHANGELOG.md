# Changelog

What changed, for people who use the editor. Two things are kept apart here:

- **On `dev`, in no release** is what the branch `dev` holds beyond the last release. Nobody has
  it unless they build the editor themselves.
- **Releases** are the installers on the [releases page](https://github.com/dozwa/cowriting-editor/releases).
  A change is in a release only when it stands under that release's heading. The branch `main` is
  the state of the last one.

When a release is made, the entries it takes move from the first section to a new heading with
its version and date (see `DEVELOPING.md`). Numbers like #7 are issues on GitHub.

What stands under "Fixed" and "Changed" below was pushed to `main` before there was a `dev`. It
is in no release all the same.

## On `dev`, in no release

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
    address. It is still named in every row of the table. (#16, open)
  - LaTeX: the table is as wide as the text block and no longer runs past the margin. (#19)
  - LaTeX: headings no longer run past the margin, and the German heading on changes to the AI
    log is shorter. (#21)
  - LaTeX: the files begin with a note on the fonts pdfLaTeX needs. Without `lmodern`, text
    copied from the PDF has other characters for quotation marks, ß and umlauts. (#20)

### Changed

- The MIT licence has its own file.
- The README points to the user wiki.

### For those who build it

- **A dev app and an alpha.** What is built by default is "CoWriting Editor Dev", version
  `0.1.1-dev`: it installs beside a released CoWriting Editor and has its own settings and keys.
  An alpha is built by its tag, `v0.1.1-alpha`, as before under the name CoWriting Editor. The
  footer shows such a version as it is. See `RELEASING.md`.
- One script sets the version in every file, and CI checks that they agree.
- **Two branches.** Work is on `dev`; `main` is the state of the alpha and moves only when a
  tried state of `dev` is merged into it and tagged. `DEVELOPING.md` has the commands.
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
