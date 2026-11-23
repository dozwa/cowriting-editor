//! What the core asks of a model, whoever serves it. A backend makes one model call; the turn
//! (`turn.rs`) runs the tools and writes the log, so every backend is guarded and logged alike.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Text {
        text: String,
    },
    /// The model asks for a tool.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// What the tool gave back; sent in the next user message.
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
    /// Something only the backend that made it understands (a provider's reasoning block, say).
    /// It goes back to that backend unchanged within the turn and is never logged or shown.
    Opaque(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Block>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Message {
        Message { role: Role::User, content: vec![Block::Text { text: text.into() }] }
    }

    pub fn assistant(text: impl Into<String>) -> Message {
        Message { role: Role::Assistant, content: vec![Block::Text { text: text.into() }] }
    }
}

/// A tool as the model is told about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema of the input.
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    /// The model finished its answer.
    End,
    /// The model waits for tool results.
    ToolUse,
    /// The answer hit the length limit.
    MaxTokens,
    /// The provider's safety system declined.
    Refusal,
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelReply {
    /// Text and tool requests, in the order the model gave them.
    pub blocks: Vec<Block>,
    pub stop: Stop,
    /// The model that answered, when the provider names it; the log records this over what was asked for.
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendError {
    /// What the author is told.
    pub message: String,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[async_trait]
pub trait Backend: Send + Sync {
    /// 'anthropic', 'openai-compatible', 'claude-code' or 'fake': what the log records as `backend`.
    fn id(&self) -> &'static str;
    /// Who serves the model, as the disclosure names it.
    fn provider(&self) -> String;
    /// False once the backend knows its model cannot call tools. The turn then sends the document
    /// with the question instead of offering tools.
    fn uses_tools(&self) -> bool {
        true
    }
    /// One model call. Text is passed to `on_text` as it arrives and is also part of the reply.
    async fn complete(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) -> Result<ModelReply, BackendError>;
}
