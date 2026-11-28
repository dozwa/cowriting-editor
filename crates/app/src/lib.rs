//! One open document, as the editor's window uses it: open, save, ask, classify, read the log,
//! export the disclosure. The window (`src-tauri`) only passes these on, so everything here runs
//! and is tested without one.

pub mod diagnostics;
pub mod export;
pub mod feedback;
pub mod layouts;
pub mod linked;
pub mod outside;
pub mod picture;
pub mod preferences;
pub mod project_files;
pub mod sealed;
pub mod shown;
pub mod support;
pub mod terms;
pub mod update_check;
pub mod usage;
pub mod word;

use cowriting_backends::anthropic::AnthropicBackend;
use cowriting_backends::fake::FakeBackend;
use cowriting_backends::openai::{OpenAiBackend, OPENAI_BASE_URL};
use cowriting_core::backend::Backend;
use cowriting_core::briefs::{parse_briefs, set_brief};
use cowriting_core::disclosure::{disclosure_with_document, outside_appendix, prompts_appendix, turns, DocFormat, Kept, Lang, Profile};
use cowriting_core::host::Host;
use cowriting_core::log::{flatten, readable};
use cowriting_core::paste_tracking::{check, is_logged, sources, still_in_text, Found, Source};
use cowriting_core::personas::{id_from_name, is_custom_id, persona_file, shared_palette, Persona};
use cowriting_core::project::{LinkedFile, Project, LONGEST_FILE};
use cowriting_core::review::{check_points, live_review, read_check, read_plan, CheckSummary, LiveHint, LiveOutcome, LiveSettings, LiveState};
use cowriting_core::scope::{scope_progress, ScopeProgress};
use cowriting_core::sources::{
    added_files, added_style, check_references, cited_keys, format_name, load_bibliography, load_style, save_added_files, save_added_style, search, style_title, BibFile,
    CheckedRef, Hit, References, Status, Style,
};
use cowriting_core::stats::compute_stats;
use cowriting_core::stored;
use cowriting_core::thread::thread_of;
use cowriting_core::turn::{annotate, run_turn, Ask, TurnEvent, TurnOutcome, Workspace};
use cowriting_core::types::ExternalEntry;
use cowriting_core::types::{AiTextEntry, AnnotationEntry};
use cowriting_core::types::{LogEntry, Stats, ThreadMessage};
use cowriting_core::watch::Pace;
use cowriting_core::words::{changed_paragraphs, paragraphs};
use cowriting_core::words::{count_chars, count_words, Chars, Format};
pub use export::{DocumentExport, ExportTools, FileFormat, Paper, PdfChoice};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

pub const EDITOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the window shows of a document it just opened.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub path: String,
    pub name: String,
    pub text: String,
    /// The manuscript's language, 'en' or 'de'.
    pub language: String,
    /// AI logs in this folder whose document is gone, offered when this document has none: it may be that document, renamed.
    pub orphans: Vec<Orphan>,
    /// Stored files that could not be read and were set aside, or that a newer editor wrote.
    pub notices: Vec<cowriting_core::stored::Notice>,
}

/// An AI log without its document.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Orphan {
    /// The file name the log was kept for.
    pub name: String,
    /// That document's last saved text is exactly this document's text.
    pub is_same_text: bool,
    /// How many questions the log holds.
    pub questions: usize,
}

/// One logged turn as the Log tab lists it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRow {
    pub id: String,
    pub ts: String,
    pub category: String,
    pub asked: String,
    pub answer: String,
    /// The answer's beginning as plain words, for a list.
    pub excerpt: String,
    pub model: String,
    pub provider: String,
    pub tools_used: Vec<String>,
    /// 'adopted', 'adapted', 'rejected', 'info', or '' while the author has not classified it.
    #[serde(rename = "use")]
    pub use_: String,
    pub note: String,
    /// Waits for the author to classify it.
    pub is_open: bool,
    pub is_ai_text: bool,
    pub reply_to: Option<String>,
    /// The expert who answered: its id and name; '' for the assistant itself.
    pub persona: String,
    pub who: String,
}

/// A write the guard refused, or a change it undid.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardRow {
    pub ts: String,
    pub tool: String,
    pub input: String,
    pub files: Vec<String>,
    pub refused: bool,
    pub turn: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogView {
    pub stats: Stats,
    /// Newest first.
    pub turns: Vec<TurnRow>,
    pub guard: Vec<GuardRow>,
    /// AI text the editor saw enter the document, newest first.
    pub ai_text: Vec<AiTextRow>,
    /// AI use outside the editor, as the author stated it; the first statement first.
    pub external: Vec<ExternalUse>,
    pub words: usize,
    /// The document's characters, counted as the words are, with and without spaces.
    pub chars: Chars,
}

/// AI use outside the editor as the author states it. `id` is empty for a new statement, and names the one a correction replaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ExternalUse {
    pub id: String,
    pub tool: String,
    pub model: String,
    pub date: String,
    pub purpose: String,
    pub material: String,
    pub result: String,
    pub checked: String,
    /// What is kept with the statement. In a correction: the ones that stay; one that is left out is taken away.
    pub attachments: Vec<Attached>,
    /// What the author adds with this statement.
    #[serde(skip_serializing)]
    pub add: Vec<outside::NewAttachment>,
}

/// A file kept with a statement of outside AI use, as the log names it and as it is now.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Attached {
    pub file: String,
    /// 'chat', 'text' or 'file'.
    pub kind: String,
    pub bytes: u64,
    pub sha256: String,
    pub turns: u32,
    pub title: String,
    pub from: String,
    /// 'intact', 'changed' (the file is not what was kept) or 'missing'.
    pub state: String,
    /// What the author did with a kept chat or text: 'adopted', 'adapted', 'rejected', 'info', or '' while not said.
    #[serde(rename = "use")]
    pub use_: String,
    pub note: String,
}

/// A passage of the document that matches a logged answer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiTextRow {
    pub id: String,
    pub ts: String,
    /// 'paste' or 'typed'.
    pub source: String,
    /// 'exact' or 'near'.
    #[serde(rename = "match")]
    pub match_: String,
    /// 'text', or 'correction' for an answer about grammar and spelling.
    pub kind: String,
    pub words: usize,
    pub excerpt: String,
    pub line: usize,
    /// The turn whose answer it matches, and what the author had asked.
    pub turn: String,
    /// The tool, when the passage is from a chat the author kept with a statement of outside use.
    pub outside: String,
    pub asked: String,
    /// The author's comment on it.
    pub note: String,
    pub still_in_text: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// 'en' or 'de'.
    pub language: String,
    /// 'md' or 'tex'.
    pub format: String,
    /// Also write the prompts appendix.
    pub with_prompts: bool,
    /// The appendix includes the AI's answers.
    pub with_answers: bool,
    /// Also write the appendix of AI use outside the editor: the chats and files kept with the statements.
    #[serde(default)]
    pub with_outside: bool,
    /// For a file of a project: the disclosure of the whole project, from the logs of all its files.
    #[serde(default)]
    pub whole_project: bool,
}

/// The export of a whole project, as the export dialog offers it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WholeExport {
    pub name: String,
    pub files: usize,
    pub pdf: String,
    pub docx: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    /// The files written, the register first.
    pub paths: Vec<String>,
    /// The register's text, to show.
    pub register: String,
}

/// How the editor reaches a model, as the author set it up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackendSettings {
    /// 'anthropic', 'openai-compatible' or 'claude-code'; 'fake:<script file>' plays a script instead of asking a model.
    pub backend: String,
    pub model: String,
    /// Where the API is: a gateway, or a local model server. Empty for Anthropic or OpenAI itself.
    #[serde(default)]
    pub base_url: String,
    /// The longest answer the author allows, in tokens; 0 leaves it to the editor and the provider.
    #[serde(default)]
    pub max_tokens: u32,
}

impl Default for BackendSettings {
    fn default() -> Self {
        BackendSettings { backend: "anthropic".into(), model: "claude-opus-5-5".into(), base_url: String::new(), max_tokens: 0 }
    }
}

const NO_KEY: &str = "No API key is set yet. Add one in the settings to ask the AI.";

/// The backend the settings name. `api_key` is what the key store holds for it, if anything.
pub fn make_backend(settings: &BackendSettings, api_key: Option<&str>) -> Result<Box<dyn Backend>, String> {
    if let Some(script) = settings.backend.strip_prefix("fake:") {
        let json = std::fs::read_to_string(script).map_err(|e| format!("The fake backend's script {script} could not be read: {e}"))?;
        return Ok(Box::new(FakeBackend::from_json(&json)?));
    }
    match settings.backend.as_str() {
        "anthropic" => match api_key.map(str::trim).filter(|k| !k.is_empty()) {
            Some(key) => Ok(Box::new(AnthropicBackend::new(key, Some(&settings.base_url)).with_max_tokens(settings.max_tokens))),
            None => Err(NO_KEY.into()),
        },
        "openai-compatible" => {
            let key = api_key.map(str::trim).filter(|k| !k.is_empty());
            let base_url = settings.base_url.trim();
            // OpenAI itself always wants a key; a local server or a gateway may not
            if key.is_none() && (base_url.is_empty() || base_url.trim_end_matches('/') == OPENAI_BASE_URL) {
                return Err(NO_KEY.into());
            }
            Ok(Box::new(OpenAiBackend::new(base_url, key).with_max_tokens(settings.max_tokens)))
        }
        // the author's own Claude Code, with its own sign-in: no key of ours
        "claude-code" => cowriting_backends::claude_code::find_claude()
            .map(|program| Box::new(cowriting_backends::claude_code::ClaudeCodeBackend::new(program)) as Box<dyn Backend>)
            .ok_or_else(|| cowriting_backends::claude_code::NOT_INSTALLED.to_string()),
        other => Err(format!("This version cannot use the backend “{other}” yet.")),
    }
}

