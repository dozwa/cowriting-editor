//! The OpenAI-compatible backend: a stream recorded from a local model server, the form OpenAI
//! itself streams tool calls in, and whole turns against a stand-in server, including a model
//! that cannot call tools.

use cowriting_backends::openai::{is_local, request_body, Assembler, OpenAiBackend};
use cowriting_core::backend::{Backend, Block, Message, ModelRequest, Role, Stop};
use cowriting_core::host::{Host, MemoryHost};
use cowriting_core::tools::tool_defs;
use cowriting_core::turn::{run_turn, Ask, Workspace};
use cowriting_core::types::LogEntry;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

fn recorded() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/streams/openai-tool-use.sse")).unwrap()
}

/// A stream as OpenAI sends it: text, then two calls whose arguments arrive in fragments.
fn fragmented() -> String {
    let chunk = |delta: Value, finish: Value| format!("data: {}\n\n", json!({ "model": "gpt-test-2026", "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }] }));
    [
        chunk(json!({ "role": "assistant", "content": "Let me look – " }), Value::Null),
        chunk(json!({ "content": "one moment." }), Value::Null),
        chunk(json!({ "tool_calls": [{ "index": 0, "id": "call_a", "type": "function", "function": { "name": "search_document", "arguments": "" } }] }), Value::Null),
        chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "{\"que" } }] }), Value::Null),
        chunk(json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "ry\":\"Zürich\"}" } }] }), Value::Null),
        chunk(json!({ "tool_calls": [{ "index": 1, "id": "call_b", "type": "function", "function": { "name": "read_document", "arguments": "" } }] }), Value::Null),
        chunk(json!({}), json!("tool_calls")),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat()
}

fn assemble(bytes: &[u8], size: usize) -> (cowriting_core::backend::ModelReply, String) {
    let (mut assembler, mut streamed) = (Assembler::default(), String::new());
    for piece in bytes.chunks(size) {
        assembler.feed(piece, &mut |text: &str| streamed.push_str(text));
    }
    (assembler.finish().unwrap(), streamed)
}

#[test]
fn a_recorded_local_stream_gives_the_call_and_drops_the_reasoning() {
    let bytes = recorded();
    for size in [1, 3, 64, bytes.len()] {
        let (reply, streamed) = assemble(&bytes, size);
        assert_eq!(reply.blocks, [Block::ToolUse { id: "call_esbsknwg".into(), name: "search_document".into(), input: json!({ "query": "subjects" }) }], "chunks of {size}");
        assert_eq!(reply.stop, Stop::ToolUse);
        assert_eq!(reply.model.as_deref(), Some("qwen3.5:9b"));
        assert_eq!(streamed, "", "the model's reasoning is not shown as its answer");
    }
}

#[test]
fn fragmented_calls_are_put_together_wherever_the_stream_is_cut() {
    let bytes = fragmented().into_bytes();
    for size in [1, 2, 5, 33, bytes.len()] {
        let (reply, streamed) = assemble(&bytes, size);
        assert_eq!(streamed, "Let me look – one moment.");
        assert_eq!(
            reply.blocks,
            [
                Block::Text { text: "Let me look – one moment.".into() },
                Block::ToolUse { id: "call_a".into(), name: "search_document".into(), input: json!({ "query": "Zürich" }) },
                Block::ToolUse { id: "call_b".into(), name: "read_document".into(), input: json!({}) },
            ],
            "chunks of {size}"
        );
        assert_eq!(reply.stop, Stop::ToolUse);
    }
}

#[test]
fn a_stream_that_breaks_off_or_reports_an_error_is_no_answer() {
    let whole = fragmented();
    let cut = &whole.as_bytes()[..whole.len() / 2];
    let mut assembler = Assembler::default();
    assembler.feed(cut, &mut |_| {});
    assert_eq!(assembler.finish().unwrap_err().message, "The connection ended before the answer was complete.");

    let mut assembler = Assembler::default();
    assembler.feed(b"data: {\"error\":{\"message\":\"model overloaded\"}}\n\n", &mut |_| {});
    assert_eq!(assembler.finish().unwrap_err().message, "The provider stopped the answer: model overloaded");

    // the length limit and a content filter are told apart from a finished answer
    for (reason, stop) in [("length", Stop::MaxTokens), ("content_filter", Stop::Refusal), ("stop", Stop::End)] {
        let mut assembler = Assembler::default();
        assembler.feed(format!("data: {}\n\n", json!({ "choices": [{ "delta": { "content": "x" }, "finish_reason": reason }] })).as_bytes(), &mut |_| {});
        assert_eq!(assembler.finish().unwrap().stop, stop);
    }
}

