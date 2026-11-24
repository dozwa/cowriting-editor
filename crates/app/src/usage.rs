//! Usage figures for the editor's maker, and only with the author's consent: a sign of use at each
//! start, and (asked for separately) the figures of a session when the editor closes.
//!
//! Nothing here is derived from who the author is: an installation is a random number made when
//! they agreed, and deleted when they take both consents back. What is sent is built in this file
//! and nowhere else, from counts the editor keeps in memory; the same text is shown in Settings.
//! Without a server's address, or while the privacy statement still has parts to fill in, the
//! editor does not ask and does not send.

use crate::diagnostics::name_only;
use crate::preferences::Preferences;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

/// Where the figures go. Empty: the editor asks for nothing and sends nothing.
pub const SERVER: &str = "";

const STATEMENT_EN: &str = include_str!("../../../docs/PRIVACY.md");
const STATEMENT_DE: &str = include_str!("../../../docs/DATENSCHUTZ.md");

/// The privacy statement, in German or in English.
pub fn statement(language: &str) -> &'static str {
    if language == "de" {
        STATEMENT_DE
    } else {
        STATEMENT_EN
    }
}

/// A statement with a part still to fill in (`[[…]]`) is a draft: nobody is asked to agree to it.
pub fn is_complete(statement: &str) -> bool {
    !statement.contains("[[")
}

/// The server the figures go to, if there is one and the statement is in force. `trial` is an
/// address for tests and trials (`COWRITING_USAGE_SERVER`); only this computer is accepted there.
pub fn server(trial: Option<&str>) -> Option<String> {
    match trial.map(str::trim) {
        Some(address) if address.starts_with("http://127.0.0.1:") || address.starts_with("http://localhost:") => Some(address.to_string()),
        Some(_) => None,
        None if !SERVER.is_empty() && is_complete(STATEMENT_EN) && is_complete(STATEMENT_DE) => Some(SERVER.to_string()),
        None => None,
    }
}

fn read(file: &Path) -> Value {
    std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

fn write(file: &Path, json: &Value) {
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let _ = std::fs::write(file, format!("{}\n", serde_json::to_string_pretty(json).expect("plain data")));
}

/// The author was asked once whether figures may be sent; they are not asked by a dialog again.
pub fn was_asked(state: &Path) -> bool {
    read(state)["usage"]["asked"] == json!(true)
}

/// The installation's number, while a consent stands.
pub fn installation(state: &Path) -> Option<String> {
    read(state)["usage"]["installation"].as_str().filter(|id| !id.is_empty()).map(String::from)
}

/// What the author decided holds from now on. A consent makes the installation's number if there
/// is none (`new_id` gives random characters); with both taken back the number, and figures that
/// still waited to be sent, are deleted.
pub fn decided(state: &Path, preferences: &Preferences, new_id: &dyn Fn() -> String) {
    let mut json = read(state);
    let mut usage = json["usage"].as_object().cloned().unwrap_or_default();
    usage.insert("asked".into(), json!(true));
    if preferences.usage_ping || preferences.usage_figures {
        if !usage.get("installation").and_then(Value::as_str).is_some_and(|id| !id.is_empty()) {
            usage.insert("installation".into(), json!(new_id()));
        }
    } else {
        usage.remove("installation");
    }
    if !preferences.usage_figures {
        usage.remove("waiting");
    }
    json["usage"] = Value::Object(usage);
    write(state, &json);
}

/// A session's figures that could not be sent when the editor closed: they wait for the next start.
pub fn keep_waiting(state: &Path, figures: &Value) {
    let mut json = read(state);
    if json["usage"].is_object() {
        json["usage"]["waiting"] = figures.clone();
        write(state, &json);
    }
}

/// The figures that waited, taken out of the file.
pub fn take_waiting(state: &Path) -> Option<Value> {
    let mut json = read(state);
    let waiting = json["usage"].as_object_mut()?.remove("waiting")?;
    write(state, &json);
    Some(waiting).filter(Value::is_object)
}

/// What the editor counts while it is open. Counts and the editor's own names only.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Counted {
    /// Requests to an AI: kind of provider → model → how many.
    requests: BTreeMap<String, BTreeMap<String, u64>>,
    /// The documents that were open, to count them; never sent.
    documents: BTreeSet<String>,
    /// The open document's words when last counted.
    words_now: Option<usize>,
    words_added: u64,
}

impl Counted {
    /// A request went to an AI. The provider's address is not part of it: 'anthropic', 'openai-compatible', …
    pub fn request(&mut self, backend: &str, model: &str) {
        let backend = if backend.starts_with("fake:") { "scripted".to_string() } else { name_only(backend) };
        *self.requests.entry(backend).or_default().entry(name_only(model)).or_default() += 1;
    }

    /// A document was opened, with this many words.
    pub fn opened(&mut self, path: &str, words: usize) {
        self.documents.insert(path.to_string());
        self.words_now = Some(words);
    }

    /// The open document was saved with this many words: what came on top since the last count is added.
    pub fn saved(&mut self, words: usize) {
        if let Some(before) = self.words_now {
            self.words_added += words.saturating_sub(before) as u64;
        }
        self.words_now = Some(words);
    }
}

/// Who sends: the installation's number, the editor's version and the kind of system.
#[derive(Debug, Clone, Serialize)]
pub struct Sender {
    pub installation: String,
    pub version: String,
    pub system: String,
}

/// The sign of use at start, exactly as it is sent.
pub fn start_message(sender: &Sender) -> Value {
    json!({ "kind": "start", "installation": sender.installation, "version": sender.version, "system": sender.system })
}

/// A session's figures, exactly as they are sent.
pub fn session_message(sender: &Sender, counted: &Counted, open_for: Duration) -> Value {
    json!({
        "kind": "session",
        "installation": sender.installation,
        "version": sender.version,
        "system": sender.system,
        "minutes": open_for.as_secs() / 60,
        "aiRequests": counted.requests.values().flat_map(BTreeMap::values).sum::<u64>(),
        "aiRequestsBy": counted.requests,
        "documents": counted.documents.len(),
        "wordsAdded": counted.words_added,
    })
}

/// Hands a message to the server; it waits a few seconds at most, since the editor is starting or closing.
pub async fn send(server: &str, message: &Value) -> Result<(), String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(4)).build().map_err(|e| e.to_string())?;
    let reply = client.post(server).json(message).send().await.map_err(|e| format!("The figures could not be sent: {}", e.without_url()))?;
    if reply.status().is_success() {
        Ok(())
    } else {
        Err(format!("The server answered {}.", reply.status().as_u16()))
    }
}
