//! The disclosure of AI use (AI register) and the prompts appendix, built from the log.
//! Ported from the plugin's `plugin/tools/ailog.py` (`turns`, `cmd_disclosure`, `cmd_prompts`).
//!
//! With `Profile::Plugin` the output is the plugin's, byte for byte: that is what the golden
//! tests compare. `Profile::Editor` says the same about a document written in the editor, where
//! the wording about Claude Code, the plugin and git would be untrue.

use crate::chats::Chat;
use crate::text::{first_chars, py_splitlines, py_squash, py_strip};
use crate::types::{AnnotationEntry, Attachment, ExternalEntry, GuardEntry, LogEntry, PromptEntry, ResponseEntry};
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

/// A provider as the register names it. The log says where a model on the author's own computer was reached
/// ("a local model server (localhost:11434)"); a reader of the register is told which program that is, by its
/// usual port, and that it is local. The address means nothing outside that computer.
fn provider_label(logged: &str, lang: Lang) -> String {
    let Some(host) = logged.strip_prefix("a local model server (").and_then(|rest| rest.strip_suffix(')')) else {
        return logged.to_string();
    };
    let de = lang == Lang::De;
    let program = match host.rsplit_once(':').map(|(_, port)| port) {
        Some("11434") => "Ollama",
        Some("1234") => "LM Studio",
        _ => return if de { "ein lokaler Modellserver" } else { "a local model server" }.to_string(),
    };
    format!("{program} ({})", if de { "lokal" } else { "local" })
}

/// The models that answered the turns, as the log names them.
fn models_of(items: &[&Turn]) -> String {
    sorted_set(items.iter().map(|t| t.response.model.as_deref().unwrap_or("unknown"))).join(", ")
}

/// The editor the turns went through ('CoWriting Editor 0.1.0') and who served the models ('Anthropic'; empty when the log does not say).
fn served_by(items: &[&Turn], lang: Lang) -> (String, String) {
    let versions = |of: fn(&ResponseEntry) -> Option<&str>| sorted_set(items.iter().filter_map(|t| of(t.response)).filter(|v| !v.is_empty()));
    let editors = versions(|r| r.editor.as_deref());
    let editor = if editors.is_empty() { "CoWriting Editor".to_string() } else { format!("CoWriting Editor {}", editors.join(", ")) };
    let providers: BTreeSet<String> = versions(|r| r.provider.as_deref()).into_iter().map(|logged| provider_label(logged, lang)).collect();
    (editor, providers.into_iter().collect::<Vec<_>>().join(", "))
}

