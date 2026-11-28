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