/// The models the provider in these settings offers: (id, display name).
pub async fn list_models(settings: &BackendSettings, api_key: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let key = api_key.map(str::trim).filter(|k| !k.is_empty());
    if settings.backend == "claude-code" {
        // found, not asked: whether it is signed in shows with the first question
        cowriting_backends::claude_code::find_claude().ok_or(cowriting_backends::claude_code::NOT_INSTALLED)?;
        return Ok(cowriting_backends::claude_code::MODELS.iter().map(|(id, name)| (id.to_string(), name.to_string())).collect());
    }
    let models = match settings.backend.as_str() {
        "anthropic" => AnthropicBackend::new(key.ok_or(NO_KEY)?, Some(&settings.base_url)).models().await,
        "openai-compatible" => OpenAiBackend::new(&settings.base_url, key).models().await,
        _ => return Ok(Vec::new()),
    };
    models.map_err(error)
}

/// A model the behaviour suite (`suites/`) was run on, and what came of it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestedModel {
    pub backend: String,
    pub model: String,
    /// 'passed': it kept to the assist-only rules in every case; 'failed': it did not.
    pub verdict: String,
    pub date: String,
    /// The editor's version the suite ran with.
    #[serde(default)]
    pub editor: String,
    /// Per case, how many runs held: {"asks-for-a-section": "3/3"}.
    #[serde(default)]
    pub cases: serde_json::Map<String, serde_json::Value>,
}

/// The suite's results that ship with this version.
pub fn tested_models() -> Vec<TestedModel> {
    let shipped: serde_json::Value = serde_json::from_str(include_str!("../../../tested-models.json")).unwrap_or_default();
    serde_json::from_value(shipped["models"].clone()).unwrap_or_default()
}

/// 'tested', 'failed' or 'untested': what is known about a model keeping to the rules.
pub fn model_status(tested: &[TestedModel], backend: &str, model: &str) -> &'static str {
    match tested.iter().rev().find(|t| t.backend == backend && t.model == model) {
        Some(t) if t.verdict == "passed" => "tested",
        Some(_) => "failed",
        None => "untested",
    }
}

/// One expert as the Experts tab shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpertView {
    #[serde(flatten)]
    pub persona: Persona,
    pub is_member: bool,
    /// What the author told this expert about the document.
    pub brief: String,
    /// The author reworded what this expert is told, in this project.
    pub is_prompt_edited: bool,
}

/// A linked file as the window shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkedFileView {
    pub path: String,
    pub name: String,
    /// The experts that get it; empty for everyone.
    pub personas: Vec<String>,
    /// It cannot be read now: moved, deleted, or not a text file.
    pub is_missing: bool,
    /// Its length in characters, and whether only its beginning is passed on.
    pub chars: usize,
    pub is_cut: bool,
    /// A Word file or PDF: what is passed on is the text the editor found in it.
    pub is_converted: bool,
}

/// What belongs to the open project besides text and log.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub files: Vec<LinkedFileView>,
    /// What this project sets differently from the general settings; a field that is absent is as there.
    pub overrides: Project,
    /// Where the project's settings are kept.
    pub folder: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyView {
    pub experts: Vec<ExpertView>,
    /// What the author told every expert about the document.
    pub everyone: String,
    pub shared_palette: BTreeMap<String, String>,
}

pub struct Session {
    pub workspace: Workspace,
    /// The document as it was last saved or opened: what a save is compared with.
    saved: Mutex<String>,
    /// What live review remembers while the document is open.
    live: Mutex<LiveState>,
    is_reviewing: AtomicBool,
    /// The editor's folder for project settings; None keeps them beside the document.
    projects: Option<String>,
    /// The project of several files this document is one of, by its file (`*.cowriting.json`).
    project: Option<String>,
}

/// Live review as the Live tab shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveView {
    pub is_on: bool,
    /// The experts who watch, by id.
    pub personas: Vec<String>,
    /// Newest first.
    pub hints: Vec<LiveHint>,
    /// How long the author must have paused before a review is asked for.
    pub pause_ms: u64,
    /// At least this long between two reviews.
    pub gap_ms: u64,
    /// At most this many reviews in an hour.
    pub per_hour: usize,
}

/// The document's references as the Sources tab shows them.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourcesView {
    pub files: Vec<BibFile>,
    /// How many references the files hold.
    pub entries: usize,
    /// How many different keys the document cites.
    pub cited: usize,
    /// Keys the document cites that no bibliography file holds.
    pub missing: Vec<String>,
    /// What the last check against Crossref found.
    pub checked: Option<VerifyFile>,
    /// The reference style of the export; none is pandoc's standard (Chicago author-date).
    pub style: Option<Style>,
}

/// What a reference check found (`verify.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifyFile {
    pub ts: String,
    pub rows: Vec<CheckedRef>,
    /// What a newer version of the editor wrote here and this one does not know: kept as it is.
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// The hits of a search, and where they are from.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SourceHits {
    /// 'zotero' or 'bibliography'.
    pub from: String,
    pub hits: Vec<Hit>,
}

/// The author's plan and how far the text has come.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopeView {
    /// scope.md as the author wrote it; empty when there is none.
    pub text: String,
    /// What a new plan starts from.
    pub template: String,
    /// The same for an author who plans in German.
    pub template_de: String,
    /// None while the plan says nothing yet.
    pub progress: Option<ScopeProgress>,
}

/// What a new scope.md starts from: the headings and the targets to fill in, nothing to read around.
/// How to fill it in is said beside it in the window, in the window's language.
pub const SCOPE_TEMPLATE: &str = "## Goal\n\n\n## Targets\n- Words: \n- Figures: \n- Tables: \n- References: \n- Deadline: \n\n## Sections\n\n\n## Points\n";
/// The same with the German words the plan is read by.
pub const SCOPE_TEMPLATE_DE: &str = "## Ziel\n\n\n## Umfang\n- Wörter: \n- Abbildungen: \n- Tabellen: \n- Quellen: \n- Abgabe: \n\n## Abschnitte\n\n\n## Kernaussagen\n";

fn ai_text_row(found: &AiTextEntry, entries: &[LogEntry], document_text: &str) -> AiTextRow {
    let outside = found.extra.get("outside").and_then(|tool| tool.as_str());
    let asked = found.extra.get("asked").and_then(|asked| asked.as_str()).map(str::to_string).or_else(|| {
        entries.iter().find_map(|e| match e {
            LogEntry::Prompt(p) if p.id == found.reference => Some(readable(&p.text)),
            _ => None,
        })
    });
    let note = entries.iter().rev().find_map(|e| match e {
        LogEntry::Annotation(a) if a.reference == found.id => a.note.clone(),
        _ => None,
    });
    AiTextRow {
        id: found.id.clone(),
        ts: found.ts.clone(),
        source: found.source.clone(),
        match_: found.match_.clone(),
        kind: found.kind.clone(),
        words: found.words,
        excerpt: found.excerpt.clone(),
        line: found.line,
        // a passage from a kept chat has no turn in this log to open
        turn: if outside.is_some() { String::new() } else { found.reference.clone() },
        outside: outside.unwrap_or_default().to_string(),
        asked: asked.unwrap_or_default(),
        note: note.unwrap_or_default(),
        still_in_text: still_in_text(found, document_text),
    }
}

fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn place_json(sidecar: &cowriting_core::sidecar::Sidecar) -> String {
    format!("{}/place.json", sidecar.dir())
}

/// A document that was moved or renamed outside the editor, together with its log: the project's
/// settings, which the editor keeps by where the document lies, come along. Only when the document
/// is gone from the old place, and nothing is there yet for the new one.
fn follow_move(host: &dyn Host, path: &str, projects: &str) {
    use cowriting_core::sidecar::Sidecar;
    let at = |document: &str| Sidecar::for_document(document).with_project(&format!("{}/{}", projects.trim_end_matches(['/', '\\']), Sidecar::project_key(document)));
    let here = at(path);
    let was =
        host.read(&place_json(&here)).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()).and_then(|place| place["document"].as_str().map(String::from));
    if let Some(was) = was.filter(|was| was != path && !host.exists(was)) {
        here.take_settings(host, &at(&was));
    }
}

impl Session {
    /// Opens a markdown file. Nothing is created beside it until the AI is first asked.
    pub fn open(host: Arc<dyn Host>, path: &str) -> Result<(Session, Opened), String> {
        Session::open_in(host, path, None)
    }

    /// Opens a markdown file whose project settings are kept in `projects`, the editor's own folder for them.
    /// A project's file opens the first of its files.
    pub fn open_in(host: Arc<dyn Host>, path: &str, projects: Option<&str>) -> Result<(Session, Opened), String> {
        // one spelling for a document, however it was named: its settings are found by its path
        let path = &project_files::native(&project_files::document_to_open(host.as_ref(), path)?);
        let text = host.read(path).map_err(|e| format!("{path} could not be opened: {e}"))?;
        if let Some(projects) = projects {
            follow_move(host.as_ref(), path, projects);
        }
        if stored::is_newer(host.as_ref(), &cowriting_core::sidecar::Sidecar::for_document(path).doc_json(), &stored::DOC) {
            return Err(format!("{path} was last worked on with a newer version of the editor, and this version cannot continue its AI log safely. Please update the editor."));
        }
        let mut workspace = Workspace::open_in(host, path, EDITOR_VERSION, projects);
        // one file of a project of several: the experts, linked files and sources are the project's
        let project = project_files::find(workspace.host.as_ref(), path);
        if let Some((file, project)) = &project {
            workspace.sidecar = workspace.sidecar.clone().with_shared(&project_files::shared_folder(projects, file, project), file);
        }
        let project = project.map(|(file, _)| file);
        let mut notices = workspace.sidecar.look_over(workspace.host.as_ref());
        notices.extend(stored::look_at::<VerifyFile>(workspace.host.as_ref(), &workspace.sidecar.verify_json(), &stored::VERIFY));
        let orphans = if workspace.sidecar.info(workspace.host.as_ref()).is_some() { Vec::new() } else { orphans_of(&workspace, &text) };
        let opened = Opened { path: path.to_string(), name: workspace.sidecar.name.clone(), text, language: workspace.language.clone(), orphans, notices };
        // live review left on: what is in the document now is the baseline, as when it is turned on
        let baseline = LiveSettings::load(&workspace).is_on.then(|| opened.text.clone());
        let live = Mutex::new(LiveState { baseline, ..Default::default() });
        let session = Session { workspace, saved: Mutex::new(opened.text.clone()), live, is_reviewing: AtomicBool::new(false), projects: projects.map(String::from), project };
        session.remember_place();
        session.refresh_linked();
        Ok((session, opened))
    }

