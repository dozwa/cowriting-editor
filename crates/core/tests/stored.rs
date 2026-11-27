//! How the editor's stored files survive a change of version: older ones are brought up to date
//! with a copy kept, newer ones are left alone, and one that cannot be read is never written over.

use cowriting_core::host::{Host, MemoryHost};
use cowriting_core::project::{LinkedFile, Project};
use cowriting_core::sidecar::Sidecar;
use cowriting_core::stored::{self, Kind, Notice};
use serde_json::{json, Map, Value};

/// A kind in its third form: the first step renamed a field, the second made a text a list.
const NOTES: Kind = Kind::new("a test's notes", &[renamed, listed]);

fn renamed(json: &mut Map<String, Value>) {
    if let Some(title) = json.remove("name") {
        json.insert("title".into(), title);
    }
}

fn listed(json: &mut Map<String, Value>) {
    let tags = json.get("tags").and_then(Value::as_str).map(|tags| tags.split(',').map(str::trim).filter(|tag| !tag.is_empty()).collect::<Vec<_>>()).unwrap_or_default();
    json.insert("tags".into(), json!(tags));
}

fn host_with(path: &str, text: &str) -> MemoryHost {
    let host = MemoryHost::default();
    host.write(path, text).unwrap();
    host
}

#[test]
fn an_older_file_goes_through_every_step_it_is_behind_and_the_file_itself_stays_until_it_is_written() {
    let first = r#"{"name": "Plan", "tags": "a, b"}"#;
    let host = host_with("/p/notes.json", first);
    let read = stored::read(&host, "/p/notes.json", &NOTES).unwrap();
    assert_eq!(Value::Object(read), json!({"tags": ["a", "b"], "title": "Plan", "schema": 3}));
    assert_eq!(host.read("/p/notes.json").unwrap(), first, "reading changes nothing on disk");

    // a file of the second form takes the second step only
    let host = host_with("/p/notes.json", r#"{"schema": 2, "title": "Plan", "name": "kept", "tags": "a"}"#);
    assert_eq!(Value::Object(stored::read(&host, "/p/notes.json", &NOTES).unwrap()), json!({"schema": 3, "title": "Plan", "name": "kept", "tags": ["a"]}));
}

#[test]
fn before_an_older_file_is_written_in_the_new_form_a_copy_of_it_is_kept() {
    let first = r#"{"name": "Plan", "tags": "a, b"}"#;
    let host = host_with("/p/notes.json", first);
    let mut json = stored::read(&host, "/p/notes.json", &NOTES).unwrap();
    json.insert("title".into(), json!("Plan B"));
    stored::write(&host, "/p/notes.json", &NOTES, &json).unwrap();
    assert_eq!(host.read("/p/backup/schema-1/notes.json").unwrap(), first);
    assert_eq!(serde_json::from_str::<Value>(&host.read("/p/notes.json").unwrap()).unwrap(), json!({"tags": ["a", "b"], "title": "Plan B", "schema": 3}));

    // the next write is of the current form: the copy is the first form's, and stays
    stored::write(&host, "/p/notes.json", &NOTES, &json).unwrap();
    assert_eq!(host.read("/p/backup/schema-1/notes.json").unwrap(), first);
    assert!(!host.exists("/p/backup/schema-3/notes.json"));
}

#[test]
fn a_file_from_a_newer_editor_is_read_as_far_as_it_is_understood_and_never_written() {
    let newer = r#"{"schema": 4, "title": "Plan", "tags": ["a"], "colour": "green"}"#;
    let host = host_with("/p/notes.json", newer);
    assert!(stored::is_newer(&host, "/p/notes.json", &NOTES));
    assert_eq!(stored::read(&host, "/p/notes.json", &NOTES).unwrap()["title"], "Plan");
    assert_eq!(stored::look_at::<Map<String, Value>>(&host, "/p/notes.json", &NOTES), Some(Notice::Newer { file: "/p/notes.json".into() }));
    let refused = stored::write(&host, "/p/notes.json", &NOTES, &json!({"title": "Mine"})).unwrap_err();
    assert!(refused.to_string().contains("please update the editor"), "{refused}");
    assert_eq!(host.read("/p/notes.json").unwrap(), newer);
}

#[test]
fn a_file_that_cannot_be_read_is_set_aside_and_not_replaced_by_the_standard() {
    let torn = r#"{"document": "/p/thesis.md", "files": [{"path": "/p/rules.md""#;
    let sidecar = Sidecar::for_document("/p/thesis.md");
    let host = host_with(&sidecar.project_json(), torn);
    let notices = sidecar.look_over(&host);
    let kept_as = match notices.as_slice() {
        [Notice::SetAside { file, kept_as }] if *file == sidecar.project_json() => kept_as.clone(),
        other => panic!("expected the project's settings to be set aside: {other:?}"),
    };
    assert!(kept_as.starts_with(&format!("{}.broken-", sidecar.project_json())), "{kept_as}");
    assert_eq!(host.read(&kept_as).unwrap(), torn);
    assert!(!host.exists(&sidecar.project_json()));

    // the editor goes on with the standard, and what it saves does not touch the file set aside
    assert_eq!(Project::load(&host, &sidecar), Project::default());
    Project::default().save(&host, &sidecar).unwrap();
    assert_eq!(host.read(&kept_as).unwrap(), torn);
    assert!(sidecar.look_over(&host).is_empty());
}

#[test]
fn a_file_that_does_not_fit_its_type_is_set_aside_too_and_a_second_one_gets_a_name_of_its_own() {
    let sidecar = Sidecar::for_document("/p/thesis.md");
    let host = host_with(&sidecar.project_json(), r#"{"files": "rules.md"}"#);
    assert_eq!(sidecar.look_over(&host).len(), 1);
    host.write(&sidecar.project_json(), "not json at all").unwrap();
    // without having looked first: writing sets the unreadable file aside by itself
    Project::default().save(&host, &sidecar).unwrap();
    let folder = sidecar.settings_dir();
    let aside: Vec<String> = host.list(&folder).unwrap().into_iter().filter(|name| name.starts_with("project.json.broken-")).collect();
    assert_eq!(aside.len(), 2, "{aside:?}");
    assert!(aside.iter().any(|name| name.ends_with("-2")), "{aside:?}");
}

#[test]
fn what_a_newer_editor_added_to_a_file_is_still_there_after_this_one_saved_it() {
    let sidecar = Sidecar::for_document("/p/thesis.md");
    let host = host_with(&sidecar.project_json(), r#"{"document": "/p/thesis.md", "files": [], "aiLanguage": "de", "chapters": ["intro.md"], "tone": {"formal": true}}"#);
    let mut project = Project::load(&host, &sidecar);
    assert_eq!(project.ai_language.as_deref(), Some("de"));
    project.files.push(LinkedFile { path: "/p/rules.md".into(), personas: Vec::new(), text: None });
    project.ai_language = None;
    project.save(&host, &sidecar).unwrap();
    let saved: Value = serde_json::from_str(&host.read(&sidecar.project_json()).unwrap()).unwrap();
    assert_eq!(saved["chapters"], json!(["intro.md"]));
    assert_eq!(saved["tone"], json!({"formal": true}));
    assert_eq!(saved["files"][0]["path"], "/p/rules.md");
    assert_eq!(saved.get("aiLanguage"), None, "what the author took back is gone");
    assert_eq!(saved["schema"], 1);
}

#[test]
fn every_stored_file_says_which_form_it_has_once_it_is_written() {
    let sidecar = Sidecar::for_document("/p/thesis.md");
    let host = MemoryHost::default();
    sidecar.ensure(&host, "text").unwrap();
    Project::default().save(&host, &sidecar).unwrap();
    for file in [sidecar.doc_json(), sidecar.project_json()] {
        let json: Value = serde_json::from_str(&host.read(&file).unwrap()).unwrap();
        assert_eq!(json["schema"], 1, "{file}");
    }
    assert!(sidecar.look_over(&host).is_empty());
}
