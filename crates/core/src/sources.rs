//! The author's bibliography, whatever its format, and what a reference check makes of it.
//! Ported from the plugin's `plugin/tools/bibliography.py` (BibTeX, CSL JSON, RIS, MEDLINE),
//! `zotero_search.py` (`from_bib`) and the judgement in `verify_refs.py`. Nothing here touches
//! the network: asking Crossref or Zotero is the backends crate's part.

use crate::text::{py_splitlines, py_strip};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

/// One reference, the same whatever file it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Entry {
    /// The citekey where the format has one; else `pmid:<n>` or `<family><year>`, with `derived` set.
    pub key: String,
    pub derived: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    /// Family names.
    pub authors: Vec<String>,
    pub year: String,
    pub doi: String,
    /// Everything in the record, for searching.
    pub text: String,
    pub file: String,
}

/// The format a file name stands for, or None when it is no bibliography this reads.
pub fn format_name(file: &str) -> Option<&'static str> {
    let lower = file.to_lowercase();
    [(".bib", "BibTeX"), (".bibtex", "BibTeX"), (".json", "CSL JSON"), (".ris", "RIS"), (".nbib", "MEDLINE")]
        .iter()
        .find(|(extension, _)| lower.ends_with(extension))
        .map(|(_, name)| *name)
}

static DOI_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://(dx\.)?doi\.org/").unwrap());
static DOI_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^doi:\s*").unwrap());

pub fn clean_doi(doi: &str) -> String {
    let doi = py_strip(doi);
    let doi = DOI_URL.replace(doi, "");
    let doi = DOI_PREFIX.replace(&doi, "");
    doi.trim_matches([' ', '.', ';', ',']).to_string()
}

static LATEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\[a-zA-Z]+|[{}$\\]").unwrap());
static NON_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\W+").unwrap());

/// A title without LaTeX, braces and punctuation, for comparisons.
pub fn norm(s: &str) -> String {
    let s = LATEX.replace_all(s, "");
    py_strip(&NON_WORD.replace_all(&s, " ")).to_lowercase()
}

/// The family name of 'Family, Given', 'Given Family' or '{Some Institution}'.
pub fn family(name: &str) -> String {
    let name = name.replace(['{', '}'], "");
    let name = py_strip(&name);
    match name.split_once(',') {
        Some((family, _)) => py_strip(family).to_string(),
        None => name.split_whitespace().last().unwrap_or("").to_string(),
    }
}

fn first_chars(s: &str, count: usize) -> String {
    s.chars().take(count).collect()
}

fn entry(mut e: Entry) -> Entry {
    e.doi = clean_doi(&e.doi);
    e.year = first_chars(&e.year, 4);
    e
}

fn derived_key(authors: &[String], year: &str) -> String {
    let stem: String = authors.first().map_or("anon", String::as_str).to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect();
    format!("{}{year}", if stem.is_empty() { "anon" } else { &stem })
}

/// Derived keys that collide within one file get b, c, … appended.
fn unique_keys(mut entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen: HashMap<String, u32> = HashMap::new();
    for e in entries.iter_mut().filter(|e| e.derived) {
        let n = seen.entry(e.key.clone()).or_insert(0);
        if *n > 0 {
            e.key.push(char::from_u32('a' as u32 + *n).unwrap_or('z'));
        }
        *n += 1;
    }
    entries
}

// BibTeX ---------------------------------------------------------------------

