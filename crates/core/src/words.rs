//! Counting the author's words and finding the paragraphs they changed.
//! Ported from `countWords`, `paragraphs` and `changedParagraphs` in the plugin's `plugin/hooks/core.ts`.

use crate::text::squash;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Latex,
}

/// What is no prose in markdown: front matter, code blocks, comments, citation keys.
static MD_LEFT_OUT: LazyLock<Vec<Regex>> =
    LazyLock::new(|| [r"\A---\n(?s:.*?)\n---\n", r"(?s)```.*?```", r"(?s)<!--.*?-->", r"\[@[^\]]*\]"].iter().map(|p| Regex::new(p).unwrap()).collect());
/// The characters markdown's marks are made of, wherever they stand. Enough to count words by; too coarse for characters (see `MD_MARKS`).
static MD_MARK_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[#*_>`|~-]").unwrap());

static WORDLIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\p{N}]").unwrap());
static TEX_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)(^|[^\\])%.*$").unwrap());
static TEX_STRIP: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let mut patterns: Vec<String> = ["figure", "table", "equation", "align"].iter().map(|env| format!(r"(?s)\\begin\{{{env}\*?\}}.*?\\end\{{{env}\*?\}}")).collect();
    patterns.push(r"\\([A-Za-z]*cite[a-z]*|ref|label|eqref|autoref|cref|input|include|usepackage|documentclass|bibliography|addbibresource)\*?(\[[^\]]*\])*\{[^}]*\}".into());
    patterns.push(r"\\[a-zA-Z]+\*?".into());
    patterns.push(r"[{}$\\]".into());
    patterns.iter().map(|p| Regex::new(p).unwrap()).collect()
});

/// Two characters no text holds, put where a selection begins and ends: they go through the
/// counting with the text, and what is counted between them is the selection's share.
const FROM: char = '\u{E000}';
const TO: char = '\u{E001}';

/// `pattern.replace_all`, but a mark of the selection that lies in what is replaced is kept. A
/// text without such marks, which is every text but a selection's, comes out exactly as before.
fn replaced(pattern: &Regex, text: &str, stays: &str) -> String {
    if !text.contains([FROM, TO]) {
        return pattern.replace_all(text, stays).into_owned();
    }
    pattern
        .replace_all(text, |found: &regex::Captures| {
            let mut kept = String::new();
            found.expand(stays, &mut kept);
            for mark in [FROM, TO] {
                if found[0].contains(mark) && !kept.contains(mark) {
                    kept.push(mark);
                }
            }
            kept
        })
        .into_owned()
}

/// The text with everything taken out that is not counted as a word.
fn prose_for_words(text: &str, format: Format) -> String {
    let mut t = text.to_string();
    match format {
        Format::Latex => {
            t = TEX_COMMENT.replace_all(&t, "$1").into_owned();
            for pattern in TEX_STRIP.iter() {
                t = pattern.replace_all(&t, " ").into_owned();
            }
        }
        Format::Markdown => {
            for pattern in MD_LEFT_OUT.iter() {
                t = replaced(pattern, &t, " ");
            }
            t = replaced(&MD_MARK_CHARS, &t, " ");
        }
    }
    t
}

fn words_in(prose: &str) -> usize {
    prose.split_whitespace().filter(|w| WORDLIKE.is_match(w)).count()
}

/// The words a reader reads: syntax, front matter, code, comments and citation keys are not counted.
pub fn count_words(text: &str, format: Format) -> usize {
    words_in(&prose_for_words(text, format))
}

/// How many characters a text has, as a call for papers counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chars {
    pub with_spaces: usize,
    pub without_spaces: usize,
}

/// Markdown's marks where they are marks, each with what stays of it. A hyphen in a word, a star
/// that was escaped and a number in a sentence are the author's characters and stay.
static MD_MARKS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        // a picture is not text; of a link the reader reads the text, not the address
        (r"!\[[^\]]*\]\([^)]*\)", " "),
        (r"\[([^\]]*)\]\([^)]*\)", "$1"),
        // a footnote's mark goes, its text stays
        (r"\[\^[^\]]*\]:?", " "),
        (r"\^\[([^\]]*)\]", " $1"),
        // HTML tags
        (r"</?[A-Za-z][A-Za-z0-9]*(\s[^>]*)?/?>", " "),
        // lines that are only marks: a rule, and the line under a table's head
        (r"(?m)^[ \t]*([-*_][ \t]*){3,}$", " "),
        (r"(?m)^[ \t]*\|?[ \t]*:?-+:?[ \t]*(\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$", " "),
        // what a line starts with as a block: quote marks, heading hashes, a list's mark or number
        (r"(?m)^[ \t]*(>[ \t]?)+", ""),
        (r"(?m)^[ \t]*(#{1,6}[ \t]+|[-*+][ \t]+|\d+[.)][ \t]+)", ""),
        // underscores around a word, not in one
        (r"(?m)(^|[^\p{L}\p{N}_\\])_+", "$1"),
        (r"(?m)_+($|[^\p{L}\p{N}_])", "$1"),
        // stars, backticks, strike-through, a table's bars; an escaped character stays without its backslash
        (r"\\([\\`*_{}\[\]()#+.!|~>-])|\*+|`+|~~|\|", "$1"),
    ]
    .iter()
    .map(|(pattern, stays)| (Regex::new(pattern).unwrap(), *stays))
    .collect()
});

