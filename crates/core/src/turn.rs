//! One turn: the author asks, the model answers, and the log has both.
//!
//! Whatever backend serves the model, this is the only code that runs its tools and writes the
//! log, so every backend is guarded and logged alike. Ported in spirit from the plugin's
//! `prompt.submit` and `turn.complete` hooks in `plugin/hooks/register.tsx`.

use crate::backend::{Backend, Block, Message, ModelRequest, Role, Stop};
use crate::briefs::{brief_for, parse_briefs};
use crate::contract::contract_text;
use crate::host::Host;
use crate::log::{category_for, Log, AI_TEXT_MARKER};
use crate::personas::{Party, EXPERT_RULES};
use crate::sidecar::Sidecar;
use crate::thread::thread_of;
use crate::tools::{execute, numbered, tool_defs, ToolContext};
use crate::types::{AnnotationEntry, GuardEntry, LogEntry, PromptEntry, ResponseEntry};
use std::io;
use std::sync::Arc;

/// How often the model may ask for tools in one turn before it has to answer.
const MAX_ROUNDS: usize = 12;

/// One open document: its sidecar, its log, and what the log records about where it was written.
pub struct Workspace {
    pub host: Arc<dyn Host>,
    pub sidecar: Sidecar,
    pub log: Log,
    /// One id per run of the app.
    pub session: String,
    /// The editor's version, as the log records it.
    pub editor: String,
    /// The manuscript's language: 'en' or 'de'.
    pub language: String,
}

impl Workspace {
    pub fn open(host: Arc<dyn Host>, document: &str, editor: &str) -> Workspace {
        let sidecar = Sidecar::for_document(document);
        let log = Log::new(host.clone(), sidecar.log_dir(), sidecar.rewrites_dir());
        let language = sidecar.info(host.as_ref()).map_or("en".to_string(), |info| info.language);
        Workspace { session: host.new_id(), host, sidecar, log, editor: editor.to_string(), language }
    }
}

impl Workspace {
    /// The document's party as its sidecar has it now.
    pub fn party(&self) -> Party {
        Party::load(self.host.as_ref(), &self.sidecar)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ask {
    /// What the author typed.
    pub text: String,
    /// The action that started the turn ('feedback', 'socratic', …), if one did.
    pub command: Option<String>,
    /// The logged turn this one replies to.
    pub reply_to: Option<String>,
    /// The expert who is asked; None for the assistant itself.
    pub persona: Option<String>,
    /// The document as the editor holds it now.
    pub document_text: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TurnEvent {
    /// The turn is in the log under this id.
    Started {
        id: String,
    },
    Text(String),
    Tool {
        name: String,
    },
    /// The guard refused a call.
    Refused {
        tool: String,
        input: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    pub id: String,
    pub answer: String,
    pub ai_text_flag: bool,
    pub tools_used: Vec<String>,
    /// Calls the guard refused.
    pub refused: usize,
    /// Why the answer is incomplete, when it is.
    pub cut_off: Option<String>,
    /// The model cannot call tools: it was sent the document with the question, and can write no note.
    pub without_tools: bool,
}

#[derive(Debug)]
pub enum TurnError {
    /// The log or the sidecar could not be written: nothing was sent to the model.
    Io(io::Error),
    /// The model gave no answer at all. The question is in the log; there is no turn.
    Backend(String),
    /// The question cannot be asked as it is; nothing was logged or sent.
    Invalid(String),
}

impl std::fmt::Display for TurnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurnError::Io(error) => write!(f, "The AI log could not be written, so nothing was sent: {error}"),
            TurnError::Backend(message) | TurnError::Invalid(message) => f.write_str(message),
        }
    }
}

impl From<io::Error> for TurnError {
    fn from(error: io::Error) -> Self {
        TurnError::Io(error)
    }
}

/// The messages a reply continues: the thread so far, as the model's and the author's turns.
fn history(entries: &[LogEntry], reply_to: &str, party: &Party) -> Vec<Message> {
    let thread = thread_of(entries, reply_to, party, "Assistant");
    let Some(first) = thread.first() else { return Vec::new() };
    let asked = entries.iter().find_map(|e| match e {
        LogEntry::Prompt(p) if p.id == first.id => Some(p.text.clone()),
        _ => None,
    });
    let mut out = vec![Message::user(asked.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "(the editor asked for a review)".into()))];
    for message in &thread {
        let role = if message.role == "author" { Role::User } else { Role::Assistant };
        match out.last_mut() {
            // two messages of one side in a row are one message to the model
            Some(Message { role: last, content }) if *last == role => content.push(Block::Text { text: message.text.clone() }),
            _ => out.push(Message { role, content: vec![Block::Text { text: message.text.clone() }] }),
        }
    }
    out
}

