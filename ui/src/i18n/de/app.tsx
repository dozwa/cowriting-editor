import type { ReactNode } from 'react'
import type { app as pattern } from '../en/app'

export const app: typeof pattern = {
  welcomeTitle: 'Sie schreiben.',
  welcomeMargin: 'Die KI liest mit und antwortet hier am Rand: Rückmeldungen, Fragen, Erklärungen. Ihren Text kann sie nicht ändern.',
  welcomePromise: 'Alles, was sie sagt, wird protokolliert, und aus dem Protokoll wird Ihre Offenlegung der KI-Nutzung.',
  openFile: 'Markdown-Datei öffnen…',
  newDocument: 'Neues Dokument…',
  notSetUp: (link: ReactNode) => <>Die KI ist noch nicht eingerichtet. {link}</>,
  setItUp: 'Einrichten',
  takeTour: 'Rundgang ansehen',

  open: 'Öffnen…',
  new: 'Neu…',
  save: 'Speichern',
  move: 'Verschieben…',
  moveTitle: 'Dieses Dokument umbenennen oder verschieben. Sein KI-Protokoll geht mit.',
  saved: 'gespeichert',
  notSaved: 'nicht gespeichert',
  words: (n: number) => `${n.toLocaleString('de')} ${n === 1 ? 'Wort' : 'Wörter'}`,
  chars: (n: number) => `${n.toLocaleString('de')} Zeichen`,
  charsDetail: (withSpaces: number, withoutSpaces: number) =>
    `${withSpaces.toLocaleString('de')} Zeichen mit Leerzeichen, ${withoutSpaces.toLocaleString('de')} ohne. Nicht gezählt: Kopfdaten, Code, Kommentare, Zitierschlüssel, Markdown-Zeichen, die Adressen von Links und Bilder.`,
  export: 'Exportieren…',
  disclosure: 'Offenlegung…',
  nothingToDisclose: 'Noch nichts offenzulegen: Die KI wurde nicht gefragt',
  settings: 'Einstellungen…',

  orphan: (name: ReactNode, questions: number, isSameText: boolean, document: string) => (
    <>
      In diesem Ordner liegt ein KI-Protokoll für {name} ({questions} {questions === 1 ? 'Frage' : 'Fragen'}), und diese Datei gibt es nicht mehr.{' '}
      {isSameText ? 'Sein zuletzt gespeicherter Text ist genau dieses Dokument.' : 'Sein zuletzt gespeicherter Text weicht von diesem Dokument ab.'} Ist {document} dieses Dokument, nur umbenannt?
    </>
  ),
  continueLog: 'Ja, sein Protokoll fortführen',
  newDocumentHere: 'Nein, dies ist ein neues Dokument',
  found: (words: number, isPasted: boolean, isCorrection: boolean) =>
    `Protokolliert: ${words} ${isPasted ? (words === 1 ? 'eingefügtes Wort' : 'eingefügte Wörter') : words === 1 ? 'Wort im Dokument' : 'Wörter im Dokument'} ${words === 1 ? 'stimmt' : 'stimmen'} mit ${isCorrection ? 'einer Sprachkorrektur aus einer KI-Antwort' : 'Text aus einer KI-Antwort'} überein. Nichts wurde geändert; die Stelle wird in der Offenlegung aufgeführt.`,
  showInLog: 'Im Protokoll zeigen',
  dismiss: 'Ausblenden',
  withoutTools: 'Dieses Modell kann keine Werkzeuge nutzen: Das Dokument wird mit jeder Frage mitgeschickt, und Notizen kann es nicht schreiben.',

  panelWidth: 'Breite der Seitenleiste',
  panelWidthTitle: 'Ziehen, um die Seitenleiste breiter oder schmaler zu machen. Doppelklick für die Standardbreite.',
  tabAsk: 'Fragen',
  tabAskTitle: 'Was Sie die KI zu Ihrem Text fragen und was sie antwortet',
  tabExperts: 'Experten',
  tabExpertsTitle: 'Gutachter mit einem eigenen Blickwinkel und was Sie ihnen zu diesem Dokument mitgeben',
  tabLive: 'Live-Review',
  tabLiveTitle: 'Kurze Hinweise der Experten beim Schreiben. Aus, bis Sie es einschalten',
  newHints: (n: number) => `${n} ${n === 1 ? 'neuer Hinweis' : 'neue Hinweise'}`,
  tabPlan: 'Plan',
  tabPlanTitle: 'Ihr Plan für diesen Text und wie weit er ist',
  tabSources: 'Quellen',
  tabSourcesTitle: 'Ihre Literaturangaben: finden, zitieren und prüfen',
  tabLog: 'Protokoll',
  tabLogTitle: 'Alles, was die KI gefragt wurde und geantwortet hat, und was Sie damit gemacht haben',
  toClassify: (n: number) => `${n} ${n === 1 ? 'Antwort' : 'Antworten'} einzuordnen`,

  assistant: 'Assistent',
  reviewRequest: 'Bitte mein Dokument prüfen.',
  toolSteps: {
    read_document: 'liest das Dokument',
    search_document: 'durchsucht das Dokument',
    list_notes: 'sieht in die eigenen Notizen',
    search_bibliography: 'durchsucht die Literaturangaben',
    read_note: 'liest eine Notiz',
    write_note: 'schreibt eine Notiz',
  },
  askingFor: (tool: string) => `fragt nach „${tool}“`,
  refusedWrite: (where: string) => `wollte schreiben, wo es nicht erlaubt ist (${where}): abgelehnt und protokolliert`,

  reviewing: 'Review läuft…',
  writing: 'Sie schreiben…',
  // the gap is said from the milliseconds: "alle" needs its own form for one minute
  waiting: (_every: string, ms: number) => `wartet: höchstens ein Review ${ms === 60_000 ? 'pro Minute' : ms % 60_000 === 0 ? `alle ${ms / 60_000} Minuten` : `alle ${Math.round(ms / 1000)} Sekunden`}`,
  hintsAt: (n: number, time: string) => `${n} ${n === 1 ? 'Hinweis' : 'Hinweise'} um ${time}`,
  nothingToAdd: (time: string) => `durchgesehen um ${time}: nichts anzumerken, schreiben Sie weiter`,
  paused: (perHour: number) => `pausiert: ${perHour} Reviews in der letzten Stunde`,
  reviewFailed: 'das letzte Live-Review ist fehlgeschlagen; nach der nächsten Änderung wird es erneut versucht',

  pointsFound: (covered: number, partial: number, missing: number) => `${covered} abgedeckt, ${partial} teilweise, ${missing} fehlen`,

  scripted: 'Vorgefertigte Antworten (kein Modell)',
  failedModel: 'Regeltest nicht bestanden',
  untestedModel: 'ungetestetes Modell',
  looks: 'Darstellung des Texts',
  markdownButtons: 'Markdown-Schaltflächen',
  markdownButtonsTitle: 'Schaltflächen über dem Text, die Markdown für Sie schreiben',
  lineNumbers: 'Zeilennummern',
  lineNumbersTitle: 'Die Zeilen nummerieren, so wie die Antworten der KI sie nennen',
  source: 'Quelltext',
  sourceTitle: 'Den Text als das Markdown zeigen, als das er gespeichert wird, mit jedem Zeichen',
  logged: (inputs: number, open: number, refused: number) =>
    `${inputs} ${inputs === 1 ? 'KI-Eingabe' : 'KI-Eingaben'} protokolliert${open > 0 ? ` · ${open} einzuordnen` : ''}${refused > 0 ? ` · ${refused} ${refused === 1 ? 'abgelehnter Schreibversuch' : 'abgelehnte Schreibversuche'}` : ''}`,
  reportBug: 'Fehler melden',
  reportBugTitle: 'Etwas ist schiefgegangen? Hier melden: ein kurzes Formular, von Ihnen als E-Mail gesendet',
  alpha: version => `Alpha ${version}`,
  alphaTitle: 'Eine frühe Version: Manches fehlt, manches kann brechen. Meldungen helfen.',
  calm: 'Ruhig',
  calmTitle: 'Erklärungen und Hinweise um die Bedienelemente ausblenden',
}
