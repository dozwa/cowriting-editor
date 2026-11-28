# Backlog

Asked for, not built, in no order.

- **German, what is left.** The window is in German, and so are the experts that come with the
  editor. What the app itself reports is still English: error messages about files, providers and
  the export. The log and the disclosure keep the experts' English names, so a log reads the same
  whatever the window was set to. The menu at the top of the screen (File, Edit, View) is the
  system's standard one and English as well.
- **More languages.** A language is a folder in `ui/src/i18n` and one line in its `index.ts`; the
  writing language, the AI's language and the disclosure know English and German only.
- **Projects, what is left.** A document moved outside the editor without its `.cowriting` folder
  loses its log and its project settings; the editor cannot tell it from a new file. A scanned PDF
  cannot be linked (no text recognition). The look and the shortcuts are the same for all projects.
  A project of several files has no disclosure and no export for the whole yet, the AI does not
  read its other files, and one file cannot set a shared setting differently for itself (#26).
  Its shared settings are found by the id in the project's file, so two copies of that file share them.
