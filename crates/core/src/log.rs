//! The AI log: JSON Lines in `ai-log/YYYY-MM.jsonl` (UTC month), append-only.
//! Ported from the plugin's `plugin/hooks/core.ts` (parseLog, monthFile, categoryFor, readable) and
//! `plugin/hooks/register.tsx` (readLog, appendLog, snapshotLog, restoreLog).

use crate::host::Host;
use crate::personas::{bare_id, Party};
use crate::types::LogEntry;
use regex::Regex;
use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, LazyLock, Mutex};

/// What an answer starts AI-written text with, when the author insisted on it.
pub const AI_TEXT_MARKER: &str = "[AI-TEXT]";

/// Commands that only manage the log or the workspace; their turns need no classification.
const META_COMMANDS: &[&str] = &["annotate", "disclosure", "init", "pane", "paper", "brief", "watch", "quiet", "scope"];

const COMMAND_CATEGORY: &[(&str, &str)] = &[
    ("feedback", "feedback"),
    ("socratic", "feedback"),
    ("structure", "structure"),
    ("language", "language"),
    ("sources", "literature"),
    ("explain", "concepts"),
    ("consistency", "structure"),
    ("submit", "integrity"),
];

static SUMMONED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"cowrite:([a-z0-9-]+) agent").unwrap());

/// The category a turn is logged under: from the action that started it, or the persona it asks.
pub fn category_for(command: Option<&str>, text: &str, party: &Party) -> String {
    if let Some(command) = command {
        let name = ["cowrite:", "cw:"].iter().find_map(|p| command.strip_prefix(p)).unwrap_or(command);
        if META_COMMANDS.contains(&name) {
            return "meta".into();
        }
        if name == "summon" || name == "ask" {
            let first = text.split_whitespace().next().unwrap_or("");
            return party.by_id(first).map_or("feedback".into(), |p| p.category.clone());
        }
        return COMMAND_CATEGORY.iter().find(|(c, _)| *c == name).map_or("general", |(_, category)| category).into();
    }
    if let Some(found) = SUMMONED.captures(text) {
        return party.by_id(&found[1]).map_or("general".into(), |p| p.category.clone());
    }
    "general".into()
}

/// The entries in a log file's text. A torn line is skipped, never fatal.
pub fn parse_log(text: &str) -> Vec<LogEntry> {
    text.split('\n').map(str::trim).filter(|line| !line.is_empty()).filter_map(|line| serde_json::from_str(line).ok()).filter_map(LogEntry::from_value).collect()
}

/// The log file a moment belongs to, from its ISO timestamp (UTC): `2026-11.jsonl`.
pub fn month_file(iso: &str) -> String {
    format!("{}.jsonl", &iso[..7.min(iso.len())])
}

static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static HAND_BACK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[Subagent hand-back\][^.]*\.").unwrap());
static MARKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[#*_`>|]").unwrap());
static HAND_BACK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*\[Subagent hand-back\][^\n]*\n?").unwrap());
static AGENT_FRAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"</?agent-message[^>]*>").unwrap());
static LINE_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>\n]+>").unwrap());
static BLANK_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

/// Markdown, tags and frames out: a line a person can read in a list.
pub fn flatten(s: &str) -> String {
    let s = TAGS.replace_all(s, " ");
    let s = HAND_BACK.replace_all(&s, " ");
    let s = MARKS.replace_all(&s, " ");
    crate::text::squash(&s)
}

/// The whole text as a person reads it: frames and tags out, paragraphs kept.
pub fn readable(s: &str) -> String {
    let s = HAND_BACK_LINE.replace_all(s, "");
    let s = AGENT_FRAME.replace_all(&s, "");
    let s = LINE_TAGS.replace_all(&s, "");
    BLANK_RUN.replace_all(&s, "\n\n").trim().to_string()
}

/// The log's files by name → content, as they were at one moment.
pub type LogSnapshot = BTreeMap<String, String>;

