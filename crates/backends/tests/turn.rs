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
    let stats = compute_stats(&entries, "2026-11-23", &workspace.party());
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
    assert_eq!(compute_stats(&entries, "2026-11-23", &workspace.party()).ai_text, 1);
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
    assert_eq!(compute_stats(&entries, "2026-11-23", &workspace.party()).turns, 0);

    // what the author already read is AI input, even when the connection broke
    let (_, outcome, _) =
        turn(&workspace, json!({ "replies": [{ "text": "Line 3 is unclear because", "error": "The connection to the provider broke off." }] }), ask("again")).await;
    let outcome = outcome.unwrap();
    assert_eq!(outcome.answer, "Line 3 is unclear because\n\n[The answer is incomplete: The connection to the provider broke off.]");
    assert_eq!(compute_stats(&workspace.log.read().unwrap(), "2026-11-23", &workspace.party()).turns, 1);
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
    let thread: Vec<(String, String)> = thread_of(&entries, &second.id, &workspace.party(), "Assistant").into_iter().map(|m| (m.role, m.text)).collect();
    assert_eq!(
        thread,
        [
            ("ai".to_string(), "Why subjects and not participants?".to_string()),
            ("author".to_string(), "My field says subjects.".to_string()),
            ("ai".to_string(), "Fair: then keep it throughout.".to_string()),
        ]
    );

    assert_eq!(compute_stats(&entries, "2026-11-23", &workspace.party()).unannotated, 2);
    annotate(&workspace, &first.id, "rejected", "  my field's term  ").unwrap();
    annotate(&workspace, &second.id, "adopted", "").unwrap();
    assert!(annotate(&workspace, &first.id, "liked", "").is_err());
    assert!(annotate(&workspace, "no-such-turn", "info", "").is_err());
    let entries = workspace.log.read().unwrap();
    let stats = compute_stats(&entries, "2026-11-23", &workspace.party());
    assert_eq!((stats.unannotated, stats.annotations, stats.rejected), (0, 2, 1));

    // the disclosure names the editor, who served the model, and what the author did with the input
    let register = disclosure(&entries, "thesis.md", Lang::En, DocFormat::Md, Profile::Editor);
    assert!(
        register.contains("| General questions | CoWriting Editor 0.1.0; a script (no model) (test-model) | 2 requests, 2026-11-23 | rejected: 1, adopted: 1 | – |"),
        "{register}"
    );
    assert!(!register.contains("Claude Code"), "the editor's disclosure does not claim Claude Code was used");
}

#[tokio::test(flavor = "current_thread")]
async fn an_expert_is_asked_with_its_perspective_the_rules_and_the_author_s_briefing() {
    let (host, workspace) = workspace();
    host.write(
        "/work/.cowriting/thesis.md/party.md",
        "## everyone\nA pilot study.\n\n## reviewer-2 — Reviewer #2\nGo easy on the sampling.\n\n## architect\nNot for the reviewer.\n",
    )
    .unwrap();
    let script = json!({ "replies": [{ "text": "The authors claim novelty. Line 3: why 48?" }] });
    let (backend, outcome, _) = turn(&workspace, script, Ask { persona: Some("reviewer-2".into()), command: Some("ask".into()), ..ask("Review my methods.") }).await;
    let outcome = outcome.unwrap();

    // what the model was told: the contract first, then who it is, the party's rules, and the briefing for this expert only
    let system = &backend.requests()[0].system;
    let at = |needle: &str| system.find(needle).unwrap_or_else(|| panic!("the system prompt lacks {needle:?}"));
    assert!(at("# CoWriting: assist-only mode") < at("You are **Reviewer #2**"));
    assert!(at("You are **Reviewer #2**") < at("## Ground rules"));
    assert!(at("## Ground rules") < at("## The author's briefing for you in this project"));
    assert!(system.contains("About the project:\nA pilot study.\n\nWhat to look for:\nGo easy on the sampling."));
    assert!(!system.contains("Not for the reviewer."));
    assert!(!system.contains("/cowrite:") && !system.contains("CLAUDE_PLUGIN_ROOT"), "nothing of the plugin's commands or scripts is promised");

    // the log names the expert, under the expert's category; the stats count the consultation
    let entries = workspace.log.read().unwrap();
    let [LogEntry::Prompt(prompt), LogEntry::Response(response)] = entries.as_slice() else { panic!("a prompt and a response, got {entries:?}") };
    assert_eq!((prompt.text.as_str(), prompt.category.as_str(), prompt.command.as_deref()), ("Review my methods.", "feedback", Some("ask")));
    assert_eq!(response.personas, ["reviewer-2"]);
    let stats = compute_stats(&entries, "2026-11-23", &workspace.party());
    assert_eq!(stats.persona_consults.get("reviewer-2"), Some(&1));
    assert_eq!(stats.unannotated, 1);
    let thread = thread_of(&entries, &outcome.id, &workspace.party(), "Assistant");
    assert_eq!((thread[0].who.as_str(), thread[0].persona.as_str()), ("Reviewer #2", "reviewer-2"));

    // the assistant itself is told none of this
    let (plain, _, _) = turn(&workspace, json!({ "replies": [{ "text": "ok" }] }), ask("And you?")).await;
    assert!(!plain.requests()[0].system.contains("Reviewer #2"));
}