static BIB_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@(\w+)\s*\{\s*([^,\s{}]+)\s*,").unwrap());
static AND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+and\s+").unwrap());

/// Every entry as (kind, key, body), wherever its closing brace is.
fn bib_entries(text: &str) -> Vec<(String, String, String)> {
    let (bytes, mut out, mut at) = (text.as_bytes(), Vec::new(), 0);
    while let Some(start) = BIB_START.captures_at(text, at) {
        let (mut depth, mut i) = (1, start.get(0).unwrap().end());
        let from = i;
        while i < bytes.len() && depth > 0 {
            depth += match bytes[i] {
                b'{' => 1,
                b'}' => -1,
                _ => 0,
            };
            i += 1;
        }
        out.push((start[1].to_string(), start[2].to_string(), text[from..if depth == 0 { i - 1 } else { i }].to_string()));
        at = i;
    }
    out
}

/// A field of a BibTeX entry: braced, quoted or bare.
pub fn bib_field(body: &str, name: &str) -> Option<String> {
    let found = Regex::new(&format!(r"(?i)\b{name}\s*=\s*")).ok()?.find(body)?;
    let rest = &body[found.end()..];
    if rest.starts_with('{') {
        let (mut depth, mut out) = (0, String::new());
        for c in rest.chars() {
            if c == '{' {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            out.push(c);
        }
        return Some(out);
    }
    if let Some(quoted) = rest.strip_prefix('"') {
        return Some(quoted.split('"').next().unwrap_or("").to_string());
    }
    Some(py_strip(rest.split([',', '\n']).next().unwrap_or("")).to_string())
}

pub fn load_bibtex(text: &str, file: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for (kind, key, body) in bib_entries(text) {
        let kind = kind.to_lowercase();
        if ["comment", "string", "preamble"].contains(&kind.as_str()) {
            continue;
        }
        let field = |name: &str| bib_field(&body, name).filter(|value| !value.is_empty());
        let authors = AND.split(&field("author").unwrap_or_default()).filter(|a| !py_strip(a).is_empty()).map(family).collect();
        out.push(entry(Entry {
            key,
            kind,
            title: field("title").unwrap_or_default(),
            authors,
            year: field("year").or_else(|| field("date")).unwrap_or_default(),
            doi: field("doi").unwrap_or_default(),
            text: body.clone(),
            file: file.to_string(),
            ..Default::default()
        }));
    }
    out
}

// CSL JSON -------------------------------------------------------------------

/// A JSON value as Python's `str()` writes it; nothing for null.
fn plain(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        other => other.to_string(),
    }
}

fn strings(value: &Value) -> String {
    match value {
        Value::Object(fields) => fields.values().map(strings).collect::<Vec<_>>().join(" "),
        Value::Array(items) => items.iter().map(strings).collect::<Vec<_>>().join(" "),
        other => plain(other),
    }
}

pub fn load_csl_json(text: &str, file: &str) -> Vec<Entry> {
    let Ok(mut items) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else { return Vec::new() };
    if let Some(inner) = items.get("items").filter(|inner| inner.is_array()) {
        items = inner.clone(); // the Zotero API's shape
    }
    let Some(items) = items.as_array().filter(|items| items.iter().all(Value::is_object)) else { return Vec::new() };
    let mut out = Vec::new();
    for item in items {
        if !["title", "id", "DOI"].iter().any(|key| item.get(key).is_some()) {
            continue;
        }
        let text_of = |value: Option<&Value>| value.and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
        let authors = item
            .get("author")
            .and_then(Value::as_array)
            .map(|authors| authors.iter().filter_map(|a| text_of(a.get("family")).or_else(|| text_of(a.get("literal"))).or_else(|| text_of(a.get("name")))).collect())
            .unwrap_or_default();
        let issued = item.get("issued");
        let first_part = issued
            .and_then(|i| i.get("date-parts"))
            .and_then(|parts| parts.get(0))
            .and_then(|part| part.get(0))
            .filter(|year| !year.is_null() && year.as_i64() != Some(0) && year.as_str() != Some(""));
        let year = first_part.map(plain).or_else(|| text_of(issued.and_then(|i| i.get("raw")))).or_else(|| text_of(issued.and_then(|i| i.get("literal")))).unwrap_or_default();
        let field = |key: &str| item.get(key).map(plain).unwrap_or_default();
        out.push(entry(Entry {
            key: field("id"),
            kind: field("type"),
            title: field("title"),
            authors,
            year,
            doi: field("DOI"),
            text: strings(item),
            file: file.to_string(),
            ..Default::default()
        }));
    }
    out
}

// RIS ------------------------------------------------------------------------

static RIS_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z][A-Z0-9])  - ?(.*)$").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]{4}").unwrap());
static DOI_IN_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"10\.[0-9]+/\S+").unwrap());

/// A record's tags in the order they first appeared, each with its values.
type Tags = Vec<(String, Vec<String>)>;