    /// The log's folder notes where its document lies. When document and log are moved together
    /// outside the editor, that is how the project's settings find their way after them.
    fn remember_place(&self) {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        if self.projects.is_none() || !host.exists(&sidecar.doc_json()) {
            return;
        }
        let place = format!("{}\n", serde_json::json!({ "document": sidecar.document }));
        if host.read(&place_json(sidecar)).ok().as_deref() != Some(place.as_str()) {
            let _ = host.write(&place_json(sidecar), &place);
        }
    }

    /// The only code that writes the document: the author's own save.
    /// Returns the AI text it found in what changed since the last save, now logged.
    pub fn save(&self, text: &str) -> Result<Vec<AiTextRow>, String> {
        let host = self.workspace.host.as_ref();
        host.write(&self.workspace.sidecar.document, text).map_err(|e| format!("The document could not be saved: {e}"))?;
        let before = std::mem::replace(&mut *self.saved.lock().unwrap(), text.to_string());
        // a document that has a sidecar keeps it up to date; one that never asked the AI gets none
        if self.workspace.sidecar.info(host).is_none() {
            return Ok(Vec::new());
        }
        self.workspace.sidecar.ensure(host, text).map_err(error)?;
        self.remember_place();
        let changed = changed_paragraphs(&before, text, Format::Markdown);
        self.log_ai_text(changed.iter().map(|p| (p.text.as_str(), p.start)), "typed", text)
    }

    /// What was just pasted or dropped into the document, at this line: compared with the logged
    /// answers at once. Returns what it found, now logged. Nothing is blocked.
    pub fn report_paste(&self, pasted: &str, line: usize, document_text: &str) -> Result<Vec<AiTextRow>, String> {
        let parts = paragraphs(pasted);
        self.log_ai_text(parts.iter().map(|p| (p.text.as_str(), line + p.start - 1)), "paste", document_text)
    }

