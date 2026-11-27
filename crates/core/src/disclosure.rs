//! The disclosure of AI use (AI register) and the prompts appendix, built from the log.
//! Ported from the plugin's `plugin/tools/ailog.py` (`turns`, `cmd_disclosure`, `cmd_prompts`).
//!
//! With `Profile::Plugin` the output is the plugin's, byte for byte: that is what the golden
//! tests compare. `Profile::Editor` says the same about a document written in the editor, where
//! the wording about Claude Code, the plugin and git would be untrue.

use crate::text::{first_chars, py_splitlines, py_squash, py_strip};
use crate::types::{AnnotationEntry, GuardEntry, LogEntry, PromptEntry, ResponseEntry};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocFormat {
    Md,
    Tex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// A paper folder written with the plugin in Claude Code.
    Plugin,
    /// A markdown file written in the editor, with its sidecar.
    Editor,
}

/// Prompt, response and latest annotation of one completed turn that is not bookkeeping.
#[derive(Debug, Clone)]
pub struct Turn<'a> {
    pub prompt: &'a PromptEntry,
    pub response: &'a ResponseEntry,
    pub annotation: Option<&'a AnnotationEntry>,
    pub category: String,
    /// The author owes this turn a classification.
    pub is_open: bool,
    /// Started by the app on its own: a live review, a scope check, a change summary, a briefing proposal.
    pub is_auto: bool,
    /// Only launched a reviewer; the report is its own turn.
    pub is_launch: bool,
}

fn auto_category(origin: Option<&str>) -> Option<&'static str> {
    match origin {
        Some("watch") => Some("live-review"),
        Some("scope") => Some("scope-check"),
        Some("summary") => Some("change-summary"),
        Some("brief") => Some("brief-update"),
        _ => None,
    }
}

pub fn turns(entries: &[LogEntry]) -> Vec<Turn<'_>> {
    let mut responses: HashMap<&str, &ResponseEntry> = HashMap::new();
    let mut notes: HashMap<&str, &AnnotationEntry> = HashMap::new();
    for entry in entries {
        match entry {
            LogEntry::Response(r) => {
                responses.insert(&r.reference, r);
            }
            LogEntry::Annotation(a) => {
                notes.insert(&a.reference, a);
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for entry in entries {
        let LogEntry::Prompt(p) = entry else { continue };
        if p.category == "meta" {
            continue;
        }
        let Some(response) = responses.get(p.id.as_str()).copied() else { continue };
        let annotation = notes.get(p.id.as_str()).copied();
        let auto = auto_category(p.origin.as_deref());
        let category = auto.map(String::from).or_else(|| annotation.and_then(|a| a.category.clone()).filter(|c| !c.is_empty())).unwrap_or_else(|| p.category.clone());
        let is_launch = response.agents.as_ref().is_some_and(|agents| !agents.is_empty());
        out.push(Turn { prompt: p, response, annotation, category, is_open: annotation.is_none() && auto.is_none() && !is_launch, is_auto: auto.is_some(), is_launch });
    }
    out
}

fn category_label(lang: Lang, category: &str) -> &str {
    let (en, de) = match category {
        "literature" => ("Literature search and source checks", "Literaturrecherche und Quellenprüfung"),
        "methods" => ("Methods and statistics feedback", "Feedback zu Methodik und Statistik"),
        "feedback" => ("Feedback and critique of own text", "Feedback und Kritik zum eigenen Text"),
        "structure" => ("Structure and argumentation feedback", "Feedback zu Aufbau und Argumentation"),
        "language" => ("Grammar, spelling and punctuation corrections", "Korrektur von Grammatik, Rechtschreibung und Zeichensetzung"),
        "style" => ("Hints on writing style (compared with model papers)", "Hinweise zum Schreibstil (im Vergleich mit Vorbild-Artikeln)"),
        "terminology" => ("Checks of field-specific terminology", "Prüfung der Fachterminologie"),
        "concepts" => ("Explanation of concepts", "Erklärung von Konzepten"),
        "content" => ("Domain-specific content feedback", "Fachliches Feedback"),
        "integrity" => ("Research integrity checks", "Prüfung guter wissenschaftlicher Praxis"),
        "general" => ("General questions", "Allgemeine Fragen"),
        "live-review" => ("Live review while writing (short hints on changed paragraphs)", "Live-Review beim Schreiben (kurze Hinweise zu geänderten Absätzen)"),
        "scope-check" => ("Check of the planned points against the text", "Abgleich der geplanten Punkte mit dem Text"),
        "change-summary" => (
            "Description of what the manuscript's diff shows since a piece of feedback (the author classifies)",
            "Beschreibung, was der Diff des Manuskripts seit einem Hinweis zeigt (die Einordnung trifft die Autorin bzw. der Autor)",
        ),
        "brief-update" => (
            "Briefing lines for a reviewer persona, distilled from a feedback discussion; written to party.md only once the author confirmed them",
            "Briefing-Zeilen für eine Reviewer-Persona, aus einer Feedback-Diskussion destilliert; erst nach Bestätigung durch die Autorin bzw. den Autor in party.md geschrieben",
        ),
        other => (other, other),
    };
    if lang == Lang::De {
        de
    } else {
        en
    }
}

fn use_label(lang: Lang, use_: &str) -> &str {
    let (en, de) = match use_ {
        "adopted" => ("adopted", "übernommen"),
        "adapted" => ("adapted", "angepasst"),
        "rejected" => ("rejected", "verworfen"),
        "info" => ("information only", "nur Information"),
        "open" => ("not annotated", "nicht annotiert"),
        "auto" => ("hints only, no classification required", "nur Hinweise, keine Einordnung erforderlich"),
        other => (other, other),
    };
    if lang == Lang::De {
        de
    } else {
        en
    }
}

/// Counts in the order of Python's `Counter.most_common()`: highest first, ties by first appearance.
fn most_common<'a>(items: impl Iterator<Item = &'a str>) -> Vec<(&'a str, usize)> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for item in items {
        match counts.iter_mut().find(|(key, _)| *key == item) {
            Some((_, n)) => *n += 1,
            None => counts.push((item, 1)),
        }
    }
    counts.sort_by_key(|count| std::cmp::Reverse(count.1));
    counts
}

