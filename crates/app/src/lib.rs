//! One open document, as the editor's window uses it: open, save, ask, classify, read the log,
//! export the disclosure. The window (`src-tauri`) only passes these on, so everything here runs
//! and is tested without one.

pub mod export;

use cowriting_backends::anthropic::AnthropicBackend;
use cowriting_backends::fake::FakeBackend;
use cowriting_backends::openai::{OpenAiBackend, OPENAI_BASE_URL};
use cowriting_core::backend::Backend;
use cowriting_core::briefs::{parse_briefs, set_brief};
use cowriting_core::disclosure::{disclosure_with_document, prompts_appendix, turns, DocFormat, Lang, Profile};
use cowriting_core::host::Host;
use cowriting_core::log::{flatten, readable};
use cowriting_core::paste_tracking::{check, is_logged, sources, still_in_text, Found};
use cowriting_core::personas::{id_from_name, is_custom_id, persona_file, shared_palette, Persona};
use cowriting_core::review::{check_points, live_review, read_check, read_plan, CheckSummary, LiveHint, LiveOutcome, LiveSettings, LiveState};
use cowriting_core::scope::{scope_progress, ScopeProgress};
use cowriting_core::sources::{added_files, check_references, cited_keys, format_name, load_bibliography, save_added_files, search, BibFile, CheckedRef, Hit, References, Status};
use cowriting_core::stats::compute_stats;
use cowriting_core::thread::thread_of;
use cowriting_core::turn::{annotate, run_turn, Ask, TurnEvent, TurnOutcome, Workspace};
use cowriting_core::types::{AiTextEntry, AnnotationEntry};
use cowriting_core::types::{LogEntry, Stats, ThreadMessage};
use cowriting_core::watch::PAUSE_MS;
use cowriting_core::words::{changed_paragraphs, paragraphs};
use cowriting_core::words::{count_words, Format};
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
    pub words: usize,
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
    pub asked: String,
    /// The author's comment on it.
    pub note: String,
    pub still_in_text: bool,
}

#[derive(Debug, Clone, Deserialize)]
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
    /// 'anthropic' or 'openai-compatible'; 'fake:<script file>' plays a script instead of asking a model.
    pub backend: String,
    pub model: String,
    /// Where the API is: a gateway, or a local model server. Empty for Anthropic or OpenAI itself.
    #[serde(default)]
    pub base_url: String,
}

impl Default for BackendSettings {
    fn default() -> Self {
        BackendSettings { backend: "anthropic".into(), model: "claude-opus-5-5".into(), base_url: String::new() }
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
            Some(key) => Ok(Box::new(AnthropicBackend::new(key, Some(&settings.base_url)))),
            None => Err(NO_KEY.into()),
        },
        "openai-compatible" => {
            let key = api_key.map(str::trim).filter(|k| !k.is_empty());
            let base_url = settings.base_url.trim();
            // OpenAI itself always wants a key; a local server or a gateway may not
            if key.is_none() && (base_url.is_empty() || base_url.trim_end_matches('/') == OPENAI_BASE_URL) {
                return Err(NO_KEY.into());
            }
            Ok(Box::new(OpenAiBackend::new(base_url, key)))
        }
        other => Err(format!("This version cannot use the backend “{other}” yet.")),
    }
}

/// The models the provider in these settings offers: (id, display name).
pub async fn list_models(settings: &BackendSettings, api_key: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let key = api_key.map(str::trim).filter(|k| !k.is_empty());
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
}

/// What a reference check found (`verify.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifyFile {
    pub ts: String,
    pub rows: Vec<CheckedRef>,
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
    /// None while the plan says nothing yet.
    pub progress: Option<ScopeProgress>,
}

/// What a new scope.md starts from.
pub const SCOPE_TEMPLATE: &str = "# Scope

## Goal
<!-- One or two sentences: what should the reader know or believe at the end? -->

## Targets
- Words: 
- Figures: 
- Tables: 
- References: 
- Deadline: 

## Sections
<!-- One line per section: name, target in words, stage (outline, draft, revised, final).
     For example:  - Methods: 800, draft -->

## Points
<!-- What the text must say. Tick a point yourself with [x], or let the check look for it. -->
- [ ] 
";