fn add(tags: &mut Tags, tag: &str, value: &str) {
    match tags.iter_mut().find(|(name, _)| name == tag) {
        Some((_, values)) => values.push(value.to_string()),
        None => tags.push((tag.to_string(), vec![value.to_string()])),
    }
}

fn values<'a>(tags: &'a Tags, tag: &str) -> &'a [String] {
    tags.iter().find(|(name, _)| name == tag).map_or(&[], |(_, values)| values.as_slice())
}

/// The first value of the first of these tags that has any.
fn first(tags: &Tags, names: &[&str]) -> String {
    names.iter().find_map(|name| values(tags, name).first().cloned()).unwrap_or_default()
}

fn year_of(text: &str) -> String {
    YEAR.find(text).map_or(String::new(), |found| found.as_str().to_string())
}

pub fn load_ris(text: &str, file: &str) -> Vec<Entry> {
    let (mut out, mut tags, mut lines): (Vec<Entry>, Tags, Vec<String>) = (Vec::new(), Vec::new(), Vec::new());
    for raw in py_splitlines(text.trim_start_matches('\u{feff}')) {
        let Some(found) = RIS_LINE.captures(raw.trim_matches('\u{feff}')) else {
            // a line that goes on: kept for searching, not part of the tag's value
            if let Some(last) = lines.last_mut().filter(|_| !py_strip(raw).is_empty()) {
                last.push_str(&format!(" {}", py_strip(raw)));
            }
            continue;
        };
        let (tag, value) = (&found[1], py_strip(&found[2]));
        lines.push(format!("{tag} {value}"));
        if tag == "ER" {
            out.push(ris_entry(&tags, &lines, file));
            (tags, lines) = (Vec::new(), Vec::new());
            continue;
        }
        add(&mut tags, tag, value);
    }
    if !tags.is_empty() {
        out.push(ris_entry(&tags, &lines, file));
    }
    unique_keys(out)
}

fn ris_entry(tags: &Tags, lines: &[String], file: &str) -> Entry {
    let named = if values(tags, "AU").is_empty() { values(tags, "A1") } else { values(tags, "AU") };
    let authors: Vec<String> = named.iter().map(|a| family(a)).collect();
    let year = year_of(&first(tags, &["PY", "Y1", "DA"]));
    let mut doi = first(tags, &["DO"]);
    if doi.is_empty() {
        let elsewhere = ["UR", "M3", "N1"].iter().flat_map(|tag| values(tags, tag));
        doi = elsewhere.filter(|v| v.to_lowercase().contains("doi")).find_map(|v| DOI_IN_TEXT.find(v).map(|m| m.as_str().to_string())).unwrap_or_default();
    }
    let id = first(tags, &["ID"]);
    let (key, derived) = if id.is_empty() { (derived_key(&authors, &year), true) } else { (id, false) };
    entry(Entry {
        key,
        derived,
        kind: first(tags, &["TY"]).to_lowercase(),
        title: first(tags, &["TI", "T1", "BT", "CT"]),
        authors,
        year,
        doi,
        text: lines.join("\n"),
        file: file.to_string(),
    })
}

// MEDLINE / PubMed -----------------------------------------------------------

static MEDLINE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z]{2,4}) *- (.*)$").unwrap());

pub fn load_medline(text: &str, file: &str) -> Vec<Entry> {
    let (mut out, mut tags, mut lines): (Vec<Entry>, Tags, Vec<String>) = (Vec::new(), Vec::new(), Vec::new());
    for raw in py_splitlines(text.trim_start_matches('\u{feff}')) {
        if py_strip(raw).is_empty() {
            if !tags.is_empty() {
                out.push(medline_entry(&tags, &lines, file));
            }
            (tags, lines) = (Vec::new(), Vec::new());
        } else if let Some(found) = MEDLINE_LINE.captures(raw) {
            let (tag, value) = (&found[1], py_strip(&found[2]));
            add(&mut tags, tag, value);
            lines.push(format!("{tag} {value}"));
        } else if raw.starts_with("      ") {
            // a line that goes on: the plugin adds it to the tag that appeared last for the first time
            if let Some(value) = tags.last_mut().and_then(|(_, values)| values.last_mut()) {
                value.push_str(&format!(" {}", py_strip(raw)));
                if let Some(last) = lines.last_mut() {
                    last.push_str(&format!(" {}", py_strip(raw)));
                }
            }
        }
    }
    if !tags.is_empty() {
        out.push(medline_entry(&tags, &lines, file));
    }
    unique_keys(out)
}