fn sorted_set<'a>(items: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    items.collect::<BTreeSet<_>>().into_iter().collect()
}

/// What made the turns, as the log recorded it.
fn tool_label(items: &[&Turn], profile: Profile) -> String {
    let models = sorted_set(items.iter().map(|t| t.response.model.as_deref().unwrap_or("unknown")));
    let models = if models.is_empty() { String::new() } else { format!(" ({})", models.join(", ")) };
    let versions = |of: fn(&ResponseEntry) -> Option<&str>| sorted_set(items.iter().filter_map(|t| of(t.response)).filter(|v| !v.is_empty()));
    let named = |name: &str, list: Vec<&str>| if list.is_empty() { name.to_string() } else { format!("{name} {}", list.join(", ")) };
    match profile {
        // 'Claude Code 2.1.300; CoWriting 0.1.0 (claude-opus-5-5)'
        Profile::Plugin => {
            let mut parts = vec![named("Claude Code", versions(|r| r.claude_code.as_deref()))];
            let plugin = versions(|r| r.plugin.as_deref());
            if !plugin.is_empty() {
                parts.push(named("CoWriting", plugin));
            }
            format!("{}{models}", parts.join("; "))
        }
        // 'CoWriting Editor 0.1.0; Anthropic (claude-opus-5-5)'
        Profile::Editor => {
            let mut parts = vec![named("CoWriting Editor", versions(|r| r.editor.as_deref()))];
            let providers = versions(|r| r.provider.as_deref());
            if !providers.is_empty() {
                parts.push(providers.join(", "));
            }
            format!("{}{models}", parts.join("; "))
        }
    }
}

/// The manuscript files among a guard event's files: what the disclosure must list.
fn manuscript_files(event: &GuardEntry, profile: Profile) -> Vec<&str> {
    event
        .files
        .iter()
        .map(String::as_str)
        .filter(|f| match profile {
            Profile::Plugin => f.to_lowercase().starts_with("manuscript/"),
            // beside the sidecar, everything in the document's folder is the author's
            Profile::Editor => !f.to_lowercase().starts_with(".cowriting/"),
        })
        .collect()
}

/// The log's files among a guard event's files: a rewrite that was undone.
fn log_files(event: &GuardEntry, profile: Profile) -> Vec<&str> {
    event
        .files
        .iter()
        .map(String::as_str)
        .filter(|f| match profile {
            Profile::Plugin => f.to_lowercase().starts_with("ai-log/"),
            Profile::Editor => f.to_lowercase().starts_with(".cowriting/") && f.to_lowercase().contains("/ai-log/"),
        })
        .collect()
}