/// The characters a reader reads, with and without spaces. What `count_words` leaves out is left
/// out here too (front matter, code, comments, citation keys, marks), and so are the address of a
/// link and a picture. A run of spaces and line ends is one space; a letter with its accent, or an
/// emoji, is one character however it is stored.
pub fn count_chars(text: &str, format: Format) -> Chars {
    chars_in(&prose_for_chars(text, format))
}

/// The text with everything taken out that is not counted as a character.
fn prose_for_chars(text: &str, format: Format) -> String {
    let mut t = text.to_string();
    match format {
        Format::Latex => {
            t = TEX_COMMENT.replace_all(&t, "$1").into_owned();
            for pattern in TEX_STRIP.iter() {
                t = pattern.replace_all(&t, " ").into_owned();
            }
        }
        Format::Markdown => {
            for pattern in MD_LEFT_OUT.iter() {
                t = replaced(pattern, &t, " ");
            }
            for (pattern, stays) in MD_MARKS.iter() {
                t = replaced(pattern, &t, stays);
            }
        }
    }
    t
}

fn chars_in(prose: &str) -> Chars {
    let words: Vec<&str> = prose.split_whitespace().collect();
    let without_spaces = words.iter().map(|word| word.graphemes(true).count()).sum();
    Chars { with_spaces: without_spaces + words.len().saturating_sub(1), without_spaces }
}

/// The words and characters of a part of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Counted {
    pub words: usize,
    pub chars: Chars,
}

/// What of a markdown text's count falls to the part from `from` to `to` (in UTF-16 units, as an
/// editor in a web view counts positions). The rules are the document's: a selection inside a
/// comment or a code block counts nothing, one that cuts through a link counts the part of the
/// link's text it holds, and marks count nowhere. A word that is cut counts as a word.
pub fn count_selection(text: &str, from: usize, to: usize) -> Counted {
    // a text that holds the two marks itself keeps its length and loses them
    let text = text.replace([FROM, TO], "\u{FFFD}");
    let byte_at = |units: usize| {
        let mut seen = 0;
        text.char_indices()
            .find(|(_, c)| {
                let is_here = seen >= units;
                seen += c.len_utf16();
                is_here
            })
            .map_or(text.len(), |(at, _)| at)
    };
    // the front matter is found by where the text begins: the marks stay out of it
    let front = MD_LEFT_OUT[0].find(&text).map_or(0, |found| found.end());
    let (from, to) = (byte_at(from.min(to)).max(front), byte_at(from.max(to)).max(front));
    let marked = format!("{}{FROM}{}{TO}{}", &text[..from], &text[from..to], &text[to..]);
    let between = |prose: String| match (prose.find(FROM), prose.find(TO)) {
        (Some(start), Some(end)) if start < end => prose[start + FROM.len_utf8()..end].to_string(),
        _ => String::new(),
    };
    Counted { words: words_in(&between(prose_for_words(&marked, Format::Markdown))), chars: chars_in(&between(prose_for_chars(&marked, Format::Markdown))) }
}

/// A run of non-blank lines, with 1-based line numbers; `end` is its last line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Paragraph {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub fn paragraphs(text: &str) -> Vec<Paragraph> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..=lines.len() {
        let is_blank = i == lines.len() || lines[i].trim().is_empty();
        match (is_blank, start) {
            (false, None) => start = Some(i),
            (true, Some(from)) => {
                out.push(Paragraph { start: from + 1, end: i, text: lines[from..i].join("\n") });
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// The paragraphs of `after` that are new or changed since `before`, with some prose in them.
pub fn changed_paragraphs(before: &str, after: &str, format: Format) -> Vec<Paragraph> {
    let known: HashSet<String> = paragraphs(before).iter().map(|p| squash(&p.text)).collect();
    paragraphs(after).into_iter().filter(|p| !known.contains(&squash(&p.text)) && count_words(&p.text, format) >= 4).collect()
}