fn medline_entry(tags: &Tags, lines: &[String], file: &str) -> Entry {
    let named = if values(tags, "FAU").is_empty() { values(tags, "AU") } else { values(tags, "FAU") };
    let authors: Vec<String> = named.iter().map(|a| family(a)).collect();
    let ids = values(tags, "AID").iter().chain(values(tags, "LID"));
    let doi = ids.filter_map(|v| v.strip_suffix("[doi]")).next().map(|doi| py_strip(doi).to_string()).unwrap_or_default();
    let year = year_of(&first(tags, &["DP"]));
    let pmid = first(tags, &["PMID"]);
    let key = if pmid.is_empty() { derived_key(&authors, &year) } else { format!("pmid:{pmid}") };
    entry(Entry { key, derived: true, kind: first(tags, &["PT"]).to_lowercase(), title: first(tags, &["TI"]), authors, year, doi, text: lines.join("\n"), file: file.to_string() })
}

/// The entries of one bibliography file, by its extension; None when the format is not one this reads.
pub fn load(file: &str, text: &str) -> Option<Vec<Entry>> {
    Some(match format_name(file)? {
        "BibTeX" => load_bibtex(text.trim_start_matches('\u{feff}'), file),
        "CSL JSON" => load_csl_json(text, file),
        "RIS" => load_ris(text, file),
        _ => load_medline(text, file),
    })
}

// ------------------------------------------------------------ searching

/// A reference as a search shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Hit {
    /// None when the file carries no citekey for it.
    pub citekey: Option<String>,
    pub title: String,
    /// Up to three family names, then "et al.".
    pub authors: String,
    pub year: String,
    pub doi: String,
    /// The bibliography file it is in; empty for a hit from Zotero.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file: String,
}

/// Up to three names, then "et al.".
pub fn author_line(names: &[String]) -> String {
    format!("{}{}", names.iter().take(3).cloned().collect::<Vec<_>>().join(", "), if names.len() > 3 { " et al." } else { "" })
}

/// The entries that hold every word of the query, anywhere in the record.
pub fn search(entries: &[Entry], query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let holds = |e: &&Entry| {
        let text = e.text.to_lowercase();
        words.iter().all(|w| text.contains(w))
    };
    let hit = |e: &Entry| Hit {
        citekey: (!e.derived).then(|| e.key.clone()),
        title: e.title.replace(['{', '}'], ""),
        authors: author_line(&e.authors),
        year: e.year.clone(),
        doi: e.doi.clone(),
        file: e.file.clone(),
    };
    entries.iter().filter(holds).take(limit).map(hit).collect()
}

// ------------------------------------------------------------ comparing titles

/// Python's `difflib.SequenceMatcher(None, a, b).ratio()`: twice the matching characters over all characters.
pub fn ratio(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut positions: HashMap<char, Vec<usize>> = HashMap::new();
    for (j, c) in b.iter().enumerate() {
        positions.entry(*c).or_default().push(j);
    }
    // in a long text, a character that makes up more than 1 % is too common to anchor a match
    if b.len() >= 200 {
        let limit = b.len() / 100 + 1;
        positions.retain(|_, at| at.len() <= limit);
    }
    let mut matched = 0;
    let mut todo = vec![(0, a.len(), 0, b.len())];
    while let Some((alo, ahi, blo, bhi)) = todo.pop() {
        let (mut best_i, mut best_j, mut best) = (alo, blo, 0);
        let mut runs: HashMap<usize, usize> = HashMap::new();
        for (i, c) in a.iter().enumerate().take(ahi).skip(alo) {
            let mut next: HashMap<usize, usize> = HashMap::new();
            for &j in positions.get(c).map_or(&[][..], Vec::as_slice) {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let k = if j == 0 { 1 } else { runs.get(&(j - 1)).copied().unwrap_or(0) + 1 };
                next.insert(j, k);
                if k > best {
                    (best_i, best_j, best) = (i + 1 - k, j + 1 - k, k);
                }
            }
            runs = next;
        }
        // a match grows over equal characters on both sides, common ones included
        while best_i > alo && best_j > blo && a[best_i - 1] == b[best_j - 1] {
            (best_i, best_j, best) = (best_i - 1, best_j - 1, best + 1);
        }
        while best_i + best < ahi && best_j + best < bhi && a[best_i + best] == b[best_j + best] {
            best += 1;
        }
        if best > 0 {
            matched += best;
            if alo < best_i && blo < best_j {
                todo.push((alo, best_i, blo, best_j));
            }
            if best_i + best < ahi && best_j + best < bhi {
                todo.push((best_i + best, ahi, best_j + best, bhi));
            }
        }
    }
    2.0 * matched as f64 / (a.len() + b.len()) as f64
}

