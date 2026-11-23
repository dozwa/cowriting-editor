//! Small text helpers. The plugin's TypeScript counts in UTF-16 units and its Python in code
//! points; the helpers here are named for which of the two they follow, so ported code cuts
//! text where the original does.

use regex::Regex;
use std::sync::LazyLock;

static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Runs of whitespace become one space, the ends are trimmed.
pub fn squash(s: &str) -> String {
    WHITESPACE.replace_all(s, " ").trim().to_string()
}

/// The first `units` UTF-16 units, as JavaScript's `slice(0, units)`; a pair is never split.
pub fn slice_utf16(s: &str, units: usize) -> &str {
    let mut used = 0;
    for (at, c) in s.char_indices() {
        used += c.len_utf16();
        if used > units {
            return &s[..at];
        }
    }
    s
}

/// The first `count` characters, as Python's `s[:count]`.
pub fn first_chars(s: &str, count: usize) -> &str {
    match s.char_indices().nth(count) {
        Some((at, _)) => &s[..at],
        None => s,
    }
}

/// What Python's `str.split()` and `str.strip()` take for whitespace.
fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python's `" ".join(text.split())`, cut to `limit` characters with an ellipsis.
pub fn py_squash(text: &str, limit: Option<usize>) -> String {
    let joined = text.split(is_py_space).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    match limit {
        Some(limit) if joined.chars().count() > limit => format!("{}…", first_chars(&joined, limit.saturating_sub(1))),
        _ => joined,
    }
}

/// Python's `str.strip()`.
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(is_py_space)
}

/// Python's `str.splitlines()`.
pub fn py_splitlines(text: &str) -> Vec<&str> {
    let is_break = |c: char| matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}');
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if !is_break(c) {
            continue;
        }
        out.push(&text[start..at]);
        start = at + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|&(_, next)| next == '\n') {
            chars.next();
            start += 1;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// "1 hint", "2 hints".
pub fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}
