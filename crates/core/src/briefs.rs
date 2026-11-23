//! The author's briefing of the party for one document: `party.md` in the sidecar, a section per
//! expert and one for everyone. Ported from the plugin's `plugin/hooks/core.ts` (`parseBriefs`,
//! `briefFor`, `setBrief`, `appendBrief`).

use crate::personas::Party;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^## +(.+)$").unwrap());
static HEADING_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^## +").unwrap());
static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*<!--.*-->\s*$").unwrap());
static KEY_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s—–]+").unwrap());

/// The section a heading names: 'everyone', or a persona's id.
fn brief_key(heading: &str, party: &Party) -> Option<String> {
    let first = KEY_END.split(heading.trim()).next().unwrap_or("").to_lowercase();
    if first == "everyone" || first == "all" {
        return Some("everyone".into());
    }
    party.by_id(&first).map(|p| p.id.clone())
}

/// Sections of `party.md` by key ('everyone' or a persona id), comments and blank sections removed.
pub fn parse_briefs(text: &str, party: &Party) -> BTreeMap<String, String> {
    let body = COMMENT.replace_all(text, "");
    let mut out = BTreeMap::new();
    let headings: Vec<_> = HEADING.captures_iter(&body).collect();
    for (i, heading) in headings.iter().enumerate() {
        let whole = heading.get(0).unwrap();
        let end = headings.get(i + 1).map_or(body.len(), |next| next.get(0).unwrap().start());
        let content = body[whole.end()..end].trim();
        if let (Some(key), false) = (brief_key(&heading[1], party), content.is_empty()) {
            out.insert(key, content.to_string());
        }
    }
    out
}

/// What a persona is told about this project: the shared context, then its own brief.
pub fn brief_for(briefs: &BTreeMap<String, String>, persona_id: &str) -> String {
    let part = |key: &str, title: &str| briefs.get(key).filter(|b| !b.is_empty()).map(|b| format!("{title}:\n{b}"));
    [part("everyone", "About the project"), part(persona_id, "What to look for")].into_iter().flatten().collect::<Vec<_>>().join("\n\n")
}

/// `party.md` with one section's body replaced (or the section added), the rest untouched.
/// None when the key names neither everyone nor a persona.
pub fn set_brief(text: &str, key: &str, body: &str, party: &Party) -> Option<String> {
    let heading = if key == "everyone" {
        "everyone — the whole party".to_string()
    } else {
        let persona = party.by_id(key)?;
        format!("{} — {}", persona.id, persona.name)
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let start = lines.iter().position(|line| HEADING_LINE.is_match(line) && brief_key(&HEADING_LINE.replace(line, ""), party).as_deref() == Some(key));
    let Some(start) = start else {
        return Some(format!("{}\n\n## {heading}\n{}\n", text.trim_end(), body.trim()));
    };
    let end = (start + 1..lines.len()).find(|&i| HEADING_LINE.is_match(lines[i])).unwrap_or(lines.len());
    // keep the guiding comment under the heading, replace what the author wrote
    let comments = lines[start + 1..end].iter().filter(|line| COMMENT_LINE.is_match(line));
    let section = format!("## {heading}");
    let mut out: Vec<&str> = lines[..start].to_vec();
    out.push(&section);
    out.extend(comments);
    out.push(body.trim());
    out.push("");
    out.extend(&lines[end..]);
    Some(out.join("\n"))
}

/// `party.md` with lines added under one section's body (the section created when missing), the author's own text kept.
pub fn append_brief(text: &str, key: &str, lines: &str, party: &Party) -> Option<String> {
    let added = lines.trim();
    if added.is_empty() {
        return Some(text.to_string());
    }
    let current = parse_briefs(text, party).remove(key).unwrap_or_default();
    set_brief(text, key, &if current.is_empty() { added.to_string() } else { format!("{current}\n{added}") }, party)
}