    /// Logs the passages that match a logged answer and are not in the log yet.
    fn log_ai_text<'a>(&self, passages: impl Iterator<Item = (&'a str, usize)>, source: &str, document_text: &str) -> Result<Vec<AiTextRow>, String> {
        let host = self.workspace.host.as_ref();
        let mut entries = self.workspace.log.read().map_err(error)?;
        let snapshots = self.workspace.sidecar.snapshots_dir();
        let mut known = sources(&entries, &|id| host.read(&format!("{snapshots}/{id}.md")).ok());
        let (outside, about) = self.outside_sources(&entries);
        known.extend(outside);
        let mut rows = Vec::new();
        for (passage, line) in passages {
            let Some(found) = check(passage, &known).filter(|found| !is_logged(&entries, found)) else { continue };
            let Found { reference, kind, match_, words, share, excerpt, hash } = found;
            let entry = AiTextEntry {
                id: host.new_id(),
                ts: host.now(),
                session: self.workspace.session.clone(),
                paper: self.workspace.sidecar.name.clone(),
                reference,
                source: source.to_string(),
                match_,
                kind,
                words,
                share,
                excerpt,
                hash,
                line,
                ..Default::default()
            };
            let mut entry = entry;
            if let Some((tool, asked)) = about.get(&entry.reference) {
                entry.extra.insert("outside".into(), serde_json::json!(tool));
                entry.extra.insert("asked".into(), serde_json::json!(readable(asked).chars().take(200).collect::<String>()));
            }
            self.workspace.log.append(&LogEntry::AiText(entry.clone())).map_err(error)?;
            entries.push(LogEntry::AiText(entry.clone()));
            rows.push(ai_text_row(&entry, &entries, document_text));
        }
        Ok(rows)
    }

    pub fn live_view(&self) -> LiveView {
        let settings = LiveSettings::load(&self.workspace);
        let live = self.live.lock().unwrap();
        LiveView {
            is_on: settings.is_on,
            personas: settings.personas,
            hints: live.hints.clone(),
            pause_ms: live.pace.pause_ms,
            gap_ms: live.pace.gap_ms,
            per_hour: live.pace.per_hour,
        }
    }

    /// What the author said about languages: the manuscript's, and the one the AI answers in. Holds from the next question on.
    pub fn set_languages(&self, languages: cowriting_core::turn::Languages) {
        *self.workspace.languages.write().unwrap() = languages;
    }

    /// How often live review asks from now on, as the author set it.
    pub fn set_pace(&self, pace: Pace) {
        self.live.lock().unwrap().pace = pace;
    }

    /// Turns live review on or off and says who watches. What is in the document now is not reviewed: only what changes from here on.
    pub fn set_live(&self, is_on: bool, personas: &[String], document_text: &str) -> Result<(), String> {
        let party = self.workspace.party();
        if let Some(unknown) = personas.iter().find(|id| party.by_id(id).is_none()) {
            return Err(format!("This document's party has no expert “{unknown}”."));
        }
        self.workspace.sidecar.ensure(self.workspace.host.as_ref(), document_text).map_err(error)?;
        LiveSettings { is_on, personas: personas.to_vec(), extra: LiveSettings::load(&self.workspace).extra }
            .save(&self.workspace)
            .map_err(|e| format!("The setting could not be saved: {e}"))?;
        self.live.lock().unwrap().baseline = is_on.then(|| document_text.to_string());
        Ok(())
    }

    /// Called when the author has paused: reviews what changed since the last review, if it is time.
    pub async fn live_review(&self, backend: &dyn Backend, model: &str, document_text: &str) -> Result<LiveOutcome, String> {
        // one review at a time; a second call while one runs waits for the next pause
        if self.is_reviewing.swap(true, Ordering::SeqCst) {
            return Ok(LiveOutcome::Wait { ms: self.live.lock().unwrap().pace.pause_ms });
        }
        let settings = LiveSettings::load(&self.workspace);
        let mut state = self.live.lock().unwrap().clone();
        let outcome = live_review(&self.workspace, backend, model, &settings, &mut state, document_text).await;
        {
            // a pace set while the review ran is kept
            let mut live = self.live.lock().unwrap();
            state.pace = live.pace;
            *live = state;
        }
        self.is_reviewing.store(false, Ordering::SeqCst);
        outcome.map_err(error)
    }

    pub fn scope_view(&self, document_text: &str) -> ScopeView {
        let text = self.workspace.host.read(&self.workspace.sidecar.scope_md()).unwrap_or_default();
        let check = read_check(&self.workspace);
        let today = self.workspace.host.now();
        let progress = read_plan(&self.workspace).map(|plan| {
            let (points, at) = check.map(|c| (c.points, Some(c.ts))).unwrap_or_default();
            scope_progress(&plan, document_text, &points, at.as_deref(), today.get(..10).unwrap_or(&today))
        });
        ScopeView { text, template: SCOPE_TEMPLATE.to_string(), template_de: SCOPE_TEMPLATE_DE.to_string(), progress }
    }

    /// Saves the author's plan.
    pub fn set_scope(&self, text: &str, document_text: &str) -> Result<(), String> {
        let host = self.workspace.host.as_ref();
        self.workspace.sidecar.ensure(host, document_text).map_err(error)?;
        host.write(&self.workspace.sidecar.scope_md(), text).map_err(|e| format!("The plan could not be saved: {e}"))
    }

    /// Asks the model which of the planned points the document makes. Logged.
    pub async fn check_points(&self, backend: &dyn Backend, model: &str, document_text: &str) -> Result<CheckSummary, String> {
        check_points(&self.workspace, backend, model, document_text).await.map_err(error)
    }

    pub fn sources_view(&self, document_text: &str) -> SourcesView {
        let host = self.workspace.host.as_ref();
        let bibliography = load_bibliography(host, &self.workspace.sidecar, document_text);
        let cited = cited_keys(document_text);
        // a key the files do not hold is only missing when there are files to hold it
        let missing = if bibliography.files.is_empty() {
            Vec::new()
        } else {
            cited.iter().filter(|key| !bibliography.entries.iter().any(|e| !e.derived && &e.key == *key)).cloned().collect()
        };
        let checked = stored::read_as(host, &self.workspace.sidecar.verify_json(), &stored::VERIFY);
        let style = load_style(host, &self.workspace.sidecar, document_text);
        SourcesView { entries: bibliography.entries.len(), files: bibliography.files, cited: cited.len(), missing, checked, style }
    }

    /// Sets the reference style of this document's exports: a CSL file of the author's. The file is read, never written.
    pub fn set_style(&self, path: &str, document_text: &str) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        if !path.to_lowercase().ends_with(".csl") {
            return Err("A reference style is a .csl file, as zotero.org/styles has them.".into());
        }
        let text = host.read(path).map_err(|e| format!("The file could not be read: {e}"))?;
        style_title(&text).map_err(|why| format!("That file {why}."))?;
        sidecar.ensure(host, document_text).map_err(error)?;
        save_added_style(host, sidecar, Some(path)).map_err(|e| format!("The style could not be saved: {e}"))
    }

    /// Back to pandoc's standard style. The file itself is left alone.
    pub fn remove_style(&self) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        if added_style(host, sidecar).is_none() {
            return Ok(());
        }
        save_added_style(host, sidecar, None).map_err(|e| format!("The style could not be saved: {e}"))
    }

    /// Adds a bibliography file of the author's to this document. The file is read, never written.
    pub fn add_bibliography(&self, path: &str, document_text: &str) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        if format_name(path).is_none() {
            return Err("That is not a bibliography format the editor reads: .bib, .json (CSL), .ris or .nbib.".into());
        }
        host.read(path).map_err(|e| format!("The file could not be read: {e}"))?;
        let mut files = added_files(host, sidecar);
        if load_bibliography(host, sidecar, document_text).files.iter().any(|known| known.path == path) {
            return Ok(());
        }
        files.push(path.to_string());
        sidecar.ensure(host, document_text).map_err(error)?;
        save_added_files(host, sidecar, &files).map_err(|e| format!("The list of files could not be saved: {e}"))
    }

    /// Takes a file the author added out of the list again. The file itself is left alone.
    pub fn remove_bibliography(&self, path: &str) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let files: Vec<String> = added_files(host, sidecar).into_iter().filter(|known| known != path).collect();
        save_added_files(host, sidecar, &files).map_err(|e| format!("The list of files could not be saved: {e}"))
    }

    /// The references in the document's bibliography files that hold every word of the query.
    pub fn search_bibliography(&self, query: &str, document_text: &str) -> Vec<Hit> {
        search(&load_bibliography(self.workspace.host.as_ref(), &self.workspace.sidecar, document_text).entries, query, 15)
    }

    /// The author's running Zotero first (through Better BibTeX, which knows the citekeys, else its own API); their bibliography files when Zotero is not reachable.
    pub async fn search_sources(&self, query: &str, document_text: &str) -> SourceHits {
        let zotero = match cowriting_backends::lookup::better_bibtex_search(query, 15).await {
            Some(hits) => Some(hits),
            None => cowriting_backends::lookup::zotero_search(query, 15).await,
        };
        match zotero {
            Some(hits) => SourceHits { from: "zotero".into(), hits },
            None => SourceHits { from: "bibliography".into(), hits: self.search_bibliography(query, document_text) },
        }
    }

    /// Makes a PDF or Word file of the document with pandoc. `zotero` is what the author's Zotero holds for the
    /// cited keys, as CSL JSON; without it only the document's bibliography files are used.
    /// With `pdf.whole_project`, for a file of a project, the export is of all its files in their order.
    pub fn export_document(&self, format: FileFormat, target: &str, document_text: &str, zotero: Option<&str>, pdf: &PdfChoice) -> Result<DocumentExport, String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        export::check_target(format, target, &sidecar.document)?;
        let whole = self.whole_text(pdf, document_text)?;
        let (document_text, folder) = whole.as_ref().map_or((document_text, sidecar.folder.as_str()), |whole| (whole.text.as_str(), whole.folder.as_str()));
        let tools = export::local_tools();
        let engine = pdf.engine.as_ref().or(tools.pdf_engines.first());
        if let Some(chosen) = engine.filter(|chosen| !tools.pdf_engines.contains(chosen)) {
            return Err(format!("{chosen} is not installed on this computer."));
        }
        let engine = engine.and_then(|name| export::find_program(name).map(|path| (name.as_str(), path.to_string_lossy().into_owned())));
        if format == FileFormat::Pdf && engine.is_none() && tools.has_pandoc {
            return Err(
                "A PDF needs a program that typesets it, and none is installed: a LaTeX distribution (TeX Live, MacTeX, MiKTeX) or Typst. Word export works without.".into()
            );
        }
        // what Zotero gave is pandoc's to read for this one export: in a temporary file, so that an export
        // makes no folder for the AI's record beside a document that was never put to the AI
        let kept = std::env::temp_dir().join(format!("cowriting-export-{}-{}.json", std::process::id(), host.new_id()));
        let kept = kept.to_string_lossy().into_owned();
        let used = export::references(&export::bibliography_of(&self.workspace, document_text), document_text, zotero.map(|csl| (kept.as_str(), csl)));
        // a style the document names is pandoc's to read; one chosen here is handed over, if it can be read
        let style = load_style(host, sidecar, document_text).filter(|style| !style.is_from_document);
        if let Some(Style { name, error: Some(why), .. }) = &style {
            return Err(format!("The reference style {name} {why}. Choose another in the Sources tab, or take it off there."));
        }
        let mut args = export::pandoc_args(
            format,
            target,
            folder,
            &used.files,
            engine.as_ref().map(|(name, path)| (*name, path.as_str())),
            pdf.paper,
            style.as_ref().map(|s| s.path.as_str()),
        );
        // the document's language (for hyphenation) and, for a PDF, the layout the author chose: in a file of
        // its own, so that what the document's front matter says comes first
        // a Word file has no typesetter to tell: its layout is in the styles of a Word file pandoc is shown
        let temporary = |ending: &str| std::env::temp_dir().join(format!("cowriting-export-{}-{}.{ending}", std::process::id(), host.new_id()));
        let mut styles_made = None;
        if format == FileFormat::Docx {
            let template = pdf.word_template.trim();
            let styles = if template.is_empty() {
                let made = temporary("docx");
                std::fs::write(&made, word::reference_doc(&pdf.layout, pdf.paper)?).map_err(|e| format!("The Word styles could not be written: {e}"))?;
                styles_made.insert(made).to_string_lossy().into_owned()
            } else if std::path::Path::new(template).is_file() {
                template.to_string()
            } else {
                return Err(format!("The Word template {template} is not there any more. Choose another in the export dialog, or take it off."));
            };
            args.extend(["--reference-doc".to_string(), styles]);
            if pdf.layout.numbered {
                args.push("--number-sections".into());
            }
        }
        let layout = if format == FileFormat::Pdf { pdf.layout.clone() } else { export::Layout::default() };
        let metadata = temporary("yaml");
        let written = std::fs::write(&metadata, layout.metadata(&self.workspace.language)).map_err(|e| format!("The export's settings could not be written: {e}"));
        if let (Err(_), Some(styles)) = (&written, &styles_made) {
            let _ = std::fs::remove_file(styles);
        }
        written?;
        args.extend(["--metadata-file".to_string(), metadata.to_string_lossy().into_owned()]);
        let made = match zotero.map(|csl| std::fs::write(&kept, csl)) {
            Some(Err(e)) => Err(format!("The references from Zotero could not be handed to the export: {e}")),
            _ => export::run_pandoc(document_text, folder, &args),
        };
        let _ = std::fs::remove_file(&metadata);
        let _ = std::fs::remove_file(&kept);
        if let Some(styles) = &styles_made {
            let _ = std::fs::remove_file(styles);
        }
        let warnings = made?;
        // Typst sets what it has no typeface for in another one and says nothing; a LaTeX stops, and its words were passed on above
        let missing_fonts = if matches!((format, &engine), (FileFormat::Pdf, Some(("typst", _)))) { export::fonts_not_here(&layout.fonts()) } else { Vec::new() };
        Ok(DocumentExport { path: target.to_string(), cited: cited_keys(document_text).len(), from_zotero: used.from_zotero, missing: used.missing, warnings, missing_fonts })
    }

    /// How this project's exports are set, when that was ever chosen here: a call for papers asks for a
    /// layout per document, not per computer. What cannot be read counts as never chosen.
    pub fn export_choice(&self) -> Option<PdfChoice> {
        let project = Project::load(self.workspace.host.as_ref(), &self.workspace.sidecar);
        project.export.and_then(|kept| serde_json::from_value(kept).ok())
    }

    /// Keeps how this project's exports are set, with the project's settings.
    pub fn keep_export_choice(&self, choice: &PdfChoice) -> Result<(), String> {
        // what is exported, the document or the whole project, is chosen each time and not kept
        let kept = PdfChoice { layout: choice.layout.within_limits(), whole_project: false, ..choice.clone() };
        self.change_project(|project| {
            project.export = Some(serde_json::to_value(&kept).map_err(error)?);
            Ok(())
        })
    }

    /// All files of the project as one text, when the export is to be of the whole project.
    fn whole_text(&self, pdf: &PdfChoice, document_text: &str) -> Result<Option<project_files::Whole>, String> {
        match &self.project {
            Some(project) if pdf.whole_project => project_files::whole(self.workspace.host.as_ref(), project, Some((&self.workspace.sidecar.document, document_text))).map(Some),
            _ => Ok(None),
        }
    }

    /// For a file of a project: the project's name, the number of its files, and where an export
    /// of the whole would go (PDF, Word). None for a document on its own.
    pub fn whole_export(&self) -> Option<WholeExport> {
        let project = self.project.as_deref()?;
        let files = project_files::members(self.workspace.host.as_ref(), project).ok()?.len();
        let name = cowriting_core::project_file::ProjectFile::name_of(project).to_string();
        let target = |format: FileFormat| {
            std::path::Path::new(&cowriting_core::sidecar::Sidecar::for_document(project).folder).join(format!("{name}.{}", format.extension())).to_string_lossy().into_owned()
        };
        Some(WholeExport { pdf: target(FileFormat::Pdf), docx: target(FileFormat::Docx), name, files })
    }

    /// The export with the references looked up in the author's Zotero, when it is running.
    pub async fn export_document_online(&self, format: FileFormat, target: &str, document_text: &str, pdf: &PdfChoice) -> Result<DocumentExport, String> {
        // of the whole project, the keys every file cites
        let whole = self.whole_text(pdf, document_text)?;
        let document_text_cited = whole.as_ref().map_or(document_text, |whole| whole.text.as_str());
        let cited: Vec<String> = cited_keys(document_text_cited).into_iter().collect();
        let zotero = cowriting_backends::lookup::better_bibtex_export(&cited).await;
        self.export_document(format, target, document_text, zotero.as_ref().map(|(csl, _)| csl.as_str()), pdf)
    }

    /// Where a preview is made: in the system's folder for temporary files, never beside the document.
    /// One file per document and kind, so a viewer that has it open shows the next preview in its place.
    pub fn preview_target(&self, format: FileFormat) -> String {
        let sidecar = &self.workspace.sidecar;
        let stem = sidecar.name.rsplit_once('.').map_or(sidecar.name.as_str(), |(stem, _)| stem);
        let folder = std::env::temp_dir().join("cowriting-preview").join(cowriting_core::sidecar::Sidecar::project_key(&sidecar.document));
        folder.join(format!("{stem}.{}", format.extension())).to_string_lossy().into_owned()
    }

    /// The document as the export would make it, in a temporary file: to look at a layout before anything is written beside the document.
    pub async fn preview_document_online(&self, format: FileFormat, document_text: &str, pdf: &PdfChoice) -> Result<DocumentExport, String> {
        let mut target = self.preview_target(format);
        if let (true, Some(whole)) = (pdf.whole_project, self.whole_export()) {
            // a file of its own, so that a viewer showing the one document's preview keeps it
            target = std::path::Path::new(&target).with_file_name(format!("{}.{}", whole.name, format.extension())).to_string_lossy().into_owned();
        }
        if let Some(folder) = std::path::Path::new(&target).parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("The preview could not be made: {e}"))?;
        }
        self.export_document_online(format, &target, document_text, pdf).await
    }

    /// Where an export goes when the author does not choose: beside the document, under its name.
    pub fn export_target(&self, format: FileFormat) -> String {
        let sidecar = &self.workspace.sidecar;
        let stem = sidecar.name.rsplit_once('.').map_or(sidecar.name.as_str(), |(stem, _)| stem);
        std::path::Path::new(&sidecar.folder).join(format!("{stem}.{}", format.extension())).to_string_lossy().into_owned()
    }

    /// The reference check against Crossref itself.
    pub async fn check_references_online(&self, document_text: &str) -> Result<VerifyFile, String> {
        self.check_references(&cowriting_backends::lookup::Crossref::default(), document_text).await
    }

    /// Checks every reference against Crossref (or a stand-in) and keeps the result. No AI is involved, so nothing is logged.
    pub async fn check_references(&self, references: &dyn References, document_text: &str) -> Result<VerifyFile, String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let bibliography = load_bibliography(host, sidecar, document_text);
        if bibliography.entries.is_empty() {
            return Err("There are no references to check: add a bibliography file first.".into());
        }
        let rows = check_references(&bibliography.entries, references).await;
        if rows.iter().all(|row| row.status == Status::Error) {
            return Err(format!("The check could not be made: {}", rows[0].detail.clone().unwrap_or_default()));
        }
        let file = VerifyFile { ts: host.now(), rows, extra: Default::default() };
        sidecar.ensure(host, document_text).map_err(error)?;
        stored::write(host, &sidecar.verify_json(), &stored::VERIFY, &file).map_err(|e| format!("The result could not be saved: {e}"))?;
        Ok(file)
    }

    /// Takes over the AI log of a document that is no longer in this folder: the author says this is that document, renamed.
    pub fn adopt(&self, name: &str) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let (from, _) =
            sidecar.orphans(host).into_iter().find(|(other, _)| other.name == name).ok_or_else(|| format!("This folder has no AI log for {name} without its document."))?;
        sidecar.adopt(host, &from).map_err(|e| format!("The AI log could not be taken over: {e}"))?;
        sidecar.take_settings(host, &self.elsewhere(&from.document));
        Ok(())
    }

    /// The sidecar of this project's document at another place, with its settings where the editor keeps them.
    fn elsewhere(&self, document: &str) -> cowriting_core::sidecar::Sidecar {
        let sidecar = cowriting_core::sidecar::Sidecar::for_document(document);
        match &self.projects {
            Some(projects) => sidecar.with_project(&format!("{projects}/{}", cowriting_core::sidecar::Sidecar::project_key(document))),
            None => sidecar,
        }
    }

    /// What belongs to this project besides text and log: linked files, and its own settings.
    pub fn project_view(&self) -> ProjectView {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let project = Project::load(host, sidecar);
        let files = project
            .files
            .iter()
            .map(|file| {
                let text = host.read(&file.readable(sidecar)).ok().filter(|_| host.exists(&file.path));
                let chars = text.as_deref().map_or(0, |t| t.trim().chars().count());
                LinkedFileView {
                    path: file.path.clone(),
                    name: file.name().to_string(),
                    personas: file.personas.clone(),
                    is_missing: text.is_none(),
                    chars,
                    is_cut: chars > LONGEST_FILE,
                    is_converted: file.text.is_some(),
                }
            })
            .collect();
        ProjectView { files, overrides: Project { files: Vec::new(), document: String::new(), ..project }, folder: sidecar.settings_dir() }
    }

    fn change_project(&self, change: impl FnOnce(&mut Project) -> Result<(), String>) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let mut project = Project::load(host, sidecar);
        change(&mut project)?;
        project.save(host, sidecar).map_err(|e| format!("The project's settings could not be saved: {e}"))
    }

    /// Links a file of the author's as background knowledge: for these experts, or for everyone when none is named.
    /// Linking it again changes who gets it. Only its path is kept; the file stays where it is and is read when the AI is asked.
    pub fn link_file(&self, path: &str, personas: &[String]) -> Result<(), String> {
        let host = self.workspace.host.as_ref();
        if picture::same_file(path, &self.workspace.sidecar.document) {
            return Err("That is the document itself; the AI reads it anyway.".into());
        }
        // a Word file or a PDF is read once now; the editor keeps what it says and passes that on
        let kind = linked::kind_of(path);
        let copy = if kind == linked::Kind::Text {
            host.read(path).map_err(|_| {
                format!(
                    "{path} cannot be read as text. Linked files are text: markdown, .txt, .csv, .bib and the like, or a Word file (.docx, .odt, .rtf) or a PDF with text in it."
                )
            })?;
            None
        } else {
            let text = linked::text_of(path, kind)?;
            let copy = Session::copy_name(path);
            host.write(&format!("{}/linked/{copy}", self.workspace.sidecar.settings_dir()), &text).map_err(|e| format!("The text of {path} could not be kept: {e}"))?;
            Some(copy)
        };
        let party = self.workspace.party();
        if let Some(unknown) = personas.iter().find(|id| party.by_id(id).is_none()) {
            return Err(format!("This document's party has no expert “{unknown}”."));
        }
        self.change_project(|project| {
            match project.files.iter_mut().find(|file| file.path == path) {
                Some(file) => (file.personas, file.text) = (personas.to_vec(), copy),
                None => project.files.push(LinkedFile { path: path.to_string(), personas: personas.to_vec(), text: copy }),
            }
            Ok(())
        })
    }

    /// How this document would name a picture of the author's: by its way from the document's folder.
    pub fn picture(&self, path: &str) -> Result<picture::Picture, String> {
        picture::check(path)?;
        Ok(picture::seen_from(&self.workspace.sidecar.folder, path))
    }

    /// Copies a picture into `images/` beside the document; the picture itself stays where and as it is.
    pub fn copy_picture(&self, path: &str) -> Result<picture::Picture, String> {
        picture::copy_beside(&self.workspace.sidecar.folder, &self.pictures_folder(), path)
    }

    /// The folder beside the document that copies of pictures go into: `images`, or what the author named for this project.
    pub fn pictures_folder(&self) -> String {
        Project::load(self.workspace.host.as_ref(), &self.workspace.sidecar)
            .pictures_folder
            .and_then(|name| picture::folder_name(&name).ok())
            .unwrap_or_else(|| picture::FOLDER.to_string())
    }

    /// Names the folder for this project's pictures. What is in the old folder stays there, and the text is not touched.
    pub fn set_pictures_folder(&self, name: &str) -> Result<(), String> {
        let name = picture::folder_name(name)?;
        self.change_project(|project| {
            project.pictures_folder = (name != picture::FOLDER).then_some(name);
            Ok(())
        })
    }

    /// Saves a picture that came without a file (from the clipboard) beside the document, and returns it as the document names it.
    pub fn paste_picture(&self, kind: &str, content: &[u8]) -> Result<picture::Picture, String> {
        let day: String = self.workspace.host.now().chars().take(10).collect();
        picture::save_beside(&self.workspace.sidecar.folder, &self.pictures_folder(), kind, content, &day)
    }

    /// Where the editor keeps the text of a linked Word file or PDF: in the project's folder, under the file's name.
    fn copy_name(path: &str) -> String {
        let name: String =
            path.rsplit(['/', '\\']).next().unwrap_or("file").chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).take(60).collect();
        format!("{name}-{}.txt", cowriting_core::sidecar::content_hash(path))
    }

    /// Linked Word files and PDFs that changed since the editor read them are read again. One that
    /// cannot be read now keeps the text it had. Returns how many were read again.
    pub fn refresh_linked(&self) -> usize {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let changed = |path: &str| std::fs::metadata(path).and_then(|file| file.modified()).ok();
        let mut read_again = 0;
        for file in Project::load(host, &self.workspace.sidecar).files {
            if file.text.is_none() {
                continue;
            }
            let copy = &file.readable(sidecar);
            let is_newer = match (changed(&file.path), changed(copy)) {
                (Some(source), Some(kept)) => source > kept,
                (Some(_), None) => true,
                _ => false,
            };
            if is_newer {
                if let Ok(text) = linked::text_of(&file.path, linked::kind_of(&file.path)) {
                    read_again += usize::from(host.write(copy, &text).is_ok());
                }
            }
        }
        read_again
    }

    pub fn unlink_file(&self, path: &str) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        if let Some(file) = Project::load(host, sidecar).files.iter().find(|file| file.path == path && file.text.is_some()) {
            let _ = host.remove(&file.readable(sidecar));
        }
        self.change_project(|project| {
            project.files.retain(|file| file.path != path);
            Ok(())
        })
    }

    /// What this project sets differently from the general settings: languages, the pace of live review, and its AI.
    /// The linked files are not touched.
    pub fn set_overrides(&self, overrides: Project) -> Result<(), String> {
        let word = |value: Option<String>, allowed: &[&str]| value.filter(|v| allowed.contains(&v.as_str()));
        self.change_project(|project| {
            project.writing_language = word(overrides.writing_language, &["document", "en", "de"]);
            project.ai_language = word(overrides.ai_language, &["auto", "en", "de"]);
            project.disclosure_language = word(overrides.disclosure_language, &["document", "en", "de"]);
            project.live_pause_seconds = overrides.live_pause_seconds;
            project.live_gap_seconds = overrides.live_gap_seconds;
            project.live_per_hour = overrides.live_per_hour;
            project.ai = match overrides.ai {
                Some(ai) if !["anthropic", "openai-compatible", "claude-code"].contains(&ai.backend.as_str()) => {
                    return Err(format!("The editor knows no provider “{}”.", ai.backend))
                }
                Some(ai) if ai.model.trim().is_empty() => return Err("A project's own AI needs a model.".into()),
                Some(ai) => Some(cowriting_core::project::ProjectAi { model: ai.model.trim().to_string(), base_url: ai.base_url.trim().to_string(), ..ai }),
                None => None,
            };
            Ok(())
        })
    }

    /// The AI this project is written with: its own if it has one, else the general one.
    pub fn settings_here(&self, general: BackendSettings) -> BackendSettings {
        match Project::load(self.workspace.host.as_ref(), &self.workspace.sidecar).ai {
            Some(ai) => BackendSettings { backend: ai.backend, model: ai.model, base_url: ai.base_url, max_tokens: ai.max_tokens },
            None => general,
        }
    }

    /// The general preferences with what this project sets for itself.
    pub fn preferences_here(&self, general: &preferences::Preferences) -> preferences::Preferences {
        let project = Project::load(self.workspace.host.as_ref(), &self.workspace.sidecar);
        let mut here = general.clone();
        here.writing_language = project.writing_language.unwrap_or(here.writing_language);
        here.ai_language = project.ai_language.unwrap_or(here.ai_language);
        here.disclosure_language = project.disclosure_language.unwrap_or(here.disclosure_language);
        here.live_pause_seconds = project.live_pause_seconds.unwrap_or(here.live_pause_seconds);
        here.live_gap_seconds = project.live_gap_seconds.unwrap_or(here.live_gap_seconds);
        here.live_per_hour = project.live_per_hour.unwrap_or(here.live_per_hour);
        here.within_limits()
    }

    /// Rewords what an expert is told, in this project. The assist-only rules are not part of it and stay as they are.
    /// For an expert that comes with the editor, an empty text (or the standard one) brings the standard back.
    pub fn set_prompt(&self, id: &str, prompt: &str) -> Result<(), String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let prompt = prompt.trim();
        let failed = |e: std::io::Error| format!("The expert could not be saved: {e}");
        if prompt.chars().count() > 20_000 {
            return Err("That is too long for what an expert is told: at most 20,000 characters. Longer material belongs in a linked file.".into());
        }
        match cowriting_core::personas::standard_prompt(id) {
            Some(standard) => {
                let file = format!("{}/{id}.md", sidecar.prompts_dir());
                if prompt.is_empty() || prompt == standard.trim() {
                    if host.exists(&file) {
                        host.remove(&file).map_err(failed)
                    } else {
                        Ok(())
                    }
                } else {
                    host.write(&file, &format!("{prompt}\n")).map_err(failed)
                }
            }
            None => {
                let file = format!("{}/{id}.md", sidecar.party_dir());
                let before = host.read(&file).map_err(|_| format!("This document's party has no expert “{id}”."))?;
                if prompt.is_empty() {
                    return Err("An expert of your own needs a few words on what they look at.".into());
                }
                let after = cowriting_core::personas::with_prompt(&before, prompt).ok_or("The expert's file cannot be read.")?;
                host.write(&file, &after).map_err(failed)
            }
        }
    }

    /// Moves or renames the document, and its AI log, notes and settings with it. The text given is what is written.
    /// The pictures a move to `target` would leave behind: the text names them by a way from the document's folder.
    pub fn pictures_to_move(&self, target: &str, document_text: &str) -> picture::ToMove {
        picture::to_move(document_text, &self.workspace.sidecar.folder, &cowriting_core::sidecar::Sidecar::for_document(target).folder)
    }

    /// Moves the document as `move_document` does, and sees to the pictures it names from its folder:
    /// they are copied along, or the text names them from the new place. That is the one change to
    /// the manuscript the editor makes itself, and only when the author chose it for this move.
    /// Returns the text as it is at the new place.
    pub fn move_with_pictures(&self, target: &str, document_text: &str, along: picture::Along) -> Result<String, String> {
        let (text, copies) = picture::carried(document_text, &self.workspace.sidecar.folder, &cowriting_core::sidecar::Sidecar::for_document(target).folder, along);
        self.move_document(target, &text)?;
        picture::copy_all(&copies).map_err(|e| format!("The document is at its new place, but not all of its pictures: {e}"))?;
        Ok(text)
    }

    pub fn move_document(&self, target: &str, document_text: &str) -> Result<(), String> {
        // the new place in the spelling it will be opened by, so its settings are found there
        let target = &project_files::native(target);
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        let to = self.elsewhere(target);
        if std::path::Path::new(target) == std::path::Path::new(&sidecar.document) {
            return Err("That is where the document already is.".into());
        }
        let lower = target.to_lowercase();
        if !(lower.ends_with(".md") || lower.ends_with(".markdown")) {
            return Err("The document needs a name that ends in .md.".into());
        }
        if host.exists(&to.dir()) {
            return Err(format!("{} has an AI log of its own there. Choose another name, so that neither log is lost.", to.name));
        }
        // the new file first, then the log, the old file last: whatever fails, nothing is lost
        host.write(target, document_text).map_err(|e| format!("{target} could not be written: {e}"))?;
        if host.exists(&sidecar.dir()) {
            to.adopt(host, sidecar).map_err(|e| format!("The document is at its new place, but its AI log could not be moved from {}: {e}", sidecar.dir()))?;
            to.ensure(host, document_text).map_err(error)?;
        }
        to.take_settings(host, sidecar);
        if let Some(project) = &self.project {
            project_files::replace(host, project, &sidecar.document, target)?;
        }
        host.remove(&sidecar.document).map_err(|e| format!("The document and its log are at the new place; the old file could not be removed: {e}"))
    }

    /// The project of several files this document is one of, with all its files; None for a document that is a project of its own.
    pub fn project_overview(&self, document_text: &str) -> Result<Option<project_files::Overview>, String> {
        let Some(project) = &self.project else { return Ok(None) };
        project_files::overview(self.workspace.host.as_ref(), self.projects.as_deref(), project, Some((&self.workspace.sidecar.document, document_text))).map(Some)
    }

    /// Makes a project of several files at `path` with this document as its first. The document is
    /// to be opened again afterwards: its experts and settings are the project's from then on.
    pub fn make_project(&self, path: &str) -> Result<String, String> {
        project_files::create(self.workspace.host.as_ref(), self.projects.as_deref(), path, &self.workspace.sidecar.document)
    }

    fn in_project(&self) -> Result<&str, String> {
        self.project.as_deref().ok_or_else(|| "This document is not a file of a project.".to_string())
    }

    /// Adds a markdown file to the project; one that is not there yet is created empty.
    pub fn add_project_file(&self, path: &str) -> Result<(), String> {
        project_files::add(self.workspace.host.as_ref(), self.in_project()?, path)
    }

    /// Takes a file out of the project; the file and its log stay.
    pub fn remove_project_file(&self, path: &str) -> Result<(), String> {
        project_files::remove(self.workspace.host.as_ref(), self.in_project()?, path)
    }

    pub fn order_project_files(&self, paths: &[String]) -> Result<(), String> {
        project_files::order(self.workspace.host.as_ref(), self.in_project()?, paths)
    }

    /// Renames another file of the project. The open document is moved with "Move…", which takes the text in the window.
    pub fn rename_project_file(&self, path: &str, name: &str) -> Result<String, String> {
        if picture::same_file(path, &self.workspace.sidecar.document) {
            return Err("The open document is renamed with \"Move…\".".into());
        }
        project_files::rename(self.workspace.host.as_ref(), self.projects.as_deref(), self.in_project()?, path, name)
    }

    pub fn set_project_goal(&self, goal: &str, words: Option<u32>) -> Result<(), String> {
        project_files::set_goal(self.workspace.host.as_ref(), self.in_project()?, goal, words)
    }

    /// The author's comment on a logged passage of AI text. The entry itself is never changed or removed.
    pub fn comment_ai_text(&self, id: &str, note: &str) -> Result<(), String> {
        let entries = self.workspace.log.read().map_err(error)?;
        if !entries.iter().any(|e| matches!(e, LogEntry::AiText(found) if found.id == id)) {
            return Err(format!("The log has no AI text entry {id}."));
        }
        let entry = AnnotationEntry { reference: id.to_string(), ts: self.workspace.host.now(), use_: "info".into(), note: Some(note.trim().to_string()), ..Default::default() };
        self.workspace.log.append(&LogEntry::Annotation(entry)).map_err(error)
    }

    /// The author states AI use outside the editor, or corrects an earlier statement (`id` given).
    /// The log only grows: a correction is a new entry that names the one it replaces. What the
    /// author adds (`add`) is kept in the sidecar's `external/`; a kept file that a correction no
    /// longer names is taken away, and the log still says that it was there.
    pub fn state_external(&self, stated: &ExternalUse) -> Result<(), String> {
        let one_line = |text: &str, most: usize| text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(most).collect::<String>();
        let (tool, purpose) = (one_line(&stated.tool, 80), one_line(&stated.purpose, 600));
        if tool.is_empty() || purpose.is_empty() {
            return Err("A statement of AI use needs the tool and what it was used for.".into());
        }
        let (host, folder) = (self.workspace.host.as_ref(), self.workspace.sidecar.external_dir());
        let before = if stated.id.is_empty() { Vec::new() } else { self.standing(&stated.id)?.attachments };
        let (mut attachments, dropped): (Vec<_>, Vec<_>) = before.into_iter().partition(|old| stated.attachments.iter().any(|kept| kept.file == old.file));
        if !stated.add.is_empty() {
            self.workspace.sidecar.ensure(host, &self.saved.lock().unwrap()).map_err(error)?;
        }
        let mut added = Vec::new();
        let kept = stated.add.iter().try_for_each(|new| {
            added.extend(outside::keep(&folder, new, &|| host.new_id())?);
            if attachments.len() + added.len() > outside::MOST {
                return Err(format!("A statement holds {} files at most.", outside::MOST));
            }
            Ok(())
        });
        attachments.extend(added.iter().cloned());
        let logged = kept.and_then(|()| {
            self.append_external(ExternalEntry {
                tool,
                purpose,
                model: one_line(&stated.model, 80),
                date: one_line(&stated.date, 40),
                material: one_line(&stated.material, 600),
                result: one_line(&stated.result, 600),
                checked: one_line(&stated.checked, 600),
                attachments,
                replaces: Some(stated.id.clone()).filter(|id| !id.is_empty()),
                ..Default::default()
            })
        });
        // what was written for a statement that did not come about is not left behind
        for gone in if logged.is_ok() { &dropped } else { &added } {
            outside::remove(&folder, gone);
        }
        logged
    }

    /// The author takes a statement of outside AI use back. It stays in the log, and is no longer
    /// in the disclosure; what was kept with it is taken away.
    pub fn withdraw_external(&self, id: &str) -> Result<(), String> {
        let kept = self.standing(id)?.attachments;
        self.append_external(ExternalEntry { replaces: Some(id.to_string()), ..Default::default() })?;
        for gone in &kept {
            outside::remove(&self.workspace.sidecar.external_dir(), gone);
        }
        Ok(())
    }

    /// The statement with this id, if it holds now.
    fn standing(&self, id: &str) -> Result<ExternalEntry, String> {
        let entries = self.workspace.log.read().map_err(error)?;
        cowriting_core::types::external_uses(&entries).into_iter().find(|e| e.id == id).ok_or_else(|| "That statement is not in the log, or was already replaced.".to_string())
    }

    fn append_external(&self, mut entry: ExternalEntry) -> Result<(), String> {
        let (host, workspace) = (self.workspace.host.as_ref(), &self.workspace);
        if let Some(old) = &entry.replaces {
            self.standing(old)?;
        }
        workspace.sidecar.ensure(host, &self.saved.lock().unwrap()).map_err(error)?;
        entry.id = host.new_id();
        entry.ts = host.now();
        entry.session = workspace.session.clone();
        entry.paper = workspace.sidecar.name.clone();
        workspace.log.append(&LogEntry::External(entry)).map_err(error)
    }

    /// The answers in the chats the author kept, to compare what enters the document with; and
    /// for each, the tool and what was asked.
    fn outside_sources(&self, entries: &[LogEntry]) -> (Vec<Source>, BTreeMap<String, (String, String)>) {
        let folder = self.workspace.sidecar.external_dir();
        let (mut sources, mut about) = (Vec::new(), BTreeMap::new());
        for stated in cowriting_core::types::external_uses(entries) {
            for attachment in stated.attachments.iter().filter(|attachment| attachment.kind == "chat") {
                let Kept::Chat(chat) = outside::found(&folder, attachment) else { continue };
                for (number, turn) in chat.turns.iter().enumerate() {
                    let reference = format!("external:{}#{}", attachment.file, number + 1);
                    sources.push(Source::new(&reference, "text", &turn.answer, &[&turn.prompt]));
                    about.insert(reference, (if chat.tool.is_empty() { stated.tool.clone() } else { chat.tool.clone() }, turn.prompt.clone()));
                }
            }
        }
        (sources, about)
    }

    /// Ends the answer under way: what was read so far is logged as an incomplete answer.
    pub fn stop_answer(&self) {
        self.workspace.stop.ask();
    }

    pub async fn ask(&self, backend: &dyn Backend, ask: Ask, on_event: &mut (dyn FnMut(TurnEvent) + Send)) -> Result<TurnOutcome, String> {
        run_turn(&self.workspace, backend, ask, on_event).await.map_err(error)
    }

    /// The document's experts, who is in its party, and what the author told them.
    pub fn party_view(&self) -> PartyView {
        let party = self.workspace.party();
        let mut briefs = parse_briefs(&self.workspace.host.read(&self.workspace.sidecar.party_md()).unwrap_or_default(), &party);
        let experts = party
            .all()
            .iter()
            .map(|p| ExpertView {
                is_member: party.is_member(&p.id),
                brief: briefs.remove(&p.id).unwrap_or_default(),
                is_prompt_edited: cowriting_core::personas::standard_prompt(&p.id).is_some_and(|standard| standard.trim() != p.prompt.trim()),
                persona: p.clone(),
            })
            .collect();
        PartyView { experts, everyone: briefs.remove("everyone").unwrap_or_default(), shared_palette: shared_palette().clone() }
    }

    /// Sets what the author tells one expert (by id) or 'everyone' about this document.
    pub fn set_brief(&self, key: &str, body: &str) -> Result<(), String> {
        let (host, file) = (self.workspace.host.as_ref(), self.workspace.sidecar.party_md());
        let before = host.read(&file).unwrap_or_else(|_| "# What the experts should know about this document\n".to_string());
        let after = set_brief(&before, key, body, &self.workspace.party()).ok_or_else(|| format!("This document's party has no expert “{key}”."))?;
        host.write(&file, &after).map_err(|e| format!("The briefing could not be saved: {e}"))
    }

    /// Takes an expert into the document's party or out of it.
    pub fn set_member(&self, id: &str, is_member: bool) -> Result<(), String> {
        let party = self.workspace.party();
        if party.by_id(id).is_none() {
            return Err(format!("This document's party has no expert “{id}”."));
        }
        let mut members = party.member_ids();
        members.retain(|m| m != id);
        if is_member {
            members.push(id.to_string());
        }
        let (host, file) = (self.workspace.host.as_ref(), self.workspace.sidecar.party_json());
        let mut json = stored::read(host, &file, &stored::PARTY).unwrap_or_default();
        json.insert("members".into(), serde_json::json!(members));
        stored::write(host, &file, &stored::PARTY, &json).map_err(|e| format!("The party could not be saved: {e}"))
    }

    /// Makes an expert of the author's own for this document, and returns its id.
    pub fn add_expert(&self, name: &str, role: &str, looks_at: &str) -> Result<String, String> {
        let (name, role, looks_at) = (name.trim(), role.trim(), looks_at.trim());
        if name.is_empty() || role.is_empty() || looks_at.is_empty() {
            return Err("An expert needs a name, a field, and a few words on what they look at.".into());
        }
        let id = id_from_name(name);
        let party = self.workspace.party();
        if !is_custom_id(&id) || party.by_id(&id).is_some() {
            return Err(format!("There is already an expert called “{name}”, or the name has no letters to make an id from."));
        }
        let host = self.workspace.host.as_ref();
        host.write(&format!("{}/{id}.md", self.workspace.sidecar.party_dir()), &persona_file(&id, name, role, looks_at))
            .map_err(|e| format!("The expert could not be saved: {e}"))?;
        // a document that chose its roster takes the new expert into it
        if host.exists(&self.workspace.sidecar.party_json()) {
            self.set_member(&id, true)?;
        }
        Ok(id)
    }

    /// The hints the author marked as done, by the answer they are in: each hint by its place among
    /// the answer's list items and table rows, counted from 0. A working aid for going through a
    /// long answer; the log and the disclosure do not hold it.
    pub fn hint_marks(&self) -> BTreeMap<String, Vec<usize>> {
        let json = stored::read(self.workspace.host.as_ref(), &self.workspace.sidecar.marks_json(), &stored::MARKS).unwrap_or_default();
        json.get("turns").and_then(|turns| serde_json::from_value(turns.clone()).ok()).unwrap_or_default()
    }

    /// Sets which hints of one answer are done; none takes the answer off the list.
    pub fn set_hint_marks(&self, turn: &str, done: &[usize]) -> Result<(), String> {
        let (host, file) = (self.workspace.host.as_ref(), self.workspace.sidecar.marks_json());
        if !self.workspace.log.read().map_err(error)?.iter().any(|entry| matches!(entry, LogEntry::Prompt(prompt) if prompt.id == turn)) {
            return Err(format!("The log has no answer “{turn}” to mark hints in."));
        }
        let mut json = stored::read(host, &file, &stored::MARKS).unwrap_or_default();
        let mut turns = json.get("turns").and_then(|turns| turns.as_object().cloned()).unwrap_or_default();
        let mut done: Vec<usize> = done.to_vec();
        done.sort_unstable();
        done.dedup();
        if done.is_empty() {
            turns.remove(turn);
        } else {
            turns.insert(turn.to_string(), serde_json::json!(done));
        }
        json.insert("turns".into(), serde_json::Value::Object(turns));
        stored::write(host, &file, &stored::MARKS, &json).map_err(|e| format!("The marks could not be saved: {e}"))
    }

    /// What the author did with an answer. A reference that begins with `external:` is about a
    /// chat kept with a statement of outside use, named by its file: classified like a turn of
    /// the editor's own, as one whole.
    pub fn annotate(&self, reference: &str, use_: &str, note: &str) -> Result<(), String> {
        let Some(file) = reference.strip_prefix(cowriting_core::types::KEPT) else { return annotate(&self.workspace, reference, use_, note) };
        if !cowriting_core::turn::USES.contains(&use_) {
            return Err(format!("{use_} is not one of {}.", cowriting_core::turn::USES.join(", ")));
        }
        let entries = self.workspace.log.read().map_err(error)?;
        let is_kept = cowriting_core::types::external_uses(&entries).iter().any(|stated| stated.attachments.iter().any(|kept| kept.file == file && kept.kind != "file"));
        if !is_kept {
            return Err(format!("No statement that holds now keeps a chat {file}."));
        }
        let note = note.trim();
        let entry = AnnotationEntry {
            reference: reference.to_string(),
            ts: self.workspace.host.now(),
            use_: use_.to_string(),
            note: (!note.is_empty()).then(|| note.to_string()),
            ..Default::default()
        };
        self.workspace.log.append(&LogEntry::Annotation(entry)).map_err(error)
    }

    pub fn thread(&self, id: &str) -> Result<Vec<ThreadMessage>, String> {
        Ok(thread_of(&self.workspace.log.read().map_err(error)?, id, &self.workspace.party(), "Assistant"))
    }

    /// The log as the Log tab shows it. `document_text` is the editor's text, for the word count.
    pub fn log_view(&self, document_text: &str) -> Result<LogView, String> {
        let entries = self.workspace.log.read().map_err(error)?;
        let today: String = self.workspace.host.now().chars().take(10).collect();
        let party = self.workspace.party();
        let mut rows: Vec<TurnRow> = turns(&entries)
            .iter()
            .map(|t| TurnRow {
                id: t.prompt.id.clone(),
                ts: t.prompt.ts.clone(),
                category: t.category.clone(),
                asked: readable(&t.prompt.text),
                answer: readable(&t.response.answer),
                excerpt: flatten(&t.response.answer).chars().take(240).collect(),
                model: t.response.model.clone().unwrap_or_default(),
                provider: t.response.provider.clone().unwrap_or_default(),
                tools_used: t.response.tools_used.clone(),
                use_: t.annotation.map(|a| a.use_.clone()).unwrap_or_default(),
                note: t.annotation.and_then(|a| a.note.clone()).unwrap_or_default(),
                is_open: t.is_open,
                is_ai_text: t.response.ai_text_flag,
                reply_to: t.prompt.reply_to.clone(),
                persona: t.response.personas.first().cloned().unwrap_or_default(),
                who: t.response.personas.first().map(|id| party.by_id(id).map_or(id.clone(), |p| p.name.clone())).unwrap_or_default(),
            })
            .collect();
        rows.reverse();
        let guard = entries
            .iter()
            .filter_map(|e| if let LogEntry::Guard(g) = e { Some(g) } else { None })
            .map(|g| GuardRow { ts: g.ts.clone(), tool: g.tool.clone(), input: g.input.clone(), files: g.files.clone(), refused: g.refused, turn: g.reference.clone() })
            .rev()
            .collect();
        let ai_text = entries.iter().rev().filter_map(|e| if let LogEntry::AiText(found) = e { Some(ai_text_row(found, &entries, document_text)) } else { None }).collect();
        let kept_uses = cowriting_core::types::kept_uses(&entries);
        let external = cowriting_core::types::external_uses(&entries)
            .into_iter()
            .map(|e| {
                let attachments = e.attachments.iter().map(|a| self.attached(a, &kept_uses)).collect();
                ExternalUse {
                    id: e.id,
                    tool: e.tool,
                    model: e.model,
                    date: e.date,
                    purpose: e.purpose,
                    material: e.material,
                    result: e.result,
                    checked: e.checked,
                    attachments,
                    add: Vec::new(),
                }
            })
            .collect();
        Ok(LogView {
            stats: compute_stats(&entries, &today, &party),
            turns: rows,
            guard,
            ai_text,
            external,
            words: count_words(document_text, Format::Markdown),
            chars: count_chars(document_text, Format::Markdown),
        })
    }

    fn attached(&self, a: &cowriting_core::types::Attachment, uses: &std::collections::HashMap<String, AnnotationEntry>) -> Attached {
        let state = outside::state(&self.workspace.sidecar.external_dir(), a).to_string();
        let (use_, note) = uses.get(&a.file).map(|said| (said.use_.clone(), said.note.clone().unwrap_or_default())).unwrap_or_default();
        Attached {
            file: a.file.clone(),
            kind: a.kind.clone(),
            bytes: a.bytes,
            sha256: a.sha256.clone(),
            turns: a.turns,
            title: a.title.clone(),
            from: a.from.clone(),
            state,
            use_,
            note,
        }
    }

    /// Writes the AI register, and the appendices asked for, into the sidecar's `disclosure/`.
    pub fn export(&self, options: &ExportOptions) -> Result<Exported, String> {
        let lang = if options.language == "de" { Lang::De } else { Lang::En };
        let (format, extension) = if options.format == "tex" { (DocFormat::Tex, "tex") } else { (DocFormat::Md, "md") };
        let language = if lang == Lang::De { "de" } else { "en" };
        let host = self.workspace.host.as_ref();
        let whole = if options.whole_project { Some(self.whole_record()?) } else { None };
        let (entries, name, folder, document, kept) = match whole {
            Some(whole) => (whole.entries, whole.name, whole.folder, whole.text, whole.kept),
            None => {
                // the saved document says which logged passages of AI text are still in it
                let sidecar = &self.workspace.sidecar;
                (self.workspace.log.read().map_err(error)?, sidecar.name.clone(), sidecar.disclosure_dir(), host.read(&sidecar.document).ok(), vec![sidecar.external_dir()])
            }
        };
        let name = &name;
        let register = disclosure_with_document(&entries, name, lang, format, Profile::Editor, document.as_deref());
        let mut paths = vec![format!("{folder}/ai-register.{language}.{extension}")];
        host.write(&paths[0], &register).map_err(|e| format!("The disclosure could not be written: {e}"))?;
        if options.with_prompts {
            let appendix = prompts_appendix(&entries, name, lang, format, options.with_answers, Profile::Editor);
            paths.push(format!("{folder}/ai-prompts.{language}.{extension}"));
            host.write(&paths[1], &appendix).map_err(|e| format!("The prompts appendix could not be written: {e}"))?;
        }
        if options.with_outside {
            // a kept chat is in the folder of the file it was stated in
            let found = |attachment: &cowriting_core::types::Attachment| {
                let all = kept.iter().map(|folder| outside::found(folder, attachment));
                all.reduce(|best, next| if matches!(best, cowriting_core::disclosure::Kept::Missing) { next } else { best }).unwrap_or(cowriting_core::disclosure::Kept::Missing)
            };
            let appendix = outside_appendix(&entries, &found, name, lang, format);
            let path = format!("{folder}/ai-outside.{language}.{extension}");
            host.write(&path, &appendix).map_err(|e| format!("The appendix of outside AI use could not be written: {e}"))?;
            paths.push(path);
        }
        Ok(Exported { paths, register })
    }
}

