# Backlog

Asked for, not built, in no order.

- **German, what is left.** The window is in German, and so are the experts that come with the
  editor. What the app itself reports is still English: error messages about files, providers and
  the export. The log and the disclosure keep the experts' English names, so a log reads the same
  whatever the window was set to. The menu at the top of the screen (File, Edit, View) is the
  system's standard one and English as well.
- **Updates, what is left.** A release checks for a newer version at a file on GitHub, but the
  key for updates is not set, so no version updates itself yet, and fetching and installing was
  never run; neither was the workflow that writes that file (#33, #38). "Check for updates…" is in Settings under About and not in the menu at the top
  of the screen. The dialog shows no progress of the download, and a version cannot be skipped:
  it stays in the status bar until it is installed.
- **More languages.** A language is a folder in `ui/src/i18n` and one line in its `index.ts`; the
  writing language, the AI's language and the disclosure know English and German only.
- **Projects, what is left.** A document moved outside the editor without its `.cowriting` folder
  loses its log and its project settings; the editor cannot tell it from a new file. A scanned PDF
  cannot be linked (no text recognition). The look and the shortcuts are the same for all projects.
  In a project of several files one file cannot set a shared setting differently for itself; an export of the whole takes the first file's front
  matter and does not shift the levels of headings (#26).
  Its shared settings are found by the id in the project's file, so two copies of that file share them.
