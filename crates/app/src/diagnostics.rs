//! The editor's own technical log: what was used, how long it took and what failed, kept beside
//! the settings so that a bug can be found. It never leaves the computer by itself; the author can
//! add its last minutes to a report, and sees them before they go.
//!
//! What may be in it is decided here, not by whoever writes a line: every message passes through
//! `scrubbed`, which takes out paths, file names, addresses, mail addresses, anything quoted and
//! anything that looks like a key. A document is described by its measures (`shape`), never by its
//! words. The AI log beside a document is something else: that one holds prompts, this one does not.

use cowriting_core::host::iso_utc;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// The file is cut to its newer half when it passes this size.
const LARGEST: u64 = 400_000;
/// The longest message in a line.
const LONGEST: usize = 300;
/// How many entries the window can hand over at once.
const MOST_AT_ONCE: usize = 60;

pub struct Diagnostics {
    file: PathBuf,
    is_on: AtomicBool,
    /// Words that are taken out wherever they appear: the author's user name, the open document's name and folders.
    private: Mutex<Vec<String>>,
    writing: Mutex<()>,
}

/// What the window saw: a command and how it ended, a view that was shown, or an error of its own.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Seen {
    /// 'call', 'view' or 'error'.
    pub kind: String,
    pub name: String,
    pub ok: bool,
    /// The longest of `count` calls, in milliseconds.
    pub ms: u64,
    pub count: u32,
    pub message: String,
}

impl Default for Seen {
    fn default() -> Self {
        Seen { kind: String::new(), name: String::new(), ok: true, ms: 0, count: 1, message: String::new() }
    }
}

/// A name the editor gave something (a command, a tab, a model): letters, digits and a few signs, nothing else.
pub fn name_only(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')).take(60).collect()
}

impl Diagnostics {
    pub fn at(file: PathBuf, is_on: bool) -> Diagnostics {
        let mut private = Vec::new();
        for variable in ["HOME", "USERPROFILE"] {
            if let Ok(home) = std::env::var(variable) {
                private.extend(parts_of(&home));
            }
        }
        for variable in ["USER", "USERNAME"] {
            private.extend(std::env::var(variable).ok());
        }
        let log = Diagnostics { file, is_on: AtomicBool::new(is_on), private: Mutex::new(Vec::new()), writing: Mutex::new(()) };
        log.keep_private(private);
        log
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    /// The author turned the log on or off. Off writes nothing; what is there stays until it is deleted.
    pub fn set_on(&self, is_on: bool) {
        self.is_on.store(is_on, Ordering::Relaxed);
    }

    pub fn is_on(&self) -> bool {
        self.is_on.load(Ordering::Relaxed)
    }

    /// Words that never appear in a line, whatever message carries them.
    pub fn keep_private(&self, words: Vec<String>) {
        let mut private = self.private.lock().unwrap();
        for word in words {
            let word = word.trim().to_string();
            if word.chars().count() >= 3 && !private.contains(&word) {
                private.push(word);
            }
        }
        // the longest first, so a file's name goes before a word inside it
        private.sort_by_key(|word| std::cmp::Reverse(word.len()));
    }

    /// A document is open: its path, its name and every folder on the way are private from now on.
    pub fn keep_document_private(&self, path: &str) {
        let mut words = parts_of(path);
        words.push(path.to_string());
        if let Some(stem) = Path::new(path).file_stem() {
            words.push(stem.to_string_lossy().into_owned());
        }
        self.keep_private(words);
    }

    /// One line: when, what kind of thing, and a message with everything private taken out.
    pub fn note(&self, kind: &str, message: &str) {
        self.note_at(SystemTime::now(), kind, message);
    }

    pub fn note_at(&self, at: SystemTime, kind: &str, message: &str) {
        if !self.is_on() {
            return;
        }
        let message = scrubbed(message, &self.private.lock().unwrap());
        self.write(&format!("{} {} {}\n", iso_utc(at), name_only(kind), message));
    }

    /// A line whose words the editor chose itself (numbers, its own names): written as it is.
    pub fn note_plain(&self, kind: &str, message: &str) {
        if self.is_on() {
            self.write(&format!("{} {} {}\n", iso_utc(SystemTime::now()), name_only(kind), message.replace(['\n', '\r'], " ")));
        }
    }

    /// The measures of a document, at a moment that matters (opened, or something failed with it).
    pub fn note_document(&self, when: &str, text: &str) {
        self.note_plain("document", &format!("{}: {}", name_only(when), shape(text)));
    }

    /// What the window saw. The window is not trusted with the wording: kinds and names are held
    /// to what they can be, and an error's message is scrubbed like every other.
    pub fn from_window(&self, seen: &[Seen]) {
        for entry in seen.iter().take(MOST_AT_ONCE) {
            let name = name_only(&entry.name);
            match entry.kind.as_str() {
                "call" if !name.is_empty() => {
                    let times = if entry.count > 1 { format!(" ×{}", entry.count) } else { String::new() };
                    let said = if entry.ok || entry.message.trim().is_empty() { String::new() } else { format!(": {}", entry.message) };
                    self.note("call", &format!("{name} {}{times} {}ms{said}", if entry.ok { "ok" } else { "failed" }, entry.ms));
                }
                "view" if !name.is_empty() => self.note_plain("view", &name),
                "error" => self.note("error", &format!("window: {}", entry.message)),
                _ => {}
            }
        }
    }

    /// The lines of the last minutes, oldest first.
    pub fn recent(&self, minutes: u64) -> String {
        self.recent_at(SystemTime::now(), minutes)
    }

    pub fn recent_at(&self, now: SystemTime, minutes: u64) -> String {
        let from = iso_utc(now.checked_sub(Duration::from_secs(minutes * 60)).unwrap_or(SystemTime::UNIX_EPOCH));
        let _writing = self.writing.lock().unwrap();
        let all = std::fs::read_to_string(&self.file).unwrap_or_default();
        // times are written the same way in every line, so they compare as text
        all.lines().filter(|line| line.split(' ').next().is_some_and(|time| time >= from.as_str())).collect::<Vec<_>>().join("\n")
    }

    /// Deletes the log.
    pub fn clear(&self) -> Result<(), String> {
        let _writing = self.writing.lock().unwrap();
        match std::fs::remove_file(&self.file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("The log could not be deleted: {e}")),
        }
    }