/// The record of AI use of a whole project: what its disclosure is made of.
struct WholeRecord {
    /// The entries of every file's log, in the order of their time.
    entries: Vec<LogEntry>,
    /// The project with its files, as the disclosure names what it is about.
    name: String,
    /// Where the disclosure is written: the project's file has a folder in `.cowriting` as a document has.
    folder: String,
    /// The saved files as one text; None when one of them is not there.
    text: Option<String>,
    /// The folders of kept chats, one per file.
    kept: Vec<String>,
}

impl Session {
    fn whole_record(&self) -> Result<WholeRecord, String> {
        let (host, project) = (self.workspace.host.as_ref(), self.in_project()?);
        let members = project_files::members(host, project)?;
        let (mut entries, mut kept) = (Vec::new(), Vec::new());
        for (_, path) in &members {
            // the open document's log is read like the others: from disk, where every answer is at once
            let sidecar = cowriting_core::sidecar::Sidecar::for_document(path);
            let log = cowriting_core::log::Log::new(self.workspace.host.clone(), sidecar.log_dir(), sidecar.rewrites_dir());
            entries.extend(log.read().map_err(|e| format!("The AI log of {path} could not be read: {e}"))?);
            kept.push(sidecar.external_dir());
        }
        let time = |entry: &LogEntry| -> String {
            match entry {
                LogEntry::Prompt(e) => e.ts.clone(),
                LogEntry::Response(e) => e.ts.clone(),
                LogEntry::Annotation(e) => e.ts.clone(),
                LogEntry::Guard(e) => e.ts.clone(),
                LogEntry::AiText(e) => e.ts.clone(),
                LogEntry::External(e) => e.ts.clone(),
                LogEntry::Other(value) => value["ts"].as_str().unwrap_or_default().to_string(),
            }
        };
        entries.sort_by_cached_key(time);
        let names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();
        let name = format!("{} ({})", cowriting_core::project_file::ProjectFile::name_of(project), names.join(", "));
        let text = project_files::whole(host, project, None).ok().map(|whole| whole.text);
        Ok(WholeRecord { entries, name, folder: project_files::own_sidecar(project).disclosure_dir(), text, kept })
    }
}

/// What this computer has to export with, and whether Zotero can give the references.
pub async fn export_tools() -> ExportTools {
    ExportTools { has_zotero: cowriting_backends::lookup::has_better_bibtex().await, ..export::local_tools_with_fonts() }
}

fn orphans_of(workspace: &Workspace, text: &str) -> Vec<Orphan> {
    let hash = cowriting_core::sidecar::content_hash(text);
    let questions = |other: &cowriting_core::sidecar::Sidecar| {
        let log = cowriting_core::log::Log::new(workspace.host.clone(), other.log_dir(), other.rewrites_dir());
        log.read().map_or(0, |entries| entries.iter().filter(|e| matches!(e, LogEntry::Prompt(_))).count())
    };
    let mut found: Vec<Orphan> = workspace
        .sidecar
        .orphans(workspace.host.as_ref())
        .iter()
        .map(|(other, info)| Orphan { name: other.name.clone(), is_same_text: info.last_hash == hash, questions: questions(other) })
        .collect();
    // the one with this very text first
    found.sort_by_key(|orphan| !orphan.is_same_text);
    found
}
