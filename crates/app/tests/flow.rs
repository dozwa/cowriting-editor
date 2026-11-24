//! The editor's first slice on a real folder: open a file, ask, classify, export, and read what
//! is on disk afterwards. The model is a script; the files are real.

use cowriting_app::{make_backend, BackendSettings, ExportOptions, Session};
use cowriting_core::host::DiskHost;
use cowriting_core::turn::Ask;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A folder of its own for one test, removed when the test ends.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }

    /// Every file below the folder, relative, sorted.
    fn files(&self) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, root, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.0, &self.0, &mut out);
        out.sort();
        out
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TEXT: &str = "# Walking breaks\n\nWe recruited 48 subjects from two offices.\n";

#[tokio::test(flavor = "current_thread")]
async fn open_ask_classify_export_and_everything_is_on_disk() {
    let folder = Folder::new("flow");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("other.md"), "another document\n").unwrap();
    std::fs::write(
        folder.path("script.json"),
        r#"{"replies": [
            {"tools": [{"name": "read_document"}, {"name": "write_note", "input": {"name": "../../../thesis.md", "text": "ghostwritten"}}]},
            {"text": "Line 3: why subjects and not participants?"}
        ]}"#,
    )
    .unwrap();

    let (session, opened) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    assert_eq!((opened.name.as_str(), opened.text.as_str(), opened.language.as_str()), ("thesis.md", TEXT, "en"));
    assert_eq!(folder.files(), ["other.md", "script.json", "thesis.md"], "opening a file creates nothing beside it");

    // the author types on, and saves: still nothing but the document
    let edited = format!("{TEXT}\nA new paragraph of my own.\n");
    session.save(&edited).unwrap();
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), edited);
    assert_eq!(folder.files().len(), 3);

    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new(), max_tokens: 0 };
    let backend = make_backend(&settings, None).unwrap();
    let ask = Ask { text: "Any feedback on my methods?".into(), document_text: edited.clone(), model: settings.model.clone(), ..Default::default() };
    let outcome = session.ask(backend.as_ref(), ask, &mut |_| {}).await.unwrap();
    assert_eq!(outcome.answer, "Line 3: why subjects and not participants?");

    // on disk: the document as the author saved it, and a sidecar for this document only
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), edited, "the model's write went nowhere");
    let files = folder.files();
    assert!(files.contains(&".cowriting/thesis.md/doc.json".to_string()));
    assert!(files.contains(&format!(".cowriting/thesis.md/snapshots/{}.md", outcome.id)));
    assert!(!files.iter().any(|f| f.contains("/notes/")), "no note was written: {files:?}");
    assert!(!files.iter().any(|f| f.starts_with(".cowriting/other.md")));
    let log_file = files.iter().find(|f| f.starts_with(".cowriting/thesis.md/ai-log/") && f.ends_with(".jsonl")).expect("a log file for this month");
    let log = std::fs::read_to_string(folder.path(log_file)).unwrap();
    let kinds: Vec<String> = log.lines().map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["type"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds, ["prompt", "guard", "response"]);

    // the Log tab: one turn waiting to be classified, one refused write
    let view = session.log_view(&edited).unwrap();
    assert_eq!((view.stats.turns, view.stats.unannotated, view.turns.len()), (1, 1, 1));
    assert!(view.turns[0].is_open);
    assert_eq!(view.turns[0].asked, "Any feedback on my methods?");
    assert_eq!(view.turns[0].excerpt, "Line 3: why subjects and not participants?");
    assert_eq!(view.guard.len(), 1);
    assert!(view.guard[0].refused);
    assert_eq!(view.words, 15);

    session.annotate(&outcome.id, "adapted", "changed it to participants").unwrap();
    let view = session.log_view(&edited).unwrap();
    assert_eq!((view.stats.unannotated, view.turns[0].use_.as_str(), view.turns[0].note.as_str()), (0, "adapted", "changed it to participants"));
    assert!(std::fs::read_to_string(folder.path(log_file)).unwrap().starts_with(&log), "the log only grew");

    // the disclosure, read from the file it was written to
    let exported = session.export(&ExportOptions { language: "en".into(), format: "md".into(), with_prompts: true, with_answers: true }).unwrap();
    assert_eq!(exported.paths.len(), 2);
    let register = std::fs::read_to_string(&exported.paths[0]).unwrap();
    assert!(exported.paths[0].ends_with(".cowriting/thesis.md/disclosure/ai-register.en.md"));
    assert!(register.contains("For “thesis.md”, generative AI was used through the CoWriting Editor in an assisting role only"));
    assert!(register.contains("| General questions | CoWriting Editor 0.1.0; a script (no model) (scripted) | 1 request, "));
    assert!(register.contains("| adapted: 1 | – |"));
    assert!(register.contains("write_note – write_note ../../../thesis.md\nThese changed nothing."));
    let appendix = std::fs::read_to_string(&exported.paths[1]).unwrap();
    assert!(appendix.contains("> Any feedback on my methods?"));
    assert!(appendix.contains("> Line 3: why subjects and not participants?"));
    assert!(appendix.contains("*Comment:* changed it to participants"));

    // a save after the first question keeps the sidecar's record of the document current
    session.save(&format!("{edited}More.\n")).unwrap();
    let info: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(folder.path(".cowriting/thesis.md/doc.json")).unwrap()).unwrap();
    assert_eq!(info["file"], "thesis.md");
    assert_eq!(info["lastHash"], cowriting_core::sidecar::content_hash(&format!("{edited}More.\n")));
}

