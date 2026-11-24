//! A project is one markdown file: its settings live in the editor's own folder, its log beside it.
//! Linked files and reworded experts are judged by what the model is sent and what is on disk.

use cowriting_app::preferences::Preferences;
use cowriting_app::Session;
use cowriting_backends::fake::FakeBackend;
use cowriting_core::host::DiskHost;
use cowriting_core::project::Project;
use cowriting_core::turn::Ask;
use std::path::PathBuf;
use std::sync::Arc;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("work")).unwrap();
        Folder(path)
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TEXT: &str = "# Walking breaks\n\nWe recruited 48 subjects from two offices.\n";

fn ask(text: &str, persona: Option<&str>) -> Ask {
    Ask { text: text.into(), persona: persona.map(String::from), document_text: TEXT.into(), model: "scripted".into(), ..Default::default() }
}

#[tokio::test(flavor = "current_thread")]
async fn linked_files_reach_whom_they_are_for_and_a_reworded_expert_speaks_as_the_author_wrote() {
    let folder = Folder::new("knowledge");
    std::fs::write(folder.path("work/thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("work/wording.md"), "Say participants, never subjects.\n").unwrap();
    std::fs::write(folder.path("work/methods.txt"), "The design is a crossover trial.\n").unwrap();
    std::fs::write(folder.path("work/scan.pdf"), [0xff_u8, 0xfe, 0x00, 0x80]).unwrap();
    let (session, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/thesis.md"), Some(&folder.path("app/projects"))).unwrap();

    session.link_file(&folder.path("work/wording.md"), &[]).unwrap();
    session.link_file(&folder.path("work/methods.txt"), &["statistics-nerd".to_string()]).unwrap();
    assert!(session.link_file(&folder.path("work/scan.pdf"), &[]).unwrap_err().contains("plain text"), "a file that is no text is refused, in plain words");
    assert!(session.link_file(&folder.path("work/thesis.md"), &[]).is_err(), "the document itself is not a linked file");
    assert!(session.link_file(&folder.path("work/wording.md"), &["nobody".to_string()]).is_err());
    let view = session.project_view();
    assert_eq!(view.files.iter().map(|f| (f.name.as_str(), f.personas.len(), f.is_missing)).collect::<Vec<_>>(), [("wording.md", 0, false), ("methods.txt", 1, false)]);

    // an expert reworded by the author, and one left as it comes
    session.set_prompt("reviewer-2", "You are the kindest reviewer there is.").unwrap();
    let party = session.party_view();
    let expert = |id: &str| party.experts.iter().find(|e| e.persona.id == id).unwrap();
    assert!(expert("reviewer-2").is_prompt_edited && !expert("statistics-nerd").is_prompt_edited);

    let backend = FakeBackend::from_json(r#"{"replies": [{"text": "one"}, {"text": "two"}, {"text": "three"}]}"#).unwrap();
    session.ask(&backend, ask("Any feedback?", None), &mut |_| {}).await.unwrap();
    session.ask(&backend, ask("Review it.", Some("statistics-nerd")), &mut |_| {}).await.unwrap();
    session.ask(&backend, ask("Review it.", Some("reviewer-2")), &mut |_| {}).await.unwrap();
    let sent = backend.requests();
    let (assistant, statistician, reviewer) = (&sent[0].system, &sent[1].system, &sent[2].system);

    // the file for everyone reaches everyone; the one for the statistician only the statistician
    assert!(assistant.contains("Say participants, never subjects.") && !assistant.contains("crossover trial"));
    assert!(statistician.contains("Say participants, never subjects.") && statistician.contains("The design is a crossover trial."));
    assert!(reviewer.contains("## wording.md") && !reviewer.contains("crossover trial"));
    // the reworded expert is told the author's words, under the same rules as before
    assert!(reviewer.contains("You are the kindest reviewer there is.") && !reviewer.contains("You are **Reviewer #2**"));
    assert!(reviewer.contains("# Your role in this conversation") && statistician.contains("Statistics Nerd"));

    // the log names the files that went along with each question
    let log_folder = folder.path("work/.cowriting/thesis.md/ai-log");
    let log = std::fs::read_to_string(std::fs::read_dir(&log_folder).unwrap().next().unwrap().unwrap().path()).unwrap();
    let prompts: Vec<serde_json::Value> = log.lines().map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()).filter(|e| e["type"] == "prompt").collect();
    assert_eq!(prompts[0]["files"], serde_json::json!(["wording.md"]));
    assert_eq!(prompts[1]["files"], serde_json::json!(["wording.md", "methods.txt"]));

    // back to the standard wording; an unlinked or vanished file goes along no more, and nothing fails
    session.set_prompt("reviewer-2", "").unwrap();
    assert!(!session.party_view().experts.iter().any(|e| e.is_prompt_edited));
    session.unlink_file(&folder.path("work/wording.md")).unwrap();
    std::fs::remove_file(folder.path("work/methods.txt")).unwrap();
    assert!(session.project_view().files[0].is_missing);
    let backend = FakeBackend::from_json(r#"{"replies": [{"text": "four"}]}"#).unwrap();
    session.ask(&backend, ask("Review it.", Some("statistics-nerd")), &mut |_| {}).await.unwrap();
    assert!(!backend.requests()[0].system.contains("background knowledge"));
    assert_eq!(std::fs::read_to_string(folder.path("work/thesis.md")).unwrap(), TEXT);
}

#[tokio::test(flavor = "current_thread")]
async fn a_projects_settings_are_in_the_editors_folder_come_along_when_it_moves_and_the_log_stays_beside_the_text() {
    let folder = Folder::new("place");
    std::fs::write(folder.path("work/thesis.md"), TEXT).unwrap();
    // an earlier version kept the briefing beside the document
    std::fs::create_dir_all(folder.path("work/.cowriting/thesis.md")).unwrap();
    std::fs::write(folder.path("work/.cowriting/thesis.md/party.md"), "# What the experts should know about this document\n\n## everyone\nA pilot study.\n").unwrap();
    let projects = folder.path("app/projects");
    let (session, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/thesis.md"), Some(&projects)).unwrap();

    // it was brought over, and what is set now goes to the same place
    assert_eq!(session.party_view().everyone, "A pilot study.");
    assert!(!std::path::Path::new(&folder.path("work/.cowriting/thesis.md/party.md")).exists());
    session.set_brief("reviewer-2", "Be hard on the sampling.").unwrap();
    session.set_scope("# Plan\n\n- Show it.\n", TEXT).unwrap();
    session.add_expert("The Health Economist", "Costs", "Looks at who pays.").unwrap();
    session.set_prompt("health-economist", "You count costs from society's perspective.").unwrap();
    assert!(session.set_prompt("health-economist", " ").is_err(), "an expert of the author's own cannot be told nothing");
    let home = session.project_view().folder;
    assert!(home.starts_with(&projects) && home.contains("thesis.md-"), "{home}");
    for file in ["party.md", "scope.md", "party/health-economist.md"] {
        assert!(std::path::Path::new(&format!("{home}/{file}")).exists(), "{file}");
    }
    let own = std::fs::read_to_string(format!("{home}/party/health-economist.md")).unwrap();
    assert!(own.starts_with("---\nname: The Health Economist\n") && own.ends_with("---\nYou count costs from society's perspective.\n"), "{own}");

    // the log is written beside the document, and nowhere else
    let backend = FakeBackend::from_json(r#"{"replies": [{"text": "Who are they?"}]}"#).unwrap();
    session.ask(&backend, ask("Is line 3 clear?", None), &mut |_| {}).await.unwrap();
    assert!(std::path::Path::new(&folder.path("work/.cowriting/thesis.md/ai-log")).is_dir());
    assert!(!std::path::Path::new(&format!("{home}/ai-log")).exists());

    // this project's own languages and pace; the general ones hold for the rest
    let general = Preferences { ai_language: "en".into(), live_per_hour: 20, ..Default::default() };
    session.set_overrides(Project { ai_language: Some("de".into()), writing_language: Some("klingon".into()), live_per_hour: Some(100_000), ..Default::default() }).unwrap();
    let here = session.preferences_here(&general);
    assert_eq!((here.ai_language.as_str(), here.writing_language.as_str(), here.live_per_hour, here.live_gap_seconds), ("de", "document", 120, general.live_gap_seconds));
    session.set_overrides(Project::default()).unwrap();
    assert_eq!(session.preferences_here(&general), general.clone().within_limits());

    // moved in the editor: the settings are found at the new place
    session.link_file(&folder.path("work/thesis.md").replace("thesis.md", "notes.txt"), &[]).unwrap_err();
    session.move_document(&folder.path("work/chapter-1.md"), TEXT).unwrap();
    let (moved, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/chapter-1.md"), Some(&projects)).unwrap();
    assert_eq!(moved.party_view().everyone, "A pilot study.");
    assert!(moved.party_view().experts.iter().any(|e| e.persona.id == "health-economist"));
    assert!(!std::path::Path::new(&home).exists());
}
