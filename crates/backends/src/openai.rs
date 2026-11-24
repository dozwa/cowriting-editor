//! Any provider that speaks the OpenAI chat-completions API: OpenAI itself, gateways, and local
//! model servers such as Ollama and LM Studio. Plain HTTP with streaming and function calling.
//!
//! A model that cannot call tools is noticed from the provider's refusal; the turn then sends
//! the document with the question instead (`Backend::uses_tools`).

use async_trait::async_trait;
use cowriting_core::backend::{Backend, BackendError, Block, Message, ModelReply, ModelRequest, Role, Stop};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiBackend {
    /// Ends in the API's root, usually `/v1`.
    base_url: String,
    /// A local server needs none.
    api_key: Option<String>,
    http: reqwest::Client,
    has_tools: AtomicBool,
    /// The longest answer the author allows; 0 leaves it to the provider.
    max_tokens: u32,
}

/// The host a base URL names, with its port: who the text is sent to.
fn host_of(base_url: &str) -> String {
    base_url.split("://").nth(1).unwrap_or(base_url).split('/').next().unwrap_or("").to_string()
}

/// Is this address on the author's own machine?
pub fn is_local(base_url: &str) -> bool {
    let host = host_of(base_url);
    let name = host.rsplit_once(':').map_or(host.as_str(), |(name, port)| if port.chars().all(|c| c.is_ascii_digit()) { name } else { host.as_str() });
    matches!(name, "localhost" | "127.0.0.1" | "[::1]" | "0.0.0.0")
}

impl OpenAiBackend {
    pub fn new(base_url: &str, api_key: Option<&str>) -> OpenAiBackend {
        let base_url = Some(base_url.trim()).filter(|u| !u.is_empty()).unwrap_or(OPENAI_BASE_URL).trim_end_matches('/').to_string();
        let api_key = api_key.map(str::trim).filter(|k| !k.is_empty()).map(String::from);
        OpenAiBackend { base_url, api_key, http: reqwest::Client::new(), has_tools: AtomicBool::new(true), max_tokens: 0 }
    }

    /// The longest answer the author allows; 0 leaves it to the provider.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> OpenAiBackend {
        self.max_tokens = max_tokens;
        self
    }

    /// The request as it is sent: with the author's limit on the answer's length, if they set one.
    /// OpenAI itself calls the limit `max_completion_tokens`; the servers that speak its API know `max_tokens`.
    pub fn body(&self, request: &ModelRequest) -> Value {
        let mut body = request_body(request);
        if self.max_tokens > 0 {
            body[if self.base_url == OPENAI_BASE_URL { "max_completion_tokens" } else { "max_tokens" }] = json!(self.max_tokens);
        }
        body
    }

    fn with_key(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    /// The models the provider offers: (id, display name).
    pub async fn models(&self) -> Result<Vec<(String, String)>, BackendError> {
        let response = self.with_key(self.http.get(format!("{}/models", self.base_url))).send().await.map_err(|e| self.unreachable(e))?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|e| self.unreachable(e))?;
        if status != 200 {
            return Err(api_error(status, &body));
        }
        let parsed: Value = serde_json::from_str(&body).map_err(|_| BackendError { message: "The provider's list of models could not be read.".into() })?;
        let mut ids: Vec<String> = parsed["data"].as_array().map(|models| models.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect()).unwrap_or_default();
        ids.sort();
        Ok(ids.into_iter().map(|id| (id.clone(), id)).collect())
    }

    fn unreachable(&self, error: reqwest::Error) -> BackendError {
        let hint = if is_local(&self.base_url) { " Is the local model server running?" } else { "" };
        BackendError { message: format!("{} could not be reached: {}.{hint}", host_of(&self.base_url), error.without_url()) }
    }
}

fn error_detail(body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).ok();
    parsed.as_ref().and_then(|v| v["error"]["message"].as_str().or(v["error"].as_str()).or(v["message"].as_str())).unwrap_or_default().to_string()
}

