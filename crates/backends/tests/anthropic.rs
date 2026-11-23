//! The Claude API backend against a recorded stream: what the model said, what it asked for, and
//! what must go back to it unchanged.

use cowriting_backends::anthropic::{request_body, AnthropicBackend, Assembler};
use cowriting_core::backend::{Backend, Block, Message, ModelRequest, Role, Stop};
use cowriting_core::tools::tool_defs;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;

fn recorded() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/streams/anthropic-tool-use.sse")).unwrap()
}

fn request() -> ModelRequest {
    ModelRequest { system: "the contract".into(), messages: vec![Message::user("Where do I say subjects?")], tools: tool_defs(), model: "claude-opus-5-5".into() }
}

/// Answers one HTTP request with `status` and `body`, and returns the address and what was asked.
fn serve_once(status: &'static str, content_type: &'static str, body: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let served = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut asked = Vec::new();
        let mut buffer = [0u8; 4096];
        // headers, then as many body bytes as Content-Length says
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
        String::from_utf8_lossy(&asked).to_string()
    });
    (address, served)
}

#[test]
fn a_stream_cut_anywhere_gives_the_same_message() {
    let bytes = recorded();
    // every chunk size, so an event and a multi-byte character are split at every possible place
    for size in [1, 2, 3, 7, 64, bytes.len()] {
        let mut assembler = Assembler::default();
        let mut streamed = String::new();
        for chunk in bytes.chunks(size) {
            assembler.feed(chunk, &mut |text: &str| streamed.push_str(text));
        }
        let reply = assembler.finish().unwrap();
        assert_eq!(streamed, "Ich lese zuerst die Einführung – Größe zählt.", "chunks of {size}");
        assert_eq!(reply.stop, Stop::ToolUse);
        assert_eq!(reply.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(reply.blocks.len(), 3);
        // the reasoning block is kept whole, to go back as it came
        assert_eq!(reply.blocks[0], Block::Opaque(json!({ "type": "thinking", "thinking": "", "signature": "c2lnbmF0dXJl" })));
        assert_eq!(reply.blocks[1], Block::Text { text: streamed.clone() });
        assert_eq!(reply.blocks[2], Block::ToolUse { id: "toolu_01".into(), name: "search_document".into(), input: json!({ "query": "subjects" }) });
    }
}

#[test]
fn a_stream_that_breaks_off_is_an_error_not_an_answer() {
    let bytes = recorded();
    let mut assembler = Assembler::default();
    assembler.feed(&bytes[..bytes.len() / 2], &mut |_: &str| {});
    assert!(assembler.finish().unwrap_err().message.contains("before the answer was complete"));

    let mut assembler = Assembler::default();
    assembler.feed(b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n", &mut |_: &str| {});
    assert!(assembler.finish().unwrap_err().message.contains("Overloaded"));
}

#[test]
fn the_request_carries_the_tools_and_sends_a_turn_back_as_it_came() {
    let mut request = request();
    request.messages.push(Message {
        role: Role::Assistant,
        content: vec![
            Block::Opaque(json!({ "type": "thinking", "thinking": "", "signature": "c2ln" })),
            Block::Text { text: String::new() },
            Block::ToolUse { id: "toolu_01".into(), name: "read_document".into(), input: json!({}) },
        ],
    });
    request.messages.push(Message { role: Role::User, content: vec![Block::ToolResult { id: "toolu_01".into(), content: "1| text".into(), is_error: false }] });
    let body = request_body(&request);
    assert_eq!(body["stream"], true);
    assert_eq!(body["system"], "the contract");
    // sampling parameters are rejected by the current API version
    for gone in ["temperature", "top_p", "top_k"] {
        assert!(body.get(gone).is_none(), "{gone} is not sent");
    }
    let names: Vec<&str> = body["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["read_document", "search_document", "list_notes", "read_note", "write_note", "search_bibliography"]);
    // the reasoning block goes back unchanged, the empty text block not at all
    assert_eq!(
        body["messages"][1]["content"],
        json!([{ "type": "thinking", "thinking": "", "signature": "c2ln" }, { "type": "tool_use", "id": "toolu_01", "name": "read_document", "input": {} }])
    );
    assert_eq!(body["messages"][2]["content"], json!([{ "type": "tool_result", "tool_use_id": "toolu_01", "content": "1| text", "is_error": false }]));
}

#[tokio::test(flavor = "current_thread")]
async fn over_http_the_answer_streams_and_the_key_goes_in_its_header() {
    let (address, served) = serve_once("200 OK", "text/event-stream", recorded());
    let backend = AnthropicBackend::new("sk-test-key", Some(&address));
    let mut streamed = String::new();
    let reply = backend.complete(&request(), &mut |text: &str| streamed.push_str(text)).await.unwrap();
    assert_eq!(streamed, "Ich lese zuerst die Einführung – Größe zählt.");
    assert_eq!(reply.stop, Stop::ToolUse);
    let asked = served.join().unwrap().to_lowercase();
    assert!(asked.starts_with("post /v1/messages "));
    assert!(asked.contains("x-api-key: sk-test-key"));
    assert!(asked.contains("anthropic-version: 2026-10-21"));
    // a gateway is named by its host, so the disclosure says who the text went to
    assert!(backend.provider().starts_with("Claude API at 127.0.0.1"));
    assert_eq!(AnthropicBackend::new("k", None).provider(), "Anthropic");
}

#[tokio::test(flavor = "current_thread")]
async fn a_refused_key_is_said_in_plain_words() {
    let body = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.to_vec();
    let (address, served) = serve_once("401 Unauthorized", "application/json", body);
    let error = AnthropicBackend::new("wrong", Some(&address)).complete(&request(), &mut |_: &str| {}).await.unwrap_err();
    assert_eq!(error.message, "The API key was not accepted (invalid x-api-key).");
    served.join().unwrap();
}
