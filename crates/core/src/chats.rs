//! Chats with AI tools outside the editor, as the author hands them in: an export of ChatGPT or
//! Claude.ai, a transcript of Claude Code, or a chat pasted as text. Each is read into one plain
//! form, what was asked and what was answered, and only that is kept beside the document. Tool
//! calls, files and pictures inside a chat are left out and counted. The editor did not see any
//! of this happen; it is the author's file. Nothing here is the plugin's: the plugin has no such thing.

use crate::host::iso_utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::{Duration, UNIX_EPOCH};

/// The word a kept chat names itself by.
pub const FORMAT: &str = "cowriting-chat";

/// One exchange: what the author asked, and what the tool answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatTurn {
    /// When it was asked, as the export says; empty when it does not.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub at: String,
    pub prompt: String,
    pub answer: String,
    /// Tool calls, files and pictures in this exchange that are not kept.
    #[serde(skip_serializing_if = "is_zero")]
    pub left_out: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// One conversation, in the form the editor keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Chat {
    /// Always `FORMAT`.
    pub format: String,
    pub version: u32,
    /// The tool, as the export shows it ("ChatGPT"); empty for pasted text.
    pub tool: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// The day it began (YYYY-MM-DD), when the export says.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub date: String,
    /// The file it was taken from, by name, and that file's SHA-256. The file itself is not kept.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub from: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub from_sha256: String,
    /// Messages on branches the author did not continue (regenerated answers, edited questions); not kept.
    #[serde(skip_serializing_if = "is_zero")]
    pub other_branches: u32,
    pub turns: Vec<ChatTurn>,
}

impl Chat {
    fn new(tool: &str) -> Chat {
        Chat { format: FORMAT.into(), version: 1, tool: tool.into(), ..Default::default() }
    }

    /// What the chat leaves out, over all its exchanges.
    pub fn left_out(&self) -> u32 {
        self.turns.iter().map(|turn| turn.left_out).sum()
    }

    /// A kept chat, read back. None for anything else.
    pub fn kept(text: &str) -> Option<Chat> {
        serde_json::from_str::<Chat>(text).ok().filter(|chat| chat.format == FORMAT)
    }

    /// The chat as the file that is kept.
    pub fn to_file(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }
}

enum Said {
    Author(String),
    Tool(String),
    /// Something that is not text: a tool call, a file, a picture.
    Other,
}

/// Messages in their order as exchanges: each begins with what the author asked.
fn exchanges(messages: Vec<(Said, String)>) -> Vec<ChatTurn> {
    let mut turns: Vec<ChatTurn> = Vec::new();
    for (said, at) in messages {
        match said {
            Said::Author(text) => turns.push(ChatTurn { at, prompt: text, ..Default::default() }),
            Said::Tool(text) => match turns.last_mut() {
                Some(turn) if turn.answer.is_empty() => turn.answer = text,
                Some(turn) => turn.answer = format!("{}\n\n{text}", turn.answer),
                None => turns.push(ChatTurn { at, answer: text, ..Default::default() }),
            },
            Said::Other => match turns.last_mut() {
                Some(turn) => turn.left_out += 1,
                None => turns.push(ChatTurn { at, left_out: 1, ..Default::default() }),
            },
        }
    }
    turns.retain(|turn| !turn.prompt.is_empty() || !turn.answer.is_empty());
    turns
}

fn day(at: &str) -> String {
    at.get(..10).filter(|d| d.as_bytes().get(4) == Some(&b'-')).unwrap_or("").to_string()
}

fn text_of(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").trim().to_string()
}

/// Seconds since 1970, as ChatGPT writes its times.
fn from_seconds(value: &Value) -> String {
    value.as_f64().filter(|s| *s > 0.0).map(|s| iso_utc(UNIX_EPOCH + Duration::from_secs_f64(s))).unwrap_or_default()
}

