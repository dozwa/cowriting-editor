# KI-Verzeichnis

Für die Arbeit „Walking breaks & 100% focus_time“ wurde das generative KI-Modell Claude (Anthropic) über Claude Code mit dem Plugin CoWriting ausschließlich assistierend eingesetzt: für Hinweise, Rückfragen, Erklärungen und Literaturhilfe. Der Text des Manuskripts wurde von der Autorin bzw. dem Autor verfasst. Das Plugin weist Schreibzugriffe der KI auf Manuskriptdateien ab, protokolliert jede Änderung, die es während eines KI-Werkzeugaufrufs dennoch feststellt (siehe unten), und die Versionsgeschichte des Manuskripts (git) zeigt, wie der Text entstanden ist. Alle KI-Eingaben wurden automatisch protokolliert (ai-log/). Einzelne Briefing-Zeilen für die Reviewer-Personas (party.md) hat das Modell aus Diskussionen mit der Autorin bzw. dem Autor vorgeschlagen; übernommen wurden sie erst nach deren Bestätigung.

| Zweck | Werkzeug / Modell | Umfang | Verwendung | Kapitel |
|---|---|---|---|---|
| Prüfung der Fachterminologie | Claude Code 2.1.300; CoWriting 0.1.0 (claude-opus-5-5, m) | 1 Anfrage, 2026-10-04 bis 2026-10-05 | nicht annotiert: 1 | – |
| Feedback und Kritik zum eigenen Text | Claude Code 2.1.300; CoWriting 0.1.0 (claude-opus-5-5) | 2 Anfragen, 2026-10-01 bis 2026-10-07 | angepasst: 1, nicht annotiert: 1 | Einleitung |
| Feedback zu Aufbau und Argumentation | Claude Code 2.1.300; CoWriting 0.1.0 (claude-opus-5-5) | 2 Anfragen, 2026-10-07 bis 2026-10-07 | verworfen: 1, übernommen: 1 | Discussion, Methods |
| Korrektur von Grammatik, Rechtschreibung und Zeichensetzung ¹ | Claude Code 2.1.300; CoWriting 0.1.0 (claude-opus-5-5) | 1 Anfrage, 2026-10-02 bis 2026-10-02 | nicht annotiert: 1 | – |
| Live-Review beim Schreiben (kurze Hinweise zu geänderten Absätzen) | Claude Code 2.1.300; CoWriting 0.1.0 (haiku) | 1 Anfrage, 2026-10-03 bis 2026-10-03 | nur Hinweise, keine Einordnung erforderlich | – |
| Briefing-Zeilen für eine Reviewer-Persona, aus einer Feedback-Diskussion destilliert; erst nach Bestätigung durch die Autorin bzw. den Autor in party.md geschrieben | Claude Code 2.1.300; CoWriting 0.1.0 (m) | 1 Anfrage, 2026-10-06 bis 2026-10-06 | nur Hinweise, keine Einordnung erforderlich; übernommen: 1 | – |
| Abgleich der geplanten Punkte mit dem Text | Claude Code 2.1.300; CoWriting 0.1.0 (haiku) | 1 Anfrage, 2026-10-07 bis 2026-10-07 | nur Hinweise, keine Einordnung erforderlich | – |

¹ Reine Sprachkorrekturen (Grammatik/Rechtschreibung) müssen laut DFG in Anträgen nicht offengelegt werden; manche Hochschulen verlangen es dennoch.

## Beispielhafte Anfragen

**Prüfung der Fachterminologie**
- „Use the cowrite:terminologist agent to review main.tex“
- „Reply to The Terminologist (the cowrite:terminologist agent) on its terminology feedback from 10:00, log entry a4. My r…“

**Feedback und Kritik zum eigenen Text**
- „Gib mir Feedback zur Einleitung & zu 50% der Methodik“
- „Feedback on the abstract, please“

**Feedback zu Aufbau und Argumentation**
- „A question over several lines, with a_underscore, a {brace}, 100% and a very long tail word word word word word word wo…“
- „Check the outline“

**Korrektur von Grammatik, Rechtschreibung und Zeichensetzung**
- „Korrigiere Rechtschreibung“

## Von der KI formulierte Textstellen ([AI-TEXT])

- 2026-10-02: Korrigiere Rechtschreibung

## Während KI-Werkzeugaufrufen festgestellte Änderungen am Manuskript

- 2026-10-04: Bash – manuscript/main.tex
Jede dieser Änderungen wurde geprüft; was davon im Text blieb, ist oben bzw. unter [AI-TEXT] angegeben.

## Nach einem Werkzeugaufruf rückgängig gemachte Änderungen am KI-Protokoll

- 2026-10-08: Bash – ai-log/2026-10.jsonl
Das Protokoll wurde jeweils wiederhergestellt; die veränderte Fassung liegt unter .cowriting/log-rewrites/.

## Vom Schutz abgewiesene Schreibversuche der KI

- 2026-10-09: Write – manuscript/main.tex
- 2026-10-09: Bash – sed -i 's/subjects/participants/g' manuscript/main.tex && echo xxxxxxxxxxxxxxxx…
Dadurch wurde nichts geändert. Die Musterprüfung kann auch harmlose Befehle abweisen.

*Die Verantwortung für Inhalt, Quellen und Formulierungen liegt vollständig bei der Autorin bzw. dem Autor. Bitte an die Vorgaben der Zielinstitution anpassen.*
