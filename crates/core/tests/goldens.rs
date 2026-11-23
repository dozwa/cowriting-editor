//! The Rust core against the plugin's own output (`fixtures/goldens`, written by
//! `scripts/make-goldens.py`): same log, same documents, same answers.

use cowriting_core::disclosure::{disclosure, prompts_appendix, DocFormat, Lang, Profile};
use cowriting_core::guard::{author_file_of, changed_files, is_log_path, lies_in, Layout};
use cowriting_core::log::{category_for, month_file, parse_log, readable, rewritten_log_files, stray_log_files, LogSnapshot};
use cowriting_core::personas::Party;
use cowriting_core::stats::compute_stats;
use cowriting_core::words::{changed_paragraphs, count_words, paragraphs, Format};
use serde_json::{json, Value};
use std::path::PathBuf;

const PAPER_NAME: &str = "Walking breaks & 100% focus_time";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(fixtures().join(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

fn names(folder: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures().join(folder)).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

fn core_goldens() -> Value {
    serde_json::from_str(&read("goldens/core.json")).unwrap()
}

fn cases() -> Value {
    serde_json::from_str(&read("core-cases.json")).unwrap()
}

fn format_of(name: &str) -> Format {
    if name.ends_with(".tex") {
        Format::Latex
    } else {
        Format::Markdown
    }
}

fn snapshot(value: &Value) -> LogSnapshot {
    value.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect()
}

#[test]
fn the_disclosure_and_the_prompts_appendix_are_the_plugins_byte_for_byte() {
    let mut compared = 0;
    for log in names("logs") {
        let case = log.trim_end_matches(".jsonl");
        let entries = parse_log(&read(&format!("logs/{log}")));
        for (lang, lang_name) in [(Lang::En, "en"), (Lang::De, "de")] {
            for (format, format_name) in [(DocFormat::Md, "md"), (DocFormat::Tex, "tex")] {
                let register = format!("goldens/{case}/ai-register.{lang_name}.{format_name}");
                assert_eq!(disclosure(&entries, PAPER_NAME, lang, format, Profile::Plugin), read(&register), "{register}");
                for (with_answers, suffix) in [(false, ""), (true, "-answers")] {
                    let appendix = format!("goldens/{case}/ai-prompts{suffix}.{lang_name}.{format_name}");
                    assert_eq!(prompts_appendix(&entries, PAPER_NAME, lang, format, with_answers, Profile::Plugin), read(&appendix), "{appendix}");
                    compared += 1;
                }
                compared += 1;
            }
        }
    }
    assert_eq!(compared, names("logs").len() * 12);
}

#[test]
fn the_log_is_read_and_counted_as_the_plugin_does() {
    let goldens = core_goldens();
    let today = cases()["now"].as_str().unwrap()[..10].to_string();
    for log in names("logs") {
        let entries = parse_log(&read(&format!("logs/{log}")));
        assert_eq!(json!(entries.len()), goldens["entries"][&log], "entries read from {log}");
        // a torn line and a line without a type are skipped; everything else the plugin reads, this reads
        let stats = serde_json::to_value(compute_stats(&entries, &today, &Party::default())).unwrap();
        // null: the plugin's stats throw on this log (a response that lists no personas); here it is counted
        if !goldens["stats"][&log].is_null() {
            assert_eq!(stats, goldens["stats"][&log], "stats of {log}");
        }
    }
}

#[test]
fn words_and_changed_paragraphs_are_counted_as_the_plugin_does() {
    let goldens = core_goldens();
    for name in names("documents") {
        let text = read(&format!("documents/{name}"));
        assert_eq!(json!(count_words(&text, format_of(&name))), goldens["words"][&name], "words in {name}");
        assert_eq!(serde_json::to_value(paragraphs(&text)).unwrap(), goldens["paragraphs"][&name], "paragraphs of {name}");
        if name.contains(".before.") {
            let after = read(&format!("documents/{}", name.replace(".before.", ".after.")));
            assert_eq!(serde_json::to_value(changed_paragraphs(&text, &after, format_of(&name))).unwrap(), goldens["changed"][&name], "changes since {name}");
        }
    }
}

#[test]
fn categories_readable_text_and_month_files_follow_the_plugin() {
    let (cases, goldens) = (cases(), core_goldens());
    let party = Party::default();
    let categories: Vec<String> = cases["category"].as_array().unwrap().iter().map(|c| category_for(c["command"].as_str(), c["text"].as_str().unwrap(), &party)).collect();
    assert_eq!(json!(categories), goldens["category"]);
    let read_as: Vec<String> = cases["readable"].as_array().unwrap().iter().map(|t| readable(t.as_str().unwrap())).collect();
    assert_eq!(json!(read_as), goldens["readable"]);
    let months: Vec<String> = cases["monthFile"].as_array().unwrap().iter().map(|t| month_file(t.as_str().unwrap())).collect();
    assert_eq!(json!(months), goldens["monthFile"]);
}

#[test]
fn the_guard_decides_about_a_paper_folder_as_the_plugin_does() {
    let (cases, goldens) = (cases(), core_goldens());
    let layout = Layout::plugin(cases["guard"]["root"].as_str().unwrap());
    let decided: Vec<Value> = cases["guard"]["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .map(|path| json!({ "path": path, "authorFile": author_file_of(path, &layout), "isLog": is_log_path(path, &layout), "liesIn": lies_in(path, &layout.root) }))
        .collect();
    assert_eq!(json!(decided), goldens["guard"]);

    let log_files: Vec<Value> = cases["logFiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let (before, after) = (snapshot(&c["before"]), snapshot(&c["after"]));
            json!({ "rewritten": rewritten_log_files(&before, &after), "stray": stray_log_files(&before, &after, c["month"].as_str().unwrap()) })
        })
        .collect();
    assert_eq!(json!(log_files), goldens["logFiles"]);
    let changed: Vec<Vec<String>> = cases["changedFiles"].as_array().unwrap().iter().map(|c| changed_files(&snapshot(&c["before"]), &snapshot(&c["after"]))).collect();
    assert_eq!(json!(changed), goldens["changedFiles"]);
}
