//! A project of several markdown files: one file beside them lists them. They share the experts,
//! the linked files and the other settings; each keeps its own plan and log.

use cowriting_app::{project_files, Session};
use cowriting_core::host::DiskHost;
use cowriting_core::project_file::ProjectFile;
use cowriting_core::sidecar::Sidecar;
use std::path::PathBuf;
use std::sync::Arc;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("book/chapters")).unwrap();
        Folder(path)
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }

    fn write(&self, relative: &str, text: &str) -> String {
        std::fs::write(self.path(relative), text).unwrap();
        self.path(relative)
    }

    fn open(&self, relative: &str) -> Session {
        Session::open_in(Arc::new(DiskHost), &self.path(relative), Some(&self.path("app/projects"))).unwrap().0
    }

    fn listed(&self) -> Vec<String> {
        ProjectFile::load(&DiskHost, &self.path("book/book.cowriting.json")).unwrap().files
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const INTRO: &str = "# Introduction\n\nWalking breaks help. That is the claim.\n";
const METHOD: &str = "# Method\n\nWe recruited 48 participants from two offices and followed them.\n";

/// A book of two files, made from the first one.
fn book(folder: &Folder) -> Session {
    folder.write("book/intro.md", INTRO);
    folder.write("book/chapters/method.md", METHOD);
    let alone = folder.open("book/intro.md");
    assert_eq!(alone.make_project(&folder.path("book/book")).unwrap(), folder.path("book/book.cowriting.json"), "the project's file gets its ending");
    let intro = folder.open("book/intro.md");
    intro.add_project_file(&folder.path("book/chapters/method.md")).unwrap();
    intro
}

#[test]
fn a_single_document_becomes_the_first_file_of_a_project_and_keeps_its_experts_plan_and_log() {
    let folder = Folder::new("first");
    folder.write("book/intro.md", INTRO);
    folder.write("book/wording.md", "Say participants.\n");
    let alone = folder.open("book/intro.md");
    alone.link_file(&folder.path("book/wording.md"), &[]).unwrap();
    alone.set_prompt("reviewer-2", "You are the kindest reviewer there is.").unwrap();
    alone.set_scope("## Goal\nSay why.\n\n## Targets\n- Words: 500\n", INTRO).unwrap();
    assert!(alone.project_overview(INTRO).unwrap().is_none(), "a document no project lists is a project of its own");
    let doc_before = std::fs::read_to_string(folder.path("book/.cowriting/intro.md/doc.json")).unwrap();

    alone.make_project(&folder.path("book/book.cowriting.json")).unwrap();
    assert!(alone.make_project(&folder.path("book/other.cowriting.json")).unwrap_err().contains("already"), "a document is in one project only");

    let intro = folder.open("book/intro.md");
    let view = intro.project_view();
    assert_eq!(view.files.iter().map(|file| file.name.as_str()).collect::<Vec<_>>(), ["wording.md"], "the linked files came along");
    assert!(view.folder.contains("project-"), "the shared settings are kept under the project's id: {}", view.folder);
    assert!(intro.party_view().experts.iter().any(|expert| expert.persona.prompt.contains("kindest")), "and so did the author's wording of an expert");
    assert!(intro.scope_view(INTRO).text.contains("Words: 500"), "the plan stays the file's own");
    assert_eq!(std::fs::read_to_string(folder.path("book/.cowriting/intro.md/doc.json")).unwrap(), doc_before, "what is beside the document stays there");
    assert_eq!(std::fs::read_to_string(folder.path("book/intro.md")).unwrap(), INTRO, "and the manuscript is as it was");
}

#[test]
fn the_files_of_a_project_share_experts_and_linked_files_and_each_has_its_own_plan() {
    let folder = Folder::new("shared");
    let intro = book(&folder);
    folder.write("book/wording.md", "Say participants.\n");
    intro.link_file(&folder.path("book/wording.md"), &[]).unwrap();
    intro.set_prompt("reviewer-2", "You are the kindest reviewer there is.").unwrap();
    intro.set_scope("## Goal\nSay why.\n\n## Targets\n- Words: 500\n", INTRO).unwrap();

    let method = folder.open("book/chapters/method.md");
    assert_eq!(method.project_view().files.len(), 1, "a file linked in one chapter is linked in every chapter");
    assert!(method.party_view().experts.iter().any(|expert| expert.persona.prompt.contains("kindest")));
    assert_eq!(method.scope_view(METHOD).text, "", "the other chapter's plan is not this one's");
    method.set_scope("## Goal\nSay how.\n\n## Targets\n- Words: 20\n\n## Points\n- [x] 48 participants\n- Two offices\n", METHOD).unwrap();
    assert!(folder.open("book/intro.md").scope_view(INTRO).text.contains("Say why"), "and it leaves the first one's alone");

    // opening the project's file opens its first file
    let (_, opened) = Session::open_in(Arc::new(DiskHost), &folder.path("book/book.cowriting.json"), Some(&folder.path("app/projects"))).unwrap();
    assert_eq!(opened.name, "intro.md");
}

#[test]
fn the_overview_counts_every_file_and_the_whole_against_the_projects_target() {
    let folder = Folder::new("overview");
    let intro = book(&folder);
    intro.set_project_goal("  A book on walking breaks. ", Some(40_000)).unwrap();
    folder.open("book/chapters/method.md").set_scope("## Targets\n- Words: 20\n\n## Points\n- [x] 48 participants\n- Two offices\n", METHOD).unwrap();
    std::fs::remove_file(folder.path("book/chapters/method.md")).unwrap();
    folder.write("book/chapters/method.md", METHOD);
    intro.add_project_file(&folder.path("book/gone.md")).unwrap();
    std::fs::remove_file(folder.path("book/gone.md")).unwrap();

    // the open file is counted as it is in the window, not as it is on disk
    let typed = format!("{INTRO}\nOne more sentence here.\n");
    let overview = intro.project_overview(&typed).unwrap().unwrap();
    assert_eq!((overview.name.as_str(), overview.goal.as_str(), overview.words_target), ("book", "A book on walking breaks.", Some(40_000)));
    let rows: Vec<_> = overview.files.iter().map(|row| (row.name.as_str(), row.is_open, row.is_missing)).collect();
    assert_eq!(rows, [("intro.md", true, false), ("chapters/method.md", false, false), ("gone.md", false, true)]);
    let (first, second) = (&overview.files[0], &overview.files[1]);
    assert_eq!(first.words, cowriting_core::words::count_words(&typed, cowriting_core::words::Format::Markdown));
    assert_eq!((first.target, first.progress), (None, None), "a file without a plan has no target");
    assert_eq!((second.target, second.points, second.points_made), (Some(20.0), 2, 1));
    assert!(second.progress.is_some());
    assert_eq!(overview.words, first.words + second.words);
    assert!(overview.chars > overview.words);
}

#[test]
fn files_are_added_removed_and_ordered_without_touching_them() {
    let folder = Folder::new("list");
    let intro = book(&folder);
    let (first, second, third) = (folder.path("book/intro.md"), folder.path("book/chapters/method.md"), folder.path("book/results.md"));

    intro.add_project_file(&third).unwrap();
    assert_eq!(std::fs::read_to_string(&third).unwrap(), "", "a name that is not there yet becomes an empty file");
    assert_eq!(folder.listed(), ["intro.md", "chapters/method.md", "results.md"], "by the way from the project's folder, in the order added");
    assert!(intro.add_project_file(&third).unwrap_err().contains("already"));
    assert!(intro.add_project_file(&folder.path("book/notes.txt")).unwrap_err().contains(".md"));

    intro.order_project_files(&[third.clone(), first.clone(), second.clone()]).unwrap();
    assert_eq!(folder.listed(), ["results.md", "intro.md", "chapters/method.md"]);
    assert!(intro.order_project_files(&[third.clone(), first.clone()]).unwrap_err().contains("leaves out"));
    assert!(intro.order_project_files(&[third.clone(), first.clone(), first.clone()]).is_err());
    // opening the project's file opens the file that is first now
    assert_eq!(project_files::document_to_open(&DiskHost, &folder.path("book/book.cowriting.json")).unwrap(), project_files::native(&third));

    intro.remove_project_file(&second).unwrap();
    assert_eq!(folder.listed(), ["results.md", "intro.md"]);
    assert_eq!(std::fs::read_to_string(&second).unwrap(), METHOD, "a file taken out of the project stays where it is");
    assert!(folder.open("book/chapters/method.md").project_overview(METHOD).unwrap().is_none(), "and is a document of its own again");
    intro.remove_project_file(&third).unwrap();
    assert!(intro.remove_project_file(&first).unwrap_err().contains("at least one"));

    // a file of one project is not added to another
    let other = folder.write("book/alone.md", "# Alone\n");
    let alone = folder.open("book/alone.md");
    alone.make_project(&folder.path("book/second.cowriting.json")).unwrap();
    assert!(folder.open("book/alone.md").add_project_file(&first).unwrap_err().contains("belongs to the project book"));
    assert!(intro.add_project_file(&other).unwrap_err().contains("belongs to the project second"));
}

#[test]
fn a_renamed_or_moved_file_stays_in_the_project_with_its_log_and_plan() {
    let folder = Folder::new("rename");
    let intro = book(&folder);
    let method = folder.open("book/chapters/method.md");
    method.set_scope("## Goal\nSay how.\n\n## Targets\n- Words: 20\n", METHOD).unwrap();
    assert!(std::path::Path::new(&folder.path("book/chapters/.cowriting/method.md/doc.json")).exists());

    // another file of the project, from the list
    assert!(intro.rename_project_file(&folder.path("book/intro.md"), "start.md").unwrap_err().contains("Move"), "the open document goes through Move, with the text in the window");
    assert!(intro.rename_project_file(&folder.path("book/chapters/method.md"), "../method.md").is_err());
    assert!(intro.rename_project_file(&folder.path("book/chapters/method.md"), "method.txt").is_err());
    let renamed = intro.rename_project_file(&folder.path("book/chapters/method.md"), "methods.md").unwrap();
    assert_eq!(folder.listed(), ["intro.md", "chapters/methods.md"]);
    assert_eq!(std::fs::read_to_string(&renamed).unwrap(), METHOD);
    assert!(!std::path::Path::new(&folder.path("book/chapters/method.md")).exists());
    assert!(std::path::Path::new(&folder.path("book/chapters/.cowriting/methods.md/doc.json")).exists(), "the log went with it");
    assert!(folder.open("book/chapters/methods.md").scope_view(METHOD).text.contains("Say how"), "and so did its plan");

    // the open document, with "Move…"
    intro.move_document(&folder.path("book/chapters/start.md"), INTRO).unwrap();
    assert_eq!(folder.listed(), ["chapters/start.md", "chapters/methods.md"]);
    let start = folder.open("book/chapters/start.md");
    assert_eq!(start.project_overview(INTRO).unwrap().unwrap().files.len(), 2, "the project is found from a folder below its file");
}

#[test]
fn without_a_folder_of_the_editors_the_shared_settings_are_beside_the_projects_file() {
    let folder = Folder::new("beside");
    folder.write("book/intro.md", INTRO);
    let open = || Session::open(Arc::new(DiskHost), &folder.path("book/intro.md")).unwrap().0;
    open().make_project(&folder.path("book/book.cowriting.json")).unwrap();
    assert_eq!(open().project_view().folder, Sidecar::for_document(&folder.path("book/book.cowriting.json")).dir());
}

/// A book whose second file has front matter, a footnote named like one of the first, and a picture in its own folder.
fn book_to_export(folder: &Folder) -> (Session, String) {
    let intro = "---\ntitle: Walking\nlang: en\n---\n\n# Introduction\n\nWalking breaks help.[^1]\n\n[^1]: The first chapter's note.\n";
    let method = "---\ntitle: Method\n---\n\n# Method\n\nWe followed 48 participants.[^1]\n\n![the offices](images/picture.png)\n\n[^1]: The second chapter's note.\n";
    folder.write("book/intro.md", intro);
    folder.write("book/chapters/method.md", method);
    std::fs::create_dir_all(folder.path("book/chapters/images")).unwrap();
    std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pictures/picture.png"), folder.path("book/chapters/images/picture.png")).unwrap();
    folder.open("book/intro.md").make_project(&folder.path("book/book")).unwrap();
    let session = folder.open("book/intro.md");
    session.add_project_file(&folder.path("book/chapters/method.md")).unwrap();
    (session, intro.to_string())
}

#[test]
fn the_whole_project_is_one_text_in_the_files_order_with_notes_and_pictures_that_still_fit() {
    let folder = Folder::new("whole");
    let (_, intro) = book_to_export(&folder);
    let project = folder.path("book/book.cowriting.json");
    // the open document counts as it is in the window, saved or not
    let in_window = intro.replace("Walking breaks help.", "Walking breaks help a lot.");
    let whole = project_files::whole(&DiskHost, &project, Some((&folder.path("book/intro.md"), &in_window))).unwrap();
    assert_eq!(whole.files, ["intro.md", "chapters/method.md"]);
    assert_eq!(std::path::Path::new(&whole.folder), std::path::Path::new(&folder.path("book")));
    let expected = "---\ntitle: Walking\nlang: en\n---\n\n# Introduction\n\nWalking breaks help a lot.[^1]\n\n[^1]: The first chapter's note.\n\n\
                    # Method\n\nWe followed 48 participants.[^f2-1]\n\n![the offices](chapters/images/picture.png)\n\n[^f2-1]: The second chapter's note.\n";
    assert_eq!(whole.text, expected, "the first file's front matter holds for the whole; the second file's note and picture are its own");

    // a chapter that is gone stops the export and is named
    std::fs::remove_file(folder.path("book/chapters/method.md")).unwrap();
    let why = project_files::whole(&DiskHost, &project, None).unwrap_err();
    assert!(why.starts_with("chapters/method.md is not where the project names it."), "{why}");
}

#[test]
fn an_export_of_the_whole_project_holds_every_file_and_one_of_a_single_document_is_as_before() {
    use cowriting_app::export::find_program;
    use cowriting_app::{FileFormat, PdfChoice, WholeExport};
    let folder = Folder::new("whole-export");
    let (session, intro) = book_to_export(&folder);
    let offered = session.whole_export().expect("a file of a project is offered the whole");
    // beside the project's file, in the spelling of paths the system has
    assert_eq!(
        offered,
        WholeExport { name: "book".into(), files: 2, pdf: project_files::native(&folder.path("book/book.pdf")), docx: project_files::native(&folder.path("book/book.docx")) }
    );
    folder.write("alone.md", "# Alone\n");
    assert_eq!(folder.open("alone.md").whole_export(), None);
    if find_program("pandoc").is_none() {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "pandoc is not there: run scripts/fetch-tools.py");
        eprintln!("pandoc is not installed: the export of a whole project was not run");
        return;
    }
    let words = |path: &str| {
        let mut file = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut text = String::new();
        for name in ["word/document.xml", "word/footnotes.xml"] {
            std::io::Read::read_to_string(&mut file.by_name(name).unwrap(), &mut text).unwrap();
        }
        let has_picture = file.file_names().any(|name| name.starts_with("word/media/"));
        (text, has_picture)
    };
    let whole = PdfChoice { whole_project: true, ..Default::default() };
    let made = session.export_document(FileFormat::Docx, &offered.docx, &intro, None, &whole).unwrap();
    assert!(!made.warnings.iter().any(|warning| warning.contains("picture") || warning.contains("ote")), "{:?}", made.warnings);
    let (text, has_picture) = words(&offered.docx);
    for part in ["Walking breaks help.", "We followed 48 participants.", "The first chapter", "The second chapter"] {
        assert!(text.contains(part), "the Word file of the whole project has: {part}");
    }
    assert!(has_picture, "the picture of the second file is found from the project's folder");
    // and as a PDF with the editor's own Typst, where it is there (CI insists on it)
    if cowriting_app::export::local_tools().pdf_engines.iter().any(|engine| engine == "typst") {
        let choice = PdfChoice { engine: Some("typst".into()), whole_project: true, ..Default::default() };
        let made = session.export_document(FileFormat::Pdf, &offered.pdf, &intro, None, &choice).unwrap();
        assert!(!made.warnings.iter().any(|warning| warning.contains("picture")), "{:?}", made.warnings);
        assert!(std::fs::read(&offered.pdf).unwrap().starts_with(b"%PDF-"));
    } else {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "Typst is not there: run scripts/fetch-tools.py");
    }

    let made = session.export_document(FileFormat::Docx, &folder.path("book/intro.docx"), &intro, None, &PdfChoice::default());
    made.unwrap();
    let (text, _) = words(&folder.path("book/intro.docx"));
    assert!(text.contains("Walking breaks help.") && !text.contains("We followed 48 participants."), "one document's export is that document");
}