/// A conversation of ChatGPT's data export (`conversations.json`): a tree of messages. Kept is the
/// branch that ends in `current_node`, the one the author went on with.
fn chatgpt(conversation: &Value) -> Option<Chat> {
    let mapping = conversation["mapping"].as_object()?;
    let mut path: Vec<&str> = Vec::new();
    let mut at = conversation["current_node"].as_str().or_else(|| {
        // no current node: the last leaf
        mapping.iter().filter(|(_, node)| node["children"].as_array().is_none_or(Vec::is_empty)).map(|(id, _)| id.as_str()).next_back()
    });
    while let Some(id) = at.filter(|id| !path.contains(id)) {
        path.push(id);
        at = mapping.get(id).and_then(|node| node["parent"].as_str());
    }
    path.reverse();
    let role_of = |node: &Value| node["message"]["author"]["role"].as_str().unwrap_or("").to_string();
    let spoken = |node: &Value| matches!(role_of(node).as_str(), "user" | "assistant");
    let mut chat = Chat::new("ChatGPT");
    chat.other_branches = mapping.iter().filter(|(id, node)| spoken(node) && !path.contains(&id.as_str())).count() as u32;
    let mut messages = Vec::new();
    for id in path {
        let message = &mapping[id]["message"];
        let role = message["author"]["role"].as_str().unwrap_or("");
        let when = from_seconds(&message["create_time"]);
        if let Some(model) = message["metadata"]["model_slug"].as_str().filter(|m| !m.is_empty()) {
            chat.model = model.to_string();
        }
        if role == "tool" {
            messages.push((Said::Other, when));
            continue;
        }
        if role != "user" && role != "assistant" {
            continue;
        }
        let parts = message["content"]["parts"].as_array().cloned().unwrap_or_default();
        let is_text = message["content"]["content_type"].as_str().is_none_or(|kind| kind == "text" || kind == "multimodal_text");
        let text = parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n").trim().to_string();
        // the text first, so that pictures and files among the parts, and code for a tool, count for this exchange
        if is_text && !text.is_empty() {
            messages.push((if role == "user" { Said::Author(text) } else { Said::Tool(text) }, when.clone()));
        }
        for _ in 0..parts.iter().filter(|part| !part.is_string()).count().max(usize::from(!is_text)) {
            messages.push((Said::Other, when.clone()));
        }
    }
    chat.turns = exchanges(messages);
    chat.title = text_of(conversation, "title");
    chat.date = day(&from_seconds(&conversation["create_time"]));
    Some(chat)
}

/// A conversation of Claude.ai's data export (`conversations.json`): a list of messages.
fn claude_ai(conversation: &Value) -> Option<Chat> {
    let list = conversation["chat_messages"].as_array()?;
    let mut chat = Chat::new("Claude");
    let mut messages = Vec::new();
    for message in list {
        let when = text_of(message, "created_at");
        let blocks = message["content"].as_array().cloned().unwrap_or_default();
        let from_blocks = blocks.iter().filter(|block| block["type"] == "text").filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n");
        let text = if from_blocks.trim().is_empty() { text_of(message, "text") } else { from_blocks.trim().to_string() };
        let others = blocks.iter().filter(|block| matches!(block["type"].as_str(), Some("tool_use" | "image" | "document"))).count()
            + ["attachments", "files"].iter().map(|key| message[*key].as_array().map_or(0, Vec::len)).sum::<usize>();
        if !text.is_empty() {
            messages.push((if message["sender"] == "human" { Said::Author(text) } else { Said::Tool(text) }, when.clone()));
        }
        for _ in 0..others {
            messages.push((Said::Other, when.clone()));
        }
    }
    chat.turns = exchanges(messages);
    chat.title = text_of(conversation, "name");
    chat.date = day(&text_of(conversation, "created_at"));
    Some(chat)
}