/// Are two titles the same work's, give or take punctuation and a word?
pub fn similar(a: &str, b: &str) -> bool {
    ratio(&norm(a), &norm(b)) >= 0.85
}

// ------------------------------------------------------------ the reference check

/// What Crossref holds for a DOI, as far as the check needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Record {
    pub doi: String,
    pub title: String,
    pub year: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The DOI resolves and title and year agree.
    Ok,
    /// The DOI resolves to a different title or year.
    Mismatch,
    /// Crossref has no record for the DOI.
    NotFound,
    /// The entry has none.
    NoDoi,
    /// Network trouble.
    Error,
}

fn agrees(e: &Entry, remote: &Record) -> bool {
    similar(&e.title, &remote.title) && (e.year.is_empty() || remote.year.is_empty() || e.year == remote.year)
}

/// The verdict on an entry that has a DOI, from what Crossref holds for it (None: no record).
pub fn judge(e: &Entry, remote: Option<&Record>) -> Status {
    match remote {
        None => Status::NotFound,
        Some(remote) if agrees(e, remote) => Status::Ok,
        Some(_) => Status::Mismatch,
    }
}

/// What Crossref is asked for an entry without a DOI; None when the title is too short to tell works apart.
pub fn lookup_query(e: &Entry) -> Option<String> {
    if norm(&e.title).chars().count() < 15 {
        return None;
    }
    let author = e.authors.first().map_or("", String::as_str);
    Some([e.title.as_str(), author, e.year.as_str()].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" "))
}

/// The first of Crossref's candidates whose title is close and whose year agrees: a suggestion, never a verification.
pub fn pick<'a>(e: &Entry, candidates: &'a [Record]) -> Option<&'a Record> {
    candidates.iter().find(|candidate| agrees(e, candidate))
}

// ------------------------------------------------------------ the document's bibliography

use crate::host::Host;
use crate::sidecar::Sidecar;
use std::collections::BTreeSet;

/// A bibliography file of the document, read or not.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibFile {
    pub path: String,
    pub name: String,
    /// 'BibTeX', 'CSL JSON', 'RIS' or 'MEDLINE'; empty when the extension is none of theirs.
    pub format: String,
    pub entries: usize,
    /// True when the document's front matter names it (`bibliography:`); false when the author added it in the editor.
    pub is_from_document: bool,
    /// Why it could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bibliography {
    pub files: Vec<BibFile>,
    pub entries: Vec<Entry>,
}

static FRONT_MATTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n(?:---|\.\.\.)\s*(?:\r?\n|\z)").unwrap());

/// The files the document's front matter names as its bibliography (pandoc's `bibliography:`, one file or a list).
pub fn front_matter_files(document_text: &str) -> Vec<String> {
    let Some(front) = FRONT_MATTER.captures(document_text.trim_start_matches('\u{feff}')) else { return Vec::new() };
    let unquote = |value: &str| value.trim().trim_matches(['"', '\'']).trim().to_string();
    let lines: Vec<&str> = front[1].lines().collect();
    let Some(at) = lines.iter().position(|line| line.starts_with("bibliography:")) else { return Vec::new() };
    let value = lines[at]["bibliography:".len()..].trim();
    let files: Vec<String> = if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        list.split(',').map(unquote).collect()
    } else if !value.is_empty() {
        vec![unquote(value)]
    } else {
        lines[at + 1..].iter().take_while(|line| line.trim_start().starts_with("- ")).map(|line| unquote(&line.trim_start()[2..])).collect()
    };
    files.into_iter().filter(|file| !file.is_empty()).collect()
}

fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || path.as_bytes().get(1) == Some(&b':')
}

/// The files the author added in the editor (`sources.json` in the sidecar), as full paths.
pub fn added_files(host: &dyn Host, sidecar: &Sidecar) -> Vec<String> {
    sources_json(host, sidecar).get("files").and_then(|files| serde_json::from_value(files.clone()).ok()).unwrap_or_default()
}

/// `sources.json` as it is, to change one thing in it and keep the rest.
fn sources_json(host: &dyn Host, sidecar: &Sidecar) -> Value {
    Value::Object(crate::stored::read(host, &sidecar.sources_json(), &crate::stored::SOURCES).unwrap_or_default())
}

fn save_sources_json(host: &dyn Host, sidecar: &Sidecar, json: &Value) -> std::io::Result<()> {
    crate::stored::write(host, &sidecar.sources_json(), &crate::stored::SOURCES, json)
}

pub fn save_added_files(host: &dyn Host, sidecar: &Sidecar, files: &[String]) -> std::io::Result<()> {
    let mut json = sources_json(host, sidecar);
    json["files"] = serde_json::json!(files);
    save_sources_json(host, sidecar, &json)
}

/// The reference style a document is exported in: a CSL file. Read, never written.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub path: String,
    pub name: String,
    /// What the style calls itself ("American Psychological Association 7th edition"); empty when the file says nothing.
    pub title: String,
    /// True when the document's front matter names it (`csl:`); false when the author chose it in the editor.
    pub is_from_document: bool,
    /// Why it cannot be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

static STYLE_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<title>\s*(.*?)\s*</title>").unwrap());

/// What a CSL file calls itself, or why the text is not a CSL style.
pub fn style_title(text: &str) -> Result<String, String> {
    if !text.contains("<style") {
        return Err("is not a CSL style (a .csl file, as zotero.org/styles has them)".into());
    }
    let title = STYLE_TITLE.captures(text).map(|found| found[1].replace("&amp;", "&")).unwrap_or_default();
    Ok(title)
}

/// The style file the document's front matter names (pandoc's `csl:`).
pub fn front_matter_style(document_text: &str) -> Option<String> {
    let front = FRONT_MATTER.captures(document_text.trim_start_matches('\u{feff}'))?;
    let line = front[1].lines().find(|line| line.starts_with("csl:"))?;
    Some(line["csl:".len()..].trim().trim_matches(['"', '\'']).trim().to_string()).filter(|file| !file.is_empty())
}

/// The style file the author chose in the editor (`sources.json` in the sidecar), as a full path.
pub fn added_style(host: &dyn Host, sidecar: &Sidecar) -> Option<String> {
    sources_json(host, sidecar)["style"].as_str().filter(|path| !path.is_empty()).map(String::from)
}

pub fn save_added_style(host: &dyn Host, sidecar: &Sidecar, style: Option<&str>) -> std::io::Result<()> {
    let mut json = sources_json(host, sidecar);
    match style {
        Some(path) => json["style"] = serde_json::json!(path),
        None => {
            json.as_object_mut().expect("an object").remove("style");
        }
    }
    save_sources_json(host, sidecar, &json)
}

/// The document's reference style: the one its front matter names, else the one the author chose. None is pandoc's standard.
pub fn load_style(host: &dyn Host, sidecar: &Sidecar, document_text: &str) -> Option<Style> {
    let named = front_matter_style(document_text).map(|file| (if is_absolute(&file) { file } else { format!("{}/{file}", sidecar.folder) }, true));
    let (path, is_from_document) = named.or_else(|| added_style(host, sidecar).map(|path| (path, false)))?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
    let read = host.read(&path).map_err(|e| format!("could not be read: {e}")).and_then(|text| style_title(&text));
    let (title, error) = match read {
        Ok(title) => (title, None),
        Err(error) => (String::new(), Some(error)),
    };
    Some(Style { path, name, title, is_from_document, error })
}