fn api_error(status: u16, body: &str) -> BackendError {
    let detail = error_detail(body);
    let said = if detail.is_empty() { String::new() } else { format!(" ({detail})") };
    let message = match status {
        401 => format!("The API key was not accepted{said}."),
        403 => format!("This API key may not use that model{said}."),
        404 => format!("The provider does not know that model or address{said}."),
        429 => format!("The provider's rate limit or quota was reached{said}."),
        500..=599 => format!("The provider is having trouble (HTTP {status}); try again in a moment{said}."),
        _ => format!("The provider refused the request (HTTP {status}){said}."),
    };
    BackendError { message }
}

/// Does this refusal say that the model cannot call tools?
fn says_no_tools(status: u16, body: &str) -> bool {
    let detail = error_detail(body).to_lowercase();
    (400..500).contains(&status)
        && (detail.contains("tool") || detail.contains("function call"))
        && (detail.contains("support") || detail.contains("not available") || detail.contains("unsupported"))
}

/// One of our messages as the chat messages it becomes: tool results are messages of their own,
/// and come before anything else the user side says.
fn messages_json(message: &Message, out: &mut Vec<Value>) {
    let text: String = message.content.iter().filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n\n");
    match message.role {
        Role::User => {
            for block in &message.content {
                if let Block::ToolResult { id, content, .. } = block {
                    out.push(json!({ "role": "tool", "tool_call_id": id, "content": content }));
                }
            }
            if !text.is_empty() {
                out.push(json!({ "role": "user", "content": text }));
            }
        }
        Role::Assistant => {
            let calls: Vec<Value> = message
                .content
                .iter()
                .filter_map(|b| match b {
                    Block::ToolUse { id, name, input } => Some(json!({ "id": id, "type": "function", "function": { "name": name, "arguments": input.to_string() } })),
                    _ => None,
                })
                .collect();
            let mut assistant = json!({ "role": "assistant", "content": text });
            if !calls.is_empty() {
                assistant["tool_calls"] = Value::Array(calls);
            }
            out.push(assistant);
        }
    }
}

/// The request body. No sampling parameters and no length limit: the provider's defaults apply.
pub fn request_body(request: &ModelRequest) -> Value {
    let mut messages = vec![json!({ "role": "system", "content": request.system })];
    for message in &request.messages {
        messages_json(message, &mut messages);
    }
    let mut body = json!({ "model": request.model, "stream": true, "messages": messages });
    if !request.tools.is_empty() {
        body["tools"] =
            request.tools.iter().map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.input_schema } })).collect();
    }
    body
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

/// Puts a streamed chat completion together from its server-sent events.
#[derive(Default)]
pub struct Assembler {
    /// Bytes of a line that has not ended yet.
    pending: Vec<u8>,
    text: String,
    calls: Vec<PartialCall>,
    finish: Option<String>,
    model: Option<String>,
    error: Option<String>,
    is_done: bool,
}

