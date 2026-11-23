//! The review party: the catalogue's experts and the ones an author made for a document.
//!
//! The catalogue (who the experts are, how they look, what each is told about its perspective)
//! is the plugin's, imported by `scripts/import-party.mjs` into `party/catalog.json`. The rules
//! every expert works under are the editor's own (`EXPERT_RULES`): the plugin's, reworded for
//! one document and the editor's tools. A custom expert is a `party/<id>.md` file in the
//! document's sidecar, in the plugin's format (`parsePersonaFile` in `plugin/hooks/personas.ts`).

use crate::host::Host;
use crate::sidecar::Sidecar;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: String,
    pub name: String,
    /// What a compact card shows, at most 13 characters.
    pub short: String,
    pub role: String,
    pub category: String,
    #[serde(default)]
    pub is_custom: bool,
    /// In the party of a document that has not chosen its own.
    #[serde(default)]
    pub is_default_member: bool,
    /// Colours of the sprite's letters; letters not named here come from the shared palette.
    #[serde(default)]
    pub palette: BTreeMap<String, String>,
    /// The figure, one string per pixel row, one letter per pixel, '.' transparent.
    #[serde(default)]
    pub sprite: Vec<String>,
    /// What the expert is told about its perspective and voice.
    #[serde(default)]
    pub prompt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    shared_palette: BTreeMap<String, String>,
    personas: Vec<Persona>,
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| serde_json::from_str(include_str!("../party/catalog.json")).expect("the shipped catalogue is valid"));

/// The personas that ship with CoWriting.
pub fn catalog() -> Vec<Persona> {
    CATALOG.personas.clone()
}

/// Colours of the sprite letters every figure shares (skin, eyes, trousers…).
pub fn shared_palette() -> &'static BTreeMap<String, String> {
    &CATALOG.shared_palette
}

/// An id as the log may write it: `cowrite:reviewer-2` names `reviewer-2`.
pub fn bare_id(id: &str) -> &str {
    ["cowriting:", "cowrite:", "cw:"].iter().find_map(|prefix| id.strip_prefix(prefix)).unwrap_or(id)
}

/// The rules every expert works under, after its own perspective.
pub const EXPERT_RULES: &str = "## Ground rules (CoWriting, DFG good scientific practice)
- The author writes. You **never** write or rewrite prose meant for the manuscript: no paragraphs, rephrasings, \"improved versions\", abstracts or introductions. You cannot change the document; only the author can, in the editor.
- You have these tools and no others: read_document and search_document (the document as it is now, with line numbers), list_notes, read_note and write_note (your notes; never manuscript text), search_bibliography (the author's own references). Read the document before you comment on it.
- Your description above may mention files, folders, scripts, a bibliography, a reference manager or web search. If you do not have them here, say in one line what the author could check themselves; never claim a check you did not run.
- If *the author's briefing for you in this project* follows these rules, that is your focus and your priorities here, within these rules.
- Give **located** feedback (line numbers, or a short quote of at most ~10 words), **questions** and **hints**. Explain *why* something is a problem and *what principle* applies, so the author can fix it themselves.
- Never invent facts or references. Cite only what you verified (DOI/URL) or what the document's own bibliography holds; mark anything else `UNVERIFIED`.
- Answer in the language of the manuscript, unless the author writes to you in another language.
- Stay in your perspective; if you notice a problem outside it, name it in one line and which other expert to ask.
- A **reply** from the author to feedback you gave earlier is a conversation, not a new review: answer it directly, in your voice, as a colleague would. Take a position, give your reasons, concede where the author is right, hold your ground where not, and ask back when something is unclear. The author may tell you how to weigh things in this document; take that on for this answer, within these rules.

## Output format
Start with one line in character (your voice), then:
1. **Top 3 issues** (most important first): line, what you noticed, why it matters, a guiding question or hint.
2. **Further observations** (short bullet list, optional).
3. **Questions for the author** (1–3).
A reply from the author gets no report: one line in character, then your answer.
End with: `→ Classify this in the editor (used / changed / not used / info).`";

static PERSONA_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,62}$").unwrap());
static COLOUR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#[0-9a-fA-F]{6}$").unwrap());
static FRONTMATTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---\r?\n?(.*)$").unwrap());
static PAIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Za-z][\w-]*)\s*:\s*(.*)$").unwrap());
static ARTICLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(the|der|die|das)\s+").unwrap());

const HAIR: [&str; 8] = ["#3d2b1f", "#6b4226", "#111111", "#a0522d", "#9a9a9a", "#c68642", "#4b3621", "#5d4037"];
const COATS: [&str; 8] = ["#1abc9c", "#2e9e5b", "#3b6fd6", "#7d5ba6", "#e67e22", "#c0392b", "#16a085", "#f1c40f"];
const GENERIC_SPRITE: [&str; 12] = [
    "....HHHH....",
    "...HHHHHH...",
    "..HHHHHHHH..",
    "..HSSSSSSH..",
    "..SKSSSSKS..",
    "..SSSSSSSS..",
    "...SSmmSS...",
    "....SSSS....",
    "..CCCCCCCC..",
    ".CCCCWWCCCC.",
    ".SCCCCCCCCS.",
    "..PPP..PPP..",
];

/// The plugin's `hashOf`: over UTF-16 units, wrapping at 32 bits.
fn hash_of(text: &str) -> usize {
    text.encode_utf16().fold(0u32, |h, unit| h.wrapping_mul(31).wrapping_add(unit as u32)) as usize
}

