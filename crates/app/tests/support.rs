//! When the editor asks for support, counted over real starts on a real file.

use cowriting_app::support::{answered, started, Asking};

fn starts(file: &std::path::Path, count: u32) -> Vec<bool> {
    (0..count).map(|_| started(file).is_due).collect()
}

#[test]
fn the_editor_asks_every_tenth_start_waits_twenty_after_a_gift_and_asks_for_a_star_once() {
    let folder = std::env::temp_dir().join(format!("cowriting-support-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let file = folder.join("state.json");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(&file, r#"{ "lastDocument": "/work/thesis.md" }"#).unwrap();

    // nine quiet starts, then it asks, and goes on asking until the author answers
    assert_eq!(starts(&file, 9), vec![false; 9]);
    assert_eq!(started(&file), Asking { is_due: true, with_star: true });
    assert!(started(&file).is_due);

    // "not now": ten starts of quiet
    answered(&file, "later");
    assert_eq!(starts(&file, 9), vec![false; 9]);
    assert!(started(&file).is_due);

    // a coffee: twenty, and a "not now" afterwards does not shorten that
    answered(&file, "coffee");
    answered(&file, "later");
    assert_eq!(starts(&file, 19), vec![false; 19]);
    assert!(started(&file).is_due);

    // a star is not asked for again; the rest is, after ten
    answered(&file, "star");
    assert_eq!(starts(&file, 9), vec![false; 9]);
    assert_eq!(started(&file), Asking { is_due: true, with_star: false });

    // what else the file holds is kept
    assert!(std::fs::read_to_string(&file).unwrap().contains("/work/thesis.md"));
    let _ = std::fs::remove_dir_all(&folder);
}