const TEX_SPECIALS: &[(char, &str)] =
    &[('\\', r"\textbackslash{}"), ('&', r"\&"), ('%', r"\%"), ('$', r"\$"), ('#', r"\#"), ('_', r"\_"), ('{', r"\{"), ('}', r"\}"), ('~', r"\textasciitilde{}"), ('^', r"\^{}")];

/// Character by character, so the braces an escape introduces are not escaped again.
pub fn tex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match TEX_SPECIALS.iter().find(|(special, _)| *special == c) {
            Some((_, escaped)) => out.push_str(escaped),
            None => out.push(c),
        }
    }
    out
}

fn day(ts: &str) -> &str {
    first_chars(ts, 10)
}

fn minute(ts: &str) -> String {
    first_chars(ts, 16).replace('T', " ")
}

struct Row {
    purpose: String,
    tool: String,
    extent: String,
    use_: String,
    chapters: String,
    examples: Vec<String>,
    is_language: bool,
}

/// A list section: its lines as bullets with the note after them, or "None.".
fn md_section(lines: &[String], note: &str, none: &str) -> Vec<String> {
    if lines.is_empty() {
        return vec![none.to_string()];
    }
    lines.iter().map(|line| format!("- {line}")).chain([note.to_string()]).collect()
}

fn tex_section(lines: &[String], note: &str, none: &str) -> Vec<String> {
    if lines.is_empty() {
        return vec![tex_escape(none)];
    }
    let mut out = vec!["\\begin{itemize}".to_string()];
    out.extend(lines.iter().map(|line| format!("  \\item {}", tex_escape(line))));
    out.push("\\end{itemize}".into());
    out.push(tex_escape(note));
    out
}

/// The AI register for a document called `name`: one row per purpose, then what must be named
/// one by one (AI-written text, changes during tool calls, refused writes).
pub fn disclosure(entries: &[LogEntry], name: &str, lang: Lang, format: DocFormat, profile: Profile) -> String {
    disclosure_with_document(entries, name, lang, format, profile, None)
}