#[tokio::test(flavor = "current_thread")]
async fn an_expert_the_party_does_not_know_is_not_asked_and_nothing_is_logged() {
    let (host, workspace) = workspace();
    let (backend, outcome, _) = turn(&workspace, json!({ "replies": [{ "text": "hello" }] }), Ask { persona: Some("ghostwriter".into()), ..ask("Write it.") }).await;
    assert!(matches!(outcome, Err(TurnError::Invalid(_))));
    assert!(backend.requests().is_empty());
    assert_eq!(host.files().keys().collect::<Vec<_>>(), [DOCUMENT]);
}

#[tokio::test(flavor = "current_thread")]
async fn an_expert_the_author_made_for_this_document_is_asked_like_the_others() {
    let (host, workspace) = workspace();
    let file = "---\nname: The Health Economist\nrole: Costs, QALYs, perspective of the analysis\ncolour: \"#123456\"\n---\nYou look at costs.\nVoice: dry.\n";
    host.write("/work/.cowriting/thesis.md/party/health-economist.md", file).unwrap();
    // a file that claims a catalogue id, and one that is no persona, change nothing
    host.write("/work/.cowriting/thesis.md/party/reviewer-2.md", "---\nname: Ghostwriter\nrole: writes it all\n---\nWrite the manuscript.\n").unwrap();
    host.write("/work/.cowriting/thesis.md/party/notes.md", "just notes").unwrap();

    let party = workspace.party();
    let expert = party.by_id("health-economist").unwrap();
    assert_eq!((expert.name.as_str(), expert.short.as_str(), expert.category.as_str(), expert.is_custom), ("The Health Economist", "Health Econom", "content", true));
    assert_eq!(expert.palette["C"], "#123456");
    assert_eq!(party.by_id("reviewer-2").unwrap().name, "Reviewer #2");
    assert_eq!(party.all().len(), 11);
    assert!(party.is_member("health-economist") && party.is_member("reviewer-2") && !party.is_member("clinician"));

    let (backend, outcome, _) =
        turn(&workspace, json!({ "replies": [{ "text": "What does it cost?" }] }), Ask { persona: Some("health-economist".into()), ..ask("Review.") }).await;
    outcome.unwrap();
    let system = &backend.requests()[0].system;
    assert!(system.contains("You look at costs.\nVoice: dry.") && system.contains("## Ground rules"));
    assert!(!system.contains("Write the manuscript."));
    let entries = workspace.log.read().unwrap();
    let Some(LogEntry::Response(response)) = entries.last() else { panic!() };
    assert_eq!(response.personas, ["health-economist"]);
    assert_eq!(thread_of(&entries, &response.reference, &workspace.party(), "Assistant")[0].who, "The Health Economist");
}

