//! The Claude API (Messages), over plain HTTP with streaming: there is no official Rust SDK.
//! A different base URL reaches a gateway that speaks the same API.

use async_trait::async_trait;
use cowriting_core::backend::{Backend, BackendError, Block, Message, ModelReply, ModelRequest, Role, Stop};
use serde_json::{json, Value};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2026-10-21";
/// Room for a long answer; a streamed request has no timeout to fear.
const MAX_TOKENS: u32 = 64_000;

pub struct AnthropicBackend {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl AnthropicBackend {
    pub fn new(api_key: impl Into<String>, base_url: Option<&str>) -> AnthropicBackend {
        let base_url = base_url.map(str::trim).filter(|u| !u.is_empty()).unwrap_or(DEFAULT_BASE_URL).trim_end_matches('/').to_string();
        AnthropicBackend { api_key: api_key.into(), base_url, http: reqwest::Client::new() }
    }

    /// The models this key can use: (id, display name), newest first as the API lists them.
    pub async fn models(&self) -> Result<Vec<(String, String)>, BackendError> {
        let response = self
            .http
            .get(format!("{}/v1/models?limit=100", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(unreachable)?;
        if status != 200 {
            return Err(api_error(status, &body));
        }
        let parsed: Value = serde_json::from_str(&body).map_err(|_| BackendError { message: "The provider's list of models could not be read.".into() })?;
        let text = |v: &Value, key: &str| v[key].as_str().unwrap_or_default().to_string();
        Ok(parsed["data"].as_array().map(|models| models.iter().map(|m| (text(m, "id"), text(m, "display_name"))).collect()).unwrap_or_default())
    }
}

fn unreachable(error: reqwest::Error) -> BackendError {
    BackendError { message: format!("The provider could not be reached: {}", error.without_url()) }
}

/// An HTTP error as the author should read it.
fn api_error(status: u16, body: &str) -> BackendError {
    let detail = serde_json::from_str::<Value>(body).ok().and_then(|v| v["error"]["message"].as_str().map(String::from)).unwrap_or_default();
    let said = if detail.is_empty() { String::new() } else { format!(" ({detail})") };
    let message = match status {
        401 => format!("The API key was not accepted{said}."),
        403 => format!("This API key may not use that model or feature{said}."),
        404 => format!("The provider does not know that model or address{said}."),
        429 => format!("The provider's rate limit was reached; try again in a moment{said}."),
        500..=599 => format!("The provider is having trouble (HTTP {status}); try again in a moment{said}."),
        _ => format!("The provider refused the request (HTTP {status}){said}."),
    };
    BackendError { message }
}

fn block_json(block: &Block) -> Option<Value> {
    match block {
        // the API rejects an empty text block
        Block::Text { text } if text.is_empty() => None,
        Block::Text { text } => Some(json!({ "type": "text", "text": text })),
        Block::ToolUse { id, name, input } => Some(json!({ "type": "tool_use", "id": id, "name": name, "input": input })),
        Block::ToolResult { id, content, is_error } => Some(json!({ "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error })),
        Block::Opaque(raw) => Some(raw.clone()),
    }
}

fn message_json(message: &Message) -> Value {
    json!({
        "role": if message.role == Role::User { "user" } else { "assistant" },
        "content": message.content.iter().filter_map(block_json).collect::<Vec<_>>(),
    })
}

/// The request body. No sampling parameters: the current API version rejects them. Thinking is
/// left to the model's default. Tool inputs are buffered by the API (no eager streaming): a
/// note's text is not shown while it is written, so there is nothing to gain from fragments.
pub fn request_body(request: &ModelRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": request.system,
        "messages": request.messages.iter().map(message_json).collect::<Vec<_>>(),
    });
    // a call without tools (live review, the point check) names none
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.input_schema })).collect();
    }
    body
}

/// Puts a streamed message together from its server-sent events.
#[derive(Default)]
pub struct Assembler {
    /// Bytes of an event that has not ended yet.
    pending: Vec<u8>,
    /// The content blocks as the API will want them back, by index.
    blocks: Vec<Value>,
    /// Tool input arrives as fragments of JSON, by block index.
    partial_input: Vec<String>,
    stop: Option<String>,
    model: Option<String>,
    error: Option<String>,
    is_done: bool,
}

