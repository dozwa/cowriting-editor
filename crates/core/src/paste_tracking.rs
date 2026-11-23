//! Noticing AI text that enters the manuscript. The editor owns the text surface, so it can
//! compare what is pasted or typed with what the AI said in this document's log. Nothing is
//! blocked and nothing is marked in the text: a match becomes an `ai_text` entry in the log and
//! a line in the disclosure.
//!
//! What is compared: runs of words from an answer that the author had not written themselves
//! (feedback quotes the author back, so whatever was in the document or the question at the
//! time is left out). Code, references, DOIs, links and citation keys are never compared.
//! Text from other AI tools cannot be detected.

use crate::sidecar::content_hash;
use crate::text::squash;
use crate::types::{AiTextEntry, LogEntry};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Words per shingle: the unit that is looked up for an exact match.
pub const SHINGLE: usize = 5;
/// Words per shingle for a near match: shorter, so that a changed word here and there costs less.
pub const NEAR_SHINGLE: usize = 3;
/// An exact match: this many words in a row, and this many characters.
pub const EXACT_WORDS: usize = 8;
pub const EXACT_CHARS: usize = 50;
/// A near match: a paragraph of at least this many words with this share of its short shingles in one answer.
pub const NEAR_WORDS: usize = 12;
pub const NEAR_SHARE: f64 = 0.6;