#[test]
fn without_a_key_the_editor_says_so_instead_of_asking() {
    let error = make_backend(&BackendSettings::default(), None).err().unwrap();
    assert_eq!(error, "No API key is set yet. Add one in the settings to ask the AI.");
    assert!(make_backend(&BackendSettings::default(), Some("sk-something")).is_ok());
    // OpenAI itself wants a key; a local model server does not
    let other = |base_url: &str| BackendSettings { backend: "openai-compatible".into(), model: "m".into(), base_url: base_url.into(), max_tokens: 0 };
    assert!(make_backend(&other(""), None).is_err());
    assert!(make_backend(&other("http://localhost:11434/v1"), None).is_ok());
    assert_eq!(make_backend(&other("http://localhost:11434/v1"), None).unwrap().provider(), "a local model server (localhost:11434)");
    // Claude Code needs no key of ours, only to be installed
    let claude_code = BackendSettings { backend: "claude-code".into(), model: "sonnet".into(), base_url: String::new(), max_tokens: 0 };
    std::env::set_var("COWRITING_CLAUDE", "/nowhere/claude");
    assert!(make_backend(&claude_code, None).err().unwrap().starts_with("Claude Code was not found on this computer."));
    std::env::set_var("COWRITING_CLAUDE", concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    assert_eq!(make_backend(&claude_code, None).unwrap().provider(), "Anthropic (through Claude Code)");
    std::env::remove_var("COWRITING_CLAUDE");
    assert!(make_backend(&BackendSettings { backend: "something-else".into(), ..Default::default() }, Some("k")).is_err());
}

#[test]
fn a_model_is_tested_only_when_the_suite_passed_it() {
    use cowriting_app::{model_status, tested_models, TestedModel};
    let entry = |model: &str, verdict: &str| TestedModel {
        backend: "openai-compatible".into(),
        model: model.into(),
        verdict: verdict.into(),
        date: "2026-11-23".into(),
        editor: "0.1.0".into(),
        cases: Default::default(),
    };
    let tested = [entry("good:1b", "passed"), entry("bad:1b", "failed"), entry("fixed:1b", "failed"), entry("fixed:1b", "passed")];
    assert_eq!(model_status(&tested, "openai-compatible", "good:1b"), "tested");
    assert_eq!(model_status(&tested, "openai-compatible", "bad:1b"), "failed");
    assert_eq!(model_status(&tested, "openai-compatible", "fixed:1b"), "tested", "the latest run counts");
    assert_eq!(model_status(&tested, "openai-compatible", "unknown:1b"), "untested");
    assert_eq!(model_status(&tested, "anthropic", "good:1b"), "untested", "a result holds for the backend it was run on");
    // what ships is readable, and nothing in it claims more than a verdict the suite gives
    assert!(tested_models().iter().all(|t| t.verdict == "passed" || t.verdict == "failed"));
}

#[tokio::test(flavor = "current_thread")]
async fn the_author_briefs_the_party_adds_an_expert_and_asks_it() {
    let folder = Folder::new("party");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("script.json"), r#"{"replies": [{"text": "Line 3: who pays for the breaks?"}]}"#).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();

    // the party a document starts with: the catalogue without its field-specific member
    let view = session.party_view();
    assert_eq!(view.experts.len(), 10);
    assert_eq!(view.experts.iter().filter(|e| e.is_member).count(), 9);
    assert!(view.experts.iter().all(|e| !e.persona.sprite.is_empty() && e.brief.is_empty()));
    assert_eq!(folder.files(), ["script.json", "thesis.md"], "looking at the party writes nothing");

    session.set_brief("everyone", "A pilot study in two offices.").unwrap();
    session.set_brief("reviewer-2", "Go easy on the sampling.").unwrap();
    session.set_brief("reviewer-2", "Be hard on the sampling after all.").unwrap();
    assert!(session.set_brief("nobody", "x").is_err());
    let id = session.add_expert("The Health Economist", "Costs and QALYs", "Looks at who pays, and from which perspective costs are counted.").unwrap();
    assert_eq!(id, "health-economist");
    assert!(session.add_expert("The Health Economist", "again", "again").is_err(), "no second expert of the same name");
    assert!(session.add_expert("Reviewer 2", "x", "y").is_err(), "a catalogue id is not taken over");
    assert!(session.add_expert("???", "x", "y").is_err());
    session.set_member("clinician", true).unwrap();
    session.set_member("grammar-guy", false).unwrap();

    let view = session.party_view();
    let expert = |id: &str| view.experts.iter().find(|e| e.persona.id == id).unwrap();
    assert_eq!(view.everyone, "A pilot study in two offices.");
    assert_eq!(expert("reviewer-2").brief, "Be hard on the sampling after all.");
    assert!(expert("health-economist").persona.is_custom && expert("health-economist").is_member);
    assert!(expert("clinician").is_member && !expert("grammar-guy").is_member);
    // what is on disk is what a person can read and edit: the briefing as markdown, the expert as a file
    let briefing = std::fs::read_to_string(folder.path(".cowriting/thesis.md/party.md")).unwrap();
    assert!(briefing.contains("## reviewer-2 — Reviewer #2\nBe hard on the sampling after all.\n") && !briefing.contains("Go easy"));
    assert!(std::fs::read_to_string(folder.path(".cowriting/thesis.md/party/health-economist.md")).unwrap().starts_with("---\nname: The Health Economist\n"));

    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new(), max_tokens: 0 };
    let backend = make_backend(&settings, None).unwrap();
    let ask = Ask {
        text: "Review my methods.".into(),
        persona: Some(id.clone()),
        command: Some("ask".into()),
        document_text: TEXT.into(),
        model: "scripted".into(),
        ..Default::default()
    };
    let outcome = session.ask(backend.as_ref(), ask, &mut |_| {}).await.unwrap();
    let view = session.log_view(TEXT).unwrap();
    assert_eq!((view.turns[0].persona.as_str(), view.turns[0].who.as_str(), view.turns[0].category.as_str()), ("health-economist", "The Health Economist", "content"));
    assert_eq!(session.thread(&outcome.id).unwrap()[0].who, "The Health Economist");
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), TEXT);
}

