//! Counting the author's words and finding the paragraphs they changed.
//! Ported from `countWords`, `paragraphs` and `changedParagraphs` in the plugin's `plugin/hooks/core.ts`.

use crate::text::squash;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Latex,
}

static MD_STRIP: LazyLock<Vec<Regex>> =
    LazyLock::new(|| [r"\A---\n(?s:.*?)\n---\n", r"(?s)```.*?```", r"(?s)<!--.*?-->", r"\[@[^\]]*\]", r"[#*_>`|~-]"].iter().map(|p| Regex::new(p).unwrap()).collect());

static WORDLIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\p{N}]").unwrap());
static TEX_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)(^|[^\\])%.*$").unwrap());
static TEX_STRIP: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let mut patterns: Vec<String> = ["figure", "table", "equation", "align"].iter().map(|env| format!(r"(?s)\\begin\{{{env}\*?\}}.*?\\end\{{{env}\*?\}}")).collect();
    patterns.push(r"\\([A-Za-z]*cite[a-z]*|ref|label|eqref|autoref|cref|input|include|usepackage|documentclass|bibliography|addbibresource)\*?(\[[^\]]*\])*\{[^}]*\}".into());
    patterns.push(r"\\[a-zA-Z]+\*?".into());
    patterns.push(r"[{}$\\]".into());
    patterns.iter().map(|p| Regex::new(p).unwrap()).collect()
});

/// The words a reader reads: syntax, front matter, code, comments and citation keys are not counted.
pub fn count_words(text: &str, format: Format) -> usize {
    let mut t = text.to_string();
    match format {
        Format::Latex => {
            t = TEX_COMMENT.replace_all(&t, "$1").into_owned();
            for pattern in TEX_STRIP.iter() {
                t = pattern.replace_all(&t, " ").into_owned();
            }
        }
        Format::Markdown => {
            for pattern in MD_STRIP.iter() {
                t = pattern.replace_all(&t, " ").into_owned();
            }
        }
    }
    t.split_whitespace().filter(|w| WORDLIKE.is_match(w)).count()
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
