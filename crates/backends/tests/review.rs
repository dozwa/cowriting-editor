//! Live review and the point check with a scripted model: what is sent, when, and what ends up
//! in the log and the sidecar. The clock is the test's.

use cowriting_backends::fake::FakeBackend;
use cowriting_core::host::{Host, MemoryHost};
use cowriting_core::review::{check_points, live_review, ms_of, read_check, LiveOutcome, LiveSettings, LiveState};
use cowriting_core::scope::PointStatus;
use cowriting_core::turn::{TurnError, Workspace};
use cowriting_core::types::LogEntry;
use cowriting_core::watch::Pace;
use serde_json::json;
use std::sync::Arc;

const DOCUMENT: &str = "/work/thesis.md";
const TEXT: &str = "# Thesis\n\nWe recruited 48 subjects from two offices.\n";
const SIDECAR: &str = "/work/.cowriting/thesis.md";

fn workspace() -> (Arc<MemoryHost>, Workspace) {
    let host = Arc::new(MemoryHost::default());
    host.write(DOCUMENT, TEXT).unwrap();
    host.set_now("2026-11-23T10:00:00.000Z");
    let workspace = Workspace::open(host.clone(), DOCUMENT, "0.1.0");
    (host, workspace)
}

fn backend(replies: serde_json::Value) -> FakeBackend {
    FakeBackend::from_json(&json!({ "replies": replies }).to_string()).unwrap()
}

fn watching(ids: &[&str]) -> LiveSettings {
    LiveSettings { is_on: true, personas: ids.iter().map(|id| id.to_string()).collect(), extra: Default::default() }
}

#[test]
fn the_clock_is_read_in_milliseconds() {
    assert_eq!(ms_of("1970-01-01T00:00:00.000Z"), 0);
    assert_eq!(ms_of("1970-01-02T01:02:03.456Z"), 86_400_000 + 3_723_456);
    assert_eq!(ms_of("2026-11-23T10:01:00Z") - ms_of("2026-11-23T10:00:00.500Z"), 59_500);
}

#[tokio::test(flavor = "current_thread")]
async fn only_what_the_author_changed_is_sent_and_the_review_is_logged_with_its_hints() {
    let (host, workspace) = workspace();
    host.write(&format!("{SIDECAR}/party.md"), "## everyone\nA pilot study.\n\n## architect\nOne point per paragraph.\n").unwrap();
    let model = backend(
        json!([{ "text": "```json\n[{\"persona\": \"grammar-guy\", \"line\": 5, \"hint\": \"'recieved' is misspelled: received\"}, {\"persona\": \"architect\", \"line\": 5, \"hint\": \"Two claims in one sentence: which one is this paragraph about?\"}, {\"persona\": \"reviewer-2\", \"line\": 5, \"hint\": \"Nobody asked me.\"}]\n```" }]),
    );
    let (settings, mut state) = (watching(&["grammar-guy", "architect"]), LiveState::default());

    // watching begins: what is there is the baseline, nothing is sent
    assert_eq!(live_review(&workspace, &model, "m", &settings, &mut state, TEXT).await.unwrap(), LiveOutcome::Unchanged);
    // a change without prose in it is nothing to review
    assert_eq!(live_review(&workspace, &model, "m", &settings, &mut state, &format!("{TEXT}\nOk.\n")).await.unwrap(), LiveOutcome::Unchanged);
    assert!(model.requests().is_empty());
    assert!(host.files().keys().all(|file| !file.contains("ai-log")), "nothing is logged while nothing is sent");

    let written = format!("{TEXT}\nThey recieved a reminder every hour and walked more.\n");
    let outcome = live_review(&workspace, &model, "m", &settings, &mut state, &written).await.unwrap();
    assert_eq!(outcome, LiveOutcome::Reviewed { hints: 2, ts: "2026-11-23T10:00:00.000Z".into() });

    // the model saw the changed paragraph with its line number, not the rest of the document
    let request = &model.requests()[0];
    assert_eq!(request.messages.len(), 1);
    let sent = format!("{:?}", request.messages[0].content);
    assert!(sent.contains("### thesis.md") && sent.contains("5| They recieved a reminder") && !sent.contains("48 subjects"));
    assert!(request.tools.is_empty());
    assert!(request.system.contains("- architect: Structure, argument flow, red thread. The author asks it to look for: One point per paragraph."));
    assert!(request.system.contains("About the project, from the author: A pilot study.") && request.system.contains("Write the hints in the language of the manuscript."));
    assert!(request.system.contains("Never write or rewrite sentences for the manuscript"));

    // hints only from those who watch, each with the expert's name and the logged review it came from
    let hints = &state.hints;
    assert_eq!(hints.iter().map(|h| (h.who.as_str(), h.line)).collect::<Vec<_>>(), [("Grammar Guy", 5), ("The Architect", 5)]);
    let entries = workspace.log.read().unwrap();
    let [LogEntry::Prompt(prompt), LogEntry::Response(response)] = entries.as_slice() else { panic!("a prompt and a response, got {entries:?}") };
    assert_eq!((prompt.origin.as_deref(), prompt.category.as_str(), prompt.text.as_str()), (Some("watch"), "language", "Live review (grammar-guy, architect) of thesis.md l.5–5"));
    assert_eq!(response.personas, ["grammar-guy", "architect"]);
    assert_eq!(response.answer, "grammar-guy l.5: 'recieved' is misspelled: received\narchitect l.5: Two claims in one sentence: which one is this paragraph about?");
    assert_eq!((response.backend.as_deref(), response.model.as_deref()), (Some("fake"), Some("m")));
    assert!(hints.iter().all(|h| h.turn == prompt.id));
    // the document as it was sent is kept, and the document itself is as the author left it
    assert_eq!(host.read(&format!("{SIDECAR}/snapshots/{}.md", prompt.id)).unwrap(), written);
    assert_eq!(host.read(DOCUMENT).unwrap(), TEXT);
}

