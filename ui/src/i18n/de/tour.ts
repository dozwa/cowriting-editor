// Der Rundgang durch den Editor: eine Karte je Teil des Fensters, in der Reihenfolge, in der man ihnen begegnet.
export const tour = {
  label: 'Rundgang durch den Editor',
  count: (now: number, of: number) => `Schritt ${now} von ${of}`,
  back: 'Zurück',
  next: 'Weiter',
  done: 'Fertig',
  skip: 'Rundgang überspringen',
  needsDocument: 'Öffnen Sie ein Dokument, um diesen Teil des Fensters zu sehen.',
  setUp: 'KI einrichten…',
  steps: {
    welcome: {
      title: 'Sie schreiben, die KI liest',
      text: 'CoWriting Editor ist ein Markdown-Editor mit einer KI, die Rückmeldung zu Ihrem Text gibt und ihn nie für Sie schreibt. Was Sie fragen und was sie antwortet, wird protokolliert, damit Sie Ihre KI-Nutzung am Ende ehrlich angeben können. Dieser Rundgang zeigt die Teile der Reihe nach und dauert etwa zwei Minuten.',
    },
    ai: {
      title: 'Die KI wählen',
      text: 'Die Antworten kommen von einem Modell auf Ihrem eigenen Computer, von einem Anbieter, für den Sie einen API-Schlüssel haben, oder von Claude Code. Das wählen Sie in den Einstellungen und können es jederzeit ändern. Schreiben geht auch ohne.',
    },
    document: {
      title: 'Ein Dokument öffnen',
      text: 'Ein Dokument ist eine Markdown-Datei, irgendwo auf Ihrem Computer. Öffnen Sie eine oder beginnen Sie eine neue. Ihre Experten, verknüpften Dateien und ihr Plan gehören zu dieser Datei, und ihr Protokoll liegt in einem versteckten Ordner daneben.',
    },
    write: {
      title: 'Schreiben',
      text: 'Das ist Ihr Text. Die Schaltflächen darüber schreiben das Markdown für Sie, und der Text wird beim Tippen gestaltet. Die KI kann nichts ändern, was hier steht; das können nur Sie.',
    },
    ask: {
      title: 'Zum Text fragen',
      text: 'Stellen Sie eine Frage in eigenen Worten oder nutzen Sie eine fertige Anfrage, etwa zu Rückmeldung, Aufbau oder Sprache. Die Antwort erscheint hier, neben Ihrem Text. Danach geben Sie an, was Sie damit gemacht haben, und das kommt ins Protokoll.',
    },
    experts: {
      title: 'Experten',
      text: 'Experten sind Gutachter mit einem eigenen Blickwinkel, etwa ein strenger zweiter Gutachter. Wählen Sie, wer zu Ihrer Gruppe gehört, geben Sie ihnen mit, was sie über Ihre Arbeit wissen sollen, und verknüpfen Sie Dateien wie Wortlisten oder Methodendefinitionen als ihr Hintergrundwissen.',
    },
    live: {
      title: 'Live-Review',
      text: 'Sobald Sie es einschalten, lesen die gewählten Experten beim Schreiben mit und geben kurze Hinweise, wenn Sie innehalten. Wie oft sie sich melden, steht in den Einstellungen.',
    },
    plan: {
      title: 'Plan',
      text: 'Halten Sie fest, was der Text abdecken muss. Der Editor zeigt, wie weit Sie sind, und kann Ihre Punkte am Text prüfen.',
    },
    sources: {
      title: 'Quellen',
      text: 'Durchsuchen Sie Ihre Zotero-Bibliothek und zitieren Sie daraus, fügen Sie Literaturdateien hinzu und prüfen Sie, ob sich die Literaturangaben in Ihrem Text finden lassen.',
    },
    log: {
      title: 'Protokoll',
      text: 'Hier steht jede Frage und jede Antwort, mit dem, was Sie damit gemacht haben. Text aus einer Antwort, der in Ihrem Dokument auftaucht, wird ebenfalls vermerkt. KI-Nutzung außerhalb des Editors, etwa in einem anderen Chat, können Sie hier von Hand angeben.',
    },
    finish: {
      title: 'Exportieren und offenlegen',
      text: 'Exportieren schreibt eine PDF- oder Word-Datei mit Ihren Literaturangaben. Offenlegung schreibt die Erklärung Ihrer KI-Nutzung aus dem Protokoll, fertig zum Beilegen.',
    },
    around: {
      title: 'Am Rand',
      text: 'In den Einstellungen stehen Aussehen, Sprachen und Tastenkürzel. „Ruhig“ in der Fußzeile blendet die Erklärungen aus. Wenn etwas schiefgeht, meldet die rote Schaltfläche einen Fehler. Diesen Rundgang finden Sie jederzeit wieder in den Einstellungen unter „Über“.',
    },
  },
}
