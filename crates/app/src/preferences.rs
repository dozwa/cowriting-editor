//! What the author set in the editor's settings besides the AI, and the one file both are kept in.
//!
//! The file is `settings.json` in the editor's configuration folder: the AI's settings at the top
//! level, as they always were, and the rest under `preferences`. Saving one never drops the other,
//! nor anything else a later version may have written there.

use crate::BackendSettings;
use cowriting_core::disclosure::Lang;
use cowriting_core::host::DiskHost;
use cowriting_core::stored;
use cowriting_core::turn::Languages;
use cowriting_core::watch::Pace;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// The author's preferences. Every field has a standard, so a file from an older version reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    /// 'system', 'light' or 'dark'.
    pub theme: String,
    /// The text's typeface: 'literata', 'sans', 'system-serif', 'mono' or 'other' (then `text_font_name`).
    pub text_font: String,
    /// A typeface installed on this computer, by name.
    pub text_font_name: String,
    /// The text's size in pixels.
    pub text_size: u32,
    /// How long a line of text may be: 'narrow', 'standard' or 'wide'.
    pub line_width: String,
    /// The typeface of buttons and panels: 'hanken' or 'system'.
    pub ui_font: String,
    /// How large buttons, panels and dialogs are drawn: 'small', 'standard', 'large' or 'larger'.
    pub ui_size: String,
    pub spellcheck: bool,
    /// Distraction-free: the explanations and hints around the controls are hidden.
    pub calm: bool,
    /// Save by itself once the author has paused this long; 0 for never.
    pub autosave_seconds: u32,
    /// Where "Open" and "New" start; empty for wherever the system's dialog was last.
    pub documents_folder: String,
    /// Where exports to PDF and Word go; empty for beside the document.
    pub export_folder: String,
    /// Open the last document again when the editor starts.
    pub reopen_last: bool,
    /// The language of the editor's own buttons and messages: 'system' (as the computer is set), 'en' or 'de'.
    pub interface_language: String,
    /// The language the author writes in: 'document' (as the editor found it when the log began), 'en' or 'de'.
    pub writing_language: String,
    /// The language the AI answers in: 'auto' (the author's and the manuscript's, as the rules say), 'en' or 'de'.
    pub ai_language: String,
    /// The disclosure's language: 'document' (the writing language), 'en' or 'de'.
    pub disclosure_language: String,
    /// Live review: the pause before a review, in seconds.
    pub live_pause_seconds: u32,
    /// Live review: at least this many seconds between two reviews.
    pub live_gap_seconds: u32,
    /// Live review: at most this many reviews in an hour.
    pub live_per_hour: u32,
    /// Keyboard shortcuts the author changed: what is done → the keys ('Mod-b'); empty for none.
    pub shortcuts: BTreeMap<String, String>,
    /// Keep the editor's own technical log (what was used, what failed; no text, no names) beside the settings.
    pub diagnostics: bool,
    /// The author agreed: a sign of use goes to the editor's maker at each start.
    pub usage_ping: bool,
    /// The author agreed, separately: a session's figures go to the editor's maker when the editor closes.
    pub usage_figures: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        let pace = Pace::default();
        Preferences {
            theme: "system".into(),
            text_font: "literata".into(),
            text_font_name: String::new(),
            text_size: 18,
            line_width: "standard".into(),
            ui_font: "hanken".into(),
            ui_size: "standard".into(),
            spellcheck: true,
            calm: false,
            autosave_seconds: 0,
            documents_folder: String::new(),
            export_folder: String::new(),
            reopen_last: false,
            interface_language: "system".into(),
            writing_language: "document".into(),
            ai_language: "auto".into(),
            disclosure_language: "document".into(),
            live_pause_seconds: (pace.pause_ms / 1000) as u32,
            live_gap_seconds: (pace.gap_ms / 1000) as u32,
            live_per_hour: pace.per_hour as u32,
            shortcuts: BTreeMap::new(),
            diagnostics: true,
            usage_ping: false,
            usage_figures: false,
        }
    }
}

/// One of the allowed words, or the standard.
fn one_of(value: &str, allowed: &[&str], standard: &str) -> String {
    if allowed.contains(&value) { value } else { standard }.to_string()
}

