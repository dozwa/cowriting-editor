//! Live review: while the author writes, a model looks at the paragraphs they just changed and
//! the watching experts leave short hints. Ported from the plugin's `plugin/hooks/core.ts`
//! (`WATCH_DEFAULT`, `watchExcerpt`, `watchSystem`, `parseHints`); the pacing constants are
//! those of its `watchTick` in `plugin/hooks/register.tsx`.

use crate::disclosure::Lang;
use crate::personas::{bare_id, Persona};
use crate::text::{slice_utf16, squash};
use crate::words::Paragraph;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// Who watches by default: the perspectives that make sense sentence by sentence.
pub const WATCH_DEFAULT: [&str; 4] = ["grammar-guy", "terminologist", "style-coach", "architect"];
/// A review waits until the author has paused this long.
pub const PAUSE_MS: u64 = 20_000;
/// At least this long between two reviews.
pub const GAP_MS: u64 = 60_000;
/// At most this many reviews in an hour.
pub const PER_HOUR: usize = 20;
/// How many hints are kept to show.
pub const KEEP_HINTS: usize = 12;

static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The changed paragraphs as the live reviewer sees them: file, then numbered lines.
pub fn watch_excerpt(parts: &[(&str, &[Paragraph])], max_chars: usize) -> String {
    let mut out = String::new();
    for (file, changed) in parts {
        out.push_str(&format!("### {file}\n"));
        for p in *changed {
            let numbered = p.text.split('\n').enumerate().map(|(i, line)| format!("{}| {line}", p.start + i)).collect::<Vec<_>>().join("\n");
            if utf16_len(&out) + utf16_len(&numbered) > max_chars {
                return format!("{out}[… more changes not shown]\n");
            }
            out.push_str(&format!("{numbered}\n\n"));
        }
    }
    out
}

/// What the live reviewer is told. `language`: the plugin knows its paper's language; the editor
/// does not, and asks for the manuscript's (`None`).
pub fn watch_system(personas: &[&Persona], language: Option<Lang>, style_profile: &str, briefs: &BTreeMap<String, String>) -> String {
    let one_line = |text: &str| WHITESPACE.replace_all(text, " ").into_owned();
    let mut lines = vec![
        "You are the live review party of CoWriting, watching a researcher write. The researcher is the sole author (DFG good scientific practice).".to_string(),
        "You see only the paragraphs they just wrote or changed, with line numbers. Give at most 3 short hints in total, only where it matters. An empty list is a good answer."
            .to_string(),
        "Each hint: at most 25 words, about one line, from exactly one of these personas:".to_string(),
    ];
    for p in personas {
        let brief = briefs.get(&p.id).filter(|b| !b.is_empty()).map(|b| format!(". The author asks it to look for: {}", one_line(b))).unwrap_or_default();
        lines.push(format!("- {}: {}{brief}", p.id, p.role));
    }
    if let Some(everyone) = briefs.get("everyone").filter(|b| !b.is_empty()) {
        lines.push(format!("About the project, from the author: {}", one_line(everyone)));
    }
    lines.extend([
        "Never write or rewrite sentences for the manuscript: name the problem and the principle, or ask a question.".to_string(),
        "Exceptions: grammar-guy may give the corrected form of a wrong word or phrase; terminologist may name the usual term of the field, without a sentence around it."
            .to_string(),
        "Never invent facts or references.".to_string(),
        match language {
            Some(Lang::De) => "Write the hints in German.".to_string(),
            Some(Lang::En) => "Write the hints in English.".to_string(),
            None => "Write the hints in the language of the manuscript.".to_string(),
        },
    ]);
    if !style_profile.is_empty() {
        lines.push(format!("The author's style profile, from model papers they like:\n{style_profile}"));
    }
    lines.push(r#"Answer with JSON only: [{"persona": "<id>", "line": <number>, "hint": "<text>"}]"#.to_string());
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedHint {
    pub persona: String,
    /// The line it is about; 0 when the answer gave none.
    pub line: usize,
    pub hint: String,
}

/// JavaScript's `Number(value) || 0` for a line: a whole positive number, or 0.
fn line_of(value: &Value) -> usize {
    let n = match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    };
    if n.is_finite() && n >= 1.0 && n.fract() == 0.0 {
        n as usize
    } else {
        0
    }
}

/// The hints in a live-review answer: at most three, from the personas that watch.
pub fn parse_hints(text: &str, personas: &[&str]) -> Vec<ParsedHint> {
    let (Some(from), Some(to)) = (text.find('['), text.rfind(']')) else { return Vec::new() };
    if to <= from {
        return Vec::new();
    }
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text[from..=to]) else { return Vec::new() };
    let mut out = Vec::new();
    for item in &items {
        let persona = item.get("persona").and_then(Value::as_str).map(bare_id).unwrap_or("");
        let hint = item.get("hint").and_then(Value::as_str).map(squash).unwrap_or_default();
        if !personas.contains(&persona) || hint.is_empty() {
            continue;
        }
        out.push(ParsedHint { persona: persona.to_string(), line: item.get("line").map_or(0, line_of), hint: slice_utf16(&hint, 240).to_string() });
    }
    out.truncate(3);
    out
}
