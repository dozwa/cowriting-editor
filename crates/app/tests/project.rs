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
    assert!(session.link_file(&folder.path("work/scan.pdf"), &[]).unwrap_err().contains("could not be read as a PDF"), "a file that cannot be read is refused, in plain words");
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

/// A PDF of one page that says `line`, written by hand: objects, their places, and the table that lists them.
fn pdf_saying(line: &str) -> Vec<u8> {
    let stream = format!("BT /F1 12 Tf 72 720 Td ({line}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut places = Vec::new();
    for (number, object) in objects.iter().enumerate() {
        places.push(out.len());
        out.push_str(&format!("{} 0 obj\n{object}\nendobj\n", number + 1));
    }
    let table = out.len();
    out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for place in places {
        out.push_str(&format!("{place:010} 00000 n \n"));
    }
    out.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{table}\n%%EOF\n", objects.len() + 1));
    out.into_bytes()
}

#[tokio::test(flavor = "current_thread")]
async fn a_linked_pdf_or_word_file_reaches_the_model_as_the_text_in_it() {
    let folder = Folder::new("converted");
    std::fs::write(folder.path("work/thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("work/guideline.pdf"), pdf_saying("Report effect sizes with intervals.")).unwrap();
    let (session, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/thesis.md"), Some(&folder.path("app/projects"))).unwrap();
    session.link_file(&folder.path("work/guideline.pdf"), &[]).unwrap();
    let mut expected = vec!["guideline.pdf"];

    // a Word file needs pandoc, which the editor brings along; where a test machine has none, the PDF alone is tried
    if cowriting_app::export::find_program("pandoc").is_some() {
        std::fs::write(folder.path("work/house.md"), "House style: write *participants*, never subjects.\n").unwrap();
        let made = std::process::Command::new(cowriting_app::export::find_program("pandoc").unwrap())
            .args([&folder.path("work/house.md"), "-o", &folder.path("work/house style.docx")])
            .status()
            .unwrap();
        assert!(made.success());
        session.link_file(&folder.path("work/house style.docx"), &[]).unwrap();
        expected.push("house style.docx");
    }
    // a PDF without text is said to be one, and is not linked
    std::fs::write(folder.path("work/empty.pdf"), pdf_saying("")).unwrap();
    assert!(session.link_file(&folder.path("work/empty.pdf"), &[]).unwrap_err().contains("may be a scan"));

    let view = session.project_view();
    assert_eq!(view.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), expected);
    assert!(view.files.iter().all(|f| f.is_converted && !f.is_missing && f.chars > 20), "{:?}", view.files);

    let backend = FakeBackend::from_json(r#"{ "replies": [ { "text": "Noted." } ] }"#).unwrap();
    session.ask(&backend, ask("Anything I should follow?", None), &mut |_| {}).await.unwrap();
    let sent = backend.requests()[0].system.clone();
    assert!(sent.contains("## guideline.pdf") && sent.contains("Report effect sizes with intervals."), "{sent}");
    if expected.len() == 2 {
        assert!(sent.contains("## house style.docx") && sent.contains("write participants, never subjects."), "{sent}");
    }
    assert!(!sent.contains("%PDF"), "the file's own bytes are not sent");

    // the author's file changed: it is read again when the document is opened
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(folder.path("work/guideline.pdf"), pdf_saying("Report medians for skewed data.")).unwrap();
    let (session, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/thesis.md"), Some(&folder.path("app/projects"))).unwrap();
    let backend = FakeBackend::from_json(r#"{ "replies": [ { "text": "Noted." } ] }"#).unwrap();
    session.ask(&backend, ask("And now?", None), &mut |_| {}).await.unwrap();
    let sent = backend.requests()[0].system.clone();
    assert!(sent.contains("Report medians for skewed data.") && !sent.contains("effect sizes"), "{sent}");

    // a source that is gone shows as missing; unlinking takes the editor's copy away with it
    let kept = std::fs::read_dir(format!("{}/linked", session.project_view().folder)).unwrap().count();
    assert_eq!(kept, expected.len());
    std::fs::remove_file(folder.path("work/guideline.pdf")).unwrap();
    assert!(session.project_view().files[0].is_missing);
    session.unlink_file(&folder.path("work/guideline.pdf")).unwrap();
    assert_eq!(std::fs::read_dir(format!("{}/linked", session.project_view().folder)).unwrap().count(), kept - 1);
}

#[tokio::test(flavor = "current_thread")]
async fn project_settings_follow_a_document_that_was_moved_with_its_log_outside_the_editor() {
    let folder = Folder::new("moved");
    let projects = folder.path("app/projects");
    std::fs::write(folder.path("work/thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("work/wording.md"), "Say participants, never subjects.\n").unwrap();
    let (session, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/thesis.md"), Some(&projects)).unwrap();
    session.link_file(&folder.path("work/wording.md"), &[]).unwrap();
    session.set_prompt("reviewer-2", "You are the kindest reviewer there is.").unwrap();
    // the log begins with the first question
    let backend = FakeBackend::from_json(r#"{ "replies": [ { "text": "Fine." } ] }"#).unwrap();
    session.ask(&backend, ask("Is it fine?", None), &mut |_| {}).await.unwrap();
    session.save(TEXT).unwrap();
    drop(session);

    // in the file manager: the folder gets another name, and the document too
    std::fs::rename(folder.path("work"), folder.path("archive")).unwrap();
    std::fs::rename(folder.path("archive/thesis.md"), folder.path("archive/dissertation.md")).unwrap();
    std::fs::rename(folder.path("archive/.cowriting/thesis.md"), folder.path("archive/.cowriting/dissertation.md")).unwrap();

    let (session, opened) = Session::open_in(Arc::new(DiskHost), &folder.path("archive/dissertation.md"), Some(&projects)).unwrap();
    assert!(opened.orphans.is_empty());
    let view = session.project_view();
    assert_eq!(view.files.len(), 1, "the linked file is still linked (it shows as missing: it was in the folder that moved)");
    assert!(session.party_view().experts.iter().any(|e| e.persona.id == "reviewer-2" && e.is_prompt_edited));
    assert_eq!(session.log_view(TEXT).unwrap().turns.len(), 1);
    assert_eq!(std::fs::read_dir(&projects).unwrap().count(), 1, "the settings moved; they were not made anew beside the old ones");
    drop(session);

    // a copy is not a move: the original is still there, so its settings stay with it
    std::fs::create_dir_all(folder.path("copy/.cowriting")).unwrap();
    std::fs::copy(folder.path("archive/dissertation.md"), folder.path("copy/dissertation.md")).unwrap();
    let copy_all = |from: std::path::PathBuf, to: std::path::PathBuf| {
        fn walk(from: &std::path::Path, to: &std::path::Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap().map(Result::unwrap) {
                if entry.path().is_dir() {
                    walk(&entry.path(), &to.join(entry.file_name()));
                } else {
                    std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
                }
            }
        }
        walk(&from, &to);
    };
    copy_all(folder.0.join("archive/.cowriting/dissertation.md"), folder.0.join("copy/.cowriting/dissertation.md"));
    let (copy, _) = Session::open_in(Arc::new(DiskHost), &folder.path("copy/dissertation.md"), Some(&projects)).unwrap();
    assert!(copy.project_view().files.is_empty());
    let (original, _) = Session::open_in(Arc::new(DiskHost), &folder.path("archive/dissertation.md"), Some(&projects)).unwrap();
    assert_eq!(original.project_view().files.len(), 1);
}

#[test]
fn a_project_can_be_written_with_an_ai_of_its_own_and_the_others_keep_the_general_one() {
    use cowriting_app::BackendSettings;
    use cowriting_core::project::ProjectAi;
    let folder = Folder::new("own-ai");
    let projects = folder.path("app/projects");
    std::fs::write(folder.path("work/thesis.md"), TEXT).unwrap();
    std::fs::write(folder.path("work/paper.md"), TEXT).unwrap();
    let general = BackendSettings { backend: "anthropic".into(), model: "claude-opus-5-5".into(), base_url: String::new(), max_tokens: 0 };
    let (thesis, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/thesis.md"), Some(&projects)).unwrap();
    assert_eq!(thesis.settings_here(general.clone()), general);

    let local = ProjectAi { backend: "openai-compatible".into(), model: " gemma3:4b ".into(), base_url: "http://localhost:11434/v1".into(), max_tokens: 2000 };
    thesis.set_overrides(Project { ai: Some(local.clone()), ai_language: Some("de".into()), ..Default::default() }).unwrap();
    let here = thesis.settings_here(general.clone());
    assert_eq!((here.backend.as_str(), here.model.as_str(), here.base_url.as_str(), here.max_tokens), ("openai-compatible", "gemma3:4b", "http://localhost:11434/v1", 2000));
    let on_disk = std::fs::read_to_string(format!("{}/project.json", thesis.project_view().folder)).unwrap();
    assert!(on_disk.contains("gemma3:4b") && !on_disk.to_lowercase().contains("key"), "{on_disk}");

    // another document is asked with the general one; nonsense is refused and changes nothing
    let (paper, _) = Session::open_in(Arc::new(DiskHost), &folder.path("work/paper.md"), Some(&projects)).unwrap();
    assert_eq!(paper.settings_here(general.clone()), general);
    assert!(thesis.set_overrides(Project { ai: Some(ProjectAi { backend: "fake:/tmp/script.json".into(), ..local.clone() }), ..Default::default() }).is_err());
    assert!(thesis.set_overrides(Project { ai: Some(ProjectAi { model: "  ".into(), ..local }), ..Default::default() }).is_err());
    assert_eq!(thesis.settings_here(general.clone()).model, "gemma3:4b");

    // taken back: the general one again
    thesis.set_overrides(Project::default()).unwrap();
    assert_eq!(thesis.settings_here(general.clone()), general);
}