#[tokio::test(flavor = "current_thread")]
async fn ai_text_that_enters_the_document_is_logged_once_and_declared() {
    const SENTENCE: &str = "Participants were randomly assigned to the walking or the sitting condition by a colleague who was not involved in data collection.";
    const CORRECTED: &str = "The participants were asked to rate their own concentration twice on every working day.";
    let folder = Folder::new("paste");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let script =
        json!({ "replies": [{ "text": format!("You do not say how people were assigned.\n\n[AI-TEXT] {SENTENCE}") }, { "text": format!("Line 5: write \"{CORRECTED}\"") }] });
    std::fs::write(folder.path("script.json"), script.to_string()).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();

    // before the AI was ever asked there is nothing to compare with, and no sidecar is made for it
    assert_eq!(session.report_paste(SENTENCE, 3, TEXT).unwrap(), []);
    assert_eq!(session.save(TEXT).unwrap(), []);
    assert_eq!(folder.files(), ["script.json", "thesis.md"]);

    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new(), max_tokens: 0 };
    let backend = make_backend(&settings, None).unwrap();
    let ask = |text: &str, command: &str| Ask { text: text.into(), command: Some(command.into()), document_text: TEXT.into(), model: "scripted".into(), ..Default::default() };
    let first = session.ask(backend.as_ref(), ask("Write me a sentence on assignment.", "ask"), &mut |_| {}).await.unwrap();
    session.ask(backend.as_ref(), ask("Check my grammar.", "language"), &mut |_| {}).await.unwrap();

    // the author pastes the AI's sentence between their own: found at once, at the line it landed on
    let pasted = format!("My own words come first here.\n\n{SENTENCE}");
    let with_paste = format!("{TEXT}\n{pasted}\n");
    let found = session.report_paste(&pasted, 5, &with_paste).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].source.as_str(), found[0].match_.as_str(), found[0].kind.as_str(), found[0].words, found[0].line), ("paste", "exact", "text", 21, 7));
    assert_eq!((found[0].turn.as_str(), found[0].asked.as_str(), found[0].still_in_text), (first.id.as_str(), "Write me a sentence on assignment.", true));
    // the same passage again, pasted or saved, is not a second entry
    assert_eq!(session.report_paste(SENTENCE, 20, &with_paste).unwrap(), []);
    assert_eq!(session.save(&with_paste).unwrap(), []);

    // the author types the correction in by hand: found when the document is saved
    let with_both = format!("{with_paste}\n{CORRECTED}\n");
    let typed = session.save(&with_both).unwrap();
    assert_eq!(typed.len(), 1);
    assert_eq!((typed[0].source.as_str(), typed[0].kind.as_str(), typed[0].line), ("typed", "correction", 9));
    // what the author writes themselves is nobody's business
    assert_eq!(session.save(&format!("{with_both}\nA colleague from another department drew lots, so nobody on the team chose who walked.\n")).unwrap(), []);

    session.comment_ai_text(&found[0].id, "Kept as a placeholder, to be rewritten.").unwrap();
    assert!(session.comment_ai_text("nope", "x").is_err());

    // the author rewrites the pasted sentence; the entry stays, and says so
    let rewritten = with_both.replace(SENTENCE, "A colleague drew lots.");
    assert_eq!(session.save(&rewritten).unwrap(), []);
    let view = session.log_view(&rewritten).unwrap();
    assert_eq!(view.ai_text.len(), 2);
    assert_eq!((view.ai_text[0].kind.as_str(), view.ai_text[0].still_in_text), ("correction", true));
    assert_eq!((view.ai_text[1].note.as_str(), view.ai_text[1].still_in_text), ("Kept as a placeholder, to be rewritten.", false));
    assert_eq!(view.stats.unannotated, 2, "a comment on a passage classifies no answer");

    // nothing was blocked or marked: the file is what the author saved
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), rewritten);
    let log = std::fs::read_to_string(folder.path(folder.files().iter().find(|f| f.ends_with(".jsonl")).unwrap())).unwrap();
    assert_eq!(log.matches("\"type\":\"ai_text\"").count(), 2);

    for (language, gone, head) in
        [("en", "no longer in the text", "## Language corrections taken over from AI answers"), ("de", "nicht mehr im Text", "## Aus KI-Antworten übernommene Sprachkorrekturen")]
    {
        let exported = session.export(&ExportOptions { language: language.into(), format: "md".into(), with_prompts: false, with_answers: false }).unwrap();
        let register = exported.register;
        let (ai_text, corrections) = register.split_once(head).unwrap_or_else(|| panic!("no section {head:?} in\n{register}"));
        assert!(ai_text.contains(&SENTENCE[..SENTENCE.len() - 1]) && ai_text.contains(gone) && ai_text.contains("Kept as a placeholder, to be rewritten."), "{register}");
        assert!(ai_text.contains("Write me a sentence on assignment."));
        assert!(corrections.contains(&CORRECTED[..CORRECTED.len() - 1]) && !ai_text.contains(&CORRECTED[..CORRECTED.len() - 1]));
        assert!(ai_text.contains(if language == "en" { "Text from other AI tools cannot be detected." } else { "Text aus anderen KI-Werkzeugen kann nicht erkannt werden." }));
    }
    let tex = session.export(&ExportOptions { language: "en".into(), format: "tex".into(), with_prompts: false, with_answers: false }).unwrap().register;
    assert!(tex.contains("\\subsection*{Language corrections taken over from AI answers}") && tex.contains("still in the text"));
}

