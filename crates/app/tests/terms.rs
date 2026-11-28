//! The terms of use: what they have to say, and when the author is asked to agree.

use cowriting_app::terms::{self, Asking};
use cowriting_app::usage;
use serde_json::Value;

#[test]
fn the_terms_exclude_liability_as_far_as_the_law_allows_and_say_where_it_does_not() {
    let (english, german) = (terms::text("en"), terms::text("de"));
    assert_eq!(terms::text("fr"), english, "another language reads the English terms");
    // what is excluded is named: damage of any kind, and lost data in particular
    assert!(english.contains("not liable for damage of any kind") && english.contains("lost, damaged or overwritten documents"));
    assert!(german.contains("haftet nicht für Schäden gleich welcher Art") && german.contains("verlorene,\nbeschädigte oder überschriebene Dokumente"));
    // an exclusion without these limits would not hold at all
    for part in ["intentionally\nor by gross negligence", "life, body or health", "Product Liability Act"] {
        assert!(english.contains(part), "{part}");
    }
    for part in ["vorsätzlich\noder grob fahrlässig", "Leben, Körper oder Gesundheit", "Produkthaftungsgesetz"] {
        assert!(german.contains(part), "{part}");
    }
    // both say that usage figures need a separate yes
    assert!(english.contains("only if you agree to them\nseparately") && german.contains("nur gesendet, wenn Sie dem gesondert zustimmen"));
}

#[test]
fn the_terms_name_the_version_the_editor_asks_for() {
    let (year, month, day) = (&terms::VERSION[..4], &terms::VERSION[5..7], &terms::VERSION[8..]);
    assert!(terms::text("en").contains(&format!("Version of {}", terms::VERSION)));
    assert!(terms::text("de").contains(&format!("Fassung vom {day}.{month}.{year}")));
}

#[test]
fn the_statement_says_installations_are_told_apart_by_their_number_and_calls_that_pseudonymous() {
    let (english, german) = (usage::statement("en"), usage::statement("de"));
    assert!(english.contains("does not store it and writes it into no log") && german.contains("speichert sie nicht und schreibt sie in kein Protokoll"));
    assert!(english.contains("pseudonymous, not anonymous") && german.contains("pseudonym, nicht anonym"));
}

#[test]
fn nobody_is_asked_to_agree_to_a_draft() {
    // as the editor is built now: the provider's name and address are still to fill in
    assert!(!usage::is_complete(terms::text("en")) && !usage::is_complete(terms::text("de")));
    assert!(!terms::is_in_force());
    let nowhere = std::env::temp_dir().join("cowriting-terms-no-such-state.json");
    assert_eq!(terms::asking(&nowhere), Asking { is_due: false, is_change: false, version: terms::VERSION });
}

#[test]
fn the_author_is_asked_at_the_first_start_and_again_when_the_texts_changed() {
    let folder = std::env::temp_dir().join(format!("cowriting-terms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let state = folder.join("state.json");
    std::fs::write(&state, r#"{ "lastDocument": "/work/thesis.md" }"#).unwrap();

    assert_eq!(terms::asking_when(&state, true), Asking { is_due: true, is_change: false, version: terms::VERSION }, "the first start");
    assert!(!terms::asking_when(&state, false).is_due, "not while the texts are drafts");
    terms::agreed(&state, "2026-12-01").unwrap();
    assert!(!terms::asking_when(&state, true).is_due, "agreed: not asked again");
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    assert_eq!(kept["terms"], serde_json::json!({ "agreed": terms::VERSION, "on": "2026-12-01" }));
    assert_eq!(kept["lastDocument"], "/work/thesis.md", "what else is remembered stays");

    // an update brought newer texts: the agreement on file is to an older version
    std::fs::write(&state, r#"{ "terms": { "agreed": "2026-01-01", "on": "2026-01-02" } }"#).unwrap();
    assert_eq!(terms::asking_when(&state, true), Asking { is_due: true, is_change: true, version: terms::VERSION });
    let _ = std::fs::remove_dir_all(&folder);
}
