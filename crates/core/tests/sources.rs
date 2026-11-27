//! The bibliography parsers and the title comparison against the plugin's own output
//! (`fixtures/goldens/sources.json`, written by `scripts/make-goldens.py`).

use cowriting_core::sources::{clean_doi, family, format_name, judge, load, lookup_query, norm, pick, ratio, search, similar, Entry, Record, Status};
use serde_json::{json, Value};

fn read(relative: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load_json(relative: &str) -> Value {
    serde_json::from_str(&read(relative)).unwrap()
}

fn all_entries(goldens: &Value) -> Vec<Entry> {
    goldens["files"].as_array().unwrap().iter().flat_map(|file| load(file.as_str().unwrap(), &read(&format!("literature/{}", file.as_str().unwrap()))).unwrap()).collect()
}

#[test]
fn every_format_is_read_as_the_plugin_reads_it() {
    let goldens = load_json("goldens/sources.json");
    for file in goldens["files"].as_array().unwrap() {
        let file = file.as_str().unwrap();
        assert_eq!(json!(format_name(file)), goldens["formats"][file]);
        let ours = load(file, &read(&format!("literature/{file}"))).unwrap();
        let theirs = goldens["entries"][file].as_array().unwrap();
        assert_eq!(ours.len(), theirs.len(), "{file}");
        for (ours, theirs) in ours.iter().zip(theirs) {
            assert_eq!(&json!(ours), theirs, "{file}: {}", theirs["key"]);
        }
    }
    assert_eq!(load("notes.txt", "x"), None);
    assert_eq!(load("broken.json", "{not json"), Some(Vec::new()));
}

#[test]
fn dois_titles_and_names_are_cleaned_as_the_plugin_cleans_them() {
    let (cases, goldens) = (load_json("sources-cases.json"), load_json("goldens/sources.json"));
    let each = |key: &str, with: &dyn Fn(&str) -> String| -> Value { cases[key].as_array().unwrap().iter().map(|v| with(v.as_str().unwrap())).collect() };
    assert_eq!(each("dois", &clean_doi), goldens["dois"]);
    assert_eq!(each("titles", &norm), goldens["norms"]);
    assert_eq!(each("families", &family), goldens["families"]);
}

#[test]
fn titles_are_compared_as_pythons_difflib_compares_them() {
    let (cases, goldens) = (load_json("sources-cases.json"), load_json("goldens/sources.json"));
    for (i, pair) in cases["pairs"].as_array().unwrap().iter().enumerate() {
        let (a, b) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
        let theirs = goldens["ratios"][i].as_f64().unwrap();
        assert!((ratio(&norm(a), &norm(b)) - theirs).abs() < 1e-12, "pair {i}: {} against {theirs}", ratio(&norm(a), &norm(b)));
        assert_eq!(json!(similar(a, b)), goldens["similar"][i], "pair {i}");
    }
}

#[test]
fn the_bibliography_is_searched_as_the_plugin_searches_it() {
    let (cases, goldens) = (load_json("sources-cases.json"), load_json("goldens/sources.json"));
    let entries = all_entries(&goldens);
    for (i, query) in cases["searches"].as_array().unwrap().iter().enumerate() {
        let ours: Vec<Value> = search(&entries, query.as_str().unwrap(), 15)
            .iter()
            .map(|hit| json!({ "citekey": hit.citekey, "title": hit.title, "authors": hit.authors, "year": hit.year, "doi": hit.doi, "file": hit.file }))
            .collect();
        assert_eq!(json!(ours), goldens["searches"][i], "search {query}");
    }
}

#[test]
fn a_reference_is_judged_by_what_crossref_holds_for_its_doi() {
    let entry = Entry {
        title: "Walking breaks and {attention} in office workers".into(),
        authors: vec!["Smith".into()],
        year: "2020".into(),
        doi: "10.1000/walk.2020".into(),
        ..Default::default()
    };
    let record = |title: &str, year: &str| Record { doi: "10.1000/walk.2020".into(), title: title.into(), year: year.into() };
    assert_eq!(judge(&entry, Some(&record("Walking Breaks and Attention in Office Workers.", "2020"))), Status::Ok);
    assert_eq!(judge(&entry, Some(&record("Walking breaks and attention in office workers", ""))), Status::Ok, "a record without a year does not disagree");
    assert_eq!(judge(&entry, Some(&record("Walking breaks and attention in office workers", "2019"))), Status::Mismatch);
    assert_eq!(judge(&entry, Some(&record("Sitting time, measured: a field study", "2020"))), Status::Mismatch);
    assert_eq!(judge(&entry, None), Status::NotFound);

    // without a DOI Crossref is asked by title, author and year; a short title tells nothing apart
    assert_eq!(lookup_query(&entry).as_deref(), Some("Walking breaks and {attention} in office workers Smith 2020"));
    assert_eq!(lookup_query(&Entry { title: "On {walking}".into(), ..Default::default() }), None);
    let candidates = [
        record("Something else entirely about offices", "2020"),
        record("Walking breaks and attention in office workers", "2021"),
        record("Walking breaks and attention in office-workers", "2020"),
    ];
    assert_eq!(pick(&entry, &candidates), Some(&candidates[2]));
    assert_eq!(pick(&entry, &candidates[..2]), None);
}

// ------------------------------------------------------------ the document's own bibliography

use cowriting_core::host::{Host, MemoryHost};
use cowriting_core::sidecar::Sidecar;
use cowriting_core::sources::{cited_keys, front_matter_files, load_bibliography};
use cowriting_core::tools::{execute, tool_defs, ToolContext};

const DOCUMENT: &str = "---\ntitle: Walking breaks\nbibliography: refs.bib\n---\n\n# Walking breaks\n\nBreaks help [@smith2020; @ghost1999, p. 3], as @lee2021 found.\nWrite to me@example.org. `@not-a-key`\n\n```\n@in_code\n```\n";

#[test]
fn the_front_matter_names_the_bibliography_one_file_or_several() {
    assert_eq!(front_matter_files(DOCUMENT), ["refs.bib"]);
    assert_eq!(front_matter_files("---\nbibliography: [\"a.bib\", 'lit/b.json']\n---\ntext"), ["a.bib", "lit/b.json"]);
    assert_eq!(front_matter_files("---\nbibliography:\n  - a.bib\n  - \"b.ris\"\ntitle: x\n---\n"), ["a.bib", "b.ris"]);
    assert_eq!(front_matter_files("# No front matter\n\nbibliography: refs.bib\n"), Vec::<String>::new());
    assert_eq!(front_matter_files("---\ntitle: x\n---\n"), Vec::<String>::new());
}

#[test]
fn only_citations_in_the_prose_count_as_cited() {
    assert_eq!(cited_keys(DOCUMENT).into_iter().collect::<Vec<_>>(), ["ghost1999", "lee2021", "smith2020"]);
}

#[test]
fn a_key_that_is_only_in_a_comment_is_not_cited_as_it_is_in_no_export() {
    let text =
        "Breaks help [@smith2020]. <!-- see also @hidden2001 -->\n\n<!--\nA paragraph set aside [@aside1999].\n\nAnd its second half, @smith2020.\n-->\n\nAs @lee2021 found.\n";
    assert_eq!(cited_keys(text).into_iter().collect::<Vec<_>>(), ["lee2021", "smith2020"]);
    // a comment that is never closed comments nothing out: pandoc exports that text too
    assert_eq!(cited_keys("An open mark <!-- and @still2020 cited.\n").into_iter().collect::<Vec<_>>(), ["still2020"]);
}

#[test]
fn the_ai_can_search_the_authors_references_and_change_none_of_them() {
    let host = MemoryHost::default();
    let sidecar = Sidecar::for_document("/work/thesis.md");
    let context = ToolContext { host: &host, sidecar: &sidecar, document_text: DOCUMENT, model: "scripted" };
    assert!(tool_defs().iter().any(|tool| tool.name == "search_bibliography"));

    // the front matter names a file that is not there: the tool says so, it does not fail the turn
    let none = execute(&context, "search_bibliography", &json!({ "query": "walking" }));
    assert!(!none.is_error && none.content.contains("No entry"), "{}", none.content);
    let bibliography = load_bibliography(&host, &sidecar, DOCUMENT);
    assert!(bibliography.files[0].error.is_some() && bibliography.files[0].is_from_document);

    host.write("/work/refs.bib", &read("literature/refs.bib")).unwrap();
    let before = host.files();
    let found = execute(&context, "search_bibliography", &json!({ "query": "Walking SMITH" }));
    assert!(!found.is_error);
    assert!(
        found.content.contains("[smith2020] Smith, van der Berg, Organization (2020). Walking breaks and attention in office workers. doi:10.1000/Walk.2020"),
        "{}",
        found.content
    );
    assert!(!found.content.contains("lee2021"));
    assert!(execute(&context, "search_bibliography", &json!({ "query": "homeopathy" })).content.contains("No entry in refs.bib"));
    assert_eq!(host.files(), before, "searching writes nothing");

    // a document without any bibliography: the tool explains where one would come from
    let bare = ToolContext { document_text: "# Nothing\n", ..context };
    assert!(execute(&bare, "search_bibliography", &json!({ "query": "x" })).content.contains("no bibliography file"));
}
