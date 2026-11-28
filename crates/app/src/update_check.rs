//! Whether a newer version of the editor exists: one question to a fixed address, and what comes back.
//!
//! The request is a plain GET of a fixed address. Nothing goes with it: no version, no number of
//! the installation, nothing about the system. The address is a file in the project's repository
//! on GitHub (the branch `updates`, written by `.github/workflows/updates.yml` when a release is
//! published), so no server of the maker's is asked. The answer is the file `latest.json` a release is published with (the form Tauri's updater
//! reads): the newest version, what changed, and where its files are on GitHub with their
//! signatures. Whether the answer is believed is not decided here: an update is installed only if
//! its file carries the signature of the editor's own key (the updater checks that), and only
//! from the project's releases on GitHub (`files_are_ours`).
//!
//! Without a server's address, and in a dev version, the editor does not ask.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// Where the editor asks. Empty: it does not ask.
pub const SERVER: &str = "https://raw.githubusercontent.com/dozwa/cowriting-editor/updates/latest.json";

/// The page the releases are on, for an author who gets the installer by hand.
pub const RELEASES: &str = "https://github.com/dozwa/cowriting-editor/releases";

/// Where a release's files are. A file from anywhere else is not fetched, whatever the server says.
const FILES: &str = "https://github.com/dozwa/cowriting-editor/releases/download/";

/// An answer longer than this is not read: a release's notes are a page or two.
const LONGEST_ANSWER: usize = 400_000;
const LONGEST_NOTES: usize = 40_000;

/// The address to ask at, if the editor asks at all. `trial` is an address for tests and trials
/// (`COWRITING_UPDATE_SERVER`); only this computer is accepted there. A dev version is not a
/// release and is told about none.
pub fn server(trial: Option<&str>, version: &str) -> Option<String> {
    match trial.map(str::trim) {
        Some(address) if address.starts_with("http://127.0.0.1:") || address.starts_with("http://localhost:") => Some(address.to_string()),
        Some(_) => None,
        None if !SERVER.is_empty() && !version.ends_with("-dev") => Some(SERVER.to_string()),
        None => None,
    }
}

/// A release newer than the editor that asked.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    /// What changed, as the release says it (markdown).
    pub notes: String,
    /// The day it was published, `YYYY-MM-DD`; empty when the answer does not say.
    pub date: String,
    /// The server's answer as it came: the updater installs from it.
    #[serde(skip)]
    pub answer: Value,
}

fn version_of(text: &str) -> Option<semver::Version> {
    semver::Version::parse(text.trim().trim_start_matches('v')).ok()
}

/// Whether `other` is a newer version than `current`: `0.1.1-alpha` is older than `0.1.2-alpha` and than `0.1.1`.
pub fn is_newer(other: &str, current: &str) -> bool {
    matches!((version_of(other), version_of(current)), (Some(other), Some(current)) if other > current)
}

/// Reads the server's answer. `None` when the version it names is not newer than `current`.
pub fn read(answer: &str, current: &str) -> Result<Option<Release>, String> {
    let unreadable = || "The server's answer could not be read.".to_string();
    let json = serde_json::from_str::<Value>(answer.trim_start_matches('\u{feff}')).map_err(|_| unreadable())?;
    let version = json["version"].as_str().and_then(version_of).ok_or_else(unreadable)?;
    if version_of(current).is_none_or(|current| version <= current) {
        return Ok(None);
    }
    let notes = json["notes"].as_str().unwrap_or_default().trim().chars().take(LONGEST_NOTES).collect();
    let date = json["pub_date"].as_str().unwrap_or_default().chars().take(10).filter(|c| c.is_ascii_digit() || *c == '-').collect::<String>();
    Ok(Some(Release { version: version.to_string(), notes, date: if date.len() == 10 { date } else { String::new() }, answer: json }))
}

/// Asks the server and gives its answer as text. It waits a few seconds at most.
pub async fn ask(server: &str) -> Result<String, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().map_err(|e| e.to_string())?;
    let reply = client.get(server).header("accept", "application/json").send().await.map_err(|e| format!("The server could not be reached: {}", e.without_url()))?;
    if !reply.status().is_success() {
        return Err(format!("The server answered {}.", reply.status().as_u16()));
    }
    let bytes = reply.bytes().await.map_err(|e| format!("The server's answer did not arrive: {}", e.without_url()))?;
    if bytes.len() > LONGEST_ANSWER {
        return Err("The server's answer is too long to be a release.".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Asks whether there is a release newer than `current`.
pub async fn check(server: &str, current: &str) -> Result<Option<Release>, String> {
    read(&ask(server).await?, current)
}

/// Whether every file the answer names is one of the project's releases on GitHub. The updater
/// fetches nothing from an answer that names another place.
pub fn files_are_ours(answer: &Value) -> bool {
    let is_ours = |file: &Value| file["url"].as_str().is_some_and(|url| url.starts_with(FILES) && !url.contains("/../"));
    match answer["platforms"].as_object() {
        Some(platforms) => !platforms.is_empty() && platforms.values().all(is_ours),
        None => is_ours(answer),
    }
}

fn state(file: &Path) -> Value {
    crate::preferences::read_json(file, &cowriting_core::stored::STATE)
}

/// Whether the author was told about this version before: the notice comes by itself once for each.
pub fn was_told(state_file: &Path, version: &str) -> bool {
    state(state_file)["update"]["told"].as_str() == Some(version)
}

pub fn told(state_file: &Path, version: &str) {
    let mut json = state(state_file);
    json["update"] = json!({ "told": version });
    let _ = crate::preferences::write_json(state_file, &cowriting_core::stored::STATE, &json);
}