/// A transcript of Claude Code: one line a message, with tool calls and their output between
/// what was asked and what was answered. What the program wrote in the author's place (commands,
/// reminders, summaries) and what helpers did among themselves is not the conversation.
fn claude_code(lines: &[Value]) -> Option<Chat> {
    if !lines.iter().any(|line| line["sessionId"].is_string() && line["message"].is_object()) {
        return None;
    }
    let mut chat = Chat::new("Claude Code");
    let mut messages = Vec::new();
    for line in lines {
        if let Some(title) = line["aiTitle"].as_str().or_else(|| line["summary"].as_str()).filter(|t| !t.trim().is_empty()) {
            chat.title = title.trim().to_string();
        }
        let role = line["type"].as_str().unwrap_or("");
        let aside = ["isSidechain", "isMeta", "isCompactSummary"].iter().any(|key| line[*key] == true);
        if aside || (role != "user" && role != "assistant") {
            continue;
        }
        let when = text_of(line, "timestamp");
        let content = &line["message"]["content"];
        let blocks = match content {
            Value::String(text) => vec![serde_json::json!({ "type": "text", "text": text })],
            Value::Array(blocks) => blocks.clone(),
            _ => continue,
        };
        if role == "assistant" {
            if let Some(model) = line["message"]["model"].as_str().filter(|m| !m.is_empty() && !m.starts_with('<')) {
                chat.model = model.to_string();
            }
        }
        for block in &blocks {
            match (block["type"].as_str(), block["text"].as_str().map(str::trim)) {
                (Some("tool_use"), _) => messages.push((Said::Other, when.clone())),
                // a line that begins with a tag is the program's, not the author's
                (Some("text"), Some(text)) if role == "user" && !text.is_empty() && !text.starts_with('<') && !text.starts_with("[Request interrupted") => {
                    messages.push((Said::Author(text.to_string()), when.clone()));
                }
                (Some("text"), Some(text)) if role == "assistant" && !text.is_empty() => messages.push((Said::Tool(text.to_string()), when.clone())),
                _ => {}
            }
        }
    }
    chat.turns = exchanges(messages);
    chat.date = chat.turns.iter().map(|turn| day(&turn.at)).find(|d| !d.is_empty()).unwrap_or_default();
    Some(chat)
}

/// An AI log in CoWriting's own form: a month's file of the CoWriting plugin for Claude Code
/// (`ai-log/2026-11.jsonl` in a paper's folder) or of this editor for another document. It is read
/// like any other tool's chats, one conversation for each session, so that work done with the
/// plugin can be stated and kept with a document written on in the editor. Nothing of it becomes
/// an entry in this document's own log. What was asked without an answer, and the plugin's own
/// housekeeping (`meta`), is left out.
fn ai_log(lines: &[Value]) -> Vec<Chat> {
    let answers: std::collections::HashMap<&str, &Value> = lines.iter().filter(|line| line["type"] == "response").filter_map(|line| Some((line["ref"].as_str()?, line))).collect();
    let mut chats: Vec<(String, Chat)> = Vec::new();
    for asked in lines.iter().filter(|line| line["type"] == "prompt" && line["category"] != "meta") {
        let Some(answer) = asked["id"].as_str().and_then(|id| answers.get(id)) else { continue };
        let session = text_of(asked, "session");
        let at = match chats.iter().position(|(of, _)| *of == session) {
            Some(at) => at,
            None => {
                let tool = if answer["plugin"].is_string() { "Claude Code with the CoWriting plugin" } else { "CoWriting Editor" };
                let mut chat = Chat::new(tool);
                chat.title = text_of(asked, "paper");
                chat.date = day(&text_of(asked, "ts"));
                chats.push((session, chat));
                chats.len() - 1
            }
        };
        let chat = &mut chats[at].1;
        if let Some(model) = answer["model"].as_str().filter(|model| !model.is_empty()) {
            chat.model = model.to_string();
        }
        let left_out = answer["tools_used"].as_array().map_or(0, |tools| tools.len() as u32);
        chat.turns.push(ChatTurn { at: text_of(asked, "ts"), prompt: text_of(asked, "text"), answer: text_of(answer, "answer"), left_out });
    }
    chats.into_iter().map(|(_, chat)| chat).collect()
}

/// The form of pasted text with the author's marks, among the forms `read` names.
pub const PASTED: &str = "pasted";

/// A form as the disclosure names it.
pub fn form_label(form: &str, german: bool) -> &str {
    match (form, german) {
        ("chatgpt", false) => "ChatGPT export",
        ("chatgpt", true) => "ChatGPT-Export",
        ("claude-ai", false) => "Claude.ai export",
        ("claude-ai", true) => "Claude.ai-Export",
        ("claude-code", false) => "Claude Code transcript",
        ("claude-code", true) => "Claude-Code-Transkript",
        ("cowriting", false) => "chat kept by the editor",
        ("cowriting", true) => "vom Editor abgelegter Chat",
        ("cowriting-log", false) => "AI log of the CoWriting plugin or editor",
        ("cowriting-log", true) => "KI-Protokoll des CoWriting-Plugins oder -Editors",
        ("pasted", false) => "pasted text",
        ("pasted", true) => "eingefügter Text",
        (other, _) => other,
    }
}