/// The register, with the document as it is now: passages of AI text the editor logged are then
/// said to be still in the text or not.
pub fn disclosure_with_document(entries: &[LogEntry], name: &str, lang: Lang, format: DocFormat, profile: Profile, document: Option<&str>) -> String {
    let de = lang == Lang::De;
    let pick = |en: &str, german: &str| if de { german.to_string() } else { en.to_string() };
    let all = turns(entries);

    // groups in the order their category first appears, then the largest first
    let mut groups: Vec<(&str, Vec<&Turn>)> = Vec::new();
    for turn in &all {
        match groups.iter_mut().find(|(category, _)| *category == turn.category) {
            Some((_, items)) => items.push(turn),
            None => groups.push((&turn.category, vec![turn])),
        }
    }
    groups.sort_by_key(|group| std::cmp::Reverse(group.1.len()));

    let mut rows = Vec::new();
    for (category, items) in &groups {
        let mut dates: Vec<&str> = items.iter().map(|t| day(&t.prompt.ts)).collect();
        dates.sort();
        // a turn that only launched a reviewer and the report it brought are one request
        let requests = items.iter().filter(|t| !t.is_launch).count().max(1);
        let counted = |uses: Vec<(&str, usize)>| uses.iter().map(|(u, n)| format!("{}: {n}", use_label(lang, u))).collect::<Vec<_>>().join(", ");
        let use_ = if items.iter().all(|t| t.is_auto) {
            let noted = most_common(items.iter().filter_map(|t| t.annotation).map(|a| a.use_.as_str()));
            format!("{}{}", use_label(lang, "auto"), if noted.is_empty() { String::new() } else { format!("; {}", counted(noted)) })
        } else {
            // a turn that only launched a reviewer is classified through its report, not itself
            let uses = most_common(items.iter().filter(|t| t.annotation.is_some() || t.is_open).map(|t| t.annotation.map_or("open", |a| a.use_.as_str())));
            if uses.is_empty() {
                "–".to_string()
            } else {
                counted(uses)
            }
        };
        let chapters = sorted_set(items.iter().filter_map(|t| t.annotation).filter_map(|a| a.chapter.as_deref()).filter(|c| !c.is_empty()));
        rows.push(Row {
            purpose: category_label(lang, category).to_string(),
            tool: tool_label(items, profile),
            extent: {
                let (first, last) = (dates[0], dates[dates.len() - 1]);
                // one day is a date, not a range; the plugin's register writes the range either way
                let days = if profile == Profile::Editor && first == last { first.to_string() } else { format!("{first} {} {last}", if de { "bis" } else { "to" }) };
                let noun = match (de, requests == 1) {
                    (true, true) => "Anfrage",
                    (true, false) => "Anfragen",
                    (false, true) => "request",
                    (false, false) => "requests",
                };
                format!("{requests} {noun}, {days}")
            },
            use_,
            chapters: if chapters.is_empty() { "–".to_string() } else { chapters.join(", ") },
            examples: items.iter().filter(|t| t.prompt.origin.is_none()).take(3).map(|t| py_squash(&t.prompt.text, Some(120))).collect(),
            is_language: *category == "language",
        });
    }

    let mut ai_text: Vec<String> = all.iter().filter(|t| t.response.ai_text_flag).map(|t| format!("{}: {}", day(&t.prompt.ts), py_squash(&t.prompt.text, Some(120)))).collect();
    // what the editor saw enter the document from a logged answer; grammar corrections are listed apart
    let mut corrections: Vec<String> = Vec::new();
    if profile == Profile::Editor {
        for found in entries.iter().filter_map(|e| if let LogEntry::AiText(found) = e { Some(found) } else { None }) {
            let asked = all.iter().find(|t| t.prompt.id == found.reference).map(|t| py_squash(&t.prompt.text, Some(80))).unwrap_or_default();
            let comment = entries.iter().rev().find_map(|e| match e {
                LogEntry::Annotation(a) if a.reference == found.id => a.note.clone(),
                _ => None,
            });
            let how = match (found.source.as_str(), de) {
                ("paste", false) => "pasted",
                ("paste", true) => "eingefügt",
                (_, false) => "typed",
                (_, true) => "getippt",
            };
            let closeness = match (found.match_.as_str(), de) {
                ("exact", false) => "word for word",
                ("exact", true) => "wörtlich",
                (_, false) => "close to the answer",
                (_, true) => "nah an der Antwort",
            };
            let state = document.map(|text| match (crate::paste_tracking::still_in_text(found, text), de) {
                (true, false) => "; still in the text",
                (true, true) => "; noch im Text",
                (false, false) => "; no longer in the text",
                (false, true) => "; nicht mehr im Text",
            });
            let mut line = if de {
                format!("{}: „{}“ ({how}, {} Wörter, {closeness}; aus der Antwort auf „{asked}“{})", day(&found.ts), found.excerpt, found.words, state.unwrap_or(""))
            } else {
                format!("{}: “{}” ({how}, {} words, {closeness}; from the answer to “{asked}”{})", day(&found.ts), found.excerpt, found.words, state.unwrap_or(""))
            };
            if let Some(comment) = comment.filter(|c| !c.trim().is_empty()) {
                line.push_str(&if de { format!(" – Kommentar: {}", comment.trim()) } else { format!(" – comment: {}", comment.trim()) });
            }
            if found.kind == "correction" {
                corrections.push(line)
            } else {
                ai_text.push(line)
            }
        }
    }
    let corrections_head = pick("Language corrections taken over from AI answers", "Aus KI-Antworten übernommene Sprachkorrekturen");
    let detection_note = pick(
        "The editor compares what is pasted or typed into the document with the AI's logged answers and lists matches here. Text from other AI tools cannot be detected.",
        "Der Editor vergleicht, was in das Dokument eingefügt oder getippt wird, mit den protokollierten KI-Antworten und führt Übereinstimmungen hier auf. Text aus anderen KI-Werkzeugen kann nicht erkannt werden.",
    );
    let guards: Vec<&GuardEntry> = entries.iter().filter_map(|e| if let LogEntry::Guard(g) = e { Some(g) } else { None }).collect();
    let changed = guards.iter().filter(|g| !g.refused);
    // only the manuscript is declared; a changed bibliography or setting is in the log, not the register
    let guard_lines: Vec<String> =
        changed.clone().filter(|g| !manuscript_files(g, profile).is_empty()).map(|g| format!("{}: {} – {}", day(&g.ts), g.tool, manuscript_files(g, profile).join(", "))).collect();
    let log_lines: Vec<String> =
        changed.filter(|g| !log_files(g, profile).is_empty()).map(|g| format!("{}: {} – {}", day(&g.ts), g.tool, log_files(g, profile).join(", "))).collect();
    let refused_lines: Vec<String> = guards.iter().filter(|g| g.refused).map(|g| format!("{}: {} – {}", day(&g.ts), g.tool, py_squash(&g.input, Some(80)))).collect();

    let title = pick("AI register (disclosure of AI use)", "KI-Verzeichnis");
    let mut intro = match (profile, de) {
        (Profile::Plugin, false) => format!(
            "For “{name}”, the generative AI model Claude (Anthropic) was used via Claude Code with the CoWriting \
             plugin in an assisting role only: for hints, questions, explanations and literature support. The manuscript text was written \
             by the author. The plugin refuses the AI's writes to manuscript files, records any change it nevertheless detects during an \
             AI tool call (listed below), and the manuscript's version history (git) shows how the text came about. All AI interactions \
             were logged automatically (ai-log/)."
        ),
        (Profile::Plugin, true) => format!(
            "Für die Arbeit „{name}“ wurde das generative KI-Modell Claude (Anthropic) über Claude Code mit dem \
             Plugin CoWriting ausschließlich assistierend eingesetzt: für Hinweise, Rückfragen, Erklärungen und Literaturhilfe. \
             Der Text des Manuskripts wurde von der Autorin bzw. dem Autor verfasst. Das Plugin weist Schreibzugriffe der KI auf \
             Manuskriptdateien ab, protokolliert jede Änderung, die es während eines KI-Werkzeugaufrufs dennoch feststellt (siehe unten), \
             und die Versionsgeschichte des Manuskripts (git) zeigt, wie der Text entstanden ist. Alle KI-Eingaben wurden automatisch \
             protokolliert (ai-log/)."
        ),
        (Profile::Editor, false) => format!(
            "For “{name}”, generative AI was used through the CoWriting Editor in an assisting role only: for hints, questions, \
             explanations and literature support. The models and who served them are named in the table. The manuscript text was written \
             by the author. The editor gives the AI no means of writing to the manuscript, records every write the AI attempted outside \
             its own notes (listed below), and logged all AI interactions automatically."
        ),
        (Profile::Editor, true) => format!(
            "Für die Arbeit „{name}“ wurde generative KI über den CoWriting Editor ausschließlich assistierend eingesetzt: für Hinweise, \
             Rückfragen, Erklärungen und Literaturhilfe. Die Modelle und ihre Anbieter sind in der Tabelle genannt. Der Text des Manuskripts \
             wurde von der Autorin bzw. dem Autor verfasst. Der Editor gibt der KI keine Möglichkeit, in das Manuskript zu schreiben, \
             protokolliert jeden Schreibversuch der KI außerhalb ihrer eigenen Notizen (siehe unten) und hat alle KI-Eingaben automatisch \
             protokolliert."
        ),
    };
    if all.iter().any(|t| t.prompt.origin.as_deref() == Some("brief")) {
        intro += &pick(
            " Some briefing lines for the reviewer personas (party.md) were proposed by the model from discussions with the author \
             and taken over only after the author confirmed them.",
            " Einzelne Briefing-Zeilen für die Reviewer-Personas (party.md) hat das Modell aus Diskussionen mit der Autorin bzw. dem Autor \
             vorgeschlagen; übernommen wurden sie erst nach deren Bestätigung.",
        );
    }
    let head: [&str; 5] = if de { ["Zweck", "Werkzeug / Modell", "Umfang", "Verwendung", "Kapitel"] } else { ["Purpose", "Tool / model", "Extent", "Use", "Chapters"] };
    let lang_note = pick(
        "Pure language corrections (grammar/spelling) need no disclosure in DFG proposals; some universities still require it.",
        "Reine Sprachkorrekturen (Grammatik/Rechtschreibung) müssen laut DFG in Anträgen nicht offengelegt werden; manche Hochschulen verlangen es dennoch.",
    );
    let ai_text_head = pick("Text formulated by the AI ([AI-TEXT])", "Von der KI formulierte Textstellen ([AI-TEXT])");
    let guard_head = pick("Manuscript changes detected during AI tool calls", "Während KI-Werkzeugaufrufen festgestellte Änderungen am Manuskript");
    let guard_note = pick(
        "Each of these changes was reviewed; what remained in the text is declared above or under [AI-TEXT].",
        "Jede dieser Änderungen wurde geprüft; was davon im Text blieb, ist oben bzw. unter [AI-TEXT] angegeben.",
    );
    // what the author states about AI tools outside the editor; the section is there only when they stated something
    let external_head = pick("AI use outside the editor, as stated by the author", "KI-Nutzung außerhalb des Editors, nach Angabe der Autorin oder des Autors");
    let external_note = pick(
        "The editor cannot see the use of other tools. These statements are the author's own.",
        "Die Nutzung anderer Werkzeuge kann der Editor nicht sehen. Diese Angaben stammen von der Autorin oder dem Autor selbst.",
    );
    let external: Vec<String> = crate::types::external_uses(entries)
        .iter()
        .map(|stated| {
            let ended = |text: &str| if text.ends_with(['.', '!', '?']) { text.to_string() } else { format!("{text}.") };
            let mut line = String::new();
            if !stated.date.trim().is_empty() {
                line.push_str(&format!("{}: ", stated.date.trim()));
            }
            line.push_str(stated.tool.trim());
            if !stated.model.trim().is_empty() {
                line.push_str(&format!(" ({})", stated.model.trim()));
            }
            line.push_str(&format!(". {} {}", pick("Used for:", "Verwendet für:"), ended(&py_squash(&stated.purpose, None))));
            for (label, text) in [
                (pick("Worked on:", "Gegenstand:"), &stated.material),
                (pick("Result and its use:", "Ergebnis und Verwendung:"), &stated.result),
                (pick("Checked by:", "Geprüft durch:"), &stated.checked),
            ] {
                if !text.trim().is_empty() {
                    line.push_str(&format!(" {label} {}", ended(&py_squash(text, None))));
                }
            }
            line
        })
        .collect();

    let refused_head = pick("Write attempts by the AI that the guard refused", "Vom Schutz abgewiesene Schreibversuche der KI");
    let refused_note = match profile {
        Profile::Plugin => pick(
            "These changed nothing. The pattern check can also refuse harmless commands.",
            "Dadurch wurde nichts geändert. Die Musterprüfung kann auch harmlose Befehle abweisen.",
        ),
        Profile::Editor => pick("These changed nothing.", "Dadurch wurde nichts geändert."),
    };
    let log_head = pick("Changes to the AI log undone after a tool call", "Nach einem Werkzeugaufruf rückgängig gemachte Änderungen am KI-Protokoll");
    let log_note = match profile {
        Profile::Plugin => pick(
            "The log was restored each time; the altered version is kept in .cowriting/log-rewrites/.",
            "Das Protokoll wurde jeweils wiederhergestellt; die veränderte Fassung liegt unter .cowriting/log-rewrites/.",
        ),
        Profile::Editor => pick(
            "The log was restored each time; the altered version is kept in the document's .cowriting folder, under log-rewrites/.",
            "Das Protokoll wurde jeweils wiederhergestellt; die veränderte Fassung liegt im .cowriting-Ordner des Dokuments unter log-rewrites/.",
        ),
    };
    let none = pick("None.", "Keine.");
    let examples_head = pick("Example prompts", "Beispielhafte Anfragen");
    let resp = pick(
        "The author bears full responsibility for content, sources and wording. Adapt this statement to the target institution's rules.",
        "Die Verantwortung für Inhalt, Quellen und Formulierungen liegt vollständig bei der Autorin bzw. dem Autor. Bitte an die Vorgaben der Zielinstitution anpassen.",
    );

    let mut out: Vec<String> = Vec::new();
    match format {
        DocFormat::Md => {
            out.extend([format!("# {title}"), String::new(), intro, String::new(), format!("| {} |", head.join(" | ")), format!("|{}", "---|".repeat(head.len()))]);
            for r in &rows {
                out.push(format!("| {}{} | {} | {} | {} | {} |", r.purpose, if r.is_language { " ¹" } else { "" }, r.tool, r.extent, r.use_, r.chapters));
            }
            out.extend([String::new(), format!("¹ {lang_note}"), String::new(), format!("## {examples_head}"), String::new()]);
            for r in rows.iter().filter(|r| !r.examples.is_empty()) {
                out.push(format!("**{}**", r.purpose));
                out.extend(r.examples.iter().map(|e| if de { format!("- „{e}“") } else { format!("- “{e}”") }));
                out.push(String::new());
            }
            out.extend([format!("## {ai_text_head}"), String::new()]);
            if ai_text.is_empty() {
                out.push(none.clone());
            } else {
                out.extend(ai_text.iter().map(|line| format!("- {line}")));
            }
            if profile == Profile::Editor {
                out.extend([String::new(), detection_note.clone()]);
                if !corrections.is_empty() {
                    out.extend([String::new(), format!("## {corrections_head}"), String::new()]);
                    out.extend(corrections.iter().map(|line| format!("- {line}")));
                }
            }
            if !external.is_empty() {
                out.extend([String::new(), format!("## {external_head}"), String::new()]);
                out.extend(external.iter().map(|line| format!("- {line}")));
                out.extend([String::new(), external_note.clone()]);
            }
            for (heading, lines, note) in [(&guard_head, &guard_lines, &guard_note), (&log_head, &log_lines, &log_note), (&refused_head, &refused_lines, &refused_note)] {
                out.extend([String::new(), format!("## {heading}"), String::new()]);
                out.extend(md_section(lines, note, &none));
            }
            out.extend([String::new(), format!("*{resp}*"), String::new()]);
        }
        DocFormat::Tex => {
            let e = tex_escape;
            out.extend([
                format!("\\section*{{{}}}", e(&title)),
                String::new(),
                e(&intro),
                String::new(),
                "\\begin{tabular}{p{.22\\linewidth}p{.18\\linewidth}p{.2\\linewidth}p{.2\\linewidth}p{.1\\linewidth}}".to_string(),
                "\\hline".to_string(),
                format!("{} \\\\", head.iter().map(|h| format!("\\textbf{{{}}}", e(h))).collect::<Vec<_>>().join(" & ")),
                "\\hline".to_string(),
            ]);
            for r in &rows {
                let purpose = format!("{}{}", r.purpose, if r.is_language { " (1)" } else { "" });
                let cells = [purpose.as_str(), &r.tool, &r.extent, &r.use_, &r.chapters].map(e);
                out.push(format!("{} \\\\", cells.join(" & ")));
            }
            out.extend([
                "\\hline".to_string(),
                "\\end{tabular}".to_string(),
                String::new(),
                format!("\\noindent(1) {}", e(&lang_note)),
                String::new(),
                format!("\\subsection*{{{}}}", e(&examples_head)),
                "\\begin{itemize}".to_string(),
            ]);
            for r in &rows {
                out.extend(r.examples.iter().map(|example| format!("  \\item \\textit{{{}:}} {}", e(&r.purpose), e(example))));
            }
            out.extend(["\\end{itemize}".to_string(), format!("\\subsection*{{{}}}", e(&ai_text_head))]);
            if ai_text.is_empty() {
                out.push(e(&none));
            } else {
                out.push("\\begin{itemize}".into());
                out.extend(ai_text.iter().map(|line| format!("  \\item {}", e(line))));
                out.push("\\end{itemize}".into());
            }
            if profile == Profile::Editor {
                out.push(e(&detection_note));
                if !corrections.is_empty() {
                    out.extend([format!("\\subsection*{{{}}}", e(&corrections_head)), "\\begin{itemize}".to_string()]);
                    out.extend(corrections.iter().map(|line| format!("  \\item {}", e(line))));
                    out.push("\\end{itemize}".into());
                }
            }
            if !external.is_empty() {
                out.extend([format!("\\subsection*{{{}}}", e(&external_head)), "\\begin{itemize}".to_string()]);
                out.extend(external.iter().map(|line| format!("  \\item {}", e(line))));
                out.extend(["\\end{itemize}".to_string(), e(&external_note)]);
            }
            for (heading, lines, note) in [(&guard_head, &guard_lines, &guard_note), (&log_head, &log_lines, &log_note), (&refused_head, &refused_lines, &refused_note)] {
                out.push(format!("\\subsection*{{{}}}", e(heading)));
                out.extend(tex_section(lines, note, &none));
            }
            out.extend([String::new(), format!("\\textit{{{}}}", e(&resp)), String::new()]);
        }
    }
    out.join("\n")
}