impl Preferences {
    /// The same preferences with every value inside what the editor can do: a file edited by hand,
    /// or a window that sends nonsense, cannot set a review every second or a text one pixel high.
    pub fn within_limits(mut self) -> Preferences {
        let standard = Preferences::default();
        self.theme = one_of(&self.theme, &["system", "light", "dark"], &standard.theme);
        self.text_font = one_of(&self.text_font, &["literata", "sans", "system-serif", "mono", "other"], &standard.text_font);
        // a name is a name: nothing that could end the style rule it is put into
        self.text_font_name = self.text_font_name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).take(80).collect::<String>().trim().to_string();
        if self.text_font == "other" && self.text_font_name.is_empty() {
            self.text_font = standard.text_font;
        }
        self.text_size = self.text_size.clamp(13, 28);
        self.line_width = one_of(&self.line_width, &["narrow", "standard", "wide"], &standard.line_width);
        self.ui_font = one_of(&self.ui_font, &["hanken", "system"], &standard.ui_font);
        self.ui_size = one_of(&self.ui_size, &["small", "standard", "large", "larger"], &standard.ui_size);
        self.autosave_seconds = if self.autosave_seconds == 0 { 0 } else { self.autosave_seconds.clamp(2, 600) };
        self.documents_folder = self.documents_folder.trim().to_string();
        self.export_folder = self.export_folder.trim().to_string();
        self.interface_language = one_of(&self.interface_language, &["system", "en", "de"], &standard.interface_language);
        self.writing_language = one_of(&self.writing_language, &["document", "en", "de"], &standard.writing_language);
        self.ai_language = one_of(&self.ai_language, &["auto", "en", "de"], &standard.ai_language);
        self.disclosure_language = one_of(&self.disclosure_language, &["document", "en", "de"], &standard.disclosure_language);
        self.live_pause_seconds = self.live_pause_seconds.clamp(5, 600);
        self.live_gap_seconds = self.live_gap_seconds.clamp(15, 3600);
        self.live_per_hour = self.live_per_hour.clamp(1, 120);
        self.shortcuts.retain(|action, keys| action.len() <= 40 && keys.len() <= 40);
        self
    }

    /// The languages for an open document: the one it is written in, if the author said so, and the one the AI answers in, if they chose one.
    pub fn languages(&self) -> Languages {
        let chosen = |value: &str| match value {
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            _ => None,
        };
        Languages { manuscript: chosen(&self.writing_language), answers: chosen(&self.ai_language) }
    }

    /// How often live review asks, as set here.
    pub fn pace(&self) -> Pace {
        Pace { pause_ms: u64::from(self.live_pause_seconds) * 1000, gap_ms: u64::from(self.live_gap_seconds) * 1000, per_hour: self.live_per_hour as usize }
    }
}

/// The settings file as it is read: the AI's settings if any were ever saved, and the preferences.
pub struct SettingsFile {
    /// None until the author has set up the AI.
    pub backend: Option<BackendSettings>,
    pub preferences: Preferences,
}

/// One of the editor's own files (`settings.json`, `state.json`) as JSON; empty when there is none.
pub(crate) fn read_json(file: &Path, kind: &stored::Kind) -> Value {
    Value::Object(stored::read(&DiskHost, &file.to_string_lossy(), kind).unwrap_or_default())
}

/// Writes one of the editor's own files. One that a newer editor wrote is left as it is, and one
/// that cannot be read is set aside first (see `cowriting_core::stored`).
pub(crate) fn write_json(file: &Path, kind: &stored::Kind, value: &Value) -> Result<(), String> {
    stored::write(&DiskHost, &file.to_string_lossy(), kind, value).map_err(|e| format!("The settings could not be saved: {e}"))
}

/// Looks at the editor's own files before they are used: one that cannot be read is set aside,
/// one from a newer editor is named. Neither is written over.
pub fn look_over(settings: &Path, state: &Path) -> Vec<stored::Notice> {
    type Any = serde_json::Map<String, Value>;
    let look = |file: &Path, kind| stored::look_at::<Any>(&DiskHost, &file.to_string_lossy(), kind);
    [look(settings, &stored::SETTINGS), look(state, &stored::STATE)].into_iter().flatten().collect()
}

impl SettingsFile {
    /// A file that is missing or unreadable gives the standard.
    pub fn read(file: &Path) -> SettingsFile {
        let json = read_json(file, &stored::SETTINGS);
        let preferences = serde_json::from_value::<Preferences>(json["preferences"].clone()).unwrap_or_default().within_limits();
        SettingsFile { backend: serde_json::from_value(json).ok(), preferences }
    }

    pub fn save_backend(file: &Path, backend: &BackendSettings) -> Result<(), String> {
        let mut json = read_json(file, &stored::SETTINGS);
        for (key, value) in json!(backend).as_object().expect("a struct") {
            json[key] = value.clone();
        }
        write_json(file, &stored::SETTINGS, &json)
    }

    /// Saves the preferences, within their limits, and returns them as saved.
    pub fn save_preferences(file: &Path, preferences: Preferences) -> Result<Preferences, String> {
        let preferences = preferences.within_limits();
        let mut json = read_json(file, &stored::SETTINGS);
        // what a newer version keeps among the preferences and this one does not know stays
        let mut kept = json["preferences"].as_object().cloned().unwrap_or_default();
        kept.extend(json!(preferences).as_object().expect("a struct").clone());
        json["preferences"] = Value::Object(kept);
        write_json(file, &stored::SETTINGS, &json)?;
        Ok(preferences)
    }
}

/// The document that was open last, kept beside the settings (`state.json`): nothing the author set.
pub fn last_document(file: &Path) -> Option<String> {
    read_json(file, &stored::STATE)["lastDocument"].as_str().filter(|path| Path::new(path).is_file()).map(String::from)
}

pub fn remember_document(file: &Path, document: &str) {
    let mut json = read_json(file, &stored::STATE);
    json["lastDocument"] = json!(document);
    let _ = write_json(file, &stored::STATE, &json);
}

/// The tour of the editor is shown by itself until the author went through it or closed it once.
pub fn is_tour_due(file: &Path) -> bool {
    read_json(file, &stored::STATE)["tourSeen"] != json!(true)
}

pub fn tour_seen(file: &Path) {
    let mut json = read_json(file, &stored::STATE);
    json["tourSeen"] = json!(true);
    let _ = write_json(file, &stored::STATE, &json);
}

/// Where an export goes when the author chose a folder for exports: the same file name, there.
/// A folder that is not there (an unplugged drive) leaves the export beside the document.
pub fn in_export_folder(target: &str, folder: &str) -> String {
    match Path::new(target).file_name() {
        Some(name) if !folder.is_empty() && Path::new(folder).is_dir() => Path::new(folder).join(name).to_string_lossy().into_owned(),
        _ => target.to_string(),
    }
}
