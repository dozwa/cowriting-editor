//! What Crossref and Zotero answer, read into the editor's own records. The answers are fixed
//! here; `fixtures/lookup/crossref-work.json` is a real one.

use cowriting_backends::lookup::{parse_work, parse_zotero};
use cowriting_core::sources::{check_references, load, Entry, Record, References, Status};
use serde_json::{json, Value};

#[test]
fn a_crossref_work_gives_its_doi_first_title_and_year() {
    let recorded: Value = serde_json::from_str(include_str!("../../../fixtures/lookup/crossref-work.json")).unwrap();
    let record = parse_work(&recorded["message"]);
    assert_eq!((record.doi.as_str(), record.title.as_str(), record.year.as_str()), ("10.1038/nature14539", "Deep learning", "2015"));
    // a record without a title or a date is still a record
    let bare = parse_work(&json!({ "DOI": "10.1000/x", "title": [], "issued": { "date-parts": [[null]] } }));
    assert_eq!((bare.doi.as_str(), bare.title.as_str(), bare.year.as_str()), ("10.1000/x", "", ""));
}

#[test]
fn zotero_items_become_hits_with_the_citekey_where_zotero_has_one() {
    let items = json!([
        { "key": "ABCD1234", "data": { "title": "Walking breaks", "date": "2020-03-01", "DOI": "10.1000/walk.2020", "citationKey": "smith2020", "creators": [{ "lastName": "Smith" }, { "name": "World Health Organization" }, { "lastName": "Lee" }, { "lastName": "Berg" }] } },
        { "key": "EFGH5678", "data": { "title": "Sitting time", "date": "", "extra": "PMID: 1\nCitation Key: lee2021\nmore", "creators": [] } },
        { "key": "IJKL9012", "data": { "title": "No key at all" } }
    ]);
    let hits = parse_zotero(&items);
    assert_eq!(hits.iter().map(|h| h.citekey.as_deref()).collect::<Vec<_>>(), [Some("smith2020"), Some("lee2021"), None]);
    assert_eq!((hits[0].authors.as_str(), hits[0].year.as_str(), hits[0].doi.as_str()), ("Smith, World Health Organization, Lee et al.", "2020", "10.1000/walk.2020"));
    assert_eq!(parse_zotero(&json!({ "error": "no" })), []);
}

/// Crossref, scripted: a DOI's record, or none; candidates for a title.
struct Scripted;

#[async_trait::async_trait]
impl References for Scripted {
    async fn work(&self, doi: &str) -> Result<Option<Record>, String> {
        match doi {
            "10.1000/Walk.2020" => Ok(Some(Record { doi: doi.into(), title: "Walking breaks and attention in office workers".into(), year: "2020".into() })),
            "10.1000/sit.2021" => Ok(Some(Record { doi: doi.into(), title: "Standing desks in schools".into(), year: "2021".into() })),
            "10.1000/down" => Err("Crossref could not be reached".into()),
            _ => Ok(None),
        }
    }

    async fn candidates(&self, query: &str) -> Result<Vec<Record>, String> {
        Ok(match query {
            "An entry with nothing else" => vec![
                Record { doi: "10.1000/other".into(), title: "Something else".into(), year: "2001".into() },
                Record { doi: "10.1000/entry".into(), title: "An entry with nothing else.".into(), year: "2001".into() },
            ],
            _ => vec![Record { doi: "10.1000/groesse".into(), title: "Sample sizes".into(), year: "2019".into() }],
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn every_reference_gets_a_verdict_and_a_suggestion_is_never_a_verification() {
    let mut entries = load("refs.bib", include_str!("../../../fixtures/literature/refs.bib")).unwrap();
    entries.push(Entry { key: "gone".into(), title: "A retracted thing".into(), doi: "10.1000/gone".into(), ..Default::default() });
    entries.push(Entry { key: "down".into(), title: "Network trouble".into(), doi: "10.1000/down".into(), ..Default::default() });
    let rows = check_references(&entries, &Scripted).await;
    let verdicts: Vec<(&str, Status)> = rows.iter().map(|row| (row.key.as_str(), row.status)).collect();
    assert_eq!(
        verdicts,
        [("smith2020", Status::Ok), ("lee2021", Status::Mismatch), ("müller_2019", Status::NoDoi), ("nodoi", Status::NoDoi), ("gone", Status::NotFound), ("down", Status::Error)]
    );
    assert_eq!(rows[1].found.as_ref().map(|found| found.title.as_str()), Some("Standing desks in schools"), "a mismatch shows what the DOI really is");
    assert_eq!(rows[0].found, None);
    // without a DOI: the first candidate with a close title is suggested, and the entry stays unverified
    assert_eq!((rows[2].suggestion.as_deref(), rows[3].suggestion.as_deref()), (None, Some("10.1000/entry")));
    assert_eq!(rows[5].detail.as_deref(), Some("Crossref could not be reached"));
    assert_eq!(json!(rows[1])["status"], "mismatch");
}

#[test]
fn better_bibtex_gives_every_hit_its_citekey_and_names_the_keys_it_lacks() {
    use cowriting_backends::lookup::{keys_not_found, parse_better_bibtex};
    let items = json!([
        { "id": "http://zotero.org/users/1/items/ABCD", "citekey": "smith2020", "title": "Walking breaks", "author": [{ "family": "Smith", "given": "Jane" }, { "literal": "World Health Organization" }], "issued": { "date-parts": [[2020, 3]] }, "DOI": "10.1000/walk.2020" },
        { "citation-key": "lee2021", "title": "Sitting time" },
        { "title": "Third" }
    ]);
    let hits = parse_better_bibtex(&items, 2);
    assert_eq!(hits.len(), 2, "no more than asked for");
    assert_eq!(
        (hits[0].citekey.as_deref(), hits[0].authors.as_str(), hits[0].year.as_str(), hits[0].doi.as_str()),
        (Some("smith2020"), "Smith, World Health Organization", "2020", "10.1000/walk.2020")
    );
    assert_eq!((hits[1].citekey.as_deref(), hits[1].year.as_str()), (Some("lee2021"), ""));
    assert_eq!(keys_not_found("not found: ghost1999, lecun2015"), ["ghost1999", "lecun2015"]);
    assert_eq!(keys_not_found("Translator not found"), Vec::<String>::new());
}
