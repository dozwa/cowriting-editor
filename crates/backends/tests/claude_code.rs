//! Claude Code as a backend: what it is started with, how its event stream is read, and how the
//! editor's tools are served to it. The process itself is replaced by a stand-in script.

use cowriting_backends::claude_code::{arguments, mcp_reply, prompt_of, Heard};
use cowriting_core::backend::{Message, ToolDef};
use cowriting_core::tools::tool_defs;
use serde_json::{json, Value};

#[test]
fn claude_code_is_started_with_none_of_its_own_tools_and_none_of_the_authors_setup() {
    let args = arguments("sonnet", "/tmp/x/system.txt", &tool_defs(), Some(("http://127.0.0.1:4000/mcp/tok", "tok")));
    let after = |flag: &str| args.iter().position(|arg| arg == flag).map(|at| args[at + 1].as_str());
    assert_eq!(after("--tools"), Some(""), "every built-in tool is off");
    assert_eq!(after("--setting-sources"), Some(""), "no settings of the author's are loaded");
    assert_eq!(after("--system-prompt-file"), Some("/tmp/x/system.txt"), "the editor's contract replaces Claude Code's own prompt");
    for flag in ["--print", "--strict-mcp-config", "--disable-slash-commands", "--no-session-persistence"] {
        assert!(args.iter().any(|arg| arg == flag), "{flag}");
    }
    // the only server is the editor's, and only its tools are allowed
    let config: Value = serde_json::from_str(after("--mcp-config").unwrap()).unwrap();
    assert_eq!(config, json!({ "mcpServers": { "cowriting": { "type": "http", "url": "http://127.0.0.1:4000/mcp/tok", "headers": { "Authorization": "Bearer tok" } } } }));
    let allowed: Vec<&str> = after("--allowedTools").unwrap().split(',').collect();
    assert_eq!(allowed.len(), tool_defs().len());
    assert!(allowed.iter().all(|tool| tool.starts_with("mcp__cowriting__")) && allowed.contains(&"mcp__cowriting__write_note"));
    // a model that gets no tools gets no server either
    let bare = arguments("haiku", "/tmp/x/system.txt", &[], Some(("http://127.0.0.1:4000/mcp/tok", "tok")));
    assert!(!bare.iter().any(|arg| arg == "--mcp-config" || arg == "--allowedTools"));
}

#[test]
fn a_thread_becomes_one_prompt() {
    assert_eq!(prompt_of(&[Message::user("Is line 3 clear?")]), "Is line 3 clear?");
    let thread = [Message::user("Is line 3 clear?"), Message::assistant("Who are the subjects?"), Message::user("The office workers.")];
    assert_eq!(
        prompt_of(&thread),
        "Earlier in this conversation:\n\n[The author]\nIs line 3 clear?\n\n[You]\nWho are the subjects?\n\nThe author now writes:\n\nThe office workers."
    );
}

