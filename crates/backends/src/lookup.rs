//! Asking the world about references: Crossref for a DOI or a title, and the author's own Zotero
//! on this computer. Follows the plugin's `plugin/tools/verify_refs.py` and `zotero_search.py`.
//! What is sent to Crossref: the DOI, or title, first author and year of a reference. Nothing of
//! the manuscript, and nothing that names the author.

use async_trait::async_trait;
use cowriting_core::sources::{author_line, Hit, Record, References};
use serde_json::Value;
use std::time::Duration;

const CROSSREF: &str = "https://api.crossref.org/works";
const ZOTERO: &str = "http://localhost:23119/api/users/0/items";

/// A DOI as part of a URL path: everything but letters, digits and a few safe marks is escaped.
fn quoted(doi: &str) -> String {
    let mut out = String::new();
    for byte in doi.bytes() {
        if byte.is_ascii_alphanumeric() || b"/:;()_.-~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// A Crossref work as the check needs it.
pub fn parse_work(message: &Value) -> Record {
    let year = message["issued"]["date-parts"][0][0].as_i64().map(|year| year.to_string()).unwrap_or_default();
    Record { doi: message["DOI"].as_str().unwrap_or("").to_string(), title: message["title"][0].as_str().unwrap_or("").to_string(), year }
}

pub struct Crossref {
    http: reqwest::Client,
    base_url: String,
    /// Between two requests, to be polite.
    pause: Duration,
}

impl Default for Crossref {
    fn default() -> Self {
        Crossref::at(CROSSREF, Duration::from_millis(200))
    }
}

impl Crossref {
    pub fn at(base_url: &str, pause: Duration) -> Crossref {
        let agent = format!("CoWriting-Editor/{}", env!("CARGO_PKG_VERSION"));
        let http = reqwest::Client::builder().user_agent(agent).timeout(Duration::from_secs(20)).build().expect("a plain HTTP client");
        Crossref { http, base_url: base_url.trim_end_matches('/').to_string(), pause }
    }

    /// The `message` of an answer; None for a 404.
    async fn get(&self, request: reqwest::RequestBuilder) -> Result<Option<Value>, String> {
        let response = request.send().await.map_err(|e| format!("Crossref could not be reached: {}", e.without_url()))?;
        tokio::time::sleep(self.pause).await;
        match response.status().as_u16() {
            404 => Ok(None),
            200 => {
                let body: Value = response.json().await.map_err(|_| "Crossref's answer could not be read".to_string())?;
                Ok(Some(body["message"].clone()))
            }
            status => Err(format!("Crossref answered with status {status}")),
        }
    }
}

#[async_trait]
impl References for Crossref {
    async fn work(&self, doi: &str) -> Result<Option<Record>, String> {
        Ok(self.get(self.http.get(format!("{}/{}", self.base_url, quoted(doi)))).await?.map(|message| parse_work(&message)))
    }

    async fn candidates(&self, query: &str) -> Result<Vec<Record>, String> {
        let request = self.http.get(&self.base_url).query(&[("query.bibliographic", query), ("rows", "3"), ("select", "DOI,title,issued")]);
        let message = self.get(request).await?.unwrap_or(Value::Null);
        Ok(message["items"].as_array().map(|items| items.iter().map(parse_work).collect()).unwrap_or_default())
    }
}

/// The items of a Zotero local-API answer as search hits.
pub fn parse_zotero(items: &Value) -> Vec<Hit> {
    let Some(items) = items.as_array() else { return Vec::new() };
    let text = |value: &Value| value.as_str().unwrap_or("").to_string();
    items
        .iter()
        .map(|item| {
            let data = &item["data"];
            let creators: Vec<String> = data["creators"]
                .as_array()
                .map(|creators| creators.iter().map(|c| c["lastName"].as_str().filter(|s| !s.is_empty()).or(c["name"].as_str()).unwrap_or("").to_string()).collect())
                .unwrap_or_default();
            // Better BibTeX keeps its key in a field of its own, or in a line of `extra`
            let in_extra = data["extra"].as_str().and_then(|extra| extra.split("Citation Key:").nth(1)).and_then(|rest| rest.split_whitespace().next()).map(String::from);
            Hit {
                citekey: data["citationKey"].as_str().filter(|key| !key.is_empty()).map(String::from).or(in_extra),
                title: text(&data["title"]),
                authors: author_line(&creators),
                year: text(&data["date"]).chars().take(4).collect(),
                doi: text(&data["DOI"]),
                file: String::new(),
            }
        })
        .collect()
}

/// Searches the author's running Zotero (its local API, on this computer only). None when Zotero
/// is not running or does not let other applications talk to it.
pub async fn zotero_search(query: &str, limit: usize) -> Option<Vec<Hit>> {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().ok()?;
    let limit = limit.to_string();
    let request = http.get(ZOTERO).query(&[("q", query), ("qmode", "titleCreatorYear"), ("limit", &limit), ("itemType", "-attachment || note")]);
    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    Some(parse_zotero(&response.json::<Value>().await.ok()?))
}

// ------------------------------------------------------------ Better BibTeX

const BETTER_BIBTEX: &str = "http://localhost:23119/better-bibtex/json-rpc";

/// Why Better BibTeX gave no result.
enum RpcError {
    /// Zotero is not running, or Better BibTeX is not installed in it.
    Unreachable,
    /// Better BibTeX answered with an error.
    Refused(String),
}

async fn better_bibtex(method: &str, params: Value) -> Result<Value, RpcError> {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().map_err(|_| RpcError::Unreachable)?;
    let call = serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": 1 });
    let response = http.post(BETTER_BIBTEX).json(&call).send().await.map_err(|_| RpcError::Unreachable)?;
    if !response.status().is_success() {
        return Err(RpcError::Unreachable);
    }
    let answer: Value = response.json().await.map_err(|_| RpcError::Unreachable)?;
    match answer["error"]["message"].as_str() {
        Some(message) => Err(RpcError::Refused(message.to_string())),
        None => Ok(answer["result"].clone()),
    }
}

/// Is Zotero running with Better BibTeX, so that citekeys can be looked up?
pub async fn has_better_bibtex() -> bool {
    better_bibtex("api.ready", Value::Array(Vec::new())).await.is_ok()
}

/// Better BibTeX's search results (CSL items with a `citekey`) as hits.
pub fn parse_better_bibtex(items: &Value, limit: usize) -> Vec<Hit> {
    let Some(items) = items.as_array() else { return Vec::new() };
    let text = |value: &Value| value.as_str().unwrap_or("").to_string();
    let hit = |item: &Value| {
        let names: Vec<String> =
            item["author"].as_array().map(|authors| authors.iter().filter_map(|a| a["family"].as_str().or(a["literal"].as_str()).map(String::from)).collect()).unwrap_or_default();
        let key = item["citekey"].as_str().or(item["citation-key"].as_str()).filter(|key| !key.is_empty());
        Hit { citekey: key.map(String::from), title: text(&item["title"]), authors: author_line(&names), year: parse_work(item).year, doi: text(&item["DOI"]), file: String::new() }
    };
    items.iter().take(limit).map(hit).collect()
}

/// Searches the author's Zotero through Better BibTeX, which knows every item's citekey. None when it is not there.
pub async fn better_bibtex_search(query: &str, limit: usize) -> Option<Vec<Hit>> {
    let items = better_bibtex("item.search", serde_json::json!([query])).await.ok()?;
    Some(parse_better_bibtex(&items, limit))
}

/// The keys Better BibTeX says it does not have ("not found: a, b"); empty when the message is about something else.
pub fn keys_not_found(message: &str) -> Vec<String> {
    message.strip_prefix("not found: ").map(|keys| keys.split(',').map(|key| key.trim().to_string()).filter(|key| !key.is_empty()).collect()).unwrap_or_default()
}

/// The references behind these citekeys as CSL JSON, from the author's Zotero, and the keys Zotero does not have.
/// None when Zotero with Better BibTeX is not reachable.
pub async fn better_bibtex_export(keys: &[String]) -> Option<(String, Vec<String>)> {
    let (mut wanted, mut unknown) = (keys.to_vec(), Vec::new());
    // Better BibTeX refuses the whole export when it lacks a key, and names the ones it lacks: ask again without them
    for _ in 0..=keys.len() {
        if wanted.is_empty() {
            return has_better_bibtex().await.then(|| ("[]".to_string(), unknown));
        }
        match better_bibtex("item.export", serde_json::json!([wanted, "Better CSL JSON"])).await {
            Ok(Value::String(csl)) => return Some((csl, unknown)),
            Ok(_) | Err(RpcError::Unreachable) => return None,
            Err(RpcError::Refused(message)) => {
                let lacking = keys_not_found(&message);
                let before = wanted.len();
                wanted.retain(|known| !lacking.contains(known));
                if wanted.len() == before {
                    return None;
                }
                unknown.extend(lacking);
            }
        }
    }
    None
}