#[tokio::test(flavor = "current_thread")]
async fn reviews_keep_a_gap_stop_at_the_hourly_limit_and_survive_a_model_that_fails() {
    let (host, workspace) = workspace();
    let replies: Vec<_> = (0..30).map(|i| if i == 1 { json!({ "error": "the server is down" }) } else { json!({ "text": "[]" }) }).collect();
    let model = backend(json!(replies));
    let (settings, mut state) = (watching(&["architect"]), LiveState { baseline: Some(TEXT.into()), ..Default::default() });
    let text = |n: usize| format!("{TEXT}\nThis is new paragraph number {n} of the author.\n");

    // off, or nobody watching: nothing happens
    let off = LiveSettings { is_on: false, ..settings.clone() };
    assert_eq!(live_review(&workspace, &model, "m", &off, &mut state, &text(0)).await.unwrap(), LiveOutcome::Off);
    assert_eq!(live_review(&workspace, &model, "m", &watching(&[]), &mut state, &text(0)).await.unwrap(), LiveOutcome::Off);

    // an empty list is a good answer; it is logged all the same
    assert!(matches!(live_review(&workspace, &model, "m", &settings, &mut state, &text(1)).await.unwrap(), LiveOutcome::Reviewed { hints: 0, .. }));
    let entries = workspace.log.read().unwrap();
    assert!(matches!(entries.last(), Some(LogEntry::Response(r)) if r.answer == "(no hints)" && r.personas.is_empty()));

    // twenty seconds later is too soon; the answer says how long to wait
    host.set_now("2026-11-23T10:00:20.000Z");
    assert_eq!(live_review(&workspace, &model, "m", &settings, &mut state, &text(2)).await.unwrap(), LiveOutcome::Wait { ms: 40_000 });
    assert_eq!(model.requests().len(), 1);

    // the model fails: said so, nothing logged, and the next change is reviewed again
    host.set_now("2026-11-23T10:01:00.000Z");
    assert!(
        matches!(live_review(&workspace, &model, "m", &settings, &mut state, &text(2)).await.unwrap(), LiveOutcome::Failed { message } if message.contains("the server is down"))
    );
    assert_eq!(workspace.log.read().unwrap().len(), 2);

    // a review a minute until the hour's twenty are used up (the failed one counts: it was sent)
    for minute in 2..20 {
        host.set_now(&format!("2026-11-23T10:{minute:02}:00.000Z"));
        assert!(matches!(live_review(&workspace, &model, "m", &settings, &mut state, &text(minute + 1)).await.unwrap(), LiveOutcome::Reviewed { .. }), "minute {minute}");
    }
    host.set_now("2026-11-23T10:30:00.000Z");
    assert_eq!(live_review(&workspace, &model, "m", &settings, &mut state, &text(99)).await.unwrap(), LiveOutcome::Paused);
    assert_eq!(model.requests().len(), 20);
    // the author's own pace: a shorter gap and a smaller hourly limit are kept instead
    let mut own = LiveState { baseline: Some(TEXT.into()), pace: Pace { pause_ms: 5_000, gap_ms: 15_000, per_hour: 2 }, ..Default::default() };
    host.set_now("2026-11-23T10:40:00.000Z");
    assert!(matches!(live_review(&workspace, &model, "m", &settings, &mut own, &text(1)).await.unwrap(), LiveOutcome::Reviewed { .. }));
    host.set_now("2026-11-23T10:40:10.000Z");
    assert_eq!(live_review(&workspace, &model, "m", &settings, &mut own, &text(2)).await.unwrap(), LiveOutcome::Wait { ms: 5_000 });
    host.set_now("2026-11-23T10:40:15.000Z");
    assert!(matches!(live_review(&workspace, &model, "m", &settings, &mut own, &text(2)).await.unwrap(), LiveOutcome::Reviewed { .. }));
    host.set_now("2026-11-23T10:41:00.000Z");
    assert_eq!(live_review(&workspace, &model, "m", &settings, &mut own, &text(3)).await.unwrap(), LiveOutcome::Paused);
    assert_eq!(model.requests().len(), 22);

    // hints in the language the author chose for the AI's answers; without a choice, the manuscript's
    assert!(model.requests().last().unwrap().system.contains("Write the hints in the language of the manuscript."));
    *workspace.languages.write().unwrap() = cowriting_core::turn::Languages { manuscript: None, answers: Some(cowriting_core::disclosure::Lang::De) };
    let mut german = LiveState { baseline: Some(TEXT.into()), ..Default::default() };
    host.set_now("2026-11-23T10:50:00.000Z");
    assert!(matches!(live_review(&workspace, &model, "m", &settings, &mut german, &text(1)).await.unwrap(), LiveOutcome::Reviewed { .. }));
    assert!(model.requests().last().unwrap().system.contains("Write the hints in German."));
    *workspace.languages.write().unwrap() = Default::default();

    // an hour after the first review there is room again
    host.set_now("2026-11-23T11:00:30.000Z");
    assert!(matches!(live_review(&workspace, &model, "m", &settings, &mut state, &text(99)).await.unwrap(), LiveOutcome::Reviewed { .. }));
}