fn ai_text_row(found: &AiTextEntry, entries: &[LogEntry], document_text: &str) -> AiTextRow {
    let asked = entries.iter().find_map(|e| match e {
        LogEntry::Prompt(p) if p.id == found.reference => Some(readable(&p.text)),
        _ => None,
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
        turn: found.reference.clone(),
        asked: asked.unwrap_or_default(),
        note: note.unwrap_or_default(),
        still_in_text: still_in_text(found, document_text),
    }
}

fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Session {
    /// Opens a markdown file. Nothing is created beside it until the AI is first asked.
    pub fn open(host: Arc<dyn Host>, path: &str) -> Result<(Session, Opened), String> {
        let text = host.read(path).map_err(|e| format!("{path} could not be opened: {e}"))?;
        let workspace = Workspace::open(host, path, EDITOR_VERSION);
        let opened = Opened { path: path.to_string(), name: workspace.sidecar.name.clone(), text, language: workspace.language.clone() };
        // live review left on: what is in the document now is the baseline, as when it is turned on
        let baseline = LiveSettings::load(&workspace).is_on.then(|| opened.text.clone());
        let live = Mutex::new(LiveState { baseline, ..Default::default() });
        Ok((Session { workspace, saved: Mutex::new(opened.text.clone()), live, is_reviewing: AtomicBool::new(false) }, opened))
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
        let known = sources(&entries, &|id| host.read(&format!("{snapshots}/{id}.md")).ok());
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
            self.workspace.log.append(&LogEntry::AiText(entry.clone())).map_err(error)?;
            entries.push(LogEntry::AiText(entry.clone()));
            rows.push(ai_text_row(&entry, &entries, document_text));
        }
        Ok(rows)
    }

    pub fn live_view(&self) -> LiveView {
        let settings = LiveSettings::load(&self.workspace);
        LiveView { is_on: settings.is_on, personas: settings.personas, hints: self.live.lock().unwrap().hints.clone(), pause_ms: PAUSE_MS }
    }

    /// Turns live review on or off and says who watches. What is in the document now is not reviewed: only what changes from here on.
    pub fn set_live(&self, is_on: bool, personas: &[String], document_text: &str) -> Result<(), String> {
        let party = self.workspace.party();
        if let Some(unknown) = personas.iter().find(|id| party.by_id(id).is_none()) {
            return Err(format!("This document's party has no expert “{unknown}”."));
        }
        self.workspace.sidecar.ensure(self.workspace.host.as_ref(), document_text).map_err(error)?;
        LiveSettings { is_on, personas: personas.to_vec() }.save(&self.workspace).map_err(|e| format!("The setting could not be saved: {e}"))?;
        self.live.lock().unwrap().baseline = is_on.then(|| document_text.to_string());
        Ok(())
    }

    /// Called when the author has paused: reviews what changed since the last review, if it is time.
    pub async fn live_review(&self, backend: &dyn Backend, model: &str, document_text: &str) -> Result<LiveOutcome, String> {
        // one review at a time; a second call while one runs waits for the next pause
        if self.is_reviewing.swap(true, Ordering::SeqCst) {
            return Ok(LiveOutcome::Wait { ms: PAUSE_MS });
        }
        let settings = LiveSettings::load(&self.workspace);
        let mut state = self.live.lock().unwrap().clone();
        let outcome = live_review(&self.workspace, backend, model, &settings, &mut state, document_text).await;
        *self.live.lock().unwrap() = state;
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
        ScopeView { text, template: SCOPE_TEMPLATE.to_string(), progress }
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
        let checked = host.read(&self.workspace.sidecar.verify_json()).ok().and_then(|text| serde_json::from_str(&text).ok());
        SourcesView { entries: bibliography.entries.len(), files: bibliography.files, cited: cited.len(), missing, checked }
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
    pub fn export_document(&self, format: FileFormat, target: &str, document_text: &str, zotero: Option<&str>, pdf: &PdfChoice) -> Result<DocumentExport, String> {
        let (host, sidecar) = (self.workspace.host.as_ref(), &self.workspace.sidecar);
        export::check_target(format, target, &sidecar.document)?;
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
        let kept = sidecar.export_references_json();
        if let Some(csl) = zotero {
            sidecar.ensure(host, document_text).map_err(error)?;
            host.write(&kept, csl).map_err(|e| format!("The references could not be saved: {e}"))?;
        }
        let used = export::references(&export::bibliography_of(&self.workspace, document_text), document_text, zotero.map(|csl| (kept.as_str(), csl)));
        let args = export::pandoc_args(format, target, &sidecar.folder, &used.files, engine.as_ref().map(|(name, path)| (*name, path.as_str())), pdf.paper);
        let warnings = export::run_pandoc(document_text, &sidecar.folder, &args)?;
        Ok(DocumentExport { path: target.to_string(), cited: cited_keys(document_text).len(), from_zotero: used.from_zotero, missing: used.missing, warnings })
    }

    /// The export with the references looked up in the author's Zotero, when it is running.
    pub async fn export_document_online(&self, format: FileFormat, target: &str, document_text: &str, pdf: &PdfChoice) -> Result<DocumentExport, String> {
        let cited: Vec<String> = cited_keys(document_text).into_iter().collect();
        let zotero = cowriting_backends::lookup::better_bibtex_export(&cited).await;
        self.export_document(format, target, document_text, zotero.as_ref().map(|(csl, _)| csl.as_str()), pdf)
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
        let file = VerifyFile { ts: host.now(), rows };
        sidecar.ensure(host, document_text).map_err(error)?;
        host.write(&sidecar.verify_json(), &format!("{}\n", serde_json::to_string_pretty(&file).expect("plain data")))
            .map_err(|e| format!("The result could not be saved: {e}"))?;
        Ok(file)
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

    pub async fn ask(&self, backend: &dyn Backend, ask: Ask, on_event: &mut (dyn FnMut(TurnEvent) + Send)) -> Result<TurnOutcome, String> {
        run_turn(&self.workspace, backend, ask, on_event).await.map_err(error)
    }

    /// The document's experts, who is in its party, and what the author told them.
    pub fn party_view(&self) -> PartyView {
        let party = self.workspace.party();
        let mut briefs = parse_briefs(&self.workspace.host.read(&self.workspace.sidecar.party_md()).unwrap_or_default(), &party);
        let experts = party.all().iter().map(|p| ExpertView { is_member: party.is_member(&p.id), brief: briefs.remove(&p.id).unwrap_or_default(), persona: p.clone() }).collect();
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
        let text = serde_json::to_string_pretty(&serde_json::json!({ "members": members })).unwrap();
        self.workspace.host.write(&self.workspace.sidecar.party_json(), &text).map_err(|e| format!("The party could not be saved: {e}"))
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

    pub fn annotate(&self, reference: &str, use_: &str, note: &str) -> Result<(), String> {
        annotate(&self.workspace, reference, use_, note)
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
        Ok(LogView { stats: compute_stats(&entries, &today, &party), turns: rows, guard, ai_text, words: count_words(document_text, Format::Markdown) })
    }

    /// Writes the AI register, and the prompts appendix if asked for, into the sidecar's `disclosure/`.
    pub fn export(&self, options: &ExportOptions) -> Result<Exported, String> {
        let lang = if options.language == "de" { Lang::De } else { Lang::En };
        let (format, extension) = if options.format == "tex" { (DocFormat::Tex, "tex") } else { (DocFormat::Md, "md") };
        let language = if lang == Lang::De { "de" } else { "en" };
        let entries = self.workspace.log.read().map_err(error)?;
        let (host, name, folder) = (self.workspace.host.as_ref(), &self.workspace.sidecar.name, self.workspace.sidecar.disclosure_dir());
        // the saved document says which logged passages of AI text are still in it
        let document = host.read(&self.workspace.sidecar.document).ok();
        let register = disclosure_with_document(&entries, name, lang, format, Profile::Editor, document.as_deref());
        let mut paths = vec![format!("{folder}/ai-register.{language}.{extension}")];
        host.write(&paths[0], &register).map_err(|e| format!("The disclosure could not be written: {e}"))?;
        if options.with_prompts {
            let appendix = prompts_appendix(&entries, name, lang, format, options.with_answers, Profile::Editor);
            paths.push(format!("{folder}/ai-prompts.{language}.{extension}"));
            host.write(&paths[1], &appendix).map_err(|e| format!("The prompts appendix could not be written: {e}"))?;
        }
        Ok(Exported { paths, register })
    }
}

/// What this computer has to export with, and whether Zotero can give the references.
pub async fn export_tools() -> ExportTools {
    ExportTools { has_zotero: cowriting_backends::lookup::has_better_bibtex().await, ..export::local_tools() }
}
