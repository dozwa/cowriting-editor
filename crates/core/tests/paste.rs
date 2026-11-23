//! Paste tracking against a labelled set of passages (`fixtures/paste/cases.json`).

use cowriting_core::log::parse_log;
use cowriting_core::paste_tracking::{check, sources, still_in_text, Source};
use cowriting_core::types::AiTextEntry;
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/paste/cases.json")).unwrap()
}

#[test]
fn passages_are_judged_as_a_person_labelled_them() {
    let fixture = fixture();
    let text = |key: &str| fixture[key].as_str().unwrap();
    let known = [Source::new("t1", "text", text("answer"), &[text("document"), text("question")])];
    let mut wrong = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let found = check(case["passage"].as_str().unwrap(), &known);
        let got = found.as_ref().map_or(Value::Null, |f| json!(f.match_));
        if got != case["expect"] {
            wrong.push(format!("{}: expected {}, got {got} ({found:?})", case["name"], case["expect"]));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_match_says_which_answer_how_much_and_what_was_taken() {
    let fixture = fixture();
    let text = |key: &str| fixture[key].as_str().unwrap();
    let known = [
        Source::new("other", "text", "Something else entirely was said in this answer, about figures and their captions.", &[]),
        Source::new("t1", "text", text("answer"), &[text("document")]),
    ];
    let passage = "We thought about it.   Participants were randomly assigned to the walking\nor the sitting condition by a colleague who was not involved in data collection. Then it began.";
    let found = check(passage, &known).unwrap();
    assert_eq!((found.reference.as_str(), found.match_.as_str(), found.words), ("t1", "exact", 21));
    // the excerpt is the passage's own words, on one line, without what the author wrote around them
    assert_eq!(found.excerpt, "Participants were randomly assigned to the walking or the sitting condition by a colleague who was not involved in data collection");
    // the same words in another form are the same passage
    let again = check("PARTICIPANTS were randomly assigned to the walking or the sitting condition — by a colleague who was not involved in data collection!", &known).unwrap();
    assert_eq!(again.hash, found.hash);

    let entry = AiTextEntry { excerpt: found.excerpt.clone(), ..Default::default() };
    assert!(still_in_text(&entry, &format!("# Methods\n\n{passage}\n")));
    assert!(!still_in_text(&entry, "# Methods\n\nA colleague drew lots.\n"));
    assert!(!still_in_text(&entry, "`Participants were randomly assigned to the walking or the sitting condition by a colleague who was not involved in data collection`"));
}

#[test]
fn the_sources_come_from_the_log_and_the_document_as_it_was_then() {
    let log = [
        json!({"type": "prompt", "id": "t1", "ts": "2026-11-23T10:00:00Z", "text": "Check my grammar.", "category": "language"}),
        json!({"type": "response", "ref": "t1", "ts": "2026-11-23T10:00:05Z", "answer": "Line 3: write \"The participants were asked to rate their concentration twice a day\" instead."}),
        json!({"type": "prompt", "id": "t2", "ts": "2026-11-23T11:00:00Z", "text": "Thoughts?", "category": "general"}),
        json!({"type": "response", "ref": "t2", "ts": "2026-11-23T11:00:05Z", "answer": "You write that the breaks were short and that nobody was forced to take one of them."}),
        json!({"type": "response", "ref": "gone", "ts": "2026-11-23T12:00:00Z", "answer": "An answer without its question in the log is no source for anything at all."}),
    ];
    let entries = parse_log(&log.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
    // at the second question the document already said this: the answer only repeats the author
    let snapshot = |id: &str| (id == "t2").then(|| "The breaks were short and that nobody was forced to take one of them.".to_string());
    let known = sources(&entries, &snapshot);

    let correction = check("The participants were asked to rate their concentration twice a day.", &known).unwrap();
    assert_eq!((correction.reference.as_str(), correction.kind.as_str()), ("t1", "correction"));
    assert_eq!(check("The breaks were short and that nobody was forced to take one of them.", &known), None);
    assert_eq!(check("An answer without its question in the log is no source for anything at all.", &known), None);
}