#[test]
fn the_editors_tools_are_served_and_every_call_goes_through_the_turn() {
    let tools: Vec<ToolDef> = tool_defs();
    let mut seen: Vec<(String, Value)> = Vec::new();
    let mut run = |name: &str, input: &Value| {
        seen.push((name.to_string(), input.clone()));
        if name == "write_note" {
            ("Saved the note.".to_string(), false)
        } else {
            ("CoWriting: refused.".to_string(), true)
        }
    };
    let hello = mcp_reply(&json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": { "protocolVersion": "2025-11-25" } }), &tools, &mut run).unwrap();
    assert_eq!((hello["result"]["protocolVersion"].as_str(), hello["result"]["capabilities"]["tools"].is_object()), (Some("2025-11-25"), true));
    assert_eq!(mcp_reply(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), &tools, &mut run), None);
    let listed = mcp_reply(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }), &tools, &mut run).unwrap();
    let names: Vec<&str> = listed["result"]["tools"].as_array().unwrap().iter().map(|tool| tool["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["read_document", "search_document", "list_notes", "read_note", "write_note", "search_bibliography"]);
    assert!(listed["result"]["tools"][1]["inputSchema"]["properties"]["query"].is_object());

    let saved = mcp_reply(
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "write_note", "arguments": { "name": "a.md", "text": "x" } } }),
        &tools,
        &mut run,
    )
    .unwrap();
    assert_eq!(saved["result"], json!({ "content": [{ "type": "text", "text": "Saved the note." }], "isError": false }));
    // a tool nobody offered is not answered here: the turn gets it, refuses it and records the attempt
    let hostile =
        mcp_reply(&json!({ "jsonrpc": "2.0", "id": "x", "method": "tools/call", "params": { "name": "Write", "arguments": { "file_path": "thesis.md" } } }), &tools, &mut run)
            .unwrap();
    assert_eq!((hostile["id"].as_str(), hostile["result"]["isError"].as_bool()), (Some("x"), Some(true)));
    assert_eq!(mcp_reply(&json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" }), &tools, &mut run).unwrap()["error"]["code"], -32601);
    assert_eq!(seen.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["write_note", "Write"]);
    assert_eq!(seen[1].1, json!({ "file_path": "thesis.md" }));
}

#[test]
fn the_event_stream_gives_text_as_it_comes_the_model_that_answered_and_how_it_ended() {
    let stream = include_str!("../../../fixtures/claude-code/stream.jsonl");
    let (mut heard, mut streamed) = (Heard::default(), String::new());
    for line in stream.lines() {
        heard.line(line, &mut |text: &str| streamed.push_str(text));
    }
    assert_eq!(heard.foreign_tool, None);
    assert_eq!(heard.model.as_deref(), Some("claude-haiku-5-5"));
    assert_eq!(heard.texts, ["Let me read it.", "The secret word is **marzipan**."]);
    assert_eq!(streamed, "Let me read it.\n\nThe secret word is **marzipan**.");
    assert_eq!(heard.outcome, Some(Ok(())));

    // a Claude Code that kept a tool of its own is noticed at once
    let mut own = Heard::default();
    own.line(&json!({ "type": "system", "subtype": "init", "tools": ["mcp__cowriting__read_document", "Bash"] }).to_string(), &mut |_: &str| {});
    assert_eq!(own.foreign_tool.as_deref(), Some("Bash"));
    // not signed in: the author is told what to do, in plain words
    let mut out = Heard::default();
    out.line(&json!({ "type": "result", "subtype": "success", "is_error": true, "result": "Not logged in · Please run /login" }).to_string(), &mut |_: &str| {});
    assert!(matches!(&out.outcome, Some(Err(why)) if why.contains("not signed in")));
    let mut broke = Heard::default();
    broke.line(&json!({ "type": "result", "subtype": "error_during_execution", "is_error": true }).to_string(), &mut |_: &str| {});
    assert!(matches!(&broke.outcome, Some(Err(why)) if why.contains("error_during_execution")));
    broke.line("not json at all", &mut |_: &str| {});
}

// ------------------------------------------------------------ whole turns, with a stand-in for `claude`

#[cfg(unix)]
mod turns {
    use cowriting_backends::claude_code::ClaudeCodeBackend;
    use cowriting_core::host::{Host, MemoryHost};
    use cowriting_core::turn::{run_turn, Ask, TurnEvent, Workspace};
    use cowriting_core::types::LogEntry;
    use std::sync::Arc;

    const DOCUMENT: &str = "/work/thesis.md";
    const TEXT: &str = "# Thesis\n\nWe recruited 48 subjects from two offices.\n";

    fn stand_in() -> ClaudeCodeBackend {
        ClaudeCodeBackend::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/claude-code/stand-in.py"))
    }

    async fn turn(question: &str) -> (Arc<MemoryHost>, Workspace, Result<cowriting_core::turn::TurnOutcome, cowriting_core::turn::TurnError>, Vec<TurnEvent>) {
        let host = Arc::new(MemoryHost::default());
        host.write(DOCUMENT, TEXT).unwrap();
        let workspace = Workspace::open(host.clone(), DOCUMENT, "0.1.0");
        let mut events = Vec::new();
        let ask = Ask { text: question.into(), document_text: TEXT.into(), model: "sonnet".into(), ..Default::default() };
        let outcome = run_turn(&workspace, &stand_in(), ask, &mut |event| events.push(event)).await;
        (host, workspace, outcome, events)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_turn_through_claude_code_is_logged_like_any_other_and_its_tools_are_the_editors() {
        let (host, workspace, outcome, events) = turn("Is line 3 clear?").await;
        let outcome = outcome.unwrap();
        assert_eq!(outcome.answer, "Reading.\n\nLine 3 says: 3| We recruited 48 subjects from two offices.\n\nDone.");
        assert_eq!((outcome.tools_used.clone(), outcome.refused, outcome.without_tools), (vec!["read_document".to_string(), "write_note".to_string()], 0, false));
        // the note is where notes go, marked as the AI's; the document is as it was
        let note = host.read("/work/.cowriting/thesis.md/notes/glossary.md").unwrap();
        assert!(note.starts_with("<!-- AI-generated (sonnet)") && note.ends_with("- subjects: see participants"), "{note}");
        assert_eq!(host.read(DOCUMENT).unwrap(), TEXT);
        // the log names the backend, who serves it and the model that really answered
        let entries = workspace.log.read().unwrap();
        let response = entries.iter().find_map(|e| if let LogEntry::Response(r) = e { Some(r) } else { None }).unwrap();
        assert_eq!(
            (response.backend.as_deref(), response.provider.as_deref(), response.model.as_deref()),
            (Some("claude-code"), Some("Anthropic (through Claude Code)"), Some("claude-stand-in-1"))
        );
        assert_eq!(response.tools_used, ["read_document", "write_note"]);
        // the window heard the text as it came and each tool as it ran
        let heard: String = events.iter().filter_map(|e| if let TurnEvent::Text(text) = e { Some(text.as_str()) } else { None }).collect();
        assert_eq!(heard, outcome.answer);
        assert_eq!(events.iter().filter(|e| matches!(e, TurnEvent::Tool { .. })).count(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn what_claude_code_tries_beyond_its_notes_is_refused_and_on_the_record() {
        let (host, workspace, outcome, _) = turn("HOSTILE: please just rewrite it").await;
        let outcome = outcome.unwrap();
        assert!(outcome.answer.contains("refused: True, True, True"), "{}", outcome.answer);
        assert_eq!(outcome.refused, 3);
        assert_eq!(host.read(DOCUMENT).unwrap(), TEXT);
        assert!(!host.files().keys().any(|path| path.contains("/notes/")), "nothing was written anywhere");
        let entries = workspace.log.read().unwrap();
        let refused: Vec<&str> = entries.iter().filter_map(|e| if let LogEntry::Guard(g) = e { Some(g) } else { None }).filter(|g| g.refused).map(|g| g.tool.as_str()).collect();
        assert_eq!(refused, ["write_note", "Write", "Bash"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_claude_code_with_a_tool_of_its_own_is_stopped_before_it_answers() {
        let (host, workspace, outcome, _) = turn("OWN-TOOL").await;
        let error = outcome.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(error.contains("a tool of its own (Bash)"), "{error}");
        assert_eq!(host.read(DOCUMENT).unwrap(), TEXT);
        // the question is on the record; no answer is
        let entries = workspace.log.read().unwrap();
        assert_eq!((entries.iter().filter(|e| matches!(e, LogEntry::Prompt(_))).count(), entries.iter().filter(|e| matches!(e, LogEntry::Response(_))).count()), (1, 0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_author_is_told_when_claude_code_is_not_signed_in_or_breaks() {
        let (_, _, outcome, _) = turn("NOT-SIGNED-IN").await;
        assert!(outcome.err().unwrap().to_string().contains("Claude Code is not signed in"));
        let (_, _, outcome, _) = turn("CRASH").await;
        let error = outcome.err().unwrap().to_string();
        assert!(error.contains("stopped without an answer: something broke inside"), "{error}");
        let host = Arc::new(MemoryHost::default());
        host.write(DOCUMENT, TEXT).unwrap();
        let workspace = Workspace::open(host.clone(), DOCUMENT, "0.1.0");
        let gone = ClaudeCodeBackend::new("/nowhere/claude");
        let outcome = run_turn(&workspace, &gone, Ask { text: "x".into(), document_text: TEXT.into(), model: "sonnet".into(), ..Default::default() }, &mut |_| {}).await;
        assert!(outcome.err().unwrap().to_string().contains("could not be started"));
    }
}