#[test]
fn the_disclosure_of_the_whole_project_is_made_of_the_logs_of_all_its_files() {
    use cowriting_app::{ExportOptions, ExternalUse};
    let folder = Folder::new("whole-disclosure");
    let (intro, _) = book_to_export(&folder);
    intro.state_external(&ExternalUse { tool: "ChatGPT".into(), purpose: "An outline of the introduction".into(), ..Default::default() }).unwrap();
    let method = folder.open("book/chapters/method.md");
    method.state_external(&ExternalUse { tool: "DeepL".into(), purpose: "Translating the method section".into(), ..Default::default() }).unwrap();

    let options = |whole_project| ExportOptions { language: "en".into(), format: "md".into(), with_prompts: true, with_answers: false, with_outside: true, whole_project };
    let made = method.export(&options(true)).unwrap();
    assert_eq!(made.paths.len(), 3);
    let own = PathBuf::from(folder.path("book/.cowriting/book.cowriting.json/disclosure"));
    assert!(made.paths.iter().all(|path| std::path::Path::new(path).starts_with(&own)), "it is kept under the project's own name: {:?}", made.paths);
    assert!(made.register.contains("book (intro.md, chapters/method.md)"), "it names the project and its files:\n{}", made.register);
    assert!(made.register.contains("ChatGPT") && made.register.contains("DeepL"), "the use stated in either file is in it");

    let one = method.export(&options(false)).unwrap();
    assert!(one.register.contains("DeepL") && !one.register.contains("ChatGPT"), "one file's disclosure is that file's");
    assert!(std::path::Path::new(&one.paths[0]).starts_with(folder.path("book/chapters/.cowriting/method.md/disclosure")));

    folder.write("alone.md", "# Alone\n");
    assert!(folder.open("alone.md").export(&options(true)).is_err(), "a document on its own has no project to disclose");
}

