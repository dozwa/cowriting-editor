//! The editor's first slice on a real folder: open a file, ask, classify, export, and read what
//! is on disk afterwards. The model is a script; the files are real.

use cowriting_app::{make_backend, BackendSettings, ExportOptions, Session};
use cowriting_core::host::DiskHost;
use cowriting_core::turn::Ask;
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
    assert!(make_backend(&BackendSettings { backend: "openai-compatible".into(), ..Default::default() }, Some("k")).is_err());
}