/// The document's bibliography: the files its front matter names, then the ones the author added. Read, never written.
pub fn load_bibliography(host: &dyn Host, sidecar: &Sidecar, document_text: &str) -> Bibliography {
    let named = front_matter_files(document_text).into_iter().map(|file| (if is_absolute(&file) { file } else { format!("{}/{file}", sidecar.folder) }, true));
    let mut out = Bibliography::default();
    for (path, is_from_document) in named.chain(added_files(host, sidecar).into_iter().map(|path| (path, false))) {
        if out.files.iter().any(|known| known.path == path) {
            continue;
        }
        let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
        let format = format_name(&name).unwrap_or("").to_string();
        let read = if format.is_empty() {
            Err("not a bibliography format the editor reads (.bib, .json, .ris, .nbib)".to_string())
        } else {
            host.read(&path).map_err(|e| format!("could not be read: {e}"))
        };
        let (entries, error) = match read {
            Ok(text) => (load(&name, &text).unwrap_or_default(), None),
            Err(error) => (Vec::new(), Some(error)),
        };
        out.files.push(BibFile { path, name, format, entries: entries.len(), is_from_document, error });
        out.entries.extend(entries);
    }
    out
}

static CITED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?-u)(?:^|[\s\[;-])@([\w:.-]*\w)").unwrap());

/// The citekeys the document cites (`@key`). Code, front matter and what is commented out are
/// left out: a key that is only in a comment is in no export.
pub fn cited_keys(document_text: &str) -> BTreeSet<String> {
    static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?(```|\z)|`[^`\n]*`").unwrap());
    let text = FRONT_MATTER.replace(document_text, "");
    let text = CODE.replace_all(&text, " ");
    let text = crate::text::without_comments(&text);
    CITED.captures_iter(&text).map(|found| found[1].to_string()).collect()
}

/// The hits of a search as the AI reads them.
pub fn hits_as_text(hits: &[Hit]) -> String {
    let line = |h: &Hit| {
        format!(
            "- [{}] {} ({}). {}{}",
            h.citekey.as_deref().unwrap_or("no citekey"),
            h.authors,
            h.year,
            h.title,
            if h.doi.is_empty() { String::new() } else { format!(". doi:{}", h.doi) }
        )
    };
    hits.iter().map(line).collect::<Vec<_>>().join("\n")
}

// ------------------------------------------------------------ checking against Crossref

/// Who answers for a DOI or a title: Crossref, or a stand-in.
#[async_trait::async_trait]
pub trait References: Send + Sync {
    /// The record of a DOI; None when there is none.
    async fn work(&self, doi: &str) -> Result<Option<Record>, String>;
    /// The best records for a title, author and year.
    async fn candidates(&self, query: &str) -> Result<Vec<Record>, String>;
}

/// One reference after the check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckedRef {
    pub key: String,
    /// The file carries no citekey; the key was made from the record.
    pub derived: bool,
    pub file: String,
    pub title: String,
    pub year: String,
    pub doi: String,
    pub status: Status,
    /// What Crossref holds for the DOI, when it disagrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found: Option<Record>,
    /// A DOI Crossref lists for a title like this: for the author to check, not verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// What went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Checks every entry: its DOI against the record, or, without one, a lookup by title whose DOI is a suggestion.
pub async fn check_references(entries: &[Entry], references: &dyn References) -> Vec<CheckedRef> {
    let mut out = Vec::new();
    for e in entries {
        let mut row = CheckedRef {
            key: e.key.clone(),
            derived: e.derived,
            file: e.file.clone(),
            title: e.title.replace(['{', '}'], ""),
            year: e.year.clone(),
            doi: e.doi.clone(),
            status: Status::NoDoi,
            found: None,
            suggestion: None,
            detail: None,
        };
        if e.doi.is_empty() {
            if let Some(query) = lookup_query(e) {
                match references.candidates(&query).await {
                    Ok(candidates) => row.suggestion = pick(e, &candidates).map(|record| record.doi.clone()),
                    Err(error) => row.detail = Some(format!("title lookup failed: {error}")),
                }
            }
        } else {
            match references.work(&e.doi).await {
                Ok(record) => {
                    row.status = judge(e, record.as_ref());
                    row.found = record.filter(|_| row.status == Status::Mismatch);
                }
                Err(error) => (row.status, row.detail) = (Status::Error, Some(error)),
            }
        }
        out.push(row);
    }
    out
}
