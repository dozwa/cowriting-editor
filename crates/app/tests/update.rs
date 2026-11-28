//! A project and the editor's own files as version 0.1.0 wrote them (`fixtures/stored/v0.1.0`,
//! made by running that version), opened with this version: everything is still there, nothing
//! is reported, and what this version saves keeps the rest.

use cowriting_app::preferences::{self, Preferences, SettingsFile};
use cowriting_app::{support, usage, Session};
use cowriting_core::host::DiskHost;
use cowriting_core::log::parse_log;
use cowriting_core::sidecar::Sidecar;
use cowriting_core::stored::Notice;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The fixture, copied to a folder of its own, with its paths pointing there. Removed when the test ends.
struct Copy(PathBuf);

impl Copy {
    fn new(name: &str) -> Copy {
        let root = std::env::temp_dir().join(format!("cowriting-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/stored/v0.1.0");
        let copy = Copy(root);
        let document = copy.path("paper/thesis.md");
        let project = format!("projects/{}", Sidecar::project_key(&cowriting_app::project_files::native(&document)));
        copy.take(&from.join("paper"), "paper");
        copy.take(&from.join("config"), "config");
        std::fs::create_dir_all(copy.path("config/projects")).unwrap();
        std::fs::rename(copy.path("config/project"), copy.path(&format!("config/{project}"))).unwrap();
        copy
    }

    fn take(&self, from: &Path, to: &str) {
        for entry in std::fs::read_dir(from).unwrap() {
            let (path, target) = (entry.as_ref().unwrap().path(), format!("{to}/{}", entry.as_ref().unwrap().file_name().to_string_lossy()));
            if path.is_dir() {
                self.take(&path, &target);
            } else {
                std::fs::create_dir_all(Path::new(&self.path(&target)).parent().unwrap()).unwrap();
                let text = std::fs::read_to_string(&path).unwrap().replace("/home/author", &self.path("").trim_end_matches('/').replace('\\', "/"));
                std::fs::write(self.path(&target), text).unwrap();
            }
        }
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().replace('\\', "/")
    }

    fn json(&self, relative: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.path(relative)).unwrap()).unwrap()
    }

    fn open(&self) -> Result<(Session, cowriting_app::Opened), String> {
        Session::open_in(Arc::new(DiskHost), &self.path("paper/thesis.md"), Some(&self.path("config/projects")))
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_project_of_version_0_1_0_opens_with_everything_still_there() {
    let copy = Copy::new("opens");
    let (session, opened) = copy.open().unwrap();
    assert_eq!(opened.notices, Vec::<Notice>::new());
    let text = opened.text;

    // the log: every line is understood, and the Log tab shows what was asked, classified and stated
    let log = std::fs::read_to_string(copy.path("paper/.cowriting/thesis.md/ai-log/2026-11.jsonl")).unwrap();
    assert_eq!(parse_log(&log).len(), log.lines().count(), "a line of the old log was not understood");
    let view = session.log_view(&text).unwrap();
    assert_eq!(view.turns.iter().map(|turn| turn.asked.as_str()).filter(|asked| ["Any feedback on my methods?", "And the sampling?"].contains(asked)).count(), 2);
    assert!(view.turns.iter().any(|turn| turn.use_ == "adapted"));
    assert_eq!(view.external.iter().map(|stated| stated.tool.as_str()).collect::<Vec<_>>(), ["DeepL"]);
    assert!(std::fs::read_to_string(copy.path("paper/.cowriting/thesis.md/notes/methods.md")).unwrap().ends_with("Check the sample size."));

    // the project's settings
    let project = session.project_view();
    assert_eq!(project.files.iter().map(|file| file.path.clone()).collect::<Vec<_>>(), [copy.path("paper/rules.md")]);
    assert_eq!((project.overrides.writing_language.as_deref(), project.overrides.ai_language.as_deref(), project.overrides.live_per_hour), (Some("en"), Some("de"), Some(12)));
    let live = session.live_view();
    assert_eq!((live.is_on, live.personas), (true, vec!["grammar-guy".to_string()]));
    assert!(session.scope_view(&text).text.contains("Show that walking breaks help."));
    let sources = session.sources_view(&text);
    assert_eq!((sources.files.len(), sources.entries, sources.missing.len()), (1, 1, 0));
    assert_eq!(sources.checked.map(|checked| checked.rows.len()), Some(1));
    let party = session.party_view();
    assert_eq!(party.everyone, "The paper is for a public-health journal.");
    let own = party.experts.iter().find(|expert| expert.persona.id == "ethics-reviewer").expect("the author's own expert");
    assert!(own.is_member && own.persona.is_custom);
    assert!(party.experts.iter().find(|expert| expert.persona.id == "grammar-guy").unwrap().is_prompt_edited);

    // the document keeps its id: its log goes on, it does not start again
    let id = copy.json("paper/.cowriting/thesis.md/doc.json")["id"].clone();
    session.save(&format!("{text}\nOne more sentence.\n")).unwrap();
    assert_eq!(copy.json("paper/.cowriting/thesis.md/doc.json")["id"], id);
}

#[test]
fn what_this_version_saves_into_an_old_project_keeps_the_rest_and_names_its_form() {
    let copy = Copy::new("saves");
    let (session, opened) = copy.open().unwrap();
    let project = format!("config/projects/{}", Sidecar::project_key(&cowriting_app::project_files::native(&copy.path("paper/thesis.md"))));
    session.set_live(false, &["grammar-guy".to_string()], &opened.text).unwrap();
    session.set_member("lay-reader", false).unwrap();
    session.unlink_file(&copy.path("paper/rules.md")).unwrap();
    session.remove_bibliography(&copy.path("paper/refs.bib")).unwrap();
    for file in ["live.json", "party.json", "project.json", "sources.json"] {
        assert_eq!(copy.json(&format!("{project}/{file}"))["schema"], 1, "{file}");
    }
    let saved = copy.json(&format!("{project}/project.json"));
    assert_eq!((saved["files"].clone(), saved["aiLanguage"].clone(), saved["livePerHour"].clone()), (json!([]), json!("de"), json!(12)));
    assert_eq!(copy.json(&format!("{project}/party.json"))["members"].as_array().unwrap().len(), 9);
    assert!(!Path::new(&copy.path(&format!("{project}/backup"))).exists(), "the form did not change: there is nothing to keep a copy of");
    assert!(copy.open().unwrap().1.notices.is_empty());
}

#[test]
fn the_editors_own_files_of_version_0_1_0_read_and_are_added_to_without_loss() {
    let copy = Copy::new("own");
    let (settings, state) = (PathBuf::from(copy.path("config/settings.json")), PathBuf::from(copy.path("config/state.json")));
    assert!(preferences::look_over(&settings, &state).is_empty());
    let file = SettingsFile::read(&settings);
    let backend = file.backend.expect("the AI was set up");
    assert_eq!(
        (backend.backend.as_str(), backend.model.as_str(), backend.base_url.as_str(), backend.max_tokens),
        ("openai-compatible", "qwen3:8b", "http://localhost:11434/v1", 4000)
    );
    assert_eq!(file.preferences, Preferences { theme: "dark".into(), ..Preferences::default() });
    assert_eq!(preferences::last_document(&state), Some(copy.path("paper/thesis.md")));
    assert!(!preferences::is_tour_due(&state));
    assert!(usage::was_asked(&state));

    SettingsFile::save_preferences(&settings, Preferences { calm: true, ..file.preferences }).unwrap();
    support::started(&state);
    let (settings, state) = (copy.json("config/settings.json"), copy.json("config/state.json"));
    assert_eq!(
        (settings["schema"].clone(), settings["model"].clone(), settings["preferences"]["theme"].clone(), settings["preferences"]["calm"].clone()),
        (json!(1), json!("qwen3:8b"), json!("dark"), json!(true))
    );
    assert_eq!(
        (state["schema"].clone(), state["tourSeen"].clone(), state["support"]["starts"].clone(), state["usage"]["asked"].clone()),
        (json!(1), json!(true), json!(2), json!(true))
    );
}

#[test]
fn settings_that_cannot_be_read_are_set_aside_and_those_of_a_newer_editor_are_not_touched() {
    let copy = Copy::new("broken");
    let (settings, state) = (PathBuf::from(copy.path("config/settings.json")), PathBuf::from(copy.path("config/state.json")));
    std::fs::write(&settings, "{\"backend\": \"anthropic\", \"mod").unwrap();
    let newer = "{\n  \"schema\": 7,\n  \"lastDocument\": \"/somewhere/else.md\",\n  \"tourSeen\": true\n}\n";
    std::fs::write(&state, newer).unwrap();

    let notices = preferences::look_over(&settings, &state);
    let kept_as = match notices.as_slice() {
        [Notice::SetAside { kept_as, .. }, Notice::Newer { .. }] => kept_as.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(std::fs::read_to_string(&kept_as).unwrap(), "{\"backend\": \"anthropic\", \"mod");
    assert!(SettingsFile::read(&settings).backend.is_none(), "the editor asks for the AI's settings again");
    SettingsFile::save_preferences(&settings, Preferences::default()).unwrap();

    // the newer file: read as far as it is understood, and every write leaves it as it is
    assert!(!preferences::is_tour_due(&state));
    preferences::remember_document(&state, &copy.path("paper/thesis.md"));
    support::started(&state);
    assert_eq!(std::fs::read_to_string(&state).unwrap(), newer);
}

#[test]
fn a_document_last_worked_on_with_a_newer_editor_is_not_opened_and_its_record_stays_as_it_is() {
    let copy = Copy::new("newer");
    let doc = copy.path("paper/.cowriting/thesis.md/doc.json");
    let newer = std::fs::read_to_string(&doc).unwrap().replace("\"schema\": 1", "\"schema\": 2");
    std::fs::write(&doc, &newer).unwrap();
    let refused = copy.open().err().expect("the document is refused");
    assert!(refused.contains("newer version of the editor") && refused.contains("update"), "{refused}");
    assert_eq!(std::fs::read_to_string(&doc).unwrap(), newer);
}

#[test]
fn a_projects_file_that_cannot_be_read_is_named_when_the_document_opens() {
    let copy = Copy::new("named");
    let project = format!("config/projects/{}/project.json", Sidecar::project_key(&cowriting_app::project_files::native(&copy.path("paper/thesis.md"))));
    std::fs::write(copy.path(&project), "{ \"files\": [").unwrap();
    let (session, opened) = copy.open().unwrap();
    match opened.notices.as_slice() {
        [Notice::SetAside { file, kept_as }] => {
            assert_eq!(file, &copy.path(&project));
            assert_eq!(std::fs::read_to_string(kept_as).unwrap(), "{ \"files\": [");
        }
        other => panic!("{other:?}"),
    }
    assert!(session.project_view().files.is_empty());
    assert!(copy.open().unwrap().1.notices.is_empty(), "it is said once");
}
