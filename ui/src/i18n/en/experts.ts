/** An expert that comes with the editor, in a language's own words. */
type OwnWords = { name: string; role: string }

export const experts = {
  // The experts that come with the editor, by id, where a language has its own words for them.
  // English has none: their names and fields come from the app. The log and the disclosure always
  // carry the names the app has, so a log reads the same whatever the window was set to.
  builtIn: {} as Partial<Record<string, OwnWords>>,
  aboutDocument: 'About this document',
  aboutDocumentHint: 'What every expert should know: the field, the journal, the kind of text.',
  briefingForAll: 'Briefing for every expert',
  briefingForAllExample: 'A field study for a psychology journal, British spelling…',
  save: 'Save',
  notSaved: 'not saved',
  cannotAskNow: 'Set up the AI first, or wait for the answer in progress',
  askForReview: 'Ask for a review',
  brief: 'Brief',
  briefed: 'Briefed',
  isInParty: (name: string) => `${name} is in the party`,
  inParty: 'in the party',
  briefingFor: (name: string) => `Briefing for ${name}`,
  briefingForHint: (name: string) => `What ${name} should look for in this document, or leave alone`,
  newExpert: 'New expert…',
  newExpertHeading: 'New expert for this document',
  name: 'Name',
  nameExample: 'Name, for example The Health Economist',
  field: 'Field',
  fieldExample: 'Field, for example costs, QALYs, perspective of the analysis',
  looksAt: 'What they look at',
  looksAtExample: 'What this expert looks at and asks: the questions of the field, what counts as evidence there, the mistakes outsiders make.',
  sameRules: 'The expert works under the same rules as the others: hints and questions, never text for your manuscript.',
  add: 'Add',
  cancel: 'Cancel',
  // background files
  files: 'Background files',
  filesHint: 'Files of yours the AI should know: wording rules, a glossary, the methods, a style sheet. Text files, Word files and PDFs with text in them. They are sent along with every question to those they are for, and the log names them.',
  linkFile: 'Link a file…',
  noFiles: 'None linked.',
  fileFor: (name: string) => `Who gets ${name}`,
  forEveryone: 'For everyone',
  forThese: (names: string) => `For ${names}`,
  forEveryoneHint: 'Nobody ticked: everyone gets it, the assistant included.',
  fileMissing: 'cannot be read',
  fileConverted: 'its text, as the editor read it',
  fileCut: 'long: only its beginning is sent',
  unlink: 'Unlink',
  unlinkFile: (name: string) => `Unlink ${name}`,
  // what an expert is told
  prompt: 'Prompt',
  promptEdited: 'Prompt (yours)',
  promptFor: (name: string) => `What ${name} is told`,
  promptNote: 'The whole description of this expert, for this project. The assist-only rules are added to it and cannot be changed here.',
  backToStandard: 'Back to the standard',
}