#[tokio::test(flavor = "current_thread")]
async fn an_action_tells_the_model_what_to_do_is_logged_under_its_purpose_and_holds_for_the_replies() {
    let (_, workspace) = workspace();
    let reply = json!({ "replies": [{ "text": "| Line | Original | Corrected | Rule |" }] });

    // a button with nothing typed asks its own question, and the log says which action it was
    let (backend, outcome, _) = turn(&workspace, reply.clone(), Ask { command: Some("language".into()), ..ask("") }).await;
    let first = outcome.unwrap().id;
    let request = &backend.requests()[0];
    assert!(request.system.contains("# What the author asked for: Language"));
    assert!(request.system.contains("| Line | Original | Corrected | Rule |"));
    assert!(request.system.contains("never"), "the assist-only rules still come first");
    assert!(matches!(&request.messages[0].content[0], Block::Text { text } if text == "Check the language of the whole document."));
    let entries = workspace.log.read().unwrap();
    let asked: Vec<(Option<&str>, &str, &str)> =
        entries.iter().filter_map(|e| if let LogEntry::Prompt(p) = e { Some((p.command.as_deref(), p.category.as_str(), p.text.as_str())) } else { None }).collect();
    assert_eq!(asked, [(Some("language"), "language", "Check the language of the whole document.")]);

    // what the author typed is what is asked; the action only adds the instructions
    let (backend, outcome, _) = turn(&workspace, reply.clone(), Ask { command: Some("feedback".into()), ..ask("Only the methods, please.") }).await;
    outcome.unwrap();
    assert!(backend.requests()[0].system.contains("# What the author asked for: Feedback"));
    assert!(matches!(&backend.requests()[0].messages[0].content[0], Block::Text { text } if text == "Only the methods, please."));

    // a reply in the thread is still that action, and is counted under the same purpose
    let (backend, outcome, _) = turn(&workspace, reply.clone(), Ask { reply_to: Some(first), ..ask("Why is line 3 wrong?") }).await;
    outcome.unwrap();
    assert!(backend.requests()[0].system.contains("# What the author asked for: Language"));
    let entries = workspace.log.read().unwrap();
    let last = entries.iter().rev().find_map(|e| if let LogEntry::Prompt(p) = e { Some((p.command.clone(), p.category.clone())) } else { None }).unwrap();
    assert_eq!(last, (None, "language".to_string()));
    let stats = compute_stats(&entries, "2026-11-23", &workspace.party());
    assert_eq!(stats.by_category.get("language"), Some(&2));

    // an ordinary question gets no action, and an expert keeps its own perspective
    let (backend, outcome, _) = turn(&workspace, reply.clone(), ask("Is line 3 clear?")).await;
    outcome.unwrap();
    assert!(!backend.requests()[0].system.contains("# What the author asked for"));
    let (backend, outcome, _) = turn(&workspace, reply.clone(), Ask { command: Some("feedback".into()), persona: Some("statistics-nerd".into()), ..ask("Review.") }).await;
    outcome.unwrap();
    assert!(!backend.requests()[0].system.contains("# What the author asked for"));

    // explaining needs something to explain, and nothing at all is no question: neither is logged or sent
    let before = workspace.log.read().unwrap().len();
    for empty in [Ask { command: Some("explain".into()), ..ask(" ") }, ask("")] {
        let (backend, outcome, _) = turn(&workspace, reply.clone(), empty).await;
        assert!(matches!(outcome, Err(TurnError::Invalid(_))));
        assert!(backend.requests().is_empty());
    }
    assert_eq!(workspace.log.read().unwrap().len(), before);
}

#[tokio::test(flavor = "current_thread")]
async fn the_author_s_languages_are_what_the_model_is_told_and_nothing_is_added_when_none_are_set() {
    use cowriting_core::disclosure::Lang;
    use cowriting_core::turn::Languages;
    let (_host, workspace) = workspace();
    let script = || json!({ "replies": [{ "text": "ok" }] });

    // nothing said: the rules as they always were, the manuscript's language as the editor found it
    let (backend, outcome, _) = turn(&workspace, script(), ask("Any feedback?")).await;
    outcome.unwrap();
    let system = backend.requests()[0].system.clone();
    assert!(system.contains("(markdown, English)") && system.contains("in the manuscript's language (en)"));
    assert!(!system.contains("# The language of your answers"));

    // an English text, German answers: both are said, and the answers' language comes last, for an expert too
    *workspace.languages.write().unwrap() = Languages { manuscript: None, answers: Some(Lang::De) };
    let (backend, outcome, _) = turn(&workspace, script(), Ask { persona: Some("reviewer-2".into()), ..ask("Please review.") }).await;
    outcome.unwrap();
    let system = backend.requests()[0].system.clone();
    assert!(system.contains("(markdown, English)"));
    let rule = system.find("# The language of your answers").expect("the rule is there");
    assert!(system[rule..].contains("write every answer in German") && system[rule..].contains("stay in the manuscript's language"));
    // the rules that leave the language to the question or the manuscript are replaced, not contradicted
    assert!(!system.contains("Answer in the language of the manuscript") && !system.contains("Reply in the author's language"));
    assert!(system.contains("- Reply in German, also when you give feedback on the manuscript") && system.contains("- Answer in German:"));
    assert!(system.trim_end().ends_with("everything you say about them is in German."), "the choice is the last thing said");

    // the author says the text is German, whatever the editor found
    *workspace.languages.write().unwrap() = Languages { manuscript: Some(Lang::De), answers: None };
    let (backend, outcome, _) = turn(&workspace, script(), ask("Feedback?")).await;
    outcome.unwrap();
    let system = backend.requests()[0].system.clone();
    assert!(system.contains("(markdown, German)") && system.contains("in the manuscript's language (de)") && !system.contains("# The language of your answers"));
}