    /// A file for the author to attach to a report: what the editor is, the log's last minutes and,
    /// only if the author said so, the text of their document.
    pub fn report(&self, about: &str, minutes: u64, text: Option<&str>) -> String {
        let log = self.recent(minutes);
        let mut out = format!("CoWriting Editor: file for a bug report\n{about}\n\n--- The editor's technical log, last {minutes} minutes ---\n");
        out.push_str(if log.is_empty() { "(nothing was recorded)" } else { &log });
        out.push('\n');
        match text {
            Some(text) => out.push_str(&format!("\n--- The document's text, added by its author ---\n{text}\n")),
            None => out.push_str("\nThe document's text is not in this file.\n"),
        }
        out
    }

    fn write(&self, line: &str) {
        use std::io::Write;
        let _writing = self.writing.lock().unwrap();
        // a log that cannot be written is no reason to stop anything
        if let Some(folder) = self.file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        if std::fs::metadata(&self.file).is_ok_and(|file| file.len() > LARGEST) {
            if let Ok(all) = std::fs::read_to_string(&self.file) {
                let mut from = all.len() / 2;
                while !all.is_char_boundary(from) {
                    from += 1;
                }
                let newer = all[from..].split_once('\n').map_or("", |(_, rest)| rest);
                let _ = std::fs::write(&self.file, newer);
            }
        }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&self.file) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

/// The folders and the file name of a path, each as a word.
fn parts_of(path: &str) -> Vec<String> {
    path.split(['/', '\\']).map(str::to_string).filter(|part| !part.is_empty() && !part.ends_with(':')).collect()
}

/// A message with everything taken out that could say who the author is, what they write or how
/// to get into something of theirs. What is left says what went wrong.
pub fn scrubbed(message: &str, private: &[String]) -> String {
    let mut text = message.replace(['\n', '\r', '\t'], " ");
    for word in private {
        text = text.replace(word.as_str(), "<private>");
    }
    let text = without_quoted(&text);
    let mut out = String::new();
    for token in text.split(' ').filter(|token| !token.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(match kind_of(token) {
            Some(instead) => instead,
            None => token,
        });
    }
    if out.chars().count() > LONGEST {
        out = out.chars().take(LONGEST).collect::<String>() + "…";
    }
    out
}

/// Whatever stands between quotation marks is somebody's words (a value, a name, a passage): it goes.
fn without_quoted(text: &str) -> String {
    let closing = |open: char| match open {
        '"' => Some('"'),
        '`' => Some('`'),
        '“' | '„' => Some('”'),
        '«' => Some('»'),
        _ => None,
    };
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match closing(c) {
            Some(close) => {
                // „…“ closes with either mark
                for inner in chars.by_ref() {
                    if inner == close || (c == '„' && inner == '“') {
                        break;
                    }
                }
                out.push_str("<quoted>");
            }
            None => out.push(c),
        }
    }
    out
}

