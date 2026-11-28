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

/// The tools for a document: those every document has, and for one file of a project of several
/// the tools that read the project's other files. They only read: no tool takes a path, a file is
/// named as the project's own list names it, and nothing but that list's files is reached.
pub fn tool_defs_for(sidecar: &Sidecar) -> Vec<ToolDef> {
    let mut tools = tool_defs();
    if sidecar.shared.is_some() {
        let no_input = json!({ "type": "object", "properties": {}, "additionalProperties": false });
        tools.extend([
            ToolDef {
                name: "list_project_files".into(),
                description: "The files of the author's project in their order, with their length in words; this document is one of them and is marked. Use it to see what else there is before you answer a question that reaches beyond this document.".into(),
                input_schema: no_input,
            },
            ToolDef {
                name: "read_project_file".into(),
                description: "Another file of the project, by its name from list_project_files, every line with its number. Read only what the question needs: a file can be long. You cannot change it.".into(),
                input_schema: json!({ "type": "object", "properties": { "name": { "type": "string", "description": "The file's name exactly as list_project_files gives it" } }, "required": ["name"], "additionalProperties": false }),
            },
            ToolDef {
                name: "search_project".into(),
                description: "The lines in all files of the project that contain a word or phrase, by file, with their numbers. Case is ignored. Use it to check that a term, a number or a claim is the same across chapters.".into(),
                input_schema: json!({ "type": "object", "properties": { "query": { "type": "string", "description": "The word or phrase to look for" } }, "required": ["query"], "additionalProperties": false }),
            },
        ]);
    }
    tools
}

/// The files of the project this document is one of: each by the name the project's file gives
/// it, with its text. The open document is given as the editor holds it; a file that is not there
/// has no text. The last part says whether it is the open document.
pub fn project_files(context: &ToolContext) -> Option<Vec<(String, Option<String>, bool)>> {
    let shared = context.sidecar.shared.as_ref()?;
    let project = crate::project_file::ProjectFile::load(context.host, &shared.file)?;
    let folder = Sidecar::for_document(&shared.file).folder;
    let plain = |path: &str| path.replace('\\', "/");
    let open = plain(&context.sidecar.document);
    Some(
        project
            .files
            .into_iter()
            .map(|name| {
                let path = format!("{}/{name}", folder.trim_end_matches(['/', '\\']));
                let is_open = plain(&path) == open;
                let text = if is_open { Some(context.document_text.to_string()) } else { context.host.read(&path).ok() };
                (name, text, is_open)
            })
            .collect(),
    )
}

/// What the AI is told about the project in its instructions, for a file of a project: nothing for a document on its own.
pub fn project_section(context: &ToolContext) -> String {
    let (Some(shared), Some(files)) = (context.sidecar.shared.as_ref(), project_files(context)) else { return String::new() };
    if files.len() < 2 {
        return String::new();
    }
    let names: Vec<&str> = files.iter().map(|(name, _, _)| name.as_str()).collect();
    format!(
        "\n\n# The project\n\nThis document, {}, is one file of the author's project “{}”. Its files, in their order: {}. \
         You are asked about this document. When a question reaches beyond it (a term used elsewhere, what another chapter says, whether the parts agree), \
         look with list_project_files, search_project and read_project_file, and say which file a remark is about. \
         The other files are the author's too: you cannot change them, and you do not write text for them.",
        context.sidecar.name,
        crate::project_file::ProjectFile::name_of(&shared.file),
        names.join(", ")
    )
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
        "list_project_files" | "read_project_file" | "search_project" => {
            let Some(files) = project_files(context) else { return ToolResult::failed("This document is not a file of a project of several files.") };
            match name {
                "list_project_files" => ToolResult::ok(
                    files
                        .iter()
                        .map(|(name, text, is_open)| match text {
                            Some(text) => format!(
                                "{name} ({}){}",
                                crate::text::plural(crate::words::count_words(text, crate::words::Format::Markdown), "word"),
                                if *is_open { ", this document" } else { "" }
                            ),
                            None => format!("{name} (not found where the project names it)"),
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                "read_project_file" => match files.iter().find(|(file, _, _)| *file == text_of("name").trim()) {
                    Some((_, Some(text), _)) => ToolResult::ok(numbered(text)),
                    Some((file, None, _)) => ToolResult::failed(format!("{file} is not where the project names it.")),
                    None => ToolResult::failed("The project has no file of that name. list_project_files gives the names."),
                },
                _ => {
                    let query = text_of("query").to_lowercase();
                    if query.trim().is_empty() {
                        return ToolResult::failed("search_project needs a query.");
                    }
                    // a word that is everywhere would be the whole project: the first lines of each file, and how many more there are
                    const MOST: usize = 40;
                    let mut found = Vec::new();
                    for (file, text) in files.iter().filter_map(|(file, text, _)| text.as_ref().map(|text| (file, text))) {
                        let hits: Vec<String> =
                            text.split('\n').enumerate().filter(|(_, line)| line.to_lowercase().contains(&query)).map(|(i, line)| format!("{}| {line}", i + 1)).collect();
                        if !hits.is_empty() {
                            let more = hits.len().saturating_sub(MOST);
                            found.push(format!(
                                "# {file}\n{}{}",
                                hits[..hits.len().min(MOST)].join("\n"),
                                if more > 0 { format!("\n… and {more} more lines") } else { String::new() }
                            ));
                        }
                    }
                    ToolResult::ok(if found.is_empty() { "No line in the project contains that.".to_string() } else { found.join("\n\n") })
                }
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