/// What made the turns, as the log recorded it.
fn tool_label(items: &[&Turn], profile: Profile, lang: Lang) -> String {
    let models = models_of(items);
    let models = if models.is_empty() { String::new() } else { format!(" ({models})") };
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
            let (editor, providers) = served_by(items, lang);
            format!("{editor}{}{models}", if providers.is_empty() { String::new() } else { format!("; {providers}") })
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

/// The shares of the line the editor's table gives its columns in LaTeX: purpose, tool and model, extent, use, chapters.
const TEX_COLUMNS: [&str; 5] = [".23", ".24", ".18", ".2", ".15"];

/// A model's name may be broken after a colon, a slash or an underscore: it has no spaces, and a long one would leave its column.
fn breakable(escaped: &str) -> String {
    escaped.replace(':', ":\\allowbreak{}").replace('/', "/\\allowbreak{}").replace("\\_", "\\_\\allowbreak{}")
}

/// What the editor's LaTeX files say first, as comments: which fonts pdfLaTeX needs for them.
/// Without outline fonts it sets „ “ ß and the like from bitmaps, and text copied from the PDF has other characters in their place.
fn tex_note(lang: Lang) -> Vec<String> {
    let lines: [&str; 3] = match lang {
        Lang::De => [
            "% Mit pdfLaTeX: \\usepackage[T1]{fontenc} und \\usepackage{lmodern} in die Präambel (inputenc: utf8).",
            "% Ohne lmodern werden Anführungszeichen, ß und Umlaute beim Kopieren aus dem PDF zu anderen Zeichen.",
            "% XeLaTeX und LuaLaTeX brauchen beides nicht.",
        ],
        Lang::En => [
            "% With pdfLaTeX: put \\usepackage[T1]{fontenc} and \\usepackage{lmodern} into the preamble (inputenc: utf8).",
            "% Without lmodern, quotation marks and accented letters become other characters in text copied from the PDF.",
            "% XeLaTeX and LuaLaTeX need neither.",
        ],
    };
    lines.iter().map(|line| line.to_string()).collect()
}

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
            tool: tool_label(items, profile, lang),
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

    // The editor's register: when every row went through the same editor and provider, that is said
    // once, in the sentence before the table, and the column names the model only. Repeated in every row it made the
    // column unreadable. The plugin's register stays as it is.
    let served = match profile {
        Profile::Editor => {
            let all_served: BTreeSet<(String, String)> = groups.iter().map(|(_, items)| served_by(items, lang)).collect();
            (all_served.len() == 1).then(|| all_served.into_iter().next().unwrap())
        }
        Profile::Plugin => None,
    };
    if served.is_some() {
        for (row, (_, items)) in rows.iter_mut().zip(&groups) {
            row.tool = models_of(items);
        }
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
            // a passage from a chat with another tool, which the author kept with a statement
            let outside = found.extra.get("outside").and_then(|tool| tool.as_str());
            let asked = found.extra.get("asked").and_then(|asked| asked.as_str()).map_or(asked, |asked| py_squash(asked, Some(80)));
            let mut line = match (outside, de) {
                (Some(tool), true) => {
                    format!(
                        "{}: „{}“ ({how}, {} Wörter, {closeness}; aus einem abgelegten Chat mit {tool}, Antwort auf „{asked}“{})",
                        day(&found.ts),
                        found.excerpt,
                        found.words,
                        state.unwrap_or("")
                    )
                }
                (Some(tool), false) => {
                    format!(
                        "{}: “{}” ({how}, {} words, {closeness}; from a kept chat with {tool}, answer to “{asked}”{})",
                        day(&found.ts),
                        found.excerpt,
                        found.words,
                        state.unwrap_or("")
                    )
                }
                (None, true) => {
                    format!("{}: „{}“ ({how}, {} Wörter, {closeness}; aus der Antwort auf „{asked}“{})", day(&found.ts), found.excerpt, found.words, state.unwrap_or(""))
                }
                (None, false) => {
                    format!("{}: “{}” ({how}, {} words, {closeness}; from the answer to “{asked}”{})", day(&found.ts), found.excerpt, found.words, state.unwrap_or(""))
                }
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
        "The editor compares what is pasted or typed into the document with the AI's logged answers and lists matches here. Text from other AI tools is found only where the author kept the chat with a statement of outside use.",
        "Der Editor vergleicht, was in das Dokument eingefügt oder getippt wird, mit den protokollierten KI-Antworten und führt Übereinstimmungen hier auf. Text aus anderen KI-Werkzeugen wird nur gefunden, wo der Chat zu einer Angabe über die Nutzung außerhalb abgelegt wurde.",
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
    // who served the models: once here when it is the same for every row, else row by row in the table
    let served_sentence = match (&served, de) {
        (Some((editor, providers)), false) if providers.is_empty() => format!("The version used was {editor}; the models are named in the table."),
        (Some((editor, providers)), false) => format!("The version used was {editor}, and the models were served by {providers}; they are named in the table."),
        (Some((editor, providers)), true) if providers.is_empty() => format!("Eingesetzt wurde {editor}; die Modelle sind in der Tabelle genannt."),
        (Some((editor, providers)), true) => format!("Eingesetzt wurde {editor}, die Modelle stellte {providers} bereit; sie sind in der Tabelle genannt."),
        (None, false) => "The models and who served them are named in the table.".to_string(),
        (None, true) => "Die Modelle und ihre Anbieter sind in der Tabelle genannt.".to_string(),
    };
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
             explanations and literature support. {served_sentence} The manuscript text was written \
             by the author. The editor gives the AI no means of writing to the manuscript, records every write the AI attempted outside \
             its own notes (listed below), and logged all AI interactions automatically."
        ),
        (Profile::Editor, true) => format!(
            "Für die Arbeit „{name}“ wurde generative KI über den CoWriting Editor ausschließlich assistierend eingesetzt: für Hinweise, \
             Rückfragen, Erklärungen und Literaturhilfe. {served_sentence} Der Text des Manuskripts \
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
    let tool_head = match (served.is_some(), de) {
        (true, true) => "Modell",
        (true, false) => "Model",
        (false, true) => "Werkzeug / Modell",
        (false, false) => "Tool / model",
    };
    let head: [&str; 5] = if de { ["Zweck", tool_head, "Umfang", "Verwendung", "Kapitel"] } else { ["Purpose", tool_head, "Extent", "Use", "Chapters"] };

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
            if !stated.attachments.is_empty() {
                let kept: Vec<String> = stated.attachments.iter().map(|attachment| attachment_line(attachment, lang, false)).collect();
                line.push_str(&format!(" {} {}.", pick("Kept with it:", "Dazu abgelegt:"), kept.join("; ")));
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
    let log_head = match profile {
        Profile::Plugin => pick("Changes to the AI log undone after a tool call", "Nach einem Werkzeugaufruf rückgängig gemachte Änderungen am KI-Protokoll"),
        // shorter in German: the long heading ran past the margin in LaTeX
        Profile::Editor => pick("Changes to the AI log undone after a tool call", "Rückgängig gemachte Änderungen am KI-Protokoll"),
    };
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
            // set ragged in the editor's register: a justified heading of two lines overflows where a word cannot be hyphenated
            let sub = |heading: &str| match profile {
                Profile::Plugin => format!("\\subsection*{{{}}}", e(heading)),
                Profile::Editor => format!("\\subsection*{{\\raggedright {}}}", e(heading)),
            };
            if profile == Profile::Editor {
                out.extend(tex_note(lang));
            }
            out.extend([format!("\\section*{{{}}}", e(&title)), String::new(), e(&intro), String::new()]);

            // The plugin's table is 0.9 of the line plus the space between its columns: wider than the text block.
            // The editor's fills the line exactly, in smaller type, with ragged cells whose first word may be hyphenated.
            let (open, cell, row_end, close) = match profile {
                Profile::Plugin => {
                    ("\\begin{tabular}{p{.22\\linewidth}p{.18\\linewidth}p{.2\\linewidth}p{.2\\linewidth}p{.1\\linewidth}}".to_string(), "", " \\\\", "\\end{tabular}")
                }
                Profile::Editor => (
                    format!(
                        "{{\\footnotesize\\noindent\\begin{{tabular}}{{{}}}",
                        TEX_COLUMNS.map(|share| format!("p{{\\dimexpr{share}\\linewidth-2\\tabcolsep\\relax}}")).concat()
                    ),
                    "\\raggedright\\hspace{0pt}",
                    " \\tabularnewline",
                    "\\end{tabular}}",
                ),
            };
            out.extend([
                open,
                "\\hline".to_string(),
                format!("{}{row_end}", head.iter().map(|h| format!("{cell}\\textbf{{{}}}", e(h))).collect::<Vec<_>>().join(" & ")),
                "\\hline".to_string(),
            ]);
            for r in &rows {
                let purpose = format!("{}{}", r.purpose, if r.is_language { " (1)" } else { "" });
                let mut cells = [purpose.as_str(), &r.tool, &r.extent, &r.use_, &r.chapters].map(e);
                if profile == Profile::Editor {
                    cells[1] = breakable(&cells[1]);
                }
                out.push(format!("{}{row_end}", cells.map(|text| format!("{cell}{text}")).join(" & ")));
            }
            out.extend([
                "\\hline".to_string(),
                close.to_string(),
                String::new(),
                format!("\\noindent(1) {}", e(&lang_note)),
                String::new(),
                sub(&examples_head),
                "\\begin{itemize}".to_string(),
            ]);
            for r in &rows {
                out.extend(r.examples.iter().map(|example| format!("  \\item \\textit{{{}:}} {}", e(&r.purpose), e(example))));
            }
            out.extend(["\\end{itemize}".to_string(), sub(&ai_text_head)]);
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
                    out.extend([sub(&corrections_head), "\\begin{itemize}".to_string()]);
                    out.extend(corrections.iter().map(|line| format!("  \\item {}", e(line))));
                    out.push("\\end{itemize}".into());
                }
            }
            if !external.is_empty() {
                out.extend([sub(&external_head), "\\begin{itemize}".to_string()]);
                out.extend(external.iter().map(|line| format!("  \\item {}", e(line))));
                out.extend(["\\end{itemize}".to_string(), e(&external_note)]);
            }
            for (heading, lines, note) in [(&guard_head, &guard_lines, &guard_note), (&log_head, &log_lines, &log_note), (&refused_head, &refused_lines, &refused_note)] {
                out.push(sub(heading));
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
            if profile == Profile::Editor {
                out.extend(tex_note(lang));
            }
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

/// One kept file as the disclosure names it. The register has the beginning of the hash, the appendix all of it.
fn attachment_line(attachment: &Attachment, lang: Lang, whole_hash: bool) -> String {
    let de = lang == Lang::De;
    let hash = if whole_hash { attachment.sha256.clone() } else { format!("{}…", attachment.sha256.chars().take(12).collect::<String>()) };
    let name = if attachment.title.is_empty() { attachment.file.clone() } else { attachment.title.clone() };
    let from = if attachment.from.is_empty() { String::new() } else { format!("{}, ", crate::chats::form_label(&attachment.from, de)) };
    // a chat without a title is named by its file only
    let titled = match (attachment.title.is_empty(), de) {
        (true, _) => String::new(),
        (false, false) => format!(" “{name}”"),
        (false, true) => format!(" „{name}“"),
    };
    let exchanges = match (attachment.turns, de) {
        (1, false) => "1 exchange".to_string(),
        (n, false) => format!("{n} exchanges"),
        (1, true) => "1 Wechsel".to_string(),
        (n, true) => format!("{n} Wechsel"),
    };
    match (attachment.kind.as_str(), de) {
        ("chat", false) => format!("chat{titled} ({from}{exchanges}, file {}, SHA-256 {hash})", attachment.file),
        ("chat", true) => format!("Chat{titled} ({from}{exchanges}, Datei {}, SHA-256 {hash})", attachment.file),
        ("text", false) => format!("pasted text (file {}, SHA-256 {hash})", attachment.file),
        ("text", true) => format!("eingefügter Text (Datei {}, SHA-256 {hash})", attachment.file),
        (_, false) => format!("file “{name}” ({}, SHA-256 {hash})", attachment.file),
        (_, true) => format!("Datei „{name}“ ({}, SHA-256 {hash})", attachment.file),
    }
}

/// A kept file as it is found when the appendix is written.
#[derive(Debug, Clone, PartialEq)]
pub enum Kept {
    /// A conversation the editor read when it was kept.
    Chat(Chat),
    /// Pasted text, as it was.
    Text(String),
    /// Another file: named, not printed.
    File,
    /// The file is no longer what the log says it was.
    Changed,
    Missing,
}

/// The appendix of AI use outside the editor: every statement that has files kept with it, each
/// file by name and SHA-256, and the chats among them in full. `kept` gives a file as it is now.
/// None of this was seen by the editor, and the appendix says so. Editor profile only.
pub fn outside_appendix(entries: &[LogEntry], kept: &dyn Fn(&Attachment) -> Kept, name: &str, lang: Lang, format: DocFormat) -> String {
    let de = lang == Lang::De;
    let pick = |en: &str, german: &str| if de { german.to_string() } else { en.to_string() };
    let title = pick("Appendix: AI use outside the editor", "Anhang: KI-Nutzung außerhalb des Editors");
    let intro = if de {
        format!(
            "Chats und Dateien aus anderen KI-Werkzeugen zur Arbeit „{name}“, wie die Autorin oder der Autor sie abgelegt hat. Der Editor hat diese Nutzung nicht gesehen. \
             Die Prüfsumme (SHA-256) zeigt, dass eine Datei seit dem Ablegen unverändert ist, nicht, dass der Chat so stattgefunden hat. \
             Werkzeugaufrufe, Dateien und Bilder innerhalb eines Chats sind ausgelassen und gezählt."
        )
    } else {
        format!(
            "Chats and files from other AI tools for “{name}”, as the author kept them. The editor did not see this use. \
             The checksum (SHA-256) shows that a file is unchanged since it was kept, not that the chat took place as it says. \
             Tool calls, files and pictures inside a chat are left out and counted."
        )
    };
    let (asked_head, answer_head) = (pick("Asked", "Gefragt"), pick("Answered", "Geantwortet"));
    let changed = pick("This file was changed after it was kept; it is not printed.", "Diese Datei wurde nach dem Ablegen verändert; sie wird nicht abgedruckt.");
    let missing = pick("This file is no longer there.", "Diese Datei ist nicht mehr vorhanden.");
    let not_printed = pick("Kept as a file; not printed here.", "Als Datei abgelegt; hier nicht abgedruckt.");
    let about = |chat: &Chat| -> String {
        let mut parts: Vec<String> = Vec::new();
        if !chat.model.is_empty() {
            parts.push(format!("{} {}", pick("Model:", "Modell:"), chat.model));
        }
        if !chat.date.is_empty() {
            parts.push(format!("{} {}", pick("Begun:", "Begonnen:"), chat.date));
        }
        if chat.left_out() > 0 {
            parts.push(format!("{} {}", pick("Tool calls, files and pictures left out:", "Ausgelassene Werkzeugaufrufe, Dateien und Bilder:"), chat.left_out()));
        }
        if chat.other_branches > 0 {
            parts.push(format!("{} {}", pick("Messages on branches not continued, left out:", "Ausgelassene Nachrichten auf nicht fortgeführten Zweigen:"), chat.other_branches));
        }
        parts.join(" · ")
    };
    let or_dash = |text: &str| if text.trim().is_empty() { "–".to_string() } else { text.trim().to_string() };
    let quoted = |text: &str| -> Vec<String> { py_splitlines(&or_dash(text)).iter().map(|line| format!("> {line}")).collect() };
    let statements: Vec<ExternalEntry> = crate::types::external_uses(entries).into_iter().filter(|stated| !stated.attachments.is_empty()).collect();
    let heading = |stated: &ExternalEntry| {
        let model = if stated.model.trim().is_empty() { String::new() } else { format!(" ({})", stated.model.trim()) };
        let date = if stated.date.trim().is_empty() { String::new() } else { format!("{}: ", stated.date.trim()) };
        format!("{date}{}{model}", stated.tool.trim())
    };

    let mut out: Vec<String> = Vec::new();
    match format {
        DocFormat::Md => {
            out.extend([format!("# {title}"), String::new(), intro, String::new()]);
            for stated in &statements {
                out.extend([
                    format!("## {}", heading(stated)),
                    String::new(),
                    format!("{} {}", pick("Used for:", "Verwendet für:"), py_squash(&stated.purpose, None)),
                    String::new(),
                ]);
                for attachment in &stated.attachments {
                    out.extend([format!("### {}", attachment_line(attachment, lang, true)), String::new()]);
                    match kept(attachment) {
                        Kept::Chat(chat) => {
                            if !about(&chat).is_empty() {
                                out.extend([about(&chat), String::new()]);
                            }
                            for (number, turn) in chat.turns.iter().enumerate() {
                                let when = if turn.at.is_empty() { String::new() } else { format!(" · {}", minute(&turn.at)) };
                                out.extend([format!("**{}. {asked_head}{when}:**", number + 1), String::new()]);
                                out.extend(quoted(&turn.prompt));
                                out.extend([String::new(), format!("**{answer_head}:**"), String::new()]);
                                out.extend(quoted(&turn.answer));
                                out.push(String::new());
                            }
                        }
                        Kept::Text(text) => {
                            out.extend(quoted(&text));
                            out.push(String::new());
                        }
                        Kept::File => out.extend([not_printed.clone(), String::new()]),
                        Kept::Changed => out.extend([changed.clone(), String::new()]),
                        Kept::Missing => out.extend([missing.clone(), String::new()]),
                    }
                }
            }
        }
        DocFormat::Tex => {
            let e = tex_escape;
            out.extend(tex_note(lang));
            out.extend([format!("\\section*{{{}}}", e(&title)), String::new(), e(&intro), String::new()]);
            for stated in &statements {
                out.extend([
                    format!("\\subsection*{{{}}}", e(&heading(stated))),
                    format!("{} {}", e(&pick("Used for:", "Verwendet für:")), e(&py_squash(&stated.purpose, None))),
                    String::new(),
                ]);
                for attachment in &stated.attachments {
                    // a checksum has no place to break; the line is ragged so it need not be stretched
                    out.extend(["{\\raggedright\\sloppy".to_string(), format!("\\textbf{{{}}}\\par}}", breakable_hash(&e(&attachment_line(attachment, lang, true))))]);
                    match kept(attachment) {
                        Kept::Chat(chat) => {
                            if !about(&chat).is_empty() {
                                out.push(format!("{}\\par", e(&about(&chat))));
                            }
                            out.push("\\begin{enumerate}".into());
                            for turn in &chat.turns {
                                let when = if turn.at.is_empty() { String::new() } else { format!(" ({})", minute(&turn.at)) };
                                out.push(format!("  \\item \\textit{{{}{}:}} {}", e(&asked_head), e(&when), e(&py_squash(&or_dash(&turn.prompt), None))));
                                out.push(format!("  \\\\ \\textit{{{}:}} {}", e(&answer_head), e(&py_squash(&or_dash(&turn.answer), None))));
                            }
                            out.push("\\end{enumerate}".into());
                        }
                        Kept::Text(text) => out.extend(["\\begin{quote}".to_string(), e(&py_squash(&or_dash(&text), None)), "\\end{quote}".to_string()]),
                        Kept::File => out.push(e(&not_printed)),
                        Kept::Changed => out.push(e(&changed)),
                        Kept::Missing => out.push(e(&missing)),
                    }
                    out.push(String::new());
                }
            }
        }
    }
    out.join("\n")
}

/// A checksum may be broken after every eight characters.
fn breakable_hash(escaped: &str) -> String {
    let mut out = String::new();
    let mut run = 0;
    for c in escaped.chars() {
        run = if c.is_ascii_hexdigit() { run + 1 } else { 0 };
        out.push(c);
        if run > 0 && run % 8 == 0 {
            out.push_str("\\allowbreak{}");
        }
    }
    out
}