/// What a model without tools is told instead: the rules, and the document itself.
fn system_without_tools(contract: &str, document_text: &str) -> String {
    format!(
        "{contract}\n- You have no tools in this conversation. The document as it is now follows, with line numbers added; you cannot write notes.\n\n# The document\n\n{}",
        numbered(document_text)
    )
}

pub async fn run_turn(workspace: &Workspace, backend: &dyn Backend, ask: Ask, on_event: &mut (dyn FnMut(TurnEvent) + Send)) -> Result<TurnOutcome, TurnError> {
    let host = workspace.host.as_ref();
    let party = workspace.party();
    let expert = match ask.persona.as_deref() {
        Some(id) => Some(party.by_id(id).cloned().ok_or_else(|| TurnError::Invalid(format!("This document's party has no expert “{id}”.")))?),
        None => None,
    };
    workspace.sidecar.ensure(host, &ask.document_text)?;
    let before = workspace.log.snapshot()?;
    let earlier = workspace.log.read()?;

    // the question is on the record, with the text it was asked about, before anything is sent
    let id = host.new_id();
    workspace.log.append(&LogEntry::Prompt(PromptEntry {
        id: id.clone(),
        ts: host.now(),
        session: workspace.session.clone(),
        paper: workspace.sidecar.name.clone(),
        category: expert.as_ref().map_or_else(|| category_for(ask.command.as_deref(), &ask.text, &party), |e| e.category.clone()),
        command: ask.command.clone(),
        reply_to: ask.reply_to.clone(),
        text: ask.text.clone(),
        ..Default::default()
    }))?;
    host.write(&format!("{}/{id}.md", workspace.sidecar.snapshots_dir()), &ask.document_text)?;
    on_event(TurnEvent::Started { id: id.clone() });

    let mut messages = ask.reply_to.as_deref().map(|to| history(&earlier, to, &party)).unwrap_or_default();
    match messages.last_mut() {
        Some(Message { role: Role::User, content }) => content.push(Block::Text { text: ask.text.clone() }),
        _ => messages.push(Message::user(ask.text.clone())),
    }
    let mut contract = contract_text(&workspace.sidecar.name, &workspace.language);
    if let Some(expert) = &expert {
        // one call per expert: the contract, its perspective, the party's rules, the author's briefing
        let briefs = parse_briefs(&host.read(&workspace.sidecar.party_md()).unwrap_or_default(), &party);
        let brief = brief_for(&briefs, &expert.id);
        contract = format!("{contract}\n\n# Your role in this conversation\n\n{}\n\n{EXPERT_RULES}", expert.prompt);
        if !brief.is_empty() {
            contract.push_str(&format!("\n\n## The author's briefing for you in this project\n\n{brief}"));
        }
    }
    let mut without_tools = !backend.uses_tools();
    let mut request = if without_tools {
        ModelRequest { system: system_without_tools(&contract, &ask.document_text), messages, tools: Vec::new(), model: ask.model.clone() }
    } else {
        ModelRequest { system: contract.clone(), messages, tools: tool_defs(), model: ask.model.clone() }
    };
    let context = ToolContext { host, sidecar: &workspace.sidecar, document_text: &ask.document_text, model: &ask.model };

    let mut answer: Vec<String> = Vec::new();
    let mut tools_used: Vec<String> = Vec::new();
    let mut refused = 0;
    let mut cut_off: Option<String> = None;
    let mut answered_by = ask.model.clone();
    for round in 0.. {
        let mut streamed = String::new();
        let reply = backend
            .complete(&request, &mut |text: &str| {
                streamed.push_str(text);
                on_event(TurnEvent::Text(text.to_string()));
            })
            .await;
        let reply = match reply {
            Ok(reply) => reply,
            // the backend just learned that its model has no tools: ask again, with the document in the question
            Err(_) if !without_tools && !backend.uses_tools() && round == 0 => {
                without_tools = true;
                request.system = system_without_tools(&contract, &ask.document_text);
                request.tools.clear();
                continue;
            }
            Err(error) => {
                // what the author already read stays on the record
                if !streamed.trim().is_empty() {
                    answer.push(streamed);
                }
                cut_off = Some(error.message);
                break;
            }
        };
        let said: String = reply.blocks.iter().filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None }).collect();
        if !said.trim().is_empty() {
            answer.push(said);
        }
        let calls: Vec<(&String, &String, &serde_json::Value)> =
            reply.blocks.iter().filter_map(|b| if let Block::ToolUse { id, name, input } = b { Some((id, name, input)) } else { None }).collect();
        if let Some(model) = &reply.model {
            answered_by = model.clone();
        }
        // a call cut off by the length limit or a refusal is never run: its input may be half of what was meant
        match reply.stop {
            Stop::MaxTokens => cut_off = Some("The answer reached the length limit.".into()),
            Stop::Refusal => cut_off = Some("The provider declined to answer.".into()),
            _ => {}
        }
        if calls.is_empty() || cut_off.is_some() {
            break;
        }
        if round + 1 >= MAX_ROUNDS {
            cut_off = Some("The model kept asking for tools instead of answering.".into());
            break;
        }
        let mut results = Vec::new();
        for (call, name, input) in calls {
            if !tools_used.contains(name) {
                tools_used.push(name.clone());
            }
            on_event(TurnEvent::Tool { name: name.clone() });
            let result = execute(&context, name, input);
            if let Some(attempt) = &result.refused {
                refused += 1;
                workspace.log.append(&LogEntry::Guard(GuardEntry {
                    ts: host.now(),
                    session: workspace.session.clone(),
                    paper: workspace.sidecar.name.clone(),
                    reference: Some(id.clone()),
                    tool: name.clone(),
                    input: attempt.clone(),
                    files: Vec::new(),
                    refused: true,
                    ..Default::default()
                }))?;
                on_event(TurnEvent::Refused { tool: name.clone(), input: attempt.clone() });
            }
            results.push(Block::ToolResult { id: call.clone(), content: result.content, is_error: result.is_error });
        }
        request.messages.push(Message { role: Role::Assistant, content: reply.blocks.clone() });
        request.messages.push(Message { role: Role::User, content: results });
    }

    // a backend that runs its own process could have touched the log: what it changed goes back, on the record
    let restored = workspace.log.restore(&before, &workspace.log.snapshot()?)?;
    if !restored.is_empty() {
        workspace.log.append(&LogEntry::Guard(GuardEntry {
            ts: host.now(),
            session: workspace.session.clone(),
            paper: workspace.sidecar.name.clone(),
            reference: Some(id.clone()),
            tool: backend.id().to_string(),
            input: "the AI log was changed during the turn".into(),
            files: restored.iter().map(|file| format!("{}/ai-log/{file}", workspace.sidecar.relative())).collect(),
            ..Default::default()
        }))?;
    }

    if answer.is_empty() {
        return Err(TurnError::Backend(cut_off.unwrap_or_else(|| "The model gave no answer.".into())));
    }
    let mut text = answer.join("\n\n");
    if let Some(reason) = &cut_off {
        text.push_str(&format!("\n\n[The answer is incomplete: {reason}]"));
    }
    let ai_text_flag = text.contains(AI_TEXT_MARKER);
    workspace.log.append(&LogEntry::Response(ResponseEntry {
        reference: id.clone(),
        ts: host.now(),
        model: Some(answered_by),
        editor: Some(workspace.editor.clone()),
        backend: Some(backend.id().to_string()),
        provider: Some(backend.provider()),
        personas: expert.iter().map(|e| e.id.clone()).collect(),
        tools_used: tools_used.clone(),
        ai_text_flag,
        answer: text.clone(),
        ..Default::default()
    }))?;
    Ok(TurnOutcome { id, answer: text, ai_text_flag, tools_used, refused, cut_off, without_tools })
}

pub const USES: [&str; 4] = ["adopted", "adapted", "rejected", "info"];

/// The author's classification of a logged turn, with an optional comment.
pub fn annotate(workspace: &Workspace, reference: &str, use_: &str, note: &str) -> Result<(), String> {
    if !USES.contains(&use_) {
        return Err(format!("{use_} is not one of {}.", USES.join(", ")));
    }
    let entries = workspace.log.read().map_err(|e| e.to_string())?;
    if !entries.iter().any(|e| matches!(e, LogEntry::Response(r) if r.reference == reference)) {
        return Err(format!("The log has no turn {reference}."));
    }
    let note = note.trim();
    workspace
        .log
        .append(&LogEntry::Annotation(AnnotationEntry {
            reference: reference.to_string(),
            ts: workspace.host.now(),
            use_: use_.to_string(),
            note: (!note.is_empty()).then(|| note.to_string()),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())
}
