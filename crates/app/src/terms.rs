//! The terms of use, and the author's agreement to them: asked for at the first start, and again
//! when a newer version of the editor comes with newer terms or a newer privacy statement.
//!
//! Agreeing to the terms is not consent to usage figures. That is asked separately and stays a
//! free choice (`crate::usage`): the editor works the same with a no.
//!
//! While the terms or the privacy statement still have a part to fill in (`[[…]]`) they are
//! drafts, and nobody is asked to agree to a draft.

use serde::Serialize;
use serde_json::json;
use std::path::Path;

const TERMS_EN: &str = include_str!("../../../docs/TERMS.md");
const TERMS_DE: &str = include_str!("../../../docs/NUTZUNGSBEDINGUNGEN.md");

/// The version of the terms and the privacy statement the author is asked to agree to: the day
/// they were last changed. Change it whenever either text changes in what it says, and every
/// author is asked again at the next start. The terms name this day at their top.
pub const VERSION: &str = "2026-11-28";

/// The terms of use, in German or in English.
pub fn text(language: &str) -> &'static str {
    if language == "de" {
        TERMS_DE
    } else {
        TERMS_EN
    }
}

/// The terms and the privacy statement are complete in both languages: only then is anyone asked.
pub fn is_in_force() -> bool {
    [TERMS_EN, TERMS_DE, crate::usage::statement("en"), crate::usage::statement("de")].into_iter().all(crate::usage::is_complete)
}

/// What the window needs to ask.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asking {
    /// The author has to agree before the editor goes on.
    pub is_due: bool,
    /// They agreed to an earlier version: the texts changed since.
    pub is_change: bool,
    pub version: &'static str,
}

fn agreed_to(state: &Path) -> Option<String> {
    crate::preferences::read_json(state, &cowriting_core::stored::STATE)["terms"]["agreed"].as_str().map(String::from)
}

/// Whether the author has to agree now, given that the texts are in force or not.
pub fn asking_when(state: &Path, in_force: bool) -> Asking {
    let agreed = agreed_to(state);
    Asking { is_due: in_force && agreed.as_deref() != Some(VERSION), is_change: agreed.is_some(), version: VERSION }
}

pub fn asking(state: &Path) -> Asking {
    asking_when(state, is_in_force())
}

/// The author agreed to this version, on this day (`YYYY-MM-DD`). Kept beside the settings, on this computer only.
pub fn agreed(state: &Path, day: &str) -> Result<(), String> {
    let mut json = crate::preferences::read_json(state, &cowriting_core::stored::STATE);
    json["terms"] = json!({ "agreed": VERSION, "on": day });
    crate::preferences::write_json(state, &cowriting_core::stored::STATE, &json)
}
