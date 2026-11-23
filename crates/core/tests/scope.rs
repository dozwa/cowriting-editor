//! Scope and live review against the plugin's own output (`fixtures/goldens/scope.json`, written
//! by `scripts/make-goldens.py` from `fixtures/scope-cases.json`).

use cowriting_core::briefs::parse_briefs;
use cowriting_core::disclosure::Lang;
use cowriting_core::personas::Party;
use cowriting_core::scope::{is_empty_plan, numbered_text, parse_check, parse_scope, scope_check_system, scope_progress, Checked, WORDS_PER_PAGE};
use cowriting_core::watch::{parse_hints, watch_excerpt, watch_system, PAUSE_MS, WATCH_DEFAULT};
use cowriting_core::words::Paragraph;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn read(relative: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load(relative: &str) -> Value {
    serde_json::from_str(&read(relative)).unwrap()
}

/// Numbers compare by value, to twelve decimal places: JavaScript writes 3000 where Rust's float
/// is 3000.0, and reading a long decimal back from JSON may be off in its last digit.
fn plain(value: &Value) -> Value {
    match value {
        Value::Number(n) => json!((n.as_f64().unwrap() * 1e12).round() / 1e12),
        Value::Array(items) => items.iter().map(plain).collect(),
        Value::Object(fields) => fields.iter().map(|(k, v)| (k.clone(), plain(v))).collect::<serde_json::Map<_, _>>().into(),
        other => other.clone(),
    }
}

fn same(ours: impl serde::Serialize, golden: &Value, what: &str) {
    let ours = plain(&serde_json::to_value(ours).unwrap());
    assert_eq!(ours, plain(golden), "{what}");
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_else(|| panic!("no text {key:?} in {value}"))
}

#[test]
fn a_plan_is_read_from_scope_md_as_the_plugin_reads_it() {
    let (cases, goldens) = (load("scope-cases.json"), load("goldens/scope.json"));
    for (name, plan) in cases["plans"].as_object().unwrap() {
        let plan = parse_scope(plan.as_str().unwrap());
        same(&plan, &goldens["plans"][name], name);
        assert_eq!(json!(is_empty_plan(&plan)), goldens["isEmpty"][name], "is {name} empty");
    }
    same(WORDS_PER_PAGE, &goldens["wordsPerPage"], "words per page");
}

#[test]
fn progress_is_measured_as_the_plugin_measures_it() {
    let (cases, goldens) = (load("scope-cases.json"), load("goldens/scope.json"));
    for (case, golden) in cases["progress"].as_array().unwrap().iter().zip(goldens["progress"].as_array().unwrap()) {
        let plan = parse_scope(text(&cases["plans"], text(case, "plan")));
        let document = case["text"].as_str().map(String::from).unwrap_or_else(|| read(&format!("documents/{}", text(case, "document"))));
        let checked: BTreeMap<String, Checked> = serde_json::from_value(case["checked"].clone()).unwrap();
        let today = case["today"].as_str().unwrap_or(text(&cases, "today"));
        same(scope_progress(&plan, &document, &checked, case["checkedAt"].as_str(), today), golden, text(case, "name"));
    }
}

#[test]
fn a_point_check_is_asked_and_read_as_the_plugin_does() {
    let (cases, goldens) = (load("scope-cases.json"), load("goldens/scope.json"));
    for (i, case) in cases["check"].as_array().unwrap().iter().enumerate() {
        same(parse_check(text(case, "text"), case["count"].as_u64().unwrap() as usize), &goldens["check"][i], &format!("check {i}"));
    }
    for (i, case) in cases["numbered"].as_array().unwrap().iter().enumerate() {
        let files: Vec<(&str, &str)> = case["files"].as_array().unwrap().iter().map(|f| (text(f, "file"), text(f, "text"))).collect();
        assert_eq!(json!(numbered_text(&files, case["maxChars"].as_u64().unwrap() as usize)), goldens["numbered"][i], "numbered {i}");
    }
    assert_eq!(json!(scope_check_system(Some(Lang::En), None)), goldens["checkSystem"]["en"]);
    assert_eq!(json!(scope_check_system(Some(Lang::De), None)), goldens["checkSystem"]["de"]);
    // the editor does not know the manuscript's language and asks for it
    let ours = scope_check_system(None, Some("thesis.md"));
    assert!(ours.contains("Write the notes in the language of the manuscript.") && ours.contains("\"where\": \"thesis.md:42\"") && !ours.contains("main.tex"));
}

#[test]
fn live_review_is_asked_and_read_as_the_plugin_does() {
    let (cases, goldens, party) = (load("scope-cases.json"), load("goldens/scope.json"), Party::default());
    assert_eq!(json!(WATCH_DEFAULT), goldens["watchDefault"]);
    assert_eq!(json!(PAUSE_MS), goldens["pauseMs"]);
    let ids = |case: &Value| -> Vec<String> { case["personas"].as_array().unwrap().iter().map(|id| id.as_str().unwrap().to_string()).collect() };
    for (i, case) in cases["hints"].as_array().unwrap().iter().enumerate() {
        let ids = ids(case);
        same(parse_hints(text(case, "text"), &ids.iter().map(String::as_str).collect::<Vec<_>>()), &goldens["hints"][i], &format!("hints {i}"));
    }
    for (i, case) in cases["excerpt"].as_array().unwrap().iter().enumerate() {
        let parts: Vec<(String, Vec<Paragraph>)> = case["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| {
                let changed = part["changed"].as_array().unwrap().iter().map(|p| Paragraph {
                    start: p["start"].as_u64().unwrap() as usize,
                    end: p["end"].as_u64().unwrap() as usize,
                    text: text(p, "text").to_string(),
                });
                (text(part, "file").to_string(), changed.collect())
            })
            .collect();
        let parts: Vec<(&str, &[Paragraph])> = parts.iter().map(|(file, changed)| (file.as_str(), changed.as_slice())).collect();
        assert_eq!(json!(watch_excerpt(&parts, case["maxChars"].as_u64().unwrap() as usize)), goldens["excerpt"][i], "excerpt {i}");
    }
    for (i, case) in cases["system"].as_array().unwrap().iter().enumerate() {
        let ids = ids(case);
        let personas: Vec<_> = ids.iter().map(|id| party.by_id(id).unwrap()).collect();
        let language = if text(case, "language") == "de" { Lang::De } else { Lang::En };
        assert_eq!(json!(watch_system(&personas, Some(language), text(case, "profile"), &parse_briefs(text(case, "briefs"), &party))), goldens["system"][i], "system {i}");
    }
}