#[tokio::test(flavor = "current_thread")]
async fn the_author_plans_checks_the_points_and_lets_experts_watch() {
    let folder = Folder::new("scope");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let script = json!({ "replies": [
        { "text": "[{\"n\": 1, \"status\": \"covered\", \"where\": \"thesis.md:3\", \"note\": \"both offices named\"}, {\"n\": 2, \"status\": \"missing\"}]" },
        { "text": "[{\"persona\": \"architect\", \"line\": 5, \"hint\": \"What does this paragraph add to the one before?\"}]" }
    ]});
    std::fs::write(folder.path("script.json"), script.to_string()).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new(), max_tokens: 0 };
    let backend = make_backend(&settings, None).unwrap();

    // no plan yet: a template to start from, no progress, nothing written
    let view = session.scope_view(TEXT);
    assert!(view.text.is_empty() && view.progress.is_none() && view.template.contains("## Points"));
    assert!(session.check_points(backend.as_ref(), "scripted", TEXT).await.is_err());
    assert_eq!(folder.files(), ["script.json", "thesis.md"]);
    // the template alone is no plan
    session.set_scope(&view.template, TEXT).unwrap();
    assert!(session.scope_view(TEXT).progress.is_none());
    // the German one neither, and filled in it is read like the English one
    session.set_scope(&view.template_de, TEXT).unwrap();
    assert!(session.scope_view(TEXT).progress.is_none());
    assert!(!view.template.contains("<!--") && !view.template_de.contains("<!--"), "nothing to read around in what the author starts from");
    let filled = view
        .template_de
        .replace("- Wörter: ", "- Wörter: 70")
        .replace("## Abschnitte\n", "## Abschnitte\n- Walking breaks: 70, Entwurf\n")
        .replace("## Kernaussagen\n", "## Kernaussagen\n- Breaks help\n")
        .replace("## Ziel\n", "## Ziel\nZeigen, dass Pausen helfen.\n");
    session.set_scope(&filled, TEXT).unwrap();
    let german = session.scope_view(TEXT).progress.unwrap();
    assert_eq!((german.words.target, german.sections.len(), german.points.len()), (Some(70.0), 1, 1));

    let plan = "## Goal\nShow that breaks help.\n\n## Targets\n- Words: 70\n\n## Sections\n- Walking breaks: 70, draft\n\n## Points\n- 48 people in two offices\n- Breaks help\n- [x] Sitting is common\n";
    session.set_scope(plan, TEXT).unwrap();
    assert_eq!(std::fs::read_to_string(folder.path(".cowriting/thesis.md/scope.md")).unwrap(), plan);
    let before = session.scope_view(TEXT).progress.unwrap();
    assert_eq!((before.words.done, before.words.target, before.sections[0].is_found, before.checked_at.as_deref()), (7, Some(70.0), true, None));

    let summary = session.check_points(backend.as_ref(), "scripted", TEXT).await.unwrap();
    assert_eq!((summary.covered, summary.partial, summary.missing), (1, 0, 2));
    let after = session.scope_view(TEXT).progress.unwrap();
    assert!(after.content > before.content && after.checked_at.is_some());
    assert_eq!(after.points[0].r#where.as_deref(), Some("thesis.md:3"));
    assert!(after.points[2].is_ticked && after.points[2].status.is_none());

    // live review: off until the author turns it on; then only what changes from there on
    let live = session.live_view();
    assert_eq!((live.is_on, live.personas.as_slice(), live.pause_ms), (false, ["grammar-guy", "terminologist", "style-coach", "architect"].map(String::from).as_slice(), 20_000));
    assert_eq!((live.gap_ms, live.per_hour), (60_000, 20));
    // the author's own pace is what the tab is told, and it goes back to the standard
    session.set_pace(cowriting_core::watch::Pace { pause_ms: 45_000, gap_ms: 120_000, per_hour: 6 });
    let slower = session.live_view();
    assert_eq!((slower.pause_ms, slower.gap_ms, slower.per_hour), (45_000, 120_000, 6));
    session.set_pace(Default::default());
    assert!(session.set_live(true, &["ghostwriter".to_string()], TEXT).is_err());
    session.set_live(true, &["architect".to_string()], TEXT).unwrap();
    let written = format!("{TEXT}\nThey walked for five minutes in every hour of work.\n");
    let outcome = session.live_review(backend.as_ref(), "scripted", &written).await.unwrap();
    assert!(matches!(outcome, cowriting_core::review::LiveOutcome::Reviewed { hints: 1, .. }), "{outcome:?}");
    let live = session.live_view();
    assert_eq!((live.is_on, live.personas.as_slice(), live.hints.len()), (true, ["architect".to_string()].as_slice(), 1));
    assert_eq!((live.hints[0].who.as_str(), live.hints[0].line), ("The Architect", 5));

    // both calls are in the log as turns the author did not type, and in the disclosure's count
    let view = session.log_view(&written).unwrap();
    assert_eq!(view.turns.len(), 2);
    assert!(view.turns[0].asked.starts_with("Live review (architect) of thesis.md") && view.turns[1].asked.starts_with("Scope check"));
    assert_eq!(view.turns[0].who, "The Architect");
    // left on, it is on when the document is opened again, and the first change after that is reviewed
    std::fs::write(folder.path("script.json"), json!({ "replies": [{ "text": "[]" }] }).to_string()).unwrap();
    let backend = make_backend(&settings, None).unwrap();
    let (reopened, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    assert!(reopened.live_view().is_on && reopened.live_view().hints.is_empty());
    let outcome = reopened.live_review(backend.as_ref(), "scripted", &written).await.unwrap();
    assert!(matches!(outcome, cowriting_core::review::LiveOutcome::Reviewed { hints: 0, .. }), "{outcome:?}");
    // turning it off is remembered with the document
    reopened.set_live(false, &["architect".to_string()], &written).unwrap();
    let (reopened, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    assert!(!reopened.live_view().is_on && reopened.live_view().personas == ["architect"]);
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), TEXT);
}

