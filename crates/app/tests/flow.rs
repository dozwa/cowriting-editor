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

    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new() };
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
    let other = |base_url: &str| BackendSettings { backend: "openai-compatible".into(), model: "m".into(), base_url: base_url.into() };
    assert!(make_backend(&other(""), None).is_err());
    assert!(make_backend(&other("http://localhost:11434/v1"), None).is_ok());
    assert_eq!(make_backend(&other("http://localhost:11434/v1"), None).unwrap().provider(), "a local model server (localhost:11434)");
    assert!(make_backend(&BackendSettings { backend: "claude-code".into(), ..Default::default() }, Some("k")).is_err());
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

    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new() };
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

    let settings = BackendSettings { backend: format!("fake:{}", folder.path("script.json")), model: "scripted".into(), base_url: String::new() };
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