impl Assembler {
    /// Takes the next bytes of the stream; text goes to `on_text` as it arrives.
    pub fn feed(&mut self, bytes: &[u8], on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) {
        self.pending.extend_from_slice(bytes);
        // only whole lines are decoded, so a character is never split
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(data) = line.trim().strip_prefix("data:").map(str::trim) else { continue };
            if data == "[DONE]" {
                self.is_done = true;
            } else if let Ok(value) = serde_json::from_str::<Value>(data) {
                self.apply(&value, on_text);
            }
        }
    }

    fn apply(&mut self, chunk: &Value, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) {
        if !chunk["error"].is_null() {
            self.error = Some(chunk["error"]["message"].as_str().or(chunk["error"].as_str()).unwrap_or("the provider reported an error").to_string());
            return;
        }
        if let Some(model) = chunk["model"].as_str().filter(|m| !m.is_empty()) {
            self.model = Some(model.to_string());
        }
        let choice = &chunk["choices"][0];
        // a model's reasoning (`delta.reasoning`, `delta.reasoning_content`) is not part of the answer and is dropped
        if let Some(more) = choice["delta"]["content"].as_str().filter(|t| !t.is_empty()) {
            self.text.push_str(more);
            on_text(more);
        }
        for call in choice["delta"]["tool_calls"].as_array().into_iter().flatten() {
            // fragments of one call share an index; a server that sends whole calls may leave it out
            let index = call["index"].as_u64().map_or(self.calls.len(), |i| i as usize);
            if self.calls.len() <= index {
                self.calls.resize_with(index + 1, PartialCall::default);
            }
            let partial = &mut self.calls[index];
            if let Some(id) = call["id"].as_str().filter(|id| !id.is_empty()) {
                partial.id = id.to_string();
            }
            if let Some(name) = call["function"]["name"].as_str() {
                partial.name.push_str(name);
            }
            match &call["function"]["arguments"] {
                Value::String(more) => partial.arguments.push_str(more),
                Value::Object(whole) => partial.arguments = Value::Object(whole.clone()).to_string(),
                _ => {}
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.finish = Some(reason.to_string());
        }
    }

    /// The message, once the stream has ended.
    pub fn finish(self) -> Result<ModelReply, BackendError> {
        if let Some(error) = self.error {
            return Err(BackendError { message: format!("The provider stopped the answer: {error}") });
        }
        if !self.is_done && self.finish.is_none() {
            return Err(BackendError { message: "The connection ended before the answer was complete.".into() });
        }
        let mut blocks = Vec::new();
        if !self.text.is_empty() {
            blocks.push(Block::Text { text: self.text });
        }
        let has_calls = self.calls.iter().any(|call| !call.name.is_empty());
        for (index, call) in self.calls.into_iter().enumerate().filter(|(_, call)| !call.name.is_empty()) {
            let input = if call.arguments.trim().is_empty() {
                json!({})
            } else {
                // strictly parsed: input that is not whole JSON is kept as it came, and the tool will say what is missing
                serde_json::from_str(&call.arguments).unwrap_or_else(|_| json!({ "_unreadable_input": call.arguments }))
            };
            blocks.push(Block::ToolUse { id: if call.id.is_empty() { format!("call_{index}") } else { call.id }, name: call.name, input });
        }
        let stop = match self.finish.as_deref() {
            Some("length") => Stop::MaxTokens,
            Some("content_filter") => Stop::Refusal,
            // some servers say "stop" although the model asked for tools
            Some("tool_calls") | Some("function_call") | Some("stop") | None if has_calls => Stop::ToolUse,
            Some("stop") | None => Stop::End,
            Some("tool_calls") | Some("function_call") => Stop::End,
            Some(other) => Stop::Other(other.to_string()),
        };
        Ok(ModelReply { blocks, stop, model: self.model })
    }
}

#[async_trait]
impl Backend for OpenAiBackend {
    fn id(&self) -> &'static str {
        "openai-compatible"
    }

    fn provider(&self) -> String {
        let host = host_of(&self.base_url);
        if self.base_url == OPENAI_BASE_URL {
            "OpenAI".into()
        } else if is_local(&self.base_url) {
            // nothing left this computer
            format!("a local model server ({host})")
        } else {
            format!("OpenAI-compatible API at {host}")
        }
    }

    fn uses_tools(&self) -> bool {
        self.has_tools.load(Ordering::Relaxed)
    }

    async fn complete(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) -> Result<ModelReply, BackendError> {
        let mut response = self.with_key(self.http.post(format!("{}/chat/completions", self.base_url))).json(&self.body(request)).send().await.map_err(|e| self.unreachable(e))?;
        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            if !request.tools.is_empty() && says_no_tools(status, &body) {
                self.has_tools.store(false, Ordering::Relaxed);
            }
            return Err(api_error(status, &body));
        }
        let mut assembler = Assembler::default();
        while let Some(chunk) = response.chunk().await.map_err(|e| BackendError { message: format!("The connection to the provider broke off: {}", e.without_url()) })? {
            assembler.feed(&chunk, on_text);
        }
        assembler.finish()
    }
}
