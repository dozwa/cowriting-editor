//! The tools the AI is offered, the same for every backend, and the one place they run.
//! None of them can write the document: the only writer is `write_note`, and the guard confines
//! it to the sidecar's notes. A call the guard refuses, and a call to a tool that is not offered,
//! is recorded in the log as a refused attempt.

use crate::backend::ToolDef;
use crate::guard::{guard_input, judge_write, Verdict};
use crate::host::Host;
use crate::sidecar::Sidecar;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
    /// Set when the guard refused the call: what the log records as the attempt.
    pub refused: Option<String>,
}

impl ToolResult {
    fn ok(content: impl Into<String>) -> ToolResult {
        ToolResult { content: content.into(), is_error: false, refused: None }
    }

    fn failed(content: impl Into<String>) -> ToolResult {
        ToolResult { content: content.into(), is_error: true, refused: None }
    }

    fn refused(content: impl Into<String>, attempt: &str) -> ToolResult {
        ToolResult { content: content.into(), is_error: true, refused: Some(guard_input(attempt)) }
    }
}

pub const NOTE_REFUSAL: &str = "CoWriting: you write only notes, inside the notes folder. The document and every other file belong to the author \
     (DFG good scientific practice: only natural persons are authors). Give located feedback, questions or corrected example sentences in your answer instead; \
     the author decides what goes into the text.";

pub fn tool_defs() -> Vec<ToolDef> {
    let no_input = json!({ "type": "object", "properties": {}, "additionalProperties": false });
    vec![
        ToolDef {
            name: "read_document".into(),
            description: "The author's document as it is now, every line with its number (`12| text`). Read it before commenting on the text.".into(),
            input_schema: no_input.clone(),
        },
        ToolDef {
            name: "search_document".into(),
            description: "The lines of the document that contain a word or phrase, with their numbers. Case is ignored.".into(),
            input_schema: json!({ "type": "object", "properties": { "query": { "type": "string", "description": "The word or phrase to look for" } }, "required": ["query"], "additionalProperties": false }),
        },
        ToolDef { name: "list_notes".into(), description: "The names of the AI notes kept beside this document.".into(), input_schema: no_input },
        ToolDef {
            name: "read_note".into(),
            description: "One AI note, by its name from list_notes.".into(),
            input_schema: json!({ "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"], "additionalProperties": false }),
        },
        ToolDef {
            name: "write_note".into(),
            description: "Save a note for the author beside the document: an overview, a checklist, a glossary entry. Never manuscript text. The editor marks it as AI-generated. An existing note of the same name is replaced.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "A file name such as glossary.md; no folders" },
                    "text": { "type": "string", "description": "The note, in Markdown" }
                },
                "required": ["name", "text"],
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "search_bibliography".into(),
            description: "The author's own references (their bibliography files) that contain every word of the query: citekey, authors, year, title, DOI. Use it before naming a source: a reference that is not in here and that you did not verify is UNVERIFIED. An empty query lists the first entries.".into(),
            input_schema: json!({ "type": "object", "properties": { "query": { "type": "string", "description": "Words from the title, an author's name, a year" } }, "required": ["query"], "additionalProperties": false }),
        },
    ]
}

/// The document with line numbers, as the AI reads it.
pub fn numbered(text: &str) -> String {
    text.split('\n').enumerate().map(|(i, line)| format!("{}| {line}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// What one turn's tools work on.
pub struct ToolContext<'a> {
    pub host: &'a dyn Host,
    pub sidecar: &'a Sidecar,
    /// The document as the editor holds it, saved or not.
    pub document_text: &'a str,
    /// The model the turn runs on, for a note's header.
    pub model: &'a str,
}

pub fn execute(context: &ToolContext, name: &str, input: &Value) -> ToolResult {
    let text_of = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let notes = context.sidecar.notes_dir();
    match name {
        "read_document" => ToolResult::ok(numbered(context.document_text)),
        "search_document" => {
            let query = text_of("query").to_lowercase();
            if query.trim().is_empty() {
                return ToolResult::failed("search_document needs a query.");
            }
            let hits: Vec<String> =
                context.document_text.split('\n').enumerate().filter(|(_, line)| line.to_lowercase().contains(&query)).map(|(i, line)| format!("{}| {line}", i + 1)).collect();
            ToolResult::ok(if hits.is_empty() { "No line contains that.".to_string() } else { hits.join("\n") })
        }
        "list_notes" => {
            let mut names = if context.host.exists(&notes) { context.host.list(&notes).unwrap_or_default() } else { Vec::new() };
            names.sort();
            ToolResult::ok(if names.is_empty() { "There are no notes yet.".to_string() } else { names.join("\n") })
        }
        "read_note" => {
            let path = format!("{notes}/{}", text_of("name"));
            // reading is confined like writing: a note's name never leads out of the notes
            if judge_write(&path, &context.sidecar.layout()) != Verdict::Allowed {
                return ToolResult::failed("There is no note of that name.");
            }
            match context.host.read(&path) {
                Ok(text) => ToolResult::ok(text),
                Err(_) => ToolResult::failed("There is no note of that name."),
            }
        }
        "search_bibliography" => {
            let bibliography = crate::sources::load_bibliography(context.host, context.sidecar, context.document_text);
            if bibliography.files.is_empty() {
                return ToolResult::ok(
                    "The document has no bibliography file: its front matter names none (bibliography: refs.bib) and the author added none in the editor's Sources tab.",
                );
            }
            let hits = crate::sources::search(&bibliography.entries, &text_of("query"), 15);
            let files = bibliography.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
            ToolResult::ok(if hits.is_empty() {
                format!("No entry in {files} contains that.")
            } else {
                format!("# {} from {files}\n{}", crate::text::plural(hits.len(), "hit"), crate::sources::hits_as_text(&hits))
            })
        }
        "write_note" => {
            let (note, text) = (text_of("name"), text_of("text"));
            let path = format!("{notes}/{note}");
            if note.trim().is_empty() || judge_write(&path, &context.sidecar.layout()) != Verdict::Allowed {
                return ToolResult::refused(NOTE_REFUSAL, &format!("write_note {note}"));
            }
            let day = context.host.now().chars().take(10).collect::<String>();
            let header = format!("<!-- AI-generated ({}), {day}, not manuscript text -->\n\n", context.model);
            match context.host.write(&path, &format!("{header}{text}")) {
                Ok(()) => ToolResult::ok(format!("Saved the note {note}.")),
                Err(error) => ToolResult::failed(format!("The note could not be saved: {error}")),
            }
        }
        // a tool that was never offered: whatever it was meant to do, it does nothing
        other => ToolResult::refused(format!("There is no tool called {other}. {NOTE_REFUSAL}"), &format!("{other} {input}")),
    }
}