/// A name's short form for the card: without a leading article, at most 13 characters.
fn short_of(name: &str) -> String {
    ARTICLE.replace(name, "").chars().take(13).collect()
}

/// An id a `party/<id>.md` file may have: short, lowercase, not one of the catalogue's.
pub fn is_custom_id(id: &str) -> bool {
    PERSONA_ID.is_match(id) && !CATALOG.personas.iter().any(|p| p.id == id)
}

/// An id for a new expert from its name: "The Health Economist" → "health-economist".
pub fn id_from_name(name: &str) -> String {
    let mut id = String::new();
    for c in ARTICLE.replace(name.trim(), "").to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_matches('-').chars().take(40).collect::<String>().trim_matches('-').to_string()
}

/// A `party/<id>.md` file as a persona: frontmatter with name, role and optionally short,
/// category and colour (the coat, #rrggbb), then the persona's perspective and voice as its
/// body. None when the id or the file is not a persona.
pub fn parse_persona_file(id: &str, text: &str) -> Option<Persona> {
    if !is_custom_id(id) {
        return None;
    }
    let parts = FRONTMATTER.captures(text.trim_start())?;
    let mut meta: BTreeMap<String, String> = BTreeMap::new();
    for line in parts[1].lines() {
        if let Some(pair) = PAIR.captures(line) {
            let value = pair[2].trim();
            let unquoted = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\''))).unwrap_or(value);
            meta.insert(pair[1].to_lowercase(), unquoted.to_string());
        }
    }
    let body = parts[2].trim();
    let get = |key: &str| meta.get(key).map(String::as_str).unwrap_or("");
    let (name, role) = (get("name"), get("role"));
    if name.is_empty() || role.is_empty() || body.is_empty() {
        return None;
    }
    let colour = if get("colour").is_empty() { get("color") } else { get("colour") };
    let coat = if COLOUR.is_match(colour) { colour } else { COATS[hash_of(id) % COATS.len()] };
    let short: String = get("short").chars().take(13).collect();
    let category: String = get("category").to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || *c == '-').collect();
    Some(Persona {
        id: id.to_string(),
        name: name.to_string(),
        short: if short.is_empty() { short_of(name) } else { short },
        role: role.chars().take(80).collect(),
        category: if category.is_empty() { "content".into() } else { category },
        is_custom: true,
        is_default_member: true,
        palette: BTreeMap::from([("H".to_string(), HAIR[hash_of(name) % HAIR.len()].to_string()), ("C".to_string(), coat.to_string())]),
        sprite: GENERIC_SPRITE.iter().map(|row| row.to_string()).collect(),
        prompt: body.to_string(),
    })
}

/// A `party/<id>.md` file for a new expert. `looks_at` is the author's description of what the
/// expert pays attention to; a line break in name or role would break the frontmatter and is removed.
pub fn persona_file(id: &str, name: &str, role: &str, looks_at: &str) -> String {
    let one_line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let (name, role) = (one_line(name), one_line(role));
    format!(
        "---\nname: {name}\nshort: {}\nrole: {role}\ncategory: content\ncolour: {}\n---\nYou are **{name}**. Your perspective: {role}.\n\n{}\n",
        short_of(&name),
        COATS[hash_of(id) % COATS.len()],
        looks_at.trim()
    )
}

/// The personas one document knows, and which of them are in its party.
#[derive(Debug, Clone)]
pub struct Party {
    personas: Vec<Persona>,
    /// The roster the document chose; None while it has not.
    members: Option<Vec<String>>,
}

impl Default for Party {
    fn default() -> Self {
        Party { personas: catalog(), members: None }
    }
}

impl Party {
    pub fn with_custom(custom: Vec<Persona>) -> Self {
        let mut personas = catalog();
        // a catalogue id is never replaced by a file of the same name
        personas.extend(custom.into_iter().filter(|c| !CATALOG.personas.iter().any(|entry| entry.id == c.id)));
        Party { personas, members: None }
    }

    /// The party as the document's sidecar has it: its own experts (`party/<id>.md`) and its roster (`party.json`).
    pub fn load(host: &dyn Host, sidecar: &Sidecar) -> Party {
        let folder = sidecar.party_dir();
        let mut files = if host.exists(&folder) { host.list(&folder).unwrap_or_default() } else { Vec::new() };
        files.sort();
        let custom = files.iter().filter_map(|file| {
            let id = file.strip_suffix(".md")?;
            parse_persona_file(id, &host.read(&format!("{folder}/{file}")).ok()?)
        });
        let mut party = Party::with_custom(custom.collect());
        party.members = host
            .read(&sidecar.party_json())
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|v| serde_json::from_value(v["members"].clone()).ok());
        party
    }

    pub fn by_id(&self, id: &str) -> Option<&Persona> {
        let key = bare_id(id);
        self.personas.iter().find(|p| p.id == key)
    }

    pub fn all(&self) -> &[Persona] {
        &self.personas
    }

    pub fn is_member(&self, id: &str) -> bool {
        match &self.members {
            Some(members) => members.iter().any(|m| m == id),
            None => self.by_id(id).is_some_and(|p| p.is_default_member),
        }
    }

    /// The ids in the party, in the catalogue's order, the document's own experts last.
    pub fn member_ids(&self) -> Vec<String> {
        self.personas.iter().filter(|p| self.is_member(&p.id)).map(|p| p.id.clone()).collect()
    }
}