/// Crossref with one record.
struct OneRecord;

#[async_trait::async_trait]
impl cowriting_core::sources::References for OneRecord {
    async fn work(&self, doi: &str) -> Result<Option<cowriting_core::sources::Record>, String> {
        Ok((doi == "10.1000/Walk.2020").then(|| cowriting_core::sources::Record {
            doi: doi.into(),
            title: "Walking breaks and attention in office workers".into(),
            year: "2020".into(),
        }))
    }

    async fn candidates(&self, _query: &str) -> Result<Vec<cowriting_core::sources::Record>, String> {
        Ok(Vec::new())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_authors_references_are_read_searched_and_checked_but_never_written() {
    let folder = Folder::new("sources");
    let text = "---\nbibliography: refs.bib\n---\n\n# Walking breaks\n\nBreaks help [@smith2020; @ghost1999].\n";
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/literature");
    std::fs::write(folder.path("thesis.md"), text).unwrap();
    std::fs::copy(fixtures.join("refs.bib"), folder.path("refs.bib")).unwrap();
    std::fs::create_dir_all(folder.path("elsewhere")).unwrap();
    std::fs::copy(fixtures.join("refs.ris"), folder.path("elsewhere/more.ris")).unwrap();
    let bib_before = std::fs::read(folder.path("refs.bib")).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();

    // the front matter's file is found without any setup, and looking writes nothing
    let view = session.sources_view(text);
    assert_eq!((view.files.len(), view.entries, view.cited), (1, 4, 2));
    assert_eq!(view.missing, ["ghost1999"]);
    assert!(view.checked.is_none());
    assert_eq!(folder.files(), ["elsewhere/more.ris", "refs.bib", "thesis.md"]);

    // a file from elsewhere is added by the author; anything else is refused
    assert!(session.add_bibliography(&folder.path("thesis.md"), text).is_err());
    assert!(session.add_bibliography(&folder.path("nowhere.bib"), text).is_err());
    session.add_bibliography(&folder.path("elsewhere/more.ris"), text).unwrap();
    session.add_bibliography(&folder.path("elsewhere/more.ris"), text).unwrap();
    let view = session.sources_view(text);
    assert_eq!(
        view.files.iter().map(|f| (f.name.as_str(), f.format.as_str(), f.entries, f.is_from_document)).collect::<Vec<_>>(),
        [("refs.bib", "BibTeX", 4, true), ("more.ris", "RIS", 5, false)]
    );
    assert_eq!(session.search_bibliography("sitting", text).iter().map(|hit| hit.file.as_str()).collect::<Vec<_>>(), ["refs.bib", "more.ris"]);

    // the check keeps what it found; no AI is asked, so the log stays empty
    let checked = session.check_references(&OneRecord, text).await.unwrap();
    assert_eq!(checked.rows.len(), 9);
    assert_eq!(serde_json::to_value(&checked.rows[0]).unwrap()["status"], "ok");
    assert_eq!(serde_json::to_value(&checked.rows[1]).unwrap()["status"], "not_found");
    assert_eq!(session.sources_view(text).checked, Some(checked));
    assert!(session.log_view(text).unwrap().turns.is_empty());

    session.remove_bibliography(&folder.path("elsewhere/more.ris")).unwrap();
    assert_eq!(session.sources_view(text).files.len(), 1);
    assert!(Path::new(&folder.path("elsewhere/more.ris")).exists(), "taking a file off the list leaves the file");
    assert_eq!(std::fs::read(folder.path("refs.bib")).unwrap(), bib_before);
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), text);
}