/// The conversations a file holds, and its form (`form_label` names it). None when the editor cannot read it
/// as a chat; conversations without any exchange are left out.
pub fn read(text: &str) -> Option<(&'static str, Vec<Chat>)> {
    let text = text.trim_start_matches('\u{feff}').trim();
    let (name, chats) = match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(list)) if list.iter().any(|c| c["mapping"].is_object()) => ("chatgpt", list.iter().filter_map(chatgpt).collect::<Vec<_>>()),
        Ok(Value::Array(list)) if list.iter().any(|c| c["chat_messages"].is_array()) => ("claude-ai", list.iter().filter_map(claude_ai).collect()),
        Ok(one) if one["mapping"].is_object() => ("chatgpt", chatgpt(&one).into_iter().collect()),
        Ok(one) if one["chat_messages"].is_array() => ("claude-ai", claude_ai(&one).into_iter().collect()),
        Ok(one) if one["format"] == FORMAT => ("cowriting", Chat::kept(text).into_iter().collect()),
        Ok(_) => return None,
        Err(_) => {
            let lines: Vec<Value> = text.lines().filter(|line| !line.trim().is_empty()).filter_map(|line| serde_json::from_str(line).ok()).filter(Value::is_object).collect();
            if lines.iter().any(|line| line["type"] == "prompt" && line["id"].is_string()) && lines.iter().any(|line| line["type"] == "response" && line["ref"].is_string()) {
                ("cowriting-log", ai_log(&lines))
            } else {
                ("claude-code", claude_code(&lines).into_iter().collect())
            }
        }
    };
    let chats: Vec<Chat> = chats.into_iter().filter(|chat| !chat.turns.is_empty()).collect();
    if chats.is_empty() {
        None
    } else {
        Some((name, chats))
    }
}

fn marker() -> &'static Regex {
    static MARKER: OnceLock<Regex> = OnceLock::new();
    MARKER.get_or_init(|| {
        Regex::new(r"(?i)^\s*(?:#{1,6}\s*|\*\*)?(?:(prompt|user|you|human|question|me|frage|ich|du)|(answer|assistant|ai|antwort|ki|chatgpt|claude|gemini|copilot))(?:\s+said)?\s*:?(?:\*\*)?\s*:?\s*(.*)$").unwrap()
    })
}

/// A chat pasted as text, with the author's marks: a line that begins with "Prompt:" or
/// "Answer:" (or "Frage:", "Antwort:", "You:", "ChatGPT:" and the like) says who speaks from
/// there on. None when the text has no mark of each kind; then it can only be kept as it is.
pub fn from_text(text: &str) -> Option<Chat> {
    let mut messages: Vec<(bool, Vec<&str>)> = Vec::new();
    for line in text.lines() {
        // a mark is a short beginning of a line, with a colon; "You said" alone on a line counts too
        let marked = marker().captures(line).filter(|found| {
            let whole = found.get(0).map_or("", |m| m.as_str());
            let rest = found.get(3).map_or("", |m| m.as_str());
            whole[..whole.len() - rest.len()].contains(':') || (rest.is_empty() && whole.to_lowercase().contains("said"))
        });
        match (marked, messages.last_mut()) {
            (Some(found), _) => messages.push((found.get(1).is_some(), vec![found.get(3).map_or("", |m| m.as_str())])),
            (None, Some((_, lines))) => lines.push(line),
            (None, None) => {}
        }
    }
    if !messages.iter().any(|(author, _)| *author) || !messages.iter().any(|(author, _)| !*author) {
        return None;
    }
    let said = messages
        .into_iter()
        .map(|(author, lines)| (author, lines.join("\n").trim().to_string()))
        .filter(|(_, text)| !text.is_empty())
        .map(|(author, text)| (if author { Said::Author(text) } else { Said::Tool(text) }, String::new()))
        .collect();
    let mut chat = Chat::new("");
    chat.turns = exchanges(said);
    Some(chat).filter(|chat| !chat.turns.is_empty())
}
