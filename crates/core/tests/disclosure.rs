//! The register of a document written in the editor (`Profile::Editor`), where it says something
//! else than the plugin's: the golden tests hold the plugin's wording.

use cowriting_core::disclosure::{disclosure, DocFormat, Lang, Profile};
use cowriting_core::log::parse_log;
use cowriting_core::types::LogEntry;
use std::path::PathBuf;

fn full_log() -> Vec<LogEntry> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl");
    parse_log(&std::fs::read_to_string(path).unwrap())
}

fn register(lang: Lang, format: DocFormat) -> String {
    disclosure(&full_log(), "thesis.md", lang, format, Profile::Editor)
}

#[test]
fn requests_of_one_day_have_a_date_and_those_of_several_a_range() {
    let (english, german) = (register(Lang::En, DocFormat::Md), register(Lang::De, DocFormat::Md));
    assert!(english.contains("| 2 requests, 2026-10-07 |") && english.contains("| 2 requests, 2026-10-01 to 2026-10-07 |"), "{english}");
    assert!(german.contains("| 2 Anfragen, 2026-10-07 |") && german.contains("| 2 Anfragen, 2026-10-01 bis 2026-10-07 |"), "{german}");
    assert!(!english.contains("2026-10-07 to 2026-10-07") && !german.contains("2026-10-07 bis 2026-10-07"));
    assert!(register(Lang::De, DocFormat::Tex).contains("2 Anfragen, 2026-10-07 &"));
}