/// What a word is replaced with, if it is one of the things that are never written.
fn kind_of(token: &str) -> Option<&'static str> {
    let core = token.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '.' | '\'' | '!' | '?' | '<' | '>' | '='));
    if core.is_empty() || core == "private" || core == "quoted" {
        return None;
    }
    let lower = core.to_lowercase();
    if core.contains('@') {
        return Some("<mail>");
    }
    if core.contains("://") {
        return Some("<address>");
    }
    if core.contains('/') || core.contains('\\') || core.starts_with('~') {
        return Some("<path>");
    }
    const FILES: [&str; 16] = [".md", ".markdown", ".bib", ".bibtex", ".json", ".jsonl", ".ris", ".nbib", ".csl", ".pdf", ".docx", ".tex", ".txt", ".yaml", ".yml", ".log"];
    if FILES.iter().any(|ending| lower.ends_with(ending) && lower.len() > ending.len()) {
        return Some("<file>");
    }
    // an address without its scheme: a host with a port, a number address, a name of three parts
    let host = core.rsplit_once(':').filter(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())).map_or(core, |(host, _)| host);
    let labels: Vec<&str> = host.split('.').collect();
    let is_named = labels.len() >= 3 && labels.iter().all(|label| !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    if is_named || (host != core && host != "localhost" && labels.len() >= 2) {
        return Some("<address>");
    }
    // a key or a token: long, and letters mixed with digits; or one that says what it is
    let is_keylike = core.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '=')) && core.chars().any(|c| c.is_ascii_digit()) && core.chars().any(|c| c.is_ascii_alphabetic());
    if (core.len() >= 20 && is_keylike) || lower.starts_with("sk-") || lower.starts_with("key-") || lower.starts_with("ghp_") {
        return Some("<secret>");
    }
    None
}

/// A document by its measures: how large it is, what its lines end with, which signs it holds and
/// which markdown constructs it uses. Enough to see what kind of text broke something; no word of it.
pub fn shape(text: &str) -> String {
    let has_bom = text.starts_with('\u{feff}');
    let (crlf, lone_cr) = (text.matches("\r\n").count(), text.matches('\r').count() - text.matches("\r\n").count());
    let lf = text.matches('\n').count() - crlf;
    let endings = match (lf > 0, crlf > 0, lone_cr > 0) {
        (false, false, false) => "one line",
        (true, false, false) => "lf",
        (false, true, false) => "crlf",
        (false, false, true) => "cr",
        _ => "mixed line ends",
    };
    let chars = text.chars().count();
    let control = text.chars().filter(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')).count();
    let non_ascii = text.chars().filter(|c| !c.is_ascii()).count();
    // signs outside the basic plane (emoji, rare scripts) take two places in the window's counting
    let astral = text.chars().filter(|c| (*c as u32) > 0xffff).count();
    let tabs = text.matches('\t').count();
    let lines: Vec<&str> = text.lines().collect();
    let longest = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
    let starting = |start: &str| lines.iter().filter(|line| line.trim_start().starts_with(start)).count();
    let fences = starting("```") + starting("~~~");
    let headings = lines.iter().filter(|line| line.starts_with('#') && line.trim_start_matches('#').starts_with(' ')).count();
    let front_matter = lines.first().is_some_and(|line| line.trim_start_matches('\u{feff}').trim_end() == "---");
    format!(
        "{} bytes, {chars} characters, {} lines (longest {longest}), {} words, {endings}, {}, {tabs} tabs, {control} control characters, {non_ascii} outside ASCII, {astral} outside the basic plane; \
         {}, {headings} headings, {fences} fence lines{}, {} table lines, {} footnote marks, {} citations, {} images, {} links, {} lines starting with a tag, {} comments, {} display formulas",
        text.len(),
        lines.len(),
        text.split_whitespace().count(),
        if has_bom { "byte order mark" } else { "no byte order mark" },
        if front_matter { "front matter" } else { "no front matter" },
        if fences % 2 == 1 { " (one not closed)" } else { "" },
        starting("|"),
        text.matches("[^").count(),
        text.matches("[@").count() + text.matches("; @").count(),
        text.matches("![").count(),
        text.matches("](").count(),
        lines.iter().filter(|line| line.trim_start().starts_with('<') && !line.trim_start().starts_with("<!--")).count(),
        text.matches("<!--").count(),
        text.matches("$$").count() / 2,
    )
}