/// The log files whose earlier content did not survive: changed or removed. A file that only grew, or is new, is fine.
pub fn rewritten_log_files(before: &LogSnapshot, after: &LogSnapshot) -> Vec<String> {
    before.iter().filter(|(file, was)| !after.get(*file).is_some_and(|now| now.starts_with(was.as_str()))).map(|(file, _)| file.clone()).collect()
}

/// New files in the log other than this month's: the log grows by its month file and nothing else.
pub fn stray_log_files(before: &LogSnapshot, after: &LogSnapshot, month: &str) -> Vec<String> {
    after.keys().filter(|file| !before.contains_key(*file) && file.as_str() != month).cloned().collect()
}

/// One document's log. Every read-then-write goes through one lock, so two appends never lose one.
pub struct Log {
    host: Arc<dyn Host>,
    dir: String,
    /// Where a rewritten log file's altered form is kept.
    rewrites_dir: String,
    writing: Mutex<()>,
}

impl Log {
    pub fn new(host: Arc<dyn Host>, dir: impl Into<String>, rewrites_dir: impl Into<String>) -> Self {
        Log { host, dir: dir.into(), rewrites_dir: rewrites_dir.into(), writing: Mutex::new(()) }
    }

    fn files(&self) -> io::Result<Vec<String>> {
        if !self.host.exists(&self.dir) {
            return Ok(Vec::new());
        }
        let mut names = self.host.list(&self.dir)?;
        names.sort();
        Ok(names)
    }

    /// Every entry, oldest month first.
    pub fn read(&self) -> io::Result<Vec<LogEntry>> {
        let _one = self.writing.lock().unwrap();
        let mut entries = Vec::new();
        for name in self.files()?.iter().filter(|n| n.ends_with(".jsonl")) {
            entries.extend(parse_log(&self.host.read(&format!("{}/{name}", self.dir))?));
        }
        Ok(entries)
    }

    /// Adds one entry to this month's file.
    pub fn append(&self, entry: &LogEntry) -> io::Result<()> {
        let _one = self.writing.lock().unwrap();
        let path = format!("{}/{}", self.dir, month_file(&self.host.now()));
        let before = if self.host.exists(&path) { self.host.read(&path)? } else { String::new() };
        let separator = if before.is_empty() || before.ends_with('\n') { "" } else { "\n" };
        self.host.append(&path, &format!("{separator}{}\n", entry.to_line()))
    }

    /// The log's files as they are now, by content.
    pub fn snapshot(&self) -> io::Result<LogSnapshot> {
        let _one = self.writing.lock().unwrap();
        let mut out = LogSnapshot::new();
        for name in self.files()? {
            out.insert(name.clone(), self.host.read(&format!("{}/{name}", self.dir)).unwrap_or_default());
        }
        Ok(out)
    }

    /// Undoes what happened to the log between two snapshots: rewritten files go back, new ones
    /// other than this month's come out. What they had become is kept in the rewrites folder.
    /// Returns the files it restored or removed.
    pub fn restore(&self, before: &LogSnapshot, after: &LogSnapshot) -> io::Result<Vec<String>> {
        let now = self.host.now();
        let rewritten = rewritten_log_files(before, after);
        let strays = stray_log_files(before, after, &month_file(&now));
        let _one = self.writing.lock().unwrap();
        let stamp = now.replace([':', '.'], "-");
        for file in rewritten.iter().chain(&strays) {
            if let Some(altered) = after.get(file) {
                self.host.write(&format!("{}/{stamp}-{file}", self.rewrites_dir), altered)?;
            }
        }
        for file in &rewritten {
            self.host.write(&format!("{}/{file}", self.dir), &before[file])?;
        }
        for file in &strays {
            self.host.remove(&format!("{}/{file}", self.dir))?;
        }
        Ok(rewritten.into_iter().chain(strays).collect())
    }
}

/// The persona an id in the log names, without a `cowrite:` prefix.
pub fn persona_key(id: &str) -> &str {
    bare_id(id)
}