/// The prompts appendix some universities ask for: every AI input in order, with the author's classification.
pub fn prompts_appendix(entries: &[LogEntry], name: &str, lang: Lang, format: DocFormat, with_answers: bool, profile: Profile) -> String {
    let de = lang == Lang::De;
    let pick = |en: &str, german: &str| if de { german.to_string() } else { en.to_string() };
    let all = turns(entries);
    let title = pick("Appendix: AI prompts", "Anhang: KI-Prompts");
    let started_by = if profile == Profile::Plugin { "plugin" } else { "editor" };
    let intro = if de {
        format!(
            "Alle Eingaben an das KI-System für die Arbeit „{name}“, in zeitlicher Reihenfolge, mit der Einordnung durch die Autorin bzw. den Autor. \
             Automatisch gestartete Live-Reviews und Abgleiche sind als solche gekennzeichnet."
        )
    } else {
        format!("Every input to the AI system for “{name}”, in order, with the author's classification. Live reviews and checks the {started_by} started on its own are marked as such.")
    };
    let answer_head = pick("Answer", "Antwort");
    let note_head = pick("Comment", "Kommentar");
    let when_of: HashMap<&str, String> = all.iter().map(|t| (t.prompt.id.as_str(), minute(&t.prompt.ts))).collect();

    let heading = |t: &Turn| -> String {
        let (p, a) = (t.prompt, t.annotation);
        let use_ = use_label(lang, a.map_or(if t.is_auto { "auto" } else { "open" }, |a| a.use_.as_str()));
        let chapter = a.and_then(|a| a.chapter.as_deref()).filter(|c| !c.is_empty()).map(|c| format!(" ({c})")).unwrap_or_default();
        let report = if p.origin.as_deref() == Some("agent") {
            format!(" · {}{}", if de { "Bericht von " } else { "report from " }, p.agent.as_deref().unwrap_or("?"))
        } else {
            String::new()
        };
        // a reply thread: the author's reply names the input it answers, the persona's answer names the reply
        let reply = p
            .reply_to
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| format!(" · {}{}", if de { "Antwort auf " } else { "in reply to " }, when_of.get(r).map_or(r, String::as_str)))
            .unwrap_or_default();
        let flag = if t.response.ai_text_flag { " · [AI-TEXT]" } else { "" };
        format!("{} · {}{report}{reply} · {use_}{chapter}{flag}", minute(&p.ts), category_label(lang, &t.category))
    };
    let or_dash = |text: &str| if text.is_empty() { "–".to_string() } else { text.to_string() };
    let quoted = |text: &str| -> Vec<String> { py_splitlines(&or_dash(py_strip(text))).iter().map(|line| format!("> {line}")).collect() };
    let note_of = |t: &Turn| t.annotation.and_then(|a| a.note.clone()).filter(|n| !n.is_empty());

    let mut out: Vec<String> = Vec::new();
    match format {
        DocFormat::Md => {
            out.extend([format!("# {title}"), String::new(), intro, String::new()]);
            for t in &all {
                out.extend([format!("### {}", heading(t)), String::new()]);
                out.extend(quoted(&t.prompt.text));
                out.push(String::new());
                if with_answers {
                    out.extend([format!("**{answer_head} ({}):**", t.response.model.as_deref().unwrap_or("?")), String::new()]);
                    out.extend(quoted(&t.response.answer));
                    out.push(String::new());
                }
                if let Some(note) = note_of(t) {
                    out.extend([format!("*{note_head}:* {note}"), String::new()]);
                }
            }
        }
        DocFormat::Tex => {
            let e = tex_escape;
            out.extend([format!("\\section*{{{}}}", e(&title)), String::new(), e(&intro), String::new(), "\\begin{enumerate}".to_string()]);
            for t in &all {
                out.push(format!("  \\item \\textbf{{{}}} \\\\", e(&heading(t))));
                out.push(format!("  {}", or_dash(&e(&py_squash(&t.prompt.text, None)))));
                if with_answers {
                    out.push(format!("  \\\\ \\textit{{{} ({}):}} {}", e(&answer_head), e(t.response.model.as_deref().unwrap_or("?")), e(&py_squash(&t.response.answer, None))));
                }
                if let Some(note) = note_of(t) {
                    out.push(format!("  \\\\ \\textit{{{}:}} {}", e(&note_head), e(&py_squash(&note, None))));
                }
            }
            out.extend(["\\end{enumerate}".to_string(), String::new()]);
        }
    }
    out.join("\n")
}
