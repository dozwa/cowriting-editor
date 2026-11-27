//! The JSON files the editor keeps, and how a new version reads what an older one wrote.
//!
//! Every such file carries a number, `schema`, that says which form it has. A file without one is
//! of the first form. The rules, so that an update never costs an author their project:
//!
//! - A file of an **older** form is brought up to date in steps (`Kind::steps`), each a small
//!   function on the JSON. Before it is written in the new form, the old file is copied to
//!   `backup/schema-<n>/` beside it.
//! - A file of a **newer** form than this version knows is read as far as it is understood and
//!   never written: a newer editor made it and would lose what this one drops.
//! - A file that **cannot be read** is never replaced by the standard and written over: it is set
//!   aside as `<name>.broken-<date>`, and the author is told.
//! - A new field needs no new form: it gets a standard (`#[serde(default)]`). Fields a version
//!   does not know are kept (`extra` in the types), so an older editor does not throw away what a
//!   newer one wrote.
//!
//! The log (`ai-log/*.jsonl`) is not one of these files: it is only added to, old lines are never
//! rewritten, and its reader understands every form a line ever had.

use crate::host::Host;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};
use std::io;

/// One step from a form to the next one, on the file's JSON.
pub type Step = fn(&mut Map<String, Value>);

/// One kind of stored file.
#[derive(Debug, Clone, Copy)]
pub struct Kind {
    /// What the file is, for a message: "the project's settings".
    pub what: &'static str,
    /// `steps[0]` takes form 1 to form 2, and so on. The form this version writes is one more than their number.
    pub steps: &'static [Step],
}

impl Kind {
    pub const fn new(what: &'static str, steps: &'static [Step]) -> Kind {
        Kind { what, steps }
    }

    /// The form this version reads and writes.
    pub fn schema(&self) -> u64 {
        self.steps.len() as u64 + 1
    }
}

pub const DOC: Kind = Kind::new("what the log knows about its document (doc.json)", &[]);
pub const PROJECT: Kind = Kind::new("the project's settings (project.json)", &[]);
pub const LIVE: Kind = Kind::new("the live review's settings (live.json)", &[]);
pub const CHECK: Kind = Kind::new("the last check of the plan's points (check.json)", &[]);
pub const VERIFY: Kind = Kind::new("the last check of the references (verify.json)", &[]);
pub const SOURCES: Kind = Kind::new("the list of bibliography files (sources.json)", &[]);
pub const PARTY: Kind = Kind::new("the list of experts (party.json)", &[]);
pub const MARKS: Kind = Kind::new("the hints marked as done (marks.json)", &[]);
pub const SETTINGS: Kind = Kind::new("the editor's settings (settings.json)", &[]);
pub const STATE: Kind = Kind::new("what the editor remembers between starts (state.json)", &[]);

/// Something about a stored file that the author should know.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "what", rename_all = "camelCase")]
pub enum Notice {
    /// The file could not be read. It was moved to `kept_as`, and the standard is used.
    #[serde(rename_all = "camelCase")]
    SetAside { file: String, kept_as: String },
    /// A newer version of the editor wrote the file. It is used as far as it is understood and not changed.
    Newer { file: String },
}

/// The form a file's JSON says it has; the first when it says nothing.
fn schema_of(json: &Map<String, Value>) -> u64 {
    json.get("schema").and_then(Value::as_u64).unwrap_or(1)
}

fn parsed(text: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
        Ok(Value::Object(json)) => Some(json),
        _ => None,
    }
}

/// The file's JSON in the form this version knows: an older file goes through its steps (in
/// memory; nothing is written). `None` when there is no file or it cannot be read. A newer file
/// comes as it is.
pub fn read(host: &dyn Host, path: &str, kind: &Kind) -> Option<Map<String, Value>> {
    let mut json = parsed(&host.read(path).ok()?)?;
    let from = schema_of(&json);
    if from < kind.schema() {
        for step in &kind.steps[(from.max(1) - 1) as usize..] {
            step(&mut json);
        }
        json.insert("schema".into(), kind.schema().into());
    }
    Some(json)
}

/// The file read into its type; `None` when there is none or it does not fit.
pub fn read_as<T: DeserializeOwned>(host: &dyn Host, path: &str, kind: &Kind) -> Option<T> {
    let mut json = read(host, path, kind)?;
    // the form's number belongs to the file, not to what it holds: `write` puts it back
    json.remove("schema");
    serde_json::from_value(Value::Object(json)).ok()
}

/// Whether a newer version of the editor wrote the file.
pub fn is_newer(host: &dyn Host, path: &str, kind: &Kind) -> bool {
    host.read(path).ok().and_then(|text| parsed(&text)).is_some_and(|json| schema_of(&json) > kind.schema())
}

fn split(path: &str) -> (&str, &str) {
    match path.rfind(['/', '\\']) {
        Some(at) => (&path[..at], &path[at + 1..]),
        None => (".", path),
    }
}

/// Moves a file that cannot be read out of the way, under a name that is not taken, and returns that name.
fn set_aside(host: &dyn Host, path: &str) -> io::Result<String> {
    let day = host.now().chars().take(10).collect::<String>();
    let mut kept_as = format!("{path}.broken-{day}");
    let mut n = 2;
    while host.exists(&kept_as) {
        kept_as = format!("{path}.broken-{day}-{n}");
        n += 1;
    }
    host.rename(path, &kept_as)?;
    Ok(kept_as)
}

/// Looks at a file before it is used: one that cannot be read, or does not fit its type, is set
/// aside; a newer one is reported. Nothing else is changed.
pub fn look_at<T: DeserializeOwned>(host: &dyn Host, path: &str, kind: &Kind) -> Option<Notice> {
    let text = host.read(path).ok()?;
    let is_sound = match parsed(&text) {
        Some(json) if schema_of(&json) > kind.schema() => return Some(Notice::Newer { file: path.to_string() }),
        Some(_) => read_as::<T>(host, path, kind).is_some(),
        None => false,
    };
    if is_sound {
        return None;
    }
    set_aside(host, path).ok().map(|kept_as| Notice::SetAside { file: path.to_string(), kept_as })
}

/// Writes a file in this version's form. A newer file is refused; an older one is copied to
/// `backup/schema-<n>/` first; one that cannot be read is set aside first.
pub fn write<T: Serialize>(host: &dyn Host, path: &str, kind: &Kind, value: &T) -> io::Result<()> {
    if let Ok(before) = host.read(path) {
        match parsed(&before) {
            Some(json) if schema_of(&json) > kind.schema() => {
                return Err(io::Error::other(format!(
                    "A newer version of the editor wrote {} (form {}, this version knows {}). It is left as it is: please update the editor.",
                    kind.what,
                    schema_of(&json),
                    kind.schema()
                )));
            }
            Some(json) if schema_of(&json) < kind.schema() => {
                let (folder, name) = split(path);
                let copy = format!("{folder}/backup/schema-{}/{name}", schema_of(&json));
                if !host.exists(&copy) {
                    host.write(&copy, &before)?;
                }
            }
            Some(_) => {}
            None => {
                set_aside(host, path)?;
            }
        }
    }
    let mut json = match serde_json::to_value(value).map_err(io::Error::other)? {
        Value::Object(json) => json,
        _ => return Err(io::Error::other("a stored file is a JSON object")),
    };
    json.insert("schema".into(), kind.schema().into());
    host.write(path, &format!("{}\n", serde_json::to_string_pretty(&json).expect("plain data")))
}
