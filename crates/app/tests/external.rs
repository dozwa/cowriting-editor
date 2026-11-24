//! AI use outside the editor is the author's own statement: judged by the log and the disclosure on disk.

use cowriting_app::{ExportOptions, ExternalUse, Session};
use cowriting_core::host::DiskHost;
use std::sync::Arc;

#[test]
fn the_author_states_outside_ai_use_corrects_it_and_takes_it_back_and_the_disclosure_follows() {
    let folder = std::env::temp_dir().join(format!("cowriting-external-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let document = folder.join("thesis.md").to_string_lossy().into_owned();
    std::fs::write(&document, "# Walking breaks\n").unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &document).unwrap();
    let export = |language: &str| {
        let options: ExportOptions = serde_json::from_value(serde_json::json!({ "language": language, "format": "md", "withPrompts": false, "withAnswers": false })).unwrap();
        session.export(&options).unwrap().register
    };

    // nothing stated: the disclosure has no such section
    assert!(session.state_external(&ExternalUse { tool: "ChatGPT".into(), ..Default::default() }).is_err(), "what it was used for is needed");
    assert!(session.state_external(&ExternalUse { purpose: "code".into(), ..Default::default() }).is_err(), "the tool is needed");

    let stated = ExternalUse {
        tool: "ChatGPT".into(),
        model: "GPT-x".into(),
        date: "2026-11-02".into(),
        purpose: "Writing R code for\nthe mixed model".into(),
        material: "The trial data (z)".into(),
        ..Default::default()
    };
    session.state_external(&stated).unwrap();
    session.state_external(&ExternalUse { tool: "DeepL".into(), purpose: "Translating the abstract".into(), ..Default::default() }).unwrap();
    let view = session.log_view("").unwrap();
    assert_eq!(view.external.iter().map(|e| e.tool.as_str()).collect::<Vec<_>>(), ["ChatGPT", "DeepL"]);
    let register = export("en");
    assert!(register.contains("## AI use outside the editor, as stated by the author"), "{register}");
    assert!(register.contains("- 2026-11-02: ChatGPT (GPT-x). Used for: Writing R code for the mixed model. Worked on: The trial data (z)."), "{register}");
    assert!(register.contains("- DeepL. Used for: Translating the abstract."));
    assert!(register.contains("These statements are the author's own."));

    // a correction takes the statement's place; a withdrawal removes it; the log keeps every line
    let first = view.external[0].id.clone();
    session.state_external(&ExternalUse { id: first.clone(), checked: "Every result recomputed by hand".into(), ..stated.clone() }).unwrap();
    assert!(session.state_external(&ExternalUse { id: first.clone(), ..stated.clone() }).is_err(), "a replaced statement cannot be replaced again");
    let view = session.log_view("").unwrap();
    assert_eq!(view.external.len(), 2);
    assert_eq!(view.external[0].checked, "Every result recomputed by hand");
    session.withdraw_external(&view.external[1].id).unwrap();
    assert!(session.withdraw_external("nothing").is_err());
    let german = export("de");
    assert!(german.contains("Geprüft durch: Every result recomputed by hand.") && !german.contains("DeepL"), "{german}");
    let log_folder = folder.join(".cowriting/thesis.md/ai-log");
    let log = std::fs::read_to_string(std::fs::read_dir(log_folder).unwrap().next().unwrap().unwrap().path()).unwrap();
    assert_eq!(log.lines().filter(|line| line.contains(r#""type":"external""#)).count(), 4);
    assert_eq!(std::fs::read_to_string(&document).unwrap(), "# Walking breaks\n");
    let _ = std::fs::remove_dir_all(&folder);
}
