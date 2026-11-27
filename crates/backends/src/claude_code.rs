//! Claude Code as a backend: the author's own installed `claude`, started headless for one turn.
//!
//! The editor never touches Claude Code's login or its files. It starts the program with every
//! built-in tool switched off (`--tools ""`), without the author's settings, plugins and other MCP
//! servers, in an empty folder, and offers it the editor's own tools through a small MCP server
//! that listens on this computer for the length of the call. So the only things Claude Code can
//! do here are the things `turn.rs` runs, guards and logs. If Claude Code reports any other tool,
//! the call is stopped before the question is answered.

use async_trait::async_trait;
use cowriting_core::backend::{Backend, BackendError, Block, Message, ModelReply, ModelRequest, Role, RunTool, Stop, ToolDef};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;

/// The name the editor's tools are served under; Claude Code calls them `mcp__cowriting__<tool>`.
const SERVER: &str = "cowriting";
/// How many tool calls one answer may make.
const MAX_CALLS: usize = 40;

pub const NOT_INSTALLED: &str =
    "Claude Code was not found on this computer. Install it from claude.com/claude-code and sign in once in a terminal, or choose another AI in the settings.";
/// What an option parser says about an option it does not know: a Claude Code from before the options the editor starts it with.
const UNKNOWN_OPTION: &str = "unknown option";
const NOT_SIGNED_IN: &str = "Claude Code is not signed in. Open a terminal, run “claude”, sign in, and ask again.";

/// The models Claude Code can be asked for, by the names it understands: (id, display name).
pub const MODELS: [(&str, &str); 3] = [("sonnet", "Claude Sonnet (the current one)"), ("opus", "Claude Opus (the current one)"), ("haiku", "Claude Haiku (the current one)")];

/// Where the author's `claude` is. An app started from the Dock or the Start menu does not get the shell's PATH.
pub fn find_claude() -> Option<PathBuf> {
    // a path given outright is the only place looked at
    if let Some(given) = std::env::var_os("COWRITING_CLAUDE").map(PathBuf::from) {
        return given.is_file().then_some(given);
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from) {
        dirs.extend([".local/bin", ".claude/local", ".claude/bin", "AppData/Roaming/npm"].iter().map(|below| home.join(below)));
    }
    if !cfg!(windows) {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].iter().map(PathBuf::from));
    }
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    dirs.iter().flat_map(|dir| names.iter().map(move |name| dir.join(name))).find(|path| path.is_file())
}

pub struct ClaudeCodeBackend {
    program: PathBuf,
}

impl ClaudeCodeBackend {
    pub fn new(program: impl Into<PathBuf>) -> ClaudeCodeBackend {
        ClaudeCodeBackend { program: program.into() }
    }
}

/// A token nobody else on this computer knows, for the length of one call.
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2).map(|_| format!("{:016x}", std::collections::hash_map::RandomState::new().build_hasher().finish())).collect()
}

fn text_of(message: &Message) -> String {
    let texts: Vec<&str> = message.content.iter().filter_map(|block| if let Block::Text { text } = block { Some(text.as_str()) } else { None }).collect();
    texts.join("\n\n")
}

/// The conversation as one prompt: Claude Code takes a question, not a list of messages.
pub fn prompt_of(messages: &[Message]) -> String {
    let Some((last, earlier)) = messages.split_last() else { return String::new() };
    if earlier.is_empty() {
        return text_of(last);
    }
    let mut out = String::from("Earlier in this conversation:\n");
    for message in earlier {
        out.push_str(&format!("\n[{}]\n{}\n", if message.role == Role::User { "The author" } else { "You" }, text_of(message)));
    }
    out.push_str(&format!("\nThe author now writes:\n\n{}", text_of(last)));
    out
}

