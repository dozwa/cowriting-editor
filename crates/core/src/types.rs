//! The AI log's entries. The shapes are the plugin's (`plugin/types/index.d.ts`): a log written by
//! either reads in both. Fields the editor adds are optional, and fields this version does not
//! know are kept as they were read.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

fn general() -> String {
    "general".to_string()
}

fn unknown_tool() -> String {
    "?".to_string()
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PromptEntry {
    pub id: String,
    pub ts: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub paper: String,
    #[serde(default = "general")]
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// 'agent', 'watch', 'scope', 'summary' or 'brief': a turn the author did not type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// The persona whose turn this is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The entry this turn answers; a thread is the chain.
    #[serde(default, rename = "replyTo", skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResponseEntry {
    #[serde(rename = "ref")]
    pub reference: String,
    pub ts: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The CoWriting plugin version that logged the turn (plugin logs only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    /// The Claude Code version the session ran on (plugin logs only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<String>,
    /// The CoWriting Editor version that logged the turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// How the editor reached the model: 'anthropic', 'openai-compatible', 'claude-code', 'fake'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Who served the model, as the author configured it: 'Anthropic', a host name, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default)]
    pub personas: Vec<String>,
    /// Subagents this turn launched as personas: agent id → persona id (plugin logs only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub tools_used: Vec<String>,
    #[serde(default)]
    pub ai_text_flag: bool,
    #[serde(default)]
    pub answer: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AnnotationEntry {
    #[serde(rename = "ref")]
    pub reference: String,
    pub ts: String,
    /// 'adopted', 'adapted', 'rejected' or 'info'.
    #[serde(rename = "use")]
    pub use_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A write the guard refused, or a change to the author's files it found after a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GuardEntry {
    pub ts: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub paper: String,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default = "unknown_tool")]
    pub tool: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub refused: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Known {
    Prompt(PromptEntry),
    Response(ResponseEntry),
    Annotation(AnnotationEntry),
    Guard(GuardEntry),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogEntry {
    Prompt(PromptEntry),
    Response(ResponseEntry),
    Annotation(AnnotationEntry),
    Guard(GuardEntry),
    /// An entry of a type this version does not know, or one it knows but cannot read: kept, not used.
    Other(Value),
}

impl LogEntry {
    /// One parsed log line; `None` when it is no object with a `type`.
    pub fn from_value(value: Value) -> Option<LogEntry> {
        if !value.as_object().is_some_and(|o| o.contains_key("type")) {
            return None;
        }
        Some(match serde_json::from_value::<Known>(value.clone()) {
            Ok(Known::Prompt(e)) => LogEntry::Prompt(e),
            Ok(Known::Response(e)) => LogEntry::Response(e),
            Ok(Known::Annotation(e)) => LogEntry::Annotation(e),
            Ok(Known::Guard(e)) => LogEntry::Guard(e),
            Err(_) => LogEntry::Other(value),
        })
    }

    /// The entry as one log line, without the newline.
    pub fn to_line(&self) -> String {
        let known = match self {
            LogEntry::Prompt(e) => Known::Prompt(e.clone()),
            LogEntry::Response(e) => Known::Response(e.clone()),
            LogEntry::Annotation(e) => Known::Annotation(e.clone()),
            LogEntry::Guard(e) => Known::Guard(e.clone()),
            LogEntry::Other(v) => return v.to_string(),
        };
        serde_json::to_string(&known).expect("a log entry is plain data")
    }
}

/// One turn as the Log tab lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecentTurn {
    pub id: String,
    pub ts: String,
    pub category: String,
    /// What the author asked, or what the turn was.
    pub preview: String,
    /// What the AI said, flattened.
    pub answer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Waits for the author to classify it.
    pub is_open: bool,
    pub personas: Vec<String>,
    pub is_annotated: bool,
    pub is_ai_text: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub turns: usize,
    pub today: usize,
    pub unannotated: usize,
    pub ai_text: usize,
    pub by_category: BTreeMap<String, usize>,
    pub persona_consults: BTreeMap<String, usize>,
    pub persona_adopted: BTreeMap<String, usize>,
    pub annotations: usize,
    /// AI inputs the author decided against.
    pub rejected: usize,
    /// Changes to the author's files found during AI tool calls; refusals are not counted.
    pub guard_events: usize,
    /// The six newest turns, newest first.
    pub recent: Vec<RecentTurn>,
    /// Turns waiting to be classified, newest first, at most five.
    pub open: Vec<RecentTurn>,
    pub open_by_persona: BTreeMap<String, usize>,
}

/// One message of a feedback thread: the feedback, the author's replies, the answers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThreadMessage {
    pub id: String,
    /// 'ai' or 'author'.
    pub role: String,
    pub who: String,
    /// The persona's id; '' for the author and for the assistant itself.
    pub persona: String,
    pub ts: String,
    pub text: String,
}
