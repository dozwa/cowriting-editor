import type { ReactNode } from 'react'

export const app = {
  // the welcome screen
  welcomeTitle: 'You write.',
  welcomeMargin: 'The AI reads along and answers here, in the margin: feedback, questions, explanations. It cannot change your text.',
  welcomePromise: 'Everything it says is logged, and the log becomes your disclosure of AI use.',
  openFile: 'Open a markdown file…',
  newDocument: 'New document…',
  notSetUp: (link: ReactNode) => <>The AI is not set up yet. {link}</>,
  setItUp: 'Set it up',
  takeTour: 'Take the tour',

  // the top bar
  open: 'Open…',
  new: 'New…',
  save: 'Save',
  move: 'Move…',
  moveTitle: 'Rename or move this document. Its AI log goes with it.',
  saved: 'saved',
  notSaved: 'not saved',
  words: (n: number) => `${n.toLocaleString('en')} ${n === 1 ? 'word' : 'words'}`,
  chars: (n: number) => `${n.toLocaleString('en')} ${n === 1 ? 'character' : 'characters'}`,
  charsDetail: (withSpaces: number, withoutSpaces: number) =>
    `${withSpaces.toLocaleString('en')} characters with spaces, ${withoutSpaces.toLocaleString('en')} without. Not counted: front matter, code, comments, citation keys, markdown's marks, the addresses of links, and pictures.`,
  export: 'Export…',
  disclosure: 'Disclosure…',
  nothingToDisclose: 'Nothing to disclose yet: the AI has not been asked',
  settings: 'Settings…',

  // notices
  orphan: (name: ReactNode, questions: number, isSameText: boolean, document: string) => (
    <>
      This folder holds an AI log for {name} ({questions} {questions === 1 ? 'question' : 'questions'}), and that file is gone.{' '}
      {isSameText ? 'Its last saved text is exactly this document.' : 'Its last saved text differs from this document.'} Is {document} that document, renamed?
    </>
  ),
  storedSetAside: (file: string, keptAs: string) =>
    `${file} could not be read. It was not written over: it is kept as ${keptAs} in the same folder, and the editor goes on with its standard for it.`,
  storedNewer: (file: string) => `A newer version of the editor wrote ${file}. This version uses what it understands of it and does not change it; what it holds cannot be changed here until the editor is updated.`,
  continueLog: 'Yes, continue its log',
  newDocumentHere: 'No, this is a new document',
  found: (words: number, isPasted: boolean, isCorrection: boolean) =>
    `Logged: ${words} words ${isPasted ? 'you pasted' : 'in your document'} match ${isCorrection ? 'a language correction from an AI answer' : 'text from an AI answer'}. Nothing was changed; the passage will be listed in your disclosure.`,
  showInLog: 'Show in the log',
  dismiss: 'Dismiss',
  withoutTools: 'This model cannot use tools: your document is sent along with each question, and it cannot write notes.',

  // the side panel
  panelWidth: 'Width of the side panel',
  panelWidthTitle: 'Drag to make the side panel wider or narrower. Double-click for the standard width.',
  tabAsk: 'Ask',
  tabAskTitle: 'What you ask the AI about your text, and what it answers',
  tabExperts: 'Experts',
  tabExpertsTitle: 'Reviewers with a point of view, and what you tell them about this document',
  tabLive: 'Live review',
  tabLiveTitle: 'Short hints from the experts while you write. Off until you turn it on',
  newHints: (n: number) => `${n} ${n === 1 ? 'new hint' : 'new hints'}`,
  tabPlan: 'Plan',
  tabPlanTitle: 'Your plan for this text, and how far it is',
  tabSources: 'Sources',
  tabSourcesTitle: 'Your references: find, cite and check them',
  tabLog: 'Log',
  tabLogTitle: 'Everything the AI was asked and answered, and what you did with it',
  toClassify: (n: number) => `${n} ${n === 1 ? 'answer' : 'answers'} to classify`,

  // asking
  assistant: 'Assistant',
  reviewRequest: 'Please review my document.',
  toolSteps: {
    read_document: 'reading your document',
    search_document: 'searching your document',
    list_notes: 'looking at its notes',
    search_bibliography: 'searching your references',
    read_note: 'reading a note',
    write_note: 'writing a note',
  },
  askingFor: (tool: string) => `asking for “${tool}”`,
  refusedWrite: (where: string) => `tried to write where it may not (${where}): refused and logged`,

  // live review, as it goes. `every` is the gap in words, `ms` the same in milliseconds.
  reviewing: 'reviewing…',
  writing: 'you are writing…',
  waiting: (every: string, _ms: number) => `waiting: at most one review every ${every}`,
  hintsAt: (n: number, time: string) => `${n} ${n === 1 ? 'hint' : 'hints'} at ${time}`,
  nothingToAdd: (time: string) => `looked at ${time}: nothing to add, keep writing`,
  paused: (perHour: number) => `paused: ${perHour} reviews in the last hour`,
  reviewFailed: 'the last live review failed; it will try again after your next change',

  // the plan's check
  pointsFound: (covered: number, partial: number, missing: number) => `${covered} covered, ${partial} partly, ${missing} missing`,

  // the status bar
  scripted: 'Scripted answers (no model)',
  failedModel: 'failed the rule test',
  untestedModel: 'untested model',
  looks: 'How the text is shown',
  markdownButtons: 'Markdown buttons',
  markdownButtonsTitle: 'Buttons above the text that write markdown for you',
  lineNumbers: 'Line numbers',
  lineNumbersTitle: "Number the lines, as the AI's answers name them",
  source: 'Source',
  sourceTitle: 'Show the text as the markdown it is saved as, with every mark',
  logged: (inputs: number, open: number, refused: number) =>
    `${inputs} ${inputs === 1 ? 'AI input' : 'AI inputs'} logged${open > 0 ? ` · ${open} to classify` : ''}${refused > 0 ? ` · ${refused} ${refused === 1 ? 'refused write' : 'refused writes'}` : ''}`,
  // the bug button and the version in the footer
  reportBug: 'Report a bug',
  reportBugTitle: 'Something went wrong? Tell it here: a short form, sent by you as a mail',
  alpha: (version: string) => `alpha ${version}`,
  alphaTitle: 'An early version: things are missing and can break. Reports help.',
  calm: 'Calm',
  calmTitle: 'Hide the explanations and hints around the controls',
}
