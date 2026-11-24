import type { ReactNode } from 'react'
import type { plan as pattern } from '../en/plan'

export const plan: typeof pattern = {
  // reads after "alle": one minute is said in seconds, since "alle Minute" is not German
  span: ms => (ms !== 60_000 && ms % 60_000 === 0 ? `${ms / 60_000} Minuten` : `${Math.round(ms / 1000)} Sekunden`),
  line: n => `Zeile ${n}`,

  liveReview: 'Live-Review',
  liveExplained: (seconds, gap, perHour) =>
    `Solange es eingeschaltet ist, werden die geänderten Absätze an die KI gesendet, sobald Sie ${seconds} ${seconds === 1 ? 'Sekunde' : 'Sekunden'} pausieren: höchstens ein Review alle ${gap} und ${perHour} pro Stunde. Diese Zahlen lassen sich in den Einstellungen ändern. Jedes Review wird protokolliert. Die Experten geben Hinweise; sie schreiben nicht für Sie.`,
  hints: 'Hinweise',
  noHintsYet: 'Noch keine Hinweise. Schreiben Sie einfach weiter.',
  turnOnForHints: 'Mit eingeschaltetem Live-Review kommen Hinweise schon beim Schreiben.',
  classifyOrReply: 'Einordnen oder antworten',
  whoWatches: 'Wer mitliest',

  ofYourPlan: (share: ReactNode) => <>{share} Ihres Plans</>,
  daysLeft: n => (n === 1 ? 'noch 1 Tag' : `noch ${n} Tage`),
  daysPast: n => `${n} ${n === 1 ? 'Tag' : 'Tage'} über der Frist`,
  wordsPerDay: n => `${n} ${n === 1 ? 'Wort' : 'Wörter'} pro Tag bis zum Ziel`,
  doneOf: (done, target) => `${done} von ${target}`,
  words: 'Wörter',
  figures: 'Abbildungen',
  tables: 'Tabellen',
  references: 'Literaturangaben',
  openMarkers: n => `${n} ${n === 1 ? 'offene Markierung' : 'offene Markierungen'} (TODO, ??, UNVERIFIED…) im Text`,
  sections: 'Abschnitte',
  sectionWords: (words, target) => (target !== undefined ? `${words} von ${target} Wörtern` : `${words} ${words === 1 ? 'Wort' : 'Wörter'}`),
  noHeadingYet: 'noch keine Überschrift mit diesem Namen',
  markers: n => `${n} ${n === 1 ? 'Markierung' : 'Markierungen'}`,
  points: 'Punkte',
  tickedByYou: '✓ von Ihnen abgehakt',
  notChecked: '· nicht geprüft',
  covered: '✓ abgedeckt',
  partial: '◐ teilweise',
  missing: '○ fehlt',
  lastChecked: time => `Zuletzt geprüft: ${time}. Eine Prüfung ist die Lesart der KI, kein Urteil.`,

  planExplained: 'Ihr Plan als einfacher Text: ein Ziel, Zielwerte, Abschnitte und die Punkte, die der Text machen muss. Er gehört Ihnen; die KI kann ihn nicht ändern.',
  yourPlan: 'Ihr Plan',
  howTo: [
    ['Ziel', <>Ein oder zwei Sätze: Was soll die Leserin am Ende wissen?</>],
    ['Umfang', <>Eine Zahl hinter dem, was Sie im Blick behalten wollen; die Abgabe als Datum wie <code>2027-03-31</code>. Leere Zeilen zählen nicht.</>],
    ['Abschnitte', <>Je eine Zeile mit der Überschrift des Abschnitts, seinen Wörtern und seinem Stand: <code>- Methoden: 800, Entwurf</code>. Die Stände sind Gliederung, Entwurf, überarbeitet und fertig.</>],
    ['Kernaussagen', <>Je eine Zeile für das, was der Text sagen muss: <code>- Die Stichprobe ist beschrieben</code>. Setzen Sie <code>[x]</code> vor eine, die Sie selbst abhaken, oder lassen Sie die Prüfung danach suchen.</>],
  ],
  savePlan: 'Plan speichern',
  cancel: 'Abbrechen',
  noPlanYet: 'Noch kein Plan. Legen Sie fest, was der Text erreichen soll (Wörter, Abschnitte, die Punkte, die er machen muss), und sehen Sie beim Schreiben, wie weit Sie sind.',
  makePlan: 'Plan anlegen',
  editPlan: 'Plan bearbeiten',
  noPointsYet: 'Ihr Plan enthält noch keine Punkte',
  checking: 'Prüft…',
  checkPoints: 'Punkte prüfen',
  checkExplained: 'Die Prüfung sendet die Punkte Ihres Plans und das Dokument an die KI und fragt, welche Punkte der Text macht. Sie wird protokolliert.',
}