impl Assembler {
    /// Takes the next bytes of the stream; text deltas go to `on_text` as they complete.
    pub fn feed(&mut self, bytes: &[u8], on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) {
        self.pending.extend_from_slice(bytes);
        // an event ends with a blank line; only whole events are decoded, so a character is never split
        while let Some(end) = find_blank_line(&self.pending) {
            let event: Vec<u8> = self.pending.drain(..end.1).collect();
            let text = String::from_utf8_lossy(&event[..end.0]).into_owned();
            let data: String = text.lines().filter_map(|line| line.strip_prefix("data:")).map(str::trim_start).collect::<Vec<_>>().join("\n");
            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                self.apply(&value, on_text);
            }
        }
    }

    fn apply(&mut self, event: &Value, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) {
        let index = event["index"].as_u64().unwrap_or(0) as usize;
        match event["type"].as_str().unwrap_or("") {
            "message_start" => self.model = event["message"]["model"].as_str().map(String::from),
            "content_block_start" => {
                if self.blocks.len() <= index {
                    self.blocks.resize(index + 1, Value::Null);
                    self.partial_input.resize(index + 1, String::new());
                }
                self.blocks[index] = event["content_block"].clone();
            }
            "content_block_delta" => {
                let Some(block) = self.blocks.get_mut(index) else { return };
                let delta = &event["delta"];
                let append = |block: &mut Value, key: &str, more: &str| {
                    let so_far = block[key].as_str().unwrap_or("").to_string();
                    block[key] = Value::String(so_far + more);
                };
                match delta["type"].as_str().unwrap_or("") {
                    "text_delta" => {
                        let more = delta["text"].as_str().unwrap_or("");
                        append(block, "text", more);
                        on_text(more);
                    }
                    "input_json_delta" => self.partial_input[index].push_str(delta["partial_json"].as_str().unwrap_or("")),
                    "thinking_delta" => append(block, "thinking", delta["thinking"].as_str().unwrap_or("")),
                    "signature_delta" => block["signature"] = delta["signature"].clone(),
                    _ => {}
                }
            }
            "content_block_stop" => {
                let (Some(block), Some(partial)) = (self.blocks.get_mut(index), self.partial_input.get(index)) else { return };
                if block["type"] == "tool_use" && !partial.is_empty() {
                    // strictly parsed: input that is not whole JSON is kept as it came, and the tool will say what is missing
                    block["input"] = serde_json::from_str(partial).unwrap_or_else(|_| json!({ "_unreadable_input": partial }));
                }
            }
            "message_delta" => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    self.stop = Some(reason.to_string());
                }
            }
            "message_stop" => self.is_done = true,
            "error" => self.error = Some(event["error"]["message"].as_str().unwrap_or("the provider reported an error").to_string()),
            _ => {}
        }
    }

    /// The message, once the stream has ended.
    pub fn finish(self) -> Result<ModelReply, BackendError> {
        if let Some(error) = self.error {
            return Err(BackendError { message: format!("The provider stopped the answer: {error}") });
        }
        if !self.is_done {
            return Err(BackendError { message: "The connection ended before the answer was complete.".into() });
        }
        let blocks = self
            .blocks
            .into_iter()
            .filter(|b| !b.is_null())
            .map(|b| match b["type"].as_str() {
                Some("text") => Block::Text { text: b["text"].as_str().unwrap_or("").to_string() },
                Some("tool_use") => {
                    Block::ToolUse { id: b["id"].as_str().unwrap_or("").to_string(), name: b["name"].as_str().unwrap_or("").to_string(), input: b["input"].clone() }
                }
                // reasoning and anything newer goes back to the API as it came
                _ => Block::Opaque(b),
            })
            .collect();
        let stop = match self.stop.as_deref() {
            Some("end_turn") | Some("stop_sequence") => Stop::End,
            Some("tool_use") => Stop::ToolUse,
            Some("max_tokens") | Some("model_context_window_exceeded") => Stop::MaxTokens,
            Some("refusal") => Stop::Refusal,
            other => Stop::Other(other.unwrap_or("unknown").to_string()),
        };
        Ok(ModelReply { blocks, stop, model: self.model })
    }
}

/// Where the first blank line ends an event: (end of the event's text, start of the next event).
fn find_blank_line(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes.windows(2).position(|w| w == b"\n\n").map(|at| (at, at + 2));
    let crlf = bytes.windows(4).position(|w| w == b"\r\n\r\n").map(|at| (at, at + 4));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(if a.0 < b.0 { a } else { b }),
        (a, b) => a.or(b),
    }
}

#[async_trait]
impl Backend for AnthropicBackend {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn provider(&self) -> String {
        if self.base_url == DEFAULT_BASE_URL {
            "Anthropic".into()
        } else {
            // a gateway is named by its host: that is who the text was sent to
            let host = self.base_url.split("://").nth(1).unwrap_or(&self.base_url).split('/').next().unwrap_or("").to_string();
            format!("Claude API at {host}")
        }
    }

    async fn complete(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) -> Result<ModelReply, BackendError> {
        let mut response = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&request_body(request))
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(api_error(status, &response.text().await.unwrap_or_default()));
        }
        let mut assembler = Assembler::default();
        while let Some(chunk) = response.chunk().await.map_err(|e| BackendError { message: format!("The connection to the provider broke off: {}", e.without_url()) })? {
            assembler.feed(&chunk, on_text);
        }
        assembler.finish()
    }
}
