//! The settings file on a real folder: what the author set survives, whatever else is saved.

use cowriting_app::preferences::{in_export_folder, is_tour_due, last_document, remember_document, tour_seen, Preferences, SettingsFile};
use cowriting_app::BackendSettings;
use cowriting_core::watch::Pace;
use serde_json::{json, Value};
use std::path::PathBuf;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-preferences-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Folder(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn on_disk(file: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

#[test]
fn preferences_and_the_ai_settings_share_a_file_and_neither_save_drops_the_other() {
    let folder = Folder::new("file");
    let file = folder.0.join("config").join("settings.json");

    // nothing there yet: the standard, and the AI was never set up
    let fresh = SettingsFile::read(&file);
    assert!(fresh.backend.is_none());
    assert_eq!(fresh.preferences, Preferences::default());
    assert_eq!(fresh.preferences.pace(), Pace::default(), "the standard pace is the plugin's");

    // preferences saved before the AI is set up do not count as setting it up
    let dark = Preferences { theme: "dark".into(), autosave_seconds: 30, ..Default::default() };
    SettingsFile::save_preferences(&file, dark.clone()).unwrap();
    let read = SettingsFile::read(&file);
    assert!(read.backend.is_none());
    assert_eq!(read.preferences, dark);

    // the AI's settings stay at the top level, where older versions wrote and read them
    let backend = BackendSettings { backend: "openai-compatible".into(), model: "gemma3:4b".into(), base_url: "http://localhost:11434/v1".into(), max_tokens: 2000 };
    SettingsFile::save_backend(&file, &backend).unwrap();
    assert_eq!(on_disk(&file)["model"], "gemma3:4b");
    let read = SettingsFile::read(&file);
    assert_eq!(read.backend, Some(backend.clone()));
    assert_eq!(read.preferences, dark, "saving the AI's settings kept the preferences");

    SettingsFile::save_preferences(&file, Preferences { theme: "light".into(), ..dark }).unwrap();
    assert_eq!(SettingsFile::read(&file).backend, Some(backend), "saving preferences kept the AI's settings");
}

#[test]
fn a_file_from_an_older_version_or_one_edited_by_hand_still_reads() {
    let folder = Folder::new("old");
    std::fs::create_dir_all(&folder.0).unwrap();
    let file = folder.0.join("settings.json");

    // as written before there were preferences
    std::fs::write(&file, r#"{ "backend": "claude-code", "model": "haiku", "baseUrl": "" }"#).unwrap();
    let read = SettingsFile::read(&file);
    assert_eq!(read.backend.map(|b| (b.backend, b.model, b.max_tokens)), Some(("claude-code".into(), "haiku".into(), 0)));
    assert_eq!(read.preferences, Preferences::default());

    // values out of range, unknown words and unknown fields: limits apply, the rest is kept on disk
    std::fs::write(
        &file,
        json!({
            "backend": "anthropic", "model": "m", "somethingNewer": true,
            "preferences": { "theme": "neon", "textSize": 400, "uiSize": "huge", "livePauseSeconds": 0, "liveGapSeconds": 1, "livePerHour": 100000, "autosaveSeconds": 1,
                             "textFont": "other", "textFontName": "Iowan Old Style\"; } body { display: none" }
        })
        .to_string(),
    )
    .unwrap();
    let read = SettingsFile::read(&file).preferences;
    assert_eq!((read.theme.as_str(), read.text_size, read.autosave_seconds), ("system", 28, 2));
    assert_eq!(read.ui_size, "standard");
    assert_eq!((read.live_pause_seconds, read.live_gap_seconds, read.live_per_hour), (5, 15, 120));
    assert_eq!(read.text_font_name, "Iowan Old Style  body  display none", "a font's name cannot carry style rules");
    SettingsFile::save_preferences(&file, read).unwrap();
    assert_eq!(on_disk(&file)["somethingNewer"], true);

    // languages: an unknown one is the standard, and only a named one is passed on to the prompts
    use cowriting_core::disclosure::Lang;
    let said = Preferences { interface_language: "tlh".into(), writing_language: "en".into(), ai_language: "de".into(), ..Default::default() }.within_limits();
    assert_eq!(said.interface_language, "system");
    assert_eq!((said.languages().manuscript, said.languages().answers), (Some(Lang::En), Some(Lang::De)));
    assert_eq!(Preferences::default().languages(), Default::default());

    // 'another font' without a name is the standard font
    let nameless = Preferences { text_font: "other".into(), ..Default::default() }.within_limits();
    assert_eq!(nameless.text_font, "literata");

    // not JSON at all: the standard, and saving makes it a proper file again
    std::fs::write(&file, "not json").unwrap();
    assert_eq!(SettingsFile::read(&file).preferences, Preferences::default());
    SettingsFile::save_preferences(&file, Preferences::default()).unwrap();
    assert_eq!(on_disk(&file)["preferences"]["theme"], "system");
}

#[test]
fn the_last_document_is_remembered_while_it_exists_and_exports_go_to_a_folder_that_exists() {
    let folder = Folder::new("state");
    std::fs::create_dir_all(&folder.0).unwrap();
    let state = folder.0.join("state.json");
    let document = folder.0.join("thesis.md");
    assert_eq!(last_document(&state), None);
    remember_document(&state, &document.to_string_lossy());
    assert_eq!(last_document(&state), None, "a document that is gone is not offered");
    std::fs::write(&document, "text").unwrap();
    assert_eq!(last_document(&state), Some(document.to_string_lossy().into_owned()));

    // the tour shows itself until it was seen once, and seeing it keeps what else is remembered
    assert!(is_tour_due(&state));
    tour_seen(&state);
    assert!(!is_tour_due(&state));
    assert_eq!(last_document(&state), Some(document.to_string_lossy().into_owned()));
    remember_document(&state, &document.to_string_lossy());
    assert!(!is_tour_due(&state), "opening a document does not bring the tour back");

    let beside = document.with_extension("pdf").to_string_lossy().into_owned();
    let exports = folder.0.join("exports");
    assert_eq!(in_export_folder(&beside, ""), beside);
    assert_eq!(in_export_folder(&beside, &exports.to_string_lossy()), beside, "a folder that is not there is not used");
    std::fs::create_dir_all(&exports).unwrap();
    assert_eq!(in_export_folder(&beside, &exports.to_string_lossy()), exports.join("thesis.pdf").to_string_lossy());
}