#[test]
fn the_request_carries_the_contract_the_tools_and_tool_results_in_the_api_s_order() {
    let request = ModelRequest {
        system: "the contract".into(),
        messages: vec![
            Message::user("Where do I say subjects?"),
            Message {
                role: Role::Assistant,
                content: vec![
                    Block::Opaque(json!({ "type": "thinking" })),
                    Block::ToolUse { id: "call_a".into(), name: "search_document".into(), input: json!({ "query": "subjects" }) },
                ],
            },
            Message { role: Role::User, content: vec![Block::ToolResult { id: "call_a".into(), content: "3| We recruited 48 subjects".into(), is_error: false }] },
        ],
        tools: tool_defs(),
        model: "some-model".into(),
    };
    let body = request_body(&request);
    assert_eq!(body["model"], "some-model");
    assert_eq!(body["stream"], true);
    let roles: Vec<&str> = body["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["system", "user", "assistant", "tool"]);
    assert_eq!(body["messages"][0]["content"], "the contract");
    assert_eq!(body["messages"][2]["tool_calls"][0]["function"], json!({ "name": "search_document", "arguments": "{\"query\":\"subjects\"}" }));
    assert_eq!(body["messages"][3], json!({ "role": "tool", "tool_call_id": "call_a", "content": "3| We recruited 48 subjects" }));
    let names: Vec<&str> = body["tools"].as_array().unwrap().iter().map(|t| t["function"]["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["read_document", "search_document", "list_notes", "read_note", "write_note", "search_bibliography"]);
    assert!(body.get("temperature").is_none() && body.get("max_tokens").is_none());

    // without tools the field is left out: some servers reject an empty list
    assert!(request_body(&ModelRequest { tools: Vec::new(), ..request }).get("tools").is_none());
}

#[test]
fn the_provider_is_named_by_where_the_text_went() {
    assert_eq!(OpenAiBackend::new("", Some("k")).provider(), "OpenAI");
    assert_eq!(OpenAiBackend::new("http://localhost:11434/v1/", None).provider(), "a local model server (localhost:11434)");
    assert_eq!(OpenAiBackend::new("https://llm.uni-example.de/v1", Some("k")).provider(), "OpenAI-compatible API at llm.uni-example.de");
    assert!(is_local("http://127.0.0.1:1234/v1") && is_local("http://localhost/v1") && !is_local("https://localhost.example.org/v1"));
}

/// Answers HTTP requests in order with (status line, content type, body); returns the address and what was asked.
fn serve(replies: Vec<(&'static str, &'static str, Vec<u8>)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let served = std::thread::spawn(move || {
        let mut all = Vec::new();
        for (status, content_type, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            let (mut asked, mut buffer) = (Vec::new(), [0u8; 4096]);
            loop {
                let n = stream.read(&mut buffer).unwrap();
                asked.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&asked).to_string();
                if let Some(at) = text.find("\r\n\r\n") {
                    let length: usize = text.to_lowercase().split("content-length:").nth(1).and_then(|rest| rest.lines().next()).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                    if asked.len() >= at + 4 + length || n == 0 {
                        break;
                    }
                }
            }
            let head = format!("HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
            all.push(String::from_utf8_lossy(&asked).to_string());
        }
        all
    });
    (address, served)
}

fn body_of(http: &str) -> Value {
    serde_json::from_str(http.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

fn answer_stream(text: &str) -> Vec<u8> {
    format!("data: {}\n\ndata: [DONE]\n\n", json!({ "model": "served-model", "choices": [{ "delta": { "content": text }, "finish_reason": "stop" }] })).into_bytes()
}

const DOCUMENT: &str = "/work/thesis.md";
const TEXT: &str = "# Thesis\n\nWe recruited 48 subjects from two offices.\n";

fn workspace() -> (Arc<MemoryHost>, Workspace) {
    let host = Arc::new(MemoryHost::default());
    host.write(DOCUMENT, TEXT).unwrap();
    let workspace = Workspace::open(host.clone(), DOCUMENT, "0.1.0");
    (host, workspace)
}

fn ask() -> Ask {
    Ask { text: "Where do I say subjects?".into(), document_text: TEXT.into(), model: "asked-model".into(), ..Default::default() }
}

#[tokio::test(flavor = "current_thread")]
async fn a_turn_runs_the_model_s_tool_call_and_logs_who_answered() {
    let (address, served) = serve(vec![("200 OK", "text/event-stream", recorded()), ("200 OK", "text/event-stream", answer_stream("Line 3 says subjects."))]);
    let (_host, workspace) = workspace();
    let backend = OpenAiBackend::new(&address, Some("sk-secret"));
    let outcome = run_turn(&workspace, &backend, ask(), &mut |_| {}).await.unwrap();
    assert_eq!((outcome.answer.as_str(), outcome.tools_used.as_slice(), outcome.without_tools), ("Line 3 says subjects.", &["search_document".to_string()][..], false));

    let asked = served.join().unwrap();
    assert!(asked[0].starts_with("POST /v1/chat/completions "));
    assert!(asked[0].to_lowercase().contains("authorization: bearer sk-secret"));
    // the second call carries what the tool found in the author's text
    let second = body_of(&asked[1]);
    let result = second["messages"].as_array().unwrap().iter().find(|m| m["role"] == "tool").unwrap();
    assert_eq!(result["tool_call_id"], "call_esbsknwg");
    assert!(result["content"].as_str().unwrap().contains("3| We recruited 48 subjects"));

    let entries = workspace.log.read().unwrap();
    let Some(LogEntry::Response(response)) = entries.last() else { panic!("a response, got {entries:?}") };
    assert_eq!(response.model.as_deref(), Some("served-model"), "the log names the model that answered");
    assert_eq!(response.backend.as_deref(), Some("openai-compatible"));
    assert!(response.provider.as_deref().unwrap().starts_with("a local model server (127.0.0.1:"));
}

#[tokio::test(flavor = "current_thread")]
async fn a_model_without_tools_is_sent_the_document_instead_and_the_turn_is_logged_all_the_same() {
    let no_tools = br#"{"error":{"message":"registry.ollama.ai/library/gemma3:4b does not support tools","type":"invalid_request_error"}}"#.to_vec();
    let (address, served) = serve(vec![
        ("400 Bad Request", "application/json", no_tools),
        ("200 OK", "text/event-stream", answer_stream("Line 3 says subjects.")),
        ("200 OK", "text/event-stream", answer_stream("Still line 3.")),
    ]);
    let (host, workspace) = workspace();
    let backend = OpenAiBackend::new(&address, None);
    let outcome = run_turn(&workspace, &backend, ask(), &mut |_| {}).await.unwrap();
    assert_eq!(outcome.answer, "Line 3 says subjects.");
    assert!(outcome.without_tools);

    // a second question does not try the tools again
    let again = run_turn(&workspace, &backend, ask(), &mut |_| {}).await.unwrap();
    assert!(again.without_tools);

    let asked = served.join().unwrap();
    assert!(!asked[0].to_lowercase().contains("authorization:"), "no key, no header");
    assert!(body_of(&asked[0]).get("tools").is_some());
    for request in &asked[1..] {
        let body = body_of(request);
        assert!(body.get("tools").is_none());
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains("assist-only") && system.contains("You have no tools"));
        assert!(system.contains("3| We recruited 48 subjects from two offices."), "the document goes with the question");
    }

    let kinds: Vec<&str> = workspace
        .log
        .read()
        .unwrap()
        .iter()
        .map(|e| match e {
            LogEntry::Prompt(_) => "prompt",
            LogEntry::Response(_) => "response",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["prompt", "response", "prompt", "response"]);
    assert_eq!(host.read(DOCUMENT).unwrap(), TEXT);
}

#[tokio::test(flavor = "current_thread")]
async fn a_refused_key_or_an_unreachable_server_is_said_in_plain_words() {
    let (address, served) = serve(vec![("401 Unauthorized", "application/json", br#"{"error":{"message":"Incorrect API key provided"}}"#.to_vec())]);
    let backend = OpenAiBackend::new(&address, Some("wrong"));
    let request = ModelRequest { system: "s".into(), messages: vec![Message::user("hi")], tools: tool_defs(), model: "m".into() };
    let error = backend.complete(&request, &mut |_| {}).await.unwrap_err();
    assert_eq!(error.message, "The API key was not accepted (Incorrect API key provided).");
    assert!(backend.uses_tools(), "a refused key says nothing about tools");
    served.join().unwrap();

    // nothing listens here any more
    let error = backend.models().await.unwrap_err();
    assert!(error.message.starts_with("127.0.0.1:") && error.message.ends_with("Is the local model server running?"), "{}", error.message);
}

#[tokio::test(flavor = "current_thread")]
async fn the_models_a_server_offers_are_listed() {
    let (address, served) = serve(vec![("200 OK", "application/json", br#"{"object":"list","data":[{"id":"qwen3.5:9b"},{"id":"gemma4:12b"}]}"#.to_vec())]);
    let models = OpenAiBackend::new(&address, None).models().await.unwrap();
    assert_eq!(models, [("gemma4:12b".to_string(), "gemma4:12b".to_string()), ("qwen3.5:9b".to_string(), "qwen3.5:9b".to_string())]);
    assert!(served.join().unwrap()[0].starts_with("GET /v1/models "));
}
