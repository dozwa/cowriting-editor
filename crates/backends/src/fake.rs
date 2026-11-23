//! A backend that plays a script: for tests and for running the app without a model.
//! The script is JSON: `{"replies": [{"text": "…", "tools": [{"name": "…", "input": {…}}], "stop": "end"}]}`.
//! Each model call takes the next reply; a script that runs out says so in its answer.

use async_trait::async_trait;
use cowriting_core::backend::{Backend, BackendError, Block, ModelReply, ModelRequest, Stop};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Script {
    pub replies: Vec<ScriptedReply>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScriptedReply {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub tools: Vec<ScriptedCall>,
    /// 'end' (the default; 'tool_use' when the reply calls tools), 'max_tokens' or 'refusal'.
    #[serde(default)]
    pub stop: Option<String>,
    /// The call fails with this message instead of answering, after `text` was streamed.
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScriptedCall {
    pub name: String,
    #[serde(default)]
    pub input: Value,
}

pub struct FakeBackend {
    replies: Mutex<std::collections::VecDeque<ScriptedReply>>,
    requests: Mutex<Vec<ModelRequest>>,
}

impl FakeBackend {
    pub fn new(script: Script) -> FakeBackend {
        FakeBackend { replies: Mutex::new(script.replies.into()), requests: Mutex::default() }
    }

    pub fn from_json(json: &str) -> Result<FakeBackend, String> {
        serde_json::from_str(json).map(FakeBackend::new).map_err(|e| format!("The fake backend's script is not valid: {e}"))
    }

    /// Every request the backend was sent, in order.
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl Backend for FakeBackend {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn provider(&self) -> String {
        "a script (no model)".into()
    }

    async fn complete(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) -> Result<ModelReply, BackendError> {
        let call = self.requests.lock().unwrap().len();
        self.requests.lock().unwrap().push(request.clone());
        let reply =
            self.replies.lock().unwrap().pop_front().unwrap_or_else(|| ScriptedReply { text: "(the fake backend's script has no more replies)".into(), ..Default::default() });
        for piece in reply.text.split_inclusive(' ') {
            on_text(piece);
        }
        if let Some(message) = reply.error {
            return Err(BackendError { message });
        }
        let mut blocks = Vec::new();
        if !reply.text.is_empty() {
            blocks.push(Block::Text { text: reply.text });
        }
        for (i, tool) in reply.tools.iter().enumerate() {
            blocks.push(Block::ToolUse { id: format!("call_{call}_{i}"), name: tool.name.clone(), input: tool.input.clone() });
        }
        let stop = match reply.stop.as_deref() {
            Some("max_tokens") => Stop::MaxTokens,
            Some("refusal") => Stop::Refusal,
            Some("end") => Stop::End,
            Some(other) if other != "tool_use" => Stop::Other(other.to_string()),
            _ if reply.tools.is_empty() => Stop::End,
            _ => Stop::ToolUse,
        };
        Ok(ModelReply { blocks, stop, model: None })
    }
}
