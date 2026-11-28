import type { ReactNode } from 'react'

export const project = {
  // a document that is a project of its own
  alone: 'Dieses Dokument ist ein Projekt für sich. Eine Abschlussarbeit, ein Buch oder ein langer Bericht kann stattdessen ein Projekt aus mehreren Markdown-Dateien sein: jedes Kapitel in einer eigenen Datei mit eigenem Plan und Protokoll, und Experten, verknüpfte Dateien, Quellen und KI-Einstellungen einmal für alle.',
  aloneHow: 'Ein Projekt ist eine kleine Datei neben den Kapiteln, die sie in ihrer Reihenfolge nennt. Dieses Dokument wird seine erste Datei, und seine Experten und Einstellungen werden die des Projekts. Sein Plan, sein Protokoll und der Text selbst bleiben, wie sie sind.',
  make: 'Projekt aus mehreren Dateien anlegen…',

  // the whole
  of: (name: ReactNode) => <>Projekt {name}</>,
  total: (words: number, target: number | undefined) => `${words.toLocaleString('de')}${target !== undefined ? ` von ${target.toLocaleString('de')}` : ''} ${target !== undefined ? 'Wörtern' : words === 1 ? 'Wort' : 'Wörter'} in allen Dateien`,
  chars: (n: number) => `${n.toLocaleString('de')} Zeichen`,
  totalWords: 'Wörter in allen Dateien',
  noGoal: 'Noch kein Ziel für das ganze Projekt.',
  editGoal: 'Ziel und Umfang des Ganzen',
  goal: 'Was das Ganze sagen oder erreichen soll',
  wordsTarget: 'Wörter in allen Dateien zusammen',
  saveGoal: 'Ziel speichern',
  cancel: 'Abbrechen',
  targetsSum: (sum: number) => `Die Pläne der Dateien ergeben zusammen ${sum.toLocaleString('de')} Wörter.`,

  // the files
  files: 'Dateien',
  filesHint: 'In der Reihenfolge des Textes. Ein Klick öffnet eine Datei; was offen ist, wird vorher gespeichert. Jede Datei hat ihren eigenen Plan und ihr eigenes Protokoll; Experten, verknüpfte Dateien, Quellen und die KI sind in allen dieselben.',
  open: (name: string) => `${name} öffnen`,
  isOpen: 'offen',
  missing: 'nicht gefunden',
  wordsOf: (words: number, target: number | undefined) => `${words.toLocaleString('de')}${target !== undefined ? ` von ${target.toLocaleString('de')}` : ''} ${target !== undefined ? 'Wörtern' : words === 1 ? 'Wort' : 'Wörter'}`,
  points: (made: number, all: number) => `${made} von ${all} ${all === 1 ? 'Kernaussage' : 'Kernaussagen'}`,
  ofPlan: (share: string) => `${share} des Plans`,
  noPlan: 'kein Plan',
  up: (name: string) => `${name} nach oben`,
  down: (name: string) => `${name} nach unten`,
  rename: 'Umbenennen',
  renameTitle: (name: string) => `${name} umbenennen; Protokoll und Plan gehen mit`,
  newName: (name: string) => `Neuer Name für ${name}`,
  remove: 'Herausnehmen',
  removeTitle: (name: string) => `${name} aus dem Projekt nehmen. Die Datei, ihr Protokoll und ihr Plan bleiben, wo sie sind.`,
  add: 'Datei hinzufügen…',
  addNew: 'Neue Datei…',
}