#[test]
fn an_export_is_a_pdf_or_word_file_with_the_references_behind_the_citekeys() {
    use cowriting_app::export::{check_target, find_program, pandoc_args, references};
    use cowriting_app::{FileFormat, Paper, PdfChoice};
    let folder = Folder::new("export");
    let text = "---\ntitle: Walking breaks\nbibliography: refs.bib\n---\n\n# Walking breaks\n\nBreaks help [@smith2020; @ghost1999], as the library says [@fromzotero2024].\n\n# References\n";
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/literature");
    std::fs::write(folder.path("thesis.md"), text).unwrap();
    let bib =
        "@article{smith2020, author = {Smith, Jane and Lee, Min-Jung and Berg, Pieter}, title = {Walking breaks and attention}, journal = {Journal of Work}, year = {2020}}\n";
    std::fs::write(folder.path("refs.bib"), bib).unwrap();
    std::fs::copy(fixtures.join("refs.nbib"), folder.path("pubmed.nbib")).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    session.add_bibliography(&folder.path("pubmed.nbib"), text).unwrap();
    let zotero = json!([{ "id": "fromzotero2024", "type": "article-journal", "title": "Standing meetings", "author": [{ "family": "Okafor", "given": "Ada" }], "issued": { "date-parts": [[2024]] } }]).to_string();

    // the file the front matter names lies beside the document, whatever the system's separator
    let beside = format!("{}/refs.bib", session.workspace.sidecar.folder);
    // which references go in: Zotero's for the cited keys first, then the files pandoc can read; a key nobody holds is named
    let bibliography = cowriting_app::export::bibliography_of(&session.workspace, text);
    let used = references(&bibliography, text, Some(("/kept/references.json", &zotero)));
    assert_eq!((used.files.clone(), used.from_zotero, used.missing.clone()), (vec!["/kept/references.json".to_string(), beside.clone()], 1, vec!["ghost1999".to_string()]));
    let offline = references(&bibliography, text, None);
    assert_eq!((offline.files.len(), offline.from_zotero, offline.missing), (1, 0, vec!["fromzotero2024".to_string(), "ghost1999".to_string()]));
    let args = pandoc_args(FileFormat::Pdf, "/out/thesis.pdf", "/work", &used.files, Some(("typst", "/tools/typst")), Paper::A4, None);
    let expected = format!("--from markdown --standalone --output /out/thesis.pdf --resource-path /work --citeproc --bibliography /kept/references.json --bibliography {} --pdf-engine /tools/typst --variable papersize=a4", beside.clone());
    assert_eq!(args.join(" "), expected);
    // Typst and LaTeX name US letter differently; a Word file has no engine and no paper setting
    let letter = |engine: &str| pandoc_args(FileFormat::Pdf, "o.pdf", "/work", &[], Some((engine, engine)), Paper::Letter, None).last().cloned();
    assert_eq!((letter("typst").as_deref(), letter("xelatex").as_deref()), (Some("papersize=us-letter"), Some("papersize=letter")));
    assert!(!pandoc_args(FileFormat::Docx, "/out/thesis.docx", "/work", &[], Some(("typst", "/tools/typst")), Paper::A4, None)
        .iter()
        .any(|arg| arg == "--pdf-engine" || arg.starts_with("papersize")));

    // the reference style: none at first, which is pandoc's standard
    let csl =
        r#"<?xml version="1.0" encoding="utf-8"?><style xmlns="http://purl.org/net/xbiblio/csl" version="1.0"><info><title>Journal of Work &amp; Rest</title></info></style>"#;
    std::fs::write(folder.path("work.csl"), csl).unwrap();
    std::fs::write(folder.path("notes.csl"), "just some notes").unwrap();
    assert_eq!(session.sources_view(text).style, None);
    // only a real style file is taken, and the refusal says why
    assert!(session.set_style(&folder.path("refs.bib"), text).unwrap_err().contains(".csl file"));
    assert!(session.set_style(&folder.path("notes.csl"), text).unwrap_err().contains("is not a CSL style"));
    assert!(session.set_style(&folder.path("gone.csl"), text).unwrap_err().contains("could not be read"));
    session.set_style(&folder.path("work.csl"), text).unwrap();
    let style = session.sources_view(text).style.unwrap();
    assert_eq!((style.name.as_str(), style.title.as_str(), style.is_from_document, style.error.clone()), ("work.csl", "Journal of Work & Rest", false, None));
    assert_eq!(session.sources_view(text).files.len(), 2, "choosing a style kept the bibliography files");
    let styled = pandoc_args(FileFormat::Docx, "/out/thesis.docx", "/work", &[], None, Paper::A4, Some(&style.path));
    assert_eq!(styled[styled.len() - 2..], ["--csl".to_string(), folder.path("work.csl")]);
    // a style the document names itself comes first, and is left to pandoc
    let naming = text.replace("bibliography: refs.bib", "bibliography: refs.bib\ncsl: \"house.csl\"");
    let named = session.sources_view(&naming).style.unwrap();
    assert_eq!((named.name.as_str(), named.is_from_document, named.error.is_some()), ("house.csl", true, true), "named, and not there");
    // the chosen file vanishes: the export says so instead of failing somewhere inside pandoc
    std::fs::remove_file(folder.path("work.csl")).unwrap();
    let failed = session.export_document(FileFormat::Docx, &folder.path("thesis.docx"), text, None, &PdfChoice { engine: None, paper: Paper::A4 }).unwrap_err();
    assert!(failed.contains("The reference style work.csl could not be read"), "{failed}");
    session.remove_style().unwrap();
    std::fs::remove_file(folder.path("notes.csl")).unwrap();
    assert_eq!(session.sources_view(text).style, None);
    assert_eq!(session.sources_view(text).files.len(), 2);

    // the target is a file of the chosen kind and never the document
    assert!(check_target(FileFormat::Pdf, &folder.path("thesis.docx"), &folder.path("thesis.md")).is_err());
    assert!(session.export_document(FileFormat::Docx, &folder.path("thesis.md"), text, None, &PdfChoice::default()).is_err());
    assert_eq!(session.export_target(FileFormat::Pdf), folder.path("thesis.pdf"));
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), text);

    // the real thing, where pandoc is installed (CI installs it and insists)
    let Some(pandoc) = find_program("pandoc") else {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "pandoc is not there: run scripts/fetch-tools.py");
        eprintln!("pandoc is not installed: the export itself was not run");
        return;
    };
    let made = session.export_document(FileFormat::Docx, &folder.path("thesis.docx"), text, Some(&zotero), &PdfChoice::default()).unwrap();
    assert_eq!((made.cited, made.from_zotero, made.missing.clone()), (3, 1, vec!["ghost1999".to_string()]));
    assert!(made.warnings.iter().any(|warning| warning.contains("ghost1999")), "{:?}", made.warnings);
    let back = std::process::Command::new(pandoc).args([folder.path("thesis.docx").as_str(), "--to", "plain", "--wrap", "none"]).output().unwrap();
    let plain = String::from_utf8_lossy(&back.stdout);
    // citations are resolved, the unknown key is left visible, and the reference list has both sources
    assert!(plain.contains("(Smith et al. 2020; ghost1999?)") && plain.contains("(Okafor 2024)"), "{plain}");
    assert!(plain.contains("Okafor, Ada. 2024") && plain.contains("Smith, Jane, Min-Jung Lee, and Pieter Berg. 2020"), "{plain}");
    // a bibliography pandoc cannot read: the author is told which file and where, and no file is made
    std::fs::write(folder.path("refs.bib"), format!("{bib}@misc{{broken, title = {{unclosed\n")).unwrap();
    let refused = session.export_document(FileFormat::Docx, &folder.path("second.docx"), text, None, &PdfChoice::default()).unwrap_err();
    assert!(refused.contains("refs.bib") && refused.contains("line"), "{refused}");
    assert!(!Path::new(&folder.path("second.docx")).exists());
    std::fs::write(folder.path("refs.bib"), bib).unwrap();
    // the export wrote the Word file and the references it took from Zotero, and left everything else as it was
    assert_eq!(
        folder.files(),
        [
            ".cowriting/thesis.md/doc.json",
            ".cowriting/thesis.md/export/references.json",
            ".cowriting/thesis.md/sources.json",
            "pubmed.nbib",
            "refs.bib",
            "thesis.docx",
            "thesis.md"
        ]
    );
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), text);

    // a PDF with the editor's own Typst, on the paper the author chose; an engine that is not there is refused
    let tools = cowriting_app::export::local_tools();
    assert!(session.export_document(FileFormat::Pdf, &folder.path("thesis.pdf"), text, None, &PdfChoice { engine: Some("no-such-tex".into()), paper: Paper::A4 }).is_err());
    if tools.pdf_engines.first().map(String::as_str) != Some("typst") {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "Typst is not there: run scripts/fetch-tools.py");
        eprintln!("Typst is not there: the PDF export was not run");
        return;
    }
    for (paper, name, width) in [(Paper::A4, "a4.pdf", "595.27"), (Paper::Letter, "letter.pdf", "612 792")] {
        session.export_document(FileFormat::Pdf, &folder.path(name), text, Some(&zotero), &PdfChoice { engine: None, paper }).unwrap();
        let pdf = std::fs::read(folder.path(name)).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{name} is a PDF");
        assert!(String::from_utf8_lossy(&pdf).replace("MediaBox [", "MediaBox[").contains(&format!("/MediaBox[0 0 {width}")), "{name} is {width} points wide");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_document_that_moves_or_was_renamed_keeps_its_log() {
    let folder = Folder::new("move");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("script.json"), json!({ "replies": [{ "text": "Who are the subjects?" }] }).to_string()).unwrap();
    let (session, opened) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    assert!(opened.orphans.is_empty());
    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new(), max_tokens: 0 };
    let backend = make_backend(&settings, None).unwrap();
    session.ask(backend.as_ref(), Ask { text: "Is line 3 clear?".into(), document_text: TEXT.into(), model: "scripted".into(), ..Default::default() }, &mut |_| {}).await.unwrap();
    let log_before = std::fs::read_to_string(
        folder.path(".cowriting/thesis.md/ai-log").to_string() + "/" + &session.workspace.host.list(&folder.path(".cowriting/thesis.md/ai-log")).unwrap()[0],
    )
    .unwrap();

    // moved inside the editor, with unsaved changes: the text, the log and the snapshot arrive, the old place is empty
    let edited = format!("{TEXT}\nA new paragraph.\n");
    assert!(session.move_document(&folder.path("thesis.md"), &edited).is_err());
    assert!(session.move_document(&folder.path("thesis.txt"), &edited).is_err());
    session.move_document(&folder.path("drafts/chapter-1.md"), &edited).unwrap();
    let files = folder.files();
    assert!(!files.iter().any(|file| file == "thesis.md" || file.starts_with(".cowriting/")), "{files:?}");
    assert_eq!(std::fs::read_to_string(folder.path("drafts/chapter-1.md")).unwrap(), edited);
    let (moved, opened) = Session::open(Arc::new(DiskHost), &folder.path("drafts/chapter-1.md")).unwrap();
    assert!(opened.orphans.is_empty());
    let view = moved.log_view(&edited).unwrap();
    assert_eq!((view.turns.len(), view.turns[0].asked.as_str()), (1, "Is line 3 clear?"));
    let month = moved.workspace.host.list(&folder.path("drafts/.cowriting/chapter-1.md/ai-log")).unwrap();
    assert_eq!(std::fs::read_to_string(folder.path(&format!("drafts/.cowriting/chapter-1.md/ai-log/{}", month[0]))).unwrap(), log_before, "the log moved, byte for byte");
    let info: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(folder.path("drafts/.cowriting/chapter-1.md/doc.json")).unwrap()).unwrap();
    assert_eq!(info["file"], "chapter-1.md");

    // a place that has a log of its own is refused: neither log may be lost
    std::fs::write(folder.path("drafts/other.md"), "# Other\n").unwrap();
    let (other, _) = Session::open(Arc::new(DiskHost), &folder.path("drafts/other.md")).unwrap();
    assert!(other.move_document(&folder.path("drafts/chapter-1.md"), "# Other\n").unwrap_err().contains("has an AI log of its own"));
    assert_eq!(std::fs::read_to_string(folder.path("drafts/chapter-1.md")).unwrap(), edited);

    // renamed outside the editor: the log is not taken silently, the author is asked, and then it is theirs
    std::fs::rename(folder.path("drafts/chapter-1.md"), folder.path("drafts/intro.md")).unwrap();
    let (renamed, opened) = Session::open(Arc::new(DiskHost), &folder.path("drafts/intro.md")).unwrap();
    assert_eq!(serde_json::to_value(&opened.orphans).unwrap(), json!([{ "name": "chapter-1.md", "isSameText": true, "questions": 1 }]));
    assert!(renamed.log_view(&edited).unwrap().turns.is_empty(), "nothing is taken over unasked");
    assert!(renamed.adopt("nothing.md").is_err());
    renamed.adopt("chapter-1.md").unwrap();
    let (renamed, opened) = Session::open(Arc::new(DiskHost), &folder.path("drafts/intro.md")).unwrap();
    assert!(opened.orphans.is_empty());
    assert_eq!(renamed.log_view(&edited).unwrap().turns.len(), 1);
    assert!(!Path::new(&folder.path("drafts/.cowriting/chapter-1.md")).exists());

    // a document that has its own log is never offered another one, and a changed text is said to be changed
    std::fs::rename(folder.path("drafts/intro.md"), folder.path("drafts/intro-2.md")).unwrap();
    std::fs::write(folder.path("drafts/intro-2.md"), "# Something else entirely\n").unwrap();
    let (_, opened) = Session::open(Arc::new(DiskHost), &folder.path("drafts/intro-2.md")).unwrap();
    assert_eq!((opened.orphans[0].name.as_str(), opened.orphans[0].is_same_text), ("intro.md", false));
}