#[tokio::test(flavor = "current_thread")]
async fn the_point_check_is_logged_and_its_result_kept_for_the_progress() {
    let (host, workspace) = workspace();
    let model = backend(
        json!([{ "text": "[{\"n\": 1, \"status\": \"covered\", \"where\": \"thesis.md:3\", \"note\": \"both offices named\"}, {\"n\": 2, \"status\": \"partial\"}, {\"n\": 9, \"status\": \"covered\"}]" }]),
    );

    // no plan, or a plan without points: nothing is sent or logged
    assert!(matches!(check_points(&workspace, &model, "m", TEXT).await, Err(TurnError::Invalid(_))));
    host.write(&format!("{SIDECAR}/scope.md"), "## Goal\nShow it.\n").unwrap();
    assert!(matches!(check_points(&workspace, &model, "m", TEXT).await, Err(TurnError::Invalid(_))));
    host.write(
        &format!("{SIDECAR}/scope.md"),
        "## Goal\nShow it.\n\n## Sections\n- Methods: 300\n\n## Points\n- 48 people in two offices (Methods)\n- Breaks help\n- Limits of self-report\n",
    )
    .unwrap();
    assert!(matches!(check_points(&workspace, &model, "m", "  \n").await, Err(TurnError::Invalid(_))));
    assert!(model.requests().is_empty());

    let summary = check_points(&workspace, &model, "m", TEXT).await.unwrap();
    assert_eq!((summary.covered, summary.partial, summary.missing), (1, 1, 1));
    let request = &model.requests()[0];
    let sent = format!("{:?}", request.messages[0].content);
    assert!(sent.contains("1. 48 people in two offices (planned for: Methods)") && sent.contains("Goal of the paper: Show it.") && sent.contains("3: We recruited 48 subjects"));
    assert!(request.tools.is_empty() && request.system.contains("You never write, rephrase or suggest text"));

    let entries = workspace.log.read().unwrap();
    let [LogEntry::Prompt(prompt), LogEntry::Response(response)] = entries.as_slice() else { panic!("a prompt and a response, got {entries:?}") };
    assert_eq!(
        (prompt.origin.as_deref(), prompt.category.as_str(), prompt.text.as_str()),
        (Some("scope"), "structure", "Scope check: which of 3 planned points the manuscript makes")
    );
    assert_eq!(response.answer, "1. covered @ thesis.md:3: both offices named\n2. partial");

    // kept by the point's text, so a plan the author reorders keeps its results
    let check = read_check(&workspace).unwrap();
    assert_eq!(check.points.keys().collect::<Vec<_>>(), ["48 people in two offices", "Breaks help"]);
    assert_eq!(check.points["48 people in two offices"].status, PointStatus::Covered);
    assert_eq!((check.ts.as_str(), check.model.as_str()), ("2026-11-23T10:00:00.000Z", "m"));
    assert_eq!(host.read(DOCUMENT).unwrap(), TEXT);
}