/// What `claude` is started with. Everything of the author's own setup stays out of the call.
pub fn arguments(model: &str, system_file: &str, tools: &[ToolDef], mcp_url: Option<(&str, &str)>) -> Vec<String> {
    let mut args: Vec<String> = [
        "--print",
        "--model",
        model,
        "--system-prompt-file",
        system_file,
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--disable-slash-commands",
        "--no-session-persistence",
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect();
    args.extend(["--output-format", "stream-json", "--include-partial-messages", "--verbose"].iter().map(|arg| arg.to_string()));
    if let Some((url, token)) = mcp_url.filter(|_| !tools.is_empty()) {
        let config = json!({ "mcpServers": { SERVER: { "type": "http", "url": url, "headers": { "Authorization": format!("Bearer {token}") } } } });
        let allowed: Vec<String> = tools.iter().map(|tool| format!("mcp__{SERVER}__{}", tool.name)).collect();
        args.extend(["--mcp-config".to_string(), config.to_string(), "--allowedTools".to_string(), allowed.join(",")]);
    }
    args
}

/// The answer to one MCP request (JSON-RPC); None for a notification, which gets none.
pub fn mcp_reply(request: &Value, tools: &[ToolDef], run_tool: &mut RunTool<'_>) -> Option<Value> {
    let id = request.get("id")?.clone();
    let result = match request["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": request["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "cowriting-editor", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => {
            Ok(json!({ "tools": tools.iter().map(|tool| json!({ "name": tool.name, "description": tool.description, "inputSchema": tool.input_schema })).collect::<Vec<_>>() }))
        }
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or("");
            let input = request["params"].get("arguments").cloned().unwrap_or_else(|| json!({}));
            // a tool that is not offered goes to the turn too: it refuses it and puts the attempt on the record
            let (content, is_error) = run_tool(name, &input);
            Ok(json!({ "content": [{ "type": "text", "text": content }], "isError": is_error }))
        }
        _ => Err(json!({ "code": -32601, "message": "Method not found" })),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

async fn respond(stream: &mut TcpStream, status: &str, body: Option<&Value>) -> std::io::Result<()> {
    let body = body.map(Value::to_string).unwrap_or_default();
    let kind = if body.is_empty() { "" } else { "content-type: application/json\r\n" };
    stream.write_all(format!("HTTP/1.1 {status}\r\n{kind}content-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await?;
    stream.shutdown().await
}

/// One HTTP request to the editor's MCP server. Only a caller with this call's token is answered.
async fn serve(mut stream: TcpStream, token: &str, tools: &[ToolDef], run_tool: &mut RunTool<'_>) -> std::io::Result<()> {
    let (mut received, mut chunk) = (Vec::new(), [0u8; 8192]);
    let head_end = loop {
        if let Some(at) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 || received.len() > 64 * 1024 {
            return Ok(());
        }
        received.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&received[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let header =
        |name: &str| head.split("\r\n").skip(1).find_map(|line| line.split_once(':').filter(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.trim().to_string()));
    if path != format!("/mcp/{token}") || header("authorization").as_deref() != Some(&format!("Bearer {token}")) {
        return respond(&mut stream, "404 Not Found", None).await;
    }
    if method != "POST" {
        // no stream of server messages, and no session to end
        return respond(&mut stream, "405 Method Not Allowed", None).await;
    }
    let length: usize = header("content-length").and_then(|value| value.parse().ok()).unwrap_or(0);
    if length > 8 * 1024 * 1024 {
        return respond(&mut stream, "413 Payload Too Large", None).await;
    }
    let mut body = received[head_end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    let Ok(request) = serde_json::from_slice::<Value>(&body) else { return respond(&mut stream, "400 Bad Request", None).await };
    match mcp_reply(&request, tools, run_tool) {
        Some(reply) => respond(&mut stream, "200 OK", Some(&reply)).await,
        None => respond(&mut stream, "202 Accepted", None).await,
    }
}

/// What the stream of Claude Code's events has told so far.
#[derive(Default)]
pub struct Heard {
    /// The text of each of the model's messages.
    pub texts: Vec<String>,
    pub model: Option<String>,
    /// Set by the last event: Ok, or what went wrong.
    pub outcome: Option<Result<(), String>>,
    /// A tool Claude Code has that is not one of the editor's.
    pub foreign_tool: Option<String>,
    has_streamed: bool,
}

impl Heard {
    /// Takes one line of `--output-format stream-json`.
    pub fn line(&mut self, line: &str, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) {
        let Ok(event) = serde_json::from_str::<Value>(line) else { return };
        match event["type"].as_str().unwrap_or("") {
            "system" if event["subtype"] == "init" => {
                let own = format!("mcp__{SERVER}__");
                self.foreign_tool = event["tools"].as_array().and_then(|tools| tools.iter().filter_map(Value::as_str).find(|tool| !tool.starts_with(&own)).map(String::from));
            }
            "stream_event" => {
                let inner = &event["event"];
                if inner["type"] == "content_block_delta" && inner["delta"]["type"] == "text_delta" {
                    let text = inner["delta"]["text"].as_str().unwrap_or("");
                    if !text.is_empty() {
                        self.has_streamed = true;
                        on_text(text);
                    }
                } else if inner["type"] == "message_start" && self.has_streamed {
                    // text after a tool call is a new paragraph
                    on_text("\n\n");
                }
            }
            "assistant" => {
                if let Some(model) = event["message"]["model"].as_str().filter(|model| !model.starts_with('<')) {
                    self.model = Some(model.to_string());
                }
                let said: Vec<&str> = event["message"]["content"]
                    .as_array()
                    .map(|blocks| blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect())
                    .unwrap_or_default();
                if !said.concat().trim().is_empty() {
                    self.texts.push(said.concat());
                }
            }
            "result" => {
                let said = event["result"].as_str().unwrap_or("");
                self.outcome = Some(if event["is_error"] == true {
                    Err(if said.contains("/login") || said.to_lowercase().contains("not logged in") {
                        NOT_SIGNED_IN.to_string()
                    } else {
                        format!("Claude Code could not answer: {}", if said.is_empty() { event["subtype"].as_str().unwrap_or("unknown error") } else { said })
                    })
                } else {
                    Ok(())
                });
            }
            _ => {}
        }
    }
}

impl ClaudeCodeBackend {
    async fn run(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send), run_tool: Option<&mut RunTool<'_>>) -> Result<ModelReply, BackendError> {
        let fail = |message: String| BackendError { message };
        let token = token();
        // an empty folder of its own: nothing of a project is found, and nothing is left behind
        let folder = std::env::temp_dir().join(format!("cowriting-claude-{token}"));
        std::fs::create_dir_all(&folder).map_err(|e| fail(format!("A working folder for Claude Code could not be made: {e}")))?;
        let outcome = self.run_in(&folder, &token, request, on_text, run_tool).await;
        let _ = std::fs::remove_dir_all(&folder);
        outcome.map_err(fail)
    }

    async fn run_in(
        &self,
        folder: &Path,
        token: &str,
        request: &ModelRequest,
        on_text: &mut (dyn for<'s> FnMut(&'s str) + Send),
        mut run_tool: Option<&mut RunTool<'_>>,
    ) -> Result<ModelReply, String> {
        let system_file = folder.join("system.txt");
        std::fs::write(&system_file, &request.system).map_err(|e| e.to_string())?;
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| format!("The editor's tools could not be offered to Claude Code: {e}"))?;
        let url = format!("http://127.0.0.1:{}/mcp/{token}", listener.local_addr().map_err(|e| e.to_string())?.port());
        let tools: &[ToolDef] = if run_tool.is_some() { &request.tools } else { &[] };
        let mut command = Command::new(&self.program);
        command.args(arguments(&request.model, &system_file.to_string_lossy(), tools, Some((&url, token)))).current_dir(folder);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // no console window
        let mut child = command.spawn().map_err(|e| format!("Claude Code could not be started: {e}"))?;
        let mut stdin = child.stdin.take().expect("piped");
        let prompt = prompt_of(&request.messages);
        let asking = tokio::spawn(async move {
            let _ = stdin.write_all(prompt.as_bytes()).await;
        });
        let mut stderr = child.stderr.take().expect("piped");
        let complaints = tokio::spawn(async move {
            let mut said = String::new();
            let _ = stderr.read_to_string(&mut said).await;
            said
        });
        let mut lines = BufReader::new(child.stdout.take().expect("piped")).lines();
        let (mut heard, mut calls) = (Heard::default(), 0);
        let stopped = loop {
            tokio::select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => {
                        heard.line(&line, on_text);
                        if let Some(tool) = &heard.foreign_tool {
                            break Some(format!("Claude Code started with a tool of its own ({tool}) that the editor did not offer. The call was stopped; nothing was answered."));
                        }
                        if heard.outcome.is_some() {
                            break None;
                        }
                    }
                    _ => break None,
                },
                accepted = listener.accept() => {
                    if let (Ok((stream, _)), Some(run_tool)) = (accepted, run_tool.as_deref_mut()) {
                        let mut counted = |name: &str, input: &Value| {
                            calls += 1;
                            run_tool(name, input)
                        };
                        let _ = tokio::time::timeout(Duration::from_secs(20), serve(stream, token, tools, &mut counted)).await;
                        if calls > MAX_CALLS {
                            break Some("The model kept asking for tools instead of answering.".to_string());
                        }
                    }
                }
            }
        };
        let _ = asking.await;
        if stopped.is_some() || heard.outcome.is_none() {
            let _ = child.kill().await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        if let Some(why) = stopped {
            return Err(why);
        }
        match heard.outcome {
            Some(Ok(())) => {}
            Some(Err(why)) => return Err(why),
            None => {
                let said = tokio::time::timeout(Duration::from_secs(2), complaints).await.ok().and_then(Result::ok).unwrap_or_default();
                let said = said.trim();
                return Err(if said.contains("/login") {
                    NOT_SIGNED_IN.to_string()
                } else if said.contains(UNKNOWN_OPTION) {
                    too_old(self.version().await.as_deref(), said.lines().last().unwrap_or(""))
                } else {
                    format!("Claude Code stopped without an answer{}", if said.is_empty() { ".".to_string() } else { format!(": {}", said.lines().last().unwrap_or("")) })
                });
            }
        }
        Ok(ModelReply { blocks: vec![Block::Text { text: heard.texts.join("\n\n") }], stop: Stop::End, model: heard.model })
    }
}

impl ClaudeCodeBackend {
    /// The installed program's version, as `claude --version` gives it ("1.0.123 (Claude Code)"): its first word.
    async fn version(&self) -> Option<String> {
        let mut command = Command::new(&self.program);
        command.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // no console window
        let output = tokio::time::timeout(Duration::from_secs(5), command.output()).await.ok()?.ok()?;
        String::from_utf8_lossy(&output.stdout).split_whitespace().next().map(String::from)
    }
}

/// What the author is told when the installed Claude Code does not know an option the editor starts it with.
/// `complaint` is the program's own last line, e.g. "error: unknown option '--tools'".
pub fn too_old(version: Option<&str>, complaint: &str) -> String {
    let version = version.map(|version| format!(" (version {version})")).unwrap_or_default();
    format!(
        "Claude Code on this computer is too old for the editor{version}. Update it: run “claude update” in a terminal, \
         or “npm install -g @anthropic-ai/claude-code@latest”, and ask again. (Claude Code said: {complaint})"
    )
}

#[async_trait]
impl Backend for ClaudeCodeBackend {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn provider(&self) -> String {
        "Anthropic (through Claude Code)".into()
    }

    fn runs_tools_itself(&self) -> bool {
        true
    }

    async fn complete(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send)) -> Result<ModelReply, BackendError> {
        self.run(request, on_text, None).await
    }

    async fn complete_with_tools(&self, request: &ModelRequest, on_text: &mut (dyn for<'s> FnMut(&'s str) + Send), run_tool: &mut RunTool<'_>) -> Result<ModelReply, BackendError> {
        self.run(request, on_text, Some(run_tool)).await
    }
}
