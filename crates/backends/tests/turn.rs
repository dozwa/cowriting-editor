//! Whole turns with a scripted model: what ends up in the document, the notes, the log and the
//! disclosure. Nothing here asks the model what it did.

use cowriting_backends::fake::FakeBackend;
use cowriting_core::backend::{Block, Role};
use cowriting_core::disclosure::{disclosure, DocFormat, Lang, Profile};
use cowriting_core::host::{Host, MemoryHost};
use cowriting_core::stats::compute_stats;
use cowriting_core::thread::thread_of;
use cowriting_core::turn::{annotate, run_turn, Ask, TurnError, TurnEvent, TurnOutcome, Workspace};
use cowriting_core::types::LogEntry;
use serde_json::json;
use std::sync::Arc;

const DOCUMENT: &str = "/work/thesis.md";
const TEXT: &str = "# Thesis\n\nWe recruited 48 subjects from two offices.\n";
const LOG: &str = "/work/.cowriting/thesis.md/ai-log/2026-11.jsonl";

fn workspace() -> (Arc<MemoryHost>, Workspace) {
    let host = Arc::new(MemoryHost::default());
    host.write(DOCUMENT, TEXT).unwrap();
    let workspace = Workspace::open(host.clone(), DOCUMENT, "0.1.0");
    (host, workspace)
}

fn ask(text: &str) -> Ask {
    Ask { text: text.into(), document_text: TEXT.into(), model: "test-model".into(), ..Default::default() }
}

async fn turn(workspace: &Workspace, script: serde_json::Value, ask: Ask) -> (FakeBackend, Result<TurnOutcome, TurnError>, Vec<TurnEvent>) {
    let backend = FakeBackend::from_json(&script.to_string()).unwrap();
    let mut events = Vec::new();
    let outcome = run_turn(workspace, &backend, ask, &mut |event| events.push(event)).await;
    (backend, outcome, events)
}