static FENCED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?(```|\z)").unwrap());
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\n]*`").unwrap());
static CITATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]\n]*@[^\]\n]*\]|@[A-Za-z][\w:.-]*").unwrap());
static REFERENCE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^.*(\b10\.\d{4,9}/\S+|https?://\S+).*$").unwrap());

/// The text with everything that is never compared blanked out, byte for byte, so positions stay.
fn masked(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in [&*FENCED, &*INLINE_CODE, &*REFERENCE_LINE, &*CITATION] {
        out = pattern
            .replace_all(&out, |found: &regex::Captures| found[0].chars().map(|c| if c == '\n' { "\n".to_string() } else { " ".repeat(c.len_utf8()) }).collect::<String>())
            .into_owned();
    }
    out
}

/// A word as it is compared (lowercase letters and digits) and where it is in the text.
struct Word {
    key: String,
    start: usize,
    end: usize,
}

fn words(text: &str) -> Vec<Word> {
    let masked = masked(text);
    let mut out: Vec<Word> = Vec::new();
    let mut open: Option<usize> = None;
    for (at, c) in masked.char_indices().chain([(masked.len(), ' ')]) {
        match (c.is_alphanumeric(), open) {
            (true, None) => open = Some(at),
            (false, Some(start)) => {
                out.push(Word { key: masked[start..at].to_lowercase(), start, end: at });
                open = None;
            }
            _ => {}
        }
    }
    out
}

fn shingle(words: &[Word]) -> u64 {
    // FNV-1a over the words, a space between them
    let mut hash: u64 = 0xcbf29ce484222325;
    for (i, word) in words.iter().enumerate() {
        for byte in word.key.bytes().chain((i > 0).then_some(b' ')) {
            hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn shingles(words: &[Word], size: usize) -> Vec<u64> {
    if words.len() < size {
        return Vec::new();
    }
    words.windows(size).map(shingle).collect()
}

/// What one answer said that the author had not: its word runs, as hashes.
#[derive(Debug, Clone)]
pub struct Source {
    /// The turn's id.
    pub reference: String,
    /// 'correction' for an answer about grammar and spelling, 'text' otherwise.
    pub kind: String,
    novel: HashSet<u64>,
    novel_near: HashSet<u64>,
}

impl Source {
    /// `own` is what the author had written when they asked: the document then, and the question.
    pub fn new(reference: &str, kind: &str, answer: &str, own: &[&str]) -> Source {
        let own: Vec<Vec<Word>> = own.iter().map(|text| words(text)).collect();
        let said = words(answer);
        let novel = |size: usize| {
            let theirs: HashSet<u64> = own.iter().flat_map(|text| shingles(text, size)).collect();
            shingles(&said, size).into_iter().filter(|s| !theirs.contains(s)).collect::<HashSet<u64>>()
        };
        Source { reference: reference.to_string(), kind: kind.to_string(), novel: novel(SHINGLE), novel_near: novel(NEAR_SHINGLE) }
    }
}

/// The sources a log holds. `snapshot_of` gives the document as it was when a turn was asked.
pub fn sources(entries: &[LogEntry], snapshot_of: &dyn Fn(&str) -> Option<String>) -> Vec<Source> {
    let mut out = Vec::new();
    for entry in entries {
        let LogEntry::Response(response) = entry else { continue };
        let prompt = entries.iter().find_map(|e| match e {
            LogEntry::Prompt(p) if p.id == response.reference => Some(p),
            _ => None,
        });
        let Some(prompt) = prompt else { continue };
        let snapshot = snapshot_of(&prompt.id).unwrap_or_default();
        let kind = if prompt.category == "language" { "correction" } else { "text" };
        let source = Source::new(&prompt.id, kind, &response.answer, &[&snapshot, &prompt.text]);
        if !source.novel_near.is_empty() {
            out.push(source);
        }
    }
    out
}

/// A passage that matches an answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub reference: String,
    pub kind: String,
    /// 'exact' or 'near'.
    pub match_: String,
    pub words: usize,
    /// The share of the passage's word runs that are the answer's.
    pub share: f64,
    pub excerpt: String,
    pub hash: String,
}

/// The best match of one passage (a paragraph, or what was pasted) against the sources.
pub fn check(passage: &str, sources: &[Source]) -> Option<Found> {
    let words = words(passage);
    let (all, short) = (shingles(&words, SHINGLE), shingles(&words, NEAR_SHINGLE));
    if short.is_empty() {
        return None;
    }
    let mut best: Option<(usize, Found)> = None;
    for source in sources {
        let hits: Vec<bool> = all.iter().map(|s| source.novel.contains(s)).collect();
        let matched = short.iter().filter(|s| source.novel_near.contains(s)).count();
        if matched == 0 {
            continue;
        }
        // the longest run of word runs in a row
        let (mut longest, mut longest_at, mut run) = (0, 0, 0);
        for (i, hit) in hits.iter().enumerate() {
            run = if *hit { run + 1 } else { 0 };
            if run > longest {
                (longest, longest_at) = (run, i + 1 - run);
            }
        }
        let run_words = if longest == 0 { 0 } else { longest + SHINGLE - 1 };
        let run_chars = if longest == 0 { 0 } else { words[longest_at + run_words - 1].end - words[longest_at].start };
        let share = matched as f64 / short.len() as f64;
        let found = if run_words >= EXACT_WORDS && run_chars >= EXACT_CHARS {
            Some(("exact", longest_at, run_words))
        } else if words.len() >= NEAR_WORDS && share >= NEAR_SHARE {
            Some(("near", 0, words.len()))
        } else {
            None
        };
        let Some((match_, from, count)) = found else { continue };
        let span = &words[from..from + count];
        let found = Found {
            reference: source.reference.clone(),
            kind: source.kind.clone(),
            match_: match_.to_string(),
            words: count,
            share: (share * 100.0).round() / 100.0,
            excerpt: squash(&passage[span[0].start..span[count - 1].end]).chars().take(200).collect(),
            hash: content_hash(&span.iter().map(|w| w.key.as_str()).collect::<Vec<_>>().join(" ")),
        };
        if best.as_ref().is_none_or(|(most, _)| matched > *most) {
            best = Some((matched, found));
        }
    }
    best.map(|(_, found)| found)
}

/// Is a logged passage still in the document? Judged by the excerpt's words, in a row.
pub fn still_in_text(entry: &AiTextEntry, document: &str) -> bool {
    let mut wanted: Vec<String> = words(&entry.excerpt).into_iter().map(|w| w.key).collect();
    // an excerpt cut at 200 characters may end in half a word
    if entry.excerpt.chars().count() >= 200 {
        wanted.pop();
    }
    let have: Vec<String> = words(document).into_iter().map(|w| w.key).collect();
    !wanted.is_empty() && have.windows(wanted.len()).any(|window| window == wanted.as_slice())
}

/// Has this passage from this answer been logged already?
pub fn is_logged(entries: &[LogEntry], found: &Found) -> bool {
    entries.iter().any(|e| matches!(e, LogEntry::AiText(known) if known.reference == found.reference && known.hash == found.hash))
}