#[tokio::test(flavor = "current_thread")]
async fn the_ai_can_read_the_other_files_of_a_project_and_nothing_else_and_changes_none() {
    use cowriting_backends::fake::FakeBackend;
    use cowriting_core::backend::Block;
    use cowriting_core::turn::Ask;
    let folder = Folder::new("ai-reads");
    let intro = book(&folder);
    folder.write("book/secret.md", "Not a file of the project.\n");
    // the model looks what there is, searches, reads the other chapter, tries a file the project does not list and one outside, then answers
    let call = |name: &str, input: serde_json::Value| serde_json::json!({ "tools": [{ "name": name, "input": input }] });
    let script = serde_json::json!({ "replies": [
        call("list_project_files", serde_json::json!({})),
        call("search_project", serde_json::json!({ "query": "PARTICIPANTS" })),
        call("read_project_file", serde_json::json!({ "name": "chapters/method.md" })),
        call("read_project_file", serde_json::json!({ "name": "secret.md" })),
        call("read_project_file", serde_json::json!({ "name": "../book/secret.md" })),
        { "text": "The method chapter says 48 participants; the introduction names no number." }
    ] });
    // until the author allows it, the AI is offered nothing of the other files and told nothing of them: it stays with the file it is asked in
    let focused = FakeBackend::from_json(
        &serde_json::json!({ "replies": [call("read_project_file", serde_json::json!({ "name": "chapters/method.md" })), { "text": "Only this file." }] }).to_string(),
    )
    .unwrap();
    let outcome = intro
        .ask(&focused, Ask { text: "What does the method chapter say?".into(), document_text: INTRO.into(), model: "scripted".into(), ..Default::default() }, &mut |_| {})
        .await
        .unwrap();
    let request = &focused.requests()[0];
    assert!(!request.tools.iter().any(|tool| tool.name.contains("project")) && !request.system.contains("# The project"), "nothing of the project without the author's word");
    assert_eq!(outcome.refused, 1, "a call for it all the same is refused and on the record");
    assert!(!intro.project_overview(INTRO).unwrap().unwrap().ai_reads_files);
    intro.set_project_ai_reads_files(true).unwrap();
    assert!(intro.project_overview(INTRO).unwrap().unwrap().ai_reads_files);

    let backend = FakeBackend::from_json(&script.to_string()).unwrap();
    // what is in the window counts for the open file, saved or not
    let in_window = format!("{INTRO}\nNot saved yet.\n");
    let outcome = intro
        .ask(&backend, Ask { text: "Do the chapters agree on the sample?".into(), document_text: in_window.clone(), model: "scripted".into(), ..Default::default() }, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(outcome.tools_used, ["list_project_files", "search_project", "read_project_file"]);

    let requests = backend.requests();
    let offered: Vec<&str> = requests[0].tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(["list_project_files", "read_project_file", "search_project"].iter().all(|tool| offered.contains(tool)), "{offered:?}");
    assert!(!offered.iter().any(|tool| tool.contains("write") && *tool != "write_note"), "the only writer is still the note: {offered:?}");
    assert!(requests[0].system.contains("one file of the author's project “book”. Its files, in their order: intro.md, chapters/method.md."), "the AI is told of the project");
    let result = |request: usize| match requests[request].messages.last().unwrap().content.last().unwrap() {
        Block::ToolResult { content, is_error, .. } => (content.clone(), *is_error),
        other => panic!("no tool result: {other:?}"),
    };
    assert_eq!(result(1), ("intro.md (11 words), this document\nchapters/method.md (11 words)".to_string(), false));
    assert_eq!(result(2), ("# chapters/method.md\n3| We recruited 48 participants from two offices and followed them.".to_string(), false));
    assert_eq!(result(3), ("1| # Method\n2| \n3| We recruited 48 participants from two offices and followed them.\n4| ".to_string(), false));
    assert_eq!(result(4), ("The project has no file of that name. list_project_files gives the names.".to_string(), true), "a file beside them that the project does not list");
    assert!(result(5).1, "and no way out of the list by a path");
    assert_eq!(std::fs::read_to_string(folder.path("book/chapters/method.md")).unwrap(), METHOD, "the other file is as it was");
    assert_eq!(std::fs::read_to_string(folder.path("book/intro.md")).unwrap(), INTRO);

    // a document on its own is offered none of it
    folder.write("alone.md", "# Alone\n");
    let alone = folder.open("alone.md");
    let backend = FakeBackend::from_json(r#"{"replies":[{"text":"Fine."}]}"#).unwrap();
    alone.ask(&backend, Ask { text: "Anything?".into(), document_text: "# Alone\n".into(), model: "scripted".into(), ..Default::default() }, &mut |_| {}).await.unwrap();
    let request = &backend.requests()[0];
    assert!(!request.tools.iter().any(|tool| tool.name.contains("project")) && !request.system.contains("# The project"));
}
