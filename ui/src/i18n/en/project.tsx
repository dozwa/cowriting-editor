import type { ReactNode } from 'react'

export const project = {
  // a document that is a project of its own
  alone: 'This document is a project of its own. A longer work can be a project of several files, one for each chapter.',
  aloneHow: 'A thesis, a book or a long report, for example. Each file then has its own plan and log, and the experts, linked files, sources and AI settings are set once for all of them. A project is one small file beside the chapters that lists them in their order. This document becomes its first file, and its experts and settings become the project’s. Its plan, its log and the text itself stay as they are.',
  make: 'Make a project of several files…',

  // the whole
  of: (name: ReactNode) => <>Project {name}</>,
  total: (words: number, target: number | undefined) => `${words.toLocaleString('en')}${target !== undefined ? ` of ${target.toLocaleString('en')}` : ''} ${words === 1 && target === undefined ? 'word' : 'words'} in all files`,
  chars: (n: number) => `${n.toLocaleString('en')} characters`,
  totalWords: 'Words in all files',
  noGoal: 'No goal for the whole project yet.',
  editGoal: 'Goal and words of the whole',
  goal: 'What the whole is to say or reach',
  wordsTarget: 'Words in all files together',
  saveGoal: 'Save the goal',
  cancel: 'Cancel',
  targetsSum: (sum: number) => `The files’ own plans add up to ${sum.toLocaleString('en')} words.`,

  // the files
  files: 'Files',
  filesHint: 'In the order of the text. A click opens a file; what is open is saved first. The AI you ask can read all of them.',
  filesHintMore: 'Every file has its own plan and log; experts, linked files, sources and the AI are the same in all of them.',
  open: (name: string) => `Open ${name}`,
  isOpen: 'open',
  missing: 'not found',
  wordsOf: (words: number, target: number | undefined) => `${words.toLocaleString('en')}${target !== undefined ? ` of ${target.toLocaleString('en')}` : ''} ${words === 1 && target === undefined ? 'word' : 'words'}`,
  points: (made: number, all: number) => `${made} of ${all} ${all === 1 ? 'point' : 'points'}`,
  ofPlan: (share: string) => `${share} of its plan`,
  noPlan: 'no plan',
  up: (name: string) => `Move ${name} up`,
  down: (name: string) => `Move ${name} down`,
  rename: 'Rename',
  renameTitle: (name: string) => `Rename ${name}; its log and plan go with it`,
  newName: (name: string) => `New name for ${name}`,
  remove: 'Take out',
  removeTitle: (name: string) => `Take ${name} out of the project. The file, its log and its plan stay where they are.`,
  add: 'Add a file…',
  addNew: 'New file…',
}
