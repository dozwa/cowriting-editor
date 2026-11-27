//! When the editor asks for a coffee, a word of feedback or a star: every tenth start, and less
//! often once the author gave something. Kept beside the settings (`state.json`), and nowhere else:
//! the editor never learns whether anyone really paid or starred, only that they followed the link.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

/// Starts between two asks, and after feedback or a coffee.
pub const EVERY: u32 = 10;
pub const AFTER_GIVING: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Support {
    /// How often the editor was started.
    pub starts: u32,
    /// The start at which it asks next.
    pub ask_at: u32,
    /// The author followed the link to the star: that is not asked for again.
    pub starred: bool,
}

impl Default for Support {
    fn default() -> Self {
        Support { starts: 0, ask_at: EVERY, starred: false }
    }
}

/// What the window is told at start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asking {
    /// This is a start at which the editor asks.
    pub is_due: bool,
    /// The star is still among what it asks for.
    pub with_star: bool,
}

fn read(file: &Path) -> (Value, Support) {
    let json = crate::preferences::read_json(file, &cowriting_core::stored::STATE);
    let support = serde_json::from_value(json["support"].clone()).unwrap_or_default();
    (json, support)
}

fn write(file: &Path, mut json: Value, support: Support) {
    json["support"] = json!(support);
    let _ = crate::preferences::write_json(file, &cowriting_core::stored::STATE, &json);
}

/// Counts one start of the editor, and says whether it asks this time.
pub fn started(file: &Path) -> Asking {
    let (json, mut support) = read(file);
    support.starts = support.starts.saturating_add(1);
    write(file, json, support);
    Asking { is_due: support.starts >= support.ask_at, with_star: !support.starred }
}

/// The author answered: 'feedback' or 'coffee' (asked again after twenty starts), 'star' (never
/// asked for again; the rest after ten), or anything else for "not now" (again after ten).
pub fn answered(file: &Path, what: &str) {
    let (json, mut support) = read(file);
    let wait = if matches!(what, "feedback" | "coffee") { AFTER_GIVING } else { EVERY };
    // a longer wait that is already running is not cut short by a later "not now"
    support.ask_at = support.ask_at.max(support.starts.saturating_add(wait));
    if what == "star" {
        support.starred = true;
    }
    write(file, json, support);
}
