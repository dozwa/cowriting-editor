// The tour of the editor: one card per part of the window, in the order an author meets them.
export const tour = {
  label: 'Tour of the editor',
  count: (now: number, of: number) => `Step ${now} of ${of}`,
  back: 'Back',
  next: 'Next',
  done: 'Done',
  skip: 'Skip the tour',
  needsDocument: 'Open a document to see this part of the window.',
  setUp: 'Set up the AI…',
  steps: {
    welcome: {
      title: 'You write, the AI reads',
      text: 'CoWriting Editor is a markdown editor with an AI that gives feedback on your text and never writes it for you. What you ask and what it answers is logged, so that you can state your AI use honestly at the end. This tour shows the parts in order and takes about two minutes.',
    },
    ai: {
      title: 'Choose the AI',
      text: 'The answers come from a model on your own computer, from a provider you have an API key for, or from Claude Code. You choose that in Settings and can change it at any time. Writing works without it.',
    },
    document: {
      title: 'Open a document',
      text: 'A document is one markdown file, anywhere on your computer. Open one, or start a new one. Its experts, linked files and plan belong to that file, and its log is kept in a hidden folder beside it.',
    },
    write: {
      title: 'Write',
      text: 'This is your text. The buttons above it write the markdown for you, and the text is styled while you type. The AI has no way to change what is here; only you do.',
    },
    ask: {
      title: 'Ask about your text',
      text: 'Ask a question in your own words, or use a ready-made request such as feedback, structure or language. The answer appears here, beside your text. Afterwards you say what you did with it, and that goes into the log.',
    },
    experts: {
      title: 'Experts',
      text: 'Experts are reviewers with a point of view, such as a strict second reviewer. Choose who is in your group, tell them what they should know about your work, and link files such as wording rules or method definitions as their background knowledge.',
    },
    live: {
      title: 'Live review',
      text: 'Once you turn it on, the experts you choose read along while you write and leave short hints when you pause. How often they speak up is set in Settings.',
    },
    plan: {
      title: 'Plan',
      text: 'Write down what the text has to cover. The editor shows how far you are, and can check your points against the text.',
    },
    sources: {
      title: 'Sources',
      text: 'Search your Zotero library and cite from it, add bibliography files, and check whether the references in your text can be found.',
    },
    log: {
      title: 'Log',
      text: 'Every question and answer is listed here, with what you did with it. Text from an answer that turns up in your document is noted as well. AI use outside the editor, in another chat for example, you can state here by hand.',
    },
    finish: {
      title: 'Export and disclose',
      text: 'Export writes a PDF or a Word file with your references. Disclosure writes the statement of your AI use from the log, ready to go with your work.',
    },
    around: {
      title: 'Around the edges',
      text: 'Settings holds the look, the languages and the shortcuts. Calm, in the footer, hides the explanations. If something goes wrong, the red button reports a bug. This tour is in Settings under About whenever you want it again.',
    },
  },
}