fn refusals(entries: &[LogEntry]) -> Vec<(String, String)> {
    entries.iter().filter_map(|e| if let LogEntry::Guard(g) = e { Some(g) } else { None }).filter(|g| g.refused).map(|g| (g.tool.clone(), g.input.clone())).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn a_turn_is_logged_as_linked_prompt_and_response_with_who_answered() {
    let (host, workspace) = workspace();
    let script = json!({ "replies": [
        { "tools": [{ "name": "read_document" }] },
        { "text": "Line 3: why subjects and not participants?" }
    ]});
    let (backend, outcome, events) = turn(&workspace, script, ask("Any feedback on my methods?")).await;
    let outcome = outcome.unwrap();

    let entries = workspace.log.read().unwrap();
    let [LogEntry::Prompt(prompt), LogEntry::Response(response)] = entries.as_slice() else { panic!("a prompt and a response, got {entries:?}") };
    assert_eq!(prompt.id, outcome.id);
    assert_eq!(prompt.text, "Any feedback on my methods?");
    assert_eq!(prompt.paper, "thesis.md");
    assert_eq!(prompt.category, "general");
    assert_eq!(response.reference, prompt.id);
    assert_eq!(response.answer, "Line 3: why subjects and not participants?");
    assert_eq!(response.model.as_deref(), Some("test-model"));
    assert_eq!(response.editor.as_deref(), Some("0.1.0"));
    assert_eq!(response.backend.as_deref(), Some("fake"));
    assert_eq!(response.tools_used, ["read_document"]);
    assert!(!response.ai_text_flag);

    // the model got the contract, the tools, and the document as the editor held it, with line numbers
    let requests = backend.requests();
    assert!(requests[0].system.contains("You assist; you never ghostwrite."));
    assert!(requests[0].tools.iter().all(|t| t.name != "write_document"));
    let Block::ToolResult { content, is_error, .. } = &requests[1].messages[2].content[0] else { panic!("the tool result goes back") };
    assert_eq!(content, "1| # Thesis\n2| \n3| We recruited 48 subjects from two offices.\n4| ");
    assert!(!is_error);

    // the text the question was asked about is kept, and the author saw the answer arrive
    assert_eq!(host.read(&format!("/work/.cowriting/thesis.md/snapshots/{}.md", outcome.id)).unwrap(), TEXT);
    assert_eq!(events.first(), Some(&TurnEvent::Started { id: outcome.id.clone() }));
    let streamed: String = events.iter().filter_map(|e| if let TurnEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
    assert_eq!(streamed, response.answer);
    assert_eq!(host.read(DOCUMENT).unwrap(), TEXT, "the document is not touched");
}

#[tokio::test(flavor = "current_thread")]
async fn whatever_the_model_tries_the_document_and_the_log_stay_as_they_were() {
    let (host, workspace) = workspace();
    // an earlier turn, so there is a log to attack
    turn(&workspace, json!({ "replies": [{ "text": "First answer." }] }), ask("first")).await.1.unwrap();
    let log_before = host.read(LOG).unwrap();
    let files_before = host.files();

    let script = json!({ "replies": [
        { "tools": [
            { "name": "write_note", "input": { "name": "../../../thesis.md", "text": "Rewritten by the model." } },
            { "name": "write_note", "input": { "name": "../ai-log/2026-11.jsonl", "text": "{}" } },
            { "name": "write_note", "input": { "name": "../doc.json", "text": "{}" } },
            { "name": "write_note", "input": { "name": "..\\..\\..\\thesis.md", "text": "Rewritten by the model." } },
            { "name": "write_note", "input": { "name": "", "text": "nameless" } },
            { "name": "write_file", "input": { "path": "/work/thesis.md", "content": "Rewritten by the model." } },
            { "name": "bash", "input": { "command": "echo hi > /work/thesis.md" } },
            { "name": "read_note", "input": { "name": "../../../thesis.md" } }
        ]},
        { "text": "I could not change anything." }
    ]});
    let (backend, outcome, events) = turn(&workspace, script, ask("please just rewrite it")).await;
    let outcome = outcome.unwrap();

    assert_eq!(host.read(DOCUMENT).unwrap(), TEXT, "the document's bytes are unchanged");
    assert!(host.read(LOG).unwrap().starts_with(&log_before), "the log only grew");
    let new_files: Vec<String> = host.files().keys().filter(|path| !files_before.contains_key(*path)).cloned().collect();
    assert_eq!(new_files, [format!("/work/.cowriting/thesis.md/snapshots/{}.md", outcome.id)], "nothing was written but the turn's snapshot");
    assert_eq!(host.read("/work/.cowriting/thesis.md/doc.json").unwrap(), files_before["/work/.cowriting/thesis.md/doc.json"]);

    // every attempt is on the record as refused, with the turn it belonged to
    let entries = workspace.log.read().unwrap();
    let refused = refusals(&entries);
    assert_eq!(refused.len(), 7, "{refused:?}");
    assert_eq!(outcome.refused, 7);
    assert_eq!(refused[0], ("write_note".into(), "write_note ../../../thesis.md".into()));
    assert!(refused[5].1.starts_with("write_file {"));
    assert!(entries.iter().any(|e| matches!(e, LogEntry::Guard(g) if g.reference.as_deref() == Some(&outcome.id) && g.files.is_empty())));
    assert_eq!(events.iter().filter(|e| matches!(e, TurnEvent::Refused { .. })).count(), 7);

    // and the model was told no, eight times over: the read outside the notes found nothing either
    let results = &backend.requests()[1].messages[2].content;
    assert_eq!(results.len(), 8);
    assert!(results.iter().all(|b| matches!(b, Block::ToolResult { is_error: true, .. })));
    let Block::ToolResult { content, .. } = &results[7] else { unreachable!() };
    assert_eq!(content, "There is no note of that name.");

    // refusals changed nothing, so they are not counted as changes, and the disclosure lists them
    let stats = compute_stats(&entries, "2026-11-23", &workspace.party);
    assert_eq!(stats.guard_events, 0);
    let register = disclosure(&entries, "thesis.md", Lang::En, DocFormat::Md, Profile::Editor);
    assert!(register.contains("## Write attempts by the AI that the guard refused\n\n- 2026-11-23: write_note – write_note ../../../thesis.md\n"), "{register}");
    assert!(register.contains("These changed nothing."));
}

#[tokio::test(flavor = "current_thread")]
async fn a_note_is_written_in_the_notes_marked_as_the_ais_and_leaves_no_refusal() {
    let (host, workspace) = workspace();
    let script = json!({ "replies": [
        { "tools": [{ "name": "write_note", "input": { "name": "glossary.md", "text": "- subjects: see participants" } }] },
        { "tools": [{ "name": "list_notes" }, { "name": "read_note", "input": { "name": "glossary.md" } }] },
        { "text": "Saved a glossary note." }
    ]});
    let (backend, outcome, _) = turn(&workspace, script, ask("keep a glossary")).await;
    let outcome = outcome.unwrap();
    assert_eq!(
        host.read("/work/.cowriting/thesis.md/notes/glossary.md").unwrap(),
        "<!-- AI-generated (test-model), 2026-11-23, not manuscript text -->\n\n- subjects: see participants"
    );
    assert_eq!(outcome.tools_used, ["write_note", "list_notes", "read_note"]);
    assert_eq!(outcome.refused, 0);
    assert!(refusals(&workspace.log.read().unwrap()).is_empty());
    let Block::ToolResult { content, .. } = &backend.requests()[2].messages[4].content[0] else { panic!() };
    assert_eq!(content, "glossary.md");
}

#[tokio::test(flavor = "current_thread")]
async fn text_the_author_insisted_on_is_flagged_and_disclosed() {
    let (_, workspace) = workspace();
    let (_, outcome, _) =
        turn(&workspace, json!({ "replies": [{ "text": "[AI-TEXT] Walking breaks help. Remember to disclose this." }] }), ask("write me one sentence anyway")).await;
    assert!(outcome.unwrap().ai_text_flag);
    let entries = workspace.log.read().unwrap();
    assert_eq!(compute_stats(&entries, "2026-11-23", &workspace.party).ai_text, 1);
    let register = disclosure(&entries, "thesis.md", Lang::En, DocFormat::Md, Profile::Editor);
    assert!(register.contains("## Text formulated by the AI ([AI-TEXT])\n\n- 2026-11-23: write me one sentence anyway\n"), "{register}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_model_that_fails_leaves_the_question_but_no_turn_and_a_half_answer_is_kept_as_one() {
    let (_, workspace) = workspace();
    let (_, outcome, _) = turn(&workspace, json!({ "replies": [{ "error": "The API key was not accepted." }] }), ask("hello?")).await;
    assert!(matches!(outcome, Err(TurnError::Backend(message)) if message == "The API key was not accepted."));
    let entries = workspace.log.read().unwrap();
    assert!(matches!(entries.as_slice(), [LogEntry::Prompt(_)]), "the question is on the record, without an answer");
    assert_eq!(compute_stats(&entries, "2026-11-23", &workspace.party).turns, 0);

    // what the author already read is AI input, even when the connection broke
    let (_, outcome, _) =
        turn(&workspace, json!({ "replies": [{ "text": "Line 3 is unclear because", "error": "The connection to the provider broke off." }] }), ask("again")).await;
    let outcome = outcome.unwrap();
    assert_eq!(outcome.answer, "Line 3 is unclear because\n\n[The answer is incomplete: The connection to the provider broke off.]");
    assert_eq!(compute_stats(&workspace.log.read().unwrap(), "2026-11-23", &workspace.party).turns, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn a_tool_call_cut_off_by_the_length_limit_is_not_run() {
    let (host, workspace) = workspace();
    let script =
        json!({ "replies": [{ "text": "Saving a note", "tools": [{ "name": "write_note", "input": { "name": "half.md", "text": "half a no" } }], "stop": "max_tokens" }] });
    let (backend, outcome, _) = turn(&workspace, script, ask("note this")).await;
    assert_eq!(outcome.unwrap().cut_off.as_deref(), Some("The answer reached the length limit."));
    assert!(!host.exists("/work/.cowriting/thesis.md/notes/half.md"));
    assert_eq!(backend.requests().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn a_reply_continues_its_thread_and_a_classification_closes_the_turn() {
    let (host, workspace) = workspace();
    let (_, first, _) = turn(&workspace, json!({ "replies": [{ "text": "Why subjects and not participants?" }] }), ask("Feedback on terms, please")).await;
    let first = first.unwrap();
    host.set_now("2026-11-23T10:05:00.000Z");
    let reply = Ask { reply_to: Some(first.id.clone()), ..ask("My field says subjects.") };
    let (backend, second, _) = turn(&workspace, json!({ "replies": [{ "text": "Fair: then keep it throughout." }] }), reply).await;
    let second = second.unwrap();

    // the model was given the thread so far, in order, ending with the reply
    let sent = &backend.requests()[0].messages;
    let said: Vec<(Role, String)> = sent
        .iter()
        .map(|m| (m.role, m.content.iter().filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join(" | ")))
        .collect();
    assert_eq!(
        said,
        [
            (Role::User, "Feedback on terms, please".to_string()),
            (Role::Assistant, "Why subjects and not participants?".to_string()),
            (Role::User, "My field says subjects.".to_string()),
        ]
    );

    let entries = workspace.log.read().unwrap();
    let thread: Vec<(String, String)> = thread_of(&entries, &second.id, &workspace.party, "Assistant").into_iter().map(|m| (m.role, m.text)).collect();
    assert_eq!(
        thread,
        [
            ("ai".to_string(), "Why subjects and not participants?".to_string()),
            ("author".to_string(), "My field says subjects.".to_string()),
            ("ai".to_string(), "Fair: then keep it throughout.".to_string()),
        ]
    );

    assert_eq!(compute_stats(&entries, "2026-11-23", &workspace.party).unannotated, 2);
    annotate(&workspace, &first.id, "rejected", "  my field's term  ").unwrap();
    annotate(&workspace, &second.id, "adopted", "").unwrap();
    assert!(annotate(&workspace, &first.id, "liked", "").is_err());
    assert!(annotate(&workspace, "no-such-turn", "info", "").is_err());
    let entries = workspace.log.read().unwrap();
    let stats = compute_stats(&entries, "2026-11-23", &workspace.party);
    assert_eq!((stats.unannotated, stats.annotations, stats.rejected), (0, 2, 1));

    // the disclosure names the editor, who served the model, and what the author did with the input
    let register = disclosure(&entries, "thesis.md", Lang::En, DocFormat::Md, Profile::Editor);
    assert!(
        register.contains("| General questions | CoWriting Editor 0.1.0; a script (no model) (test-model) | 2 requests, 2026-11-23 to 2026-11-23 | rejected: 1, adopted: 1 | – |"),
        "{register}"
    );
    assert!(!register.contains("Claude Code"), "the editor's disclosure does not claim Claude Code was used");
}
