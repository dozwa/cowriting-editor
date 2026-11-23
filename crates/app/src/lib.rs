//! One open document, as the editor's window uses it: open, save, ask, classify, read the log,
//! export the disclosure. The window (`src-tauri`) only passes these on, so everything here runs
//! and is tested without one.

use cowriting_backends::anthropic::AnthropicBackend;
use cowriting_backends::fake::FakeBackend;
use cowriting_core::backend::Backend;
use cowriting_core::disclosure::{disclosure, prompts_appendix, turns, DocFormat, Lang, Profile};
use cowriting_core::host::Host;
use cowriting_core::log::readable;
use cowriting_core::stats::compute_stats;
use cowriting_core::thread::thread_of;
use cowriting_core::turn::{annotate, run_turn, Ask, TurnEvent, TurnOutcome, Workspace};
use cowriting_core::types::{LogEntry, Stats, ThreadMessage};
use cowriting_core::words::{count_words, Format};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    pub words: usize,
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
    /// 'anthropic' for now; 'fake:<script file>' plays a script instead of asking a model.
    pub backend: String,
    pub model: String,
    /// A gateway that speaks the Claude API; empty for Anthropic itself.
    #[serde(default)]
    pub base_url: String,
}

impl Default for BackendSettings {
    fn default() -> Self {
        BackendSettings { backend: "anthropic".into(), model: "claude-opus-5-5".into(), base_url: String::new() }
    }
}

/// The backend the settings name. `api_key` is what the key store holds for it, if anything.
pub fn make_backend(settings: &BackendSettings, api_key: Option<&str>) -> Result<Box<dyn Backend>, String> {
    if let Some(script) = settings.backend.strip_prefix("fake:") {
        let json = std::fs::read_to_string(script).map_err(|e| format!("The fake backend's script {script} could not be read: {e}"))?;
        return Ok(Box::new(FakeBackend::from_json(&json)?));
    }
    match settings.backend.as_str() {
        "anthropic" => match api_key.map(str::trim).filter(|k| !k.is_empty()) {
            Some(key) => Ok(Box::new(AnthropicBackend::new(key, Some(&settings.base_url)))),
            None => Err("No API key is set yet. Add one in the settings to ask the AI.".into()),
        },
        other => Err(format!("This version cannot use the backend “{other}” yet.")),
    }
}

pub struct Session {
    pub workspace: Workspace,
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
        Ok((Session { workspace }, opened))
    }

    /// The only code that writes the document: the author's own save.
    pub fn save(&self, text: &str) -> Result<(), String> {
        let host = self.workspace.host.as_ref();
        host.write(&self.workspace.sidecar.document, text).map_err(|e| format!("The document could not be saved: {e}"))?;
        // a document that has a sidecar keeps it up to date; one that never asked the AI gets none
        if self.workspace.sidecar.info(host).is_some() {
            self.workspace.sidecar.ensure(host, text).map_err(error)?;
        }
        Ok(())
    }

    pub async fn ask(&self, backend: &dyn Backend, ask: Ask, on_event: &mut (dyn FnMut(TurnEvent) + Send)) -> Result<TurnOutcome, String> {
        run_turn(&self.workspace, backend, ask, on_event).await.map_err(error)
    }

    pub fn annotate(&self, reference: &str, use_: &str, note: &str) -> Result<(), String> {
        annotate(&self.workspace, reference, use_, note)
    }

    pub fn thread(&self, id: &str) -> Result<Vec<ThreadMessage>, String> {
        Ok(thread_of(&self.workspace.log.read().map_err(error)?, id, &self.workspace.party, "Assistant"))
    }

    /// The log as the Log tab shows it. `document_text` is the editor's text, for the word count.
    pub fn log_view(&self, document_text: &str) -> Result<LogView, String> {
        let entries = self.workspace.log.read().map_err(error)?;
        let today: String = self.workspace.host.now().chars().take(10).collect();
        let mut rows: Vec<TurnRow> = turns(&entries)
            .iter()
            .map(|t| TurnRow {
                id: t.prompt.id.clone(),
                ts: t.prompt.ts.clone(),
                category: t.category.clone(),
                asked: readable(&t.prompt.text),
                answer: readable(&t.response.answer),
                model: t.response.model.clone().unwrap_or_default(),
                provider: t.response.provider.clone().unwrap_or_default(),
                tools_used: t.response.tools_used.clone(),
                use_: t.annotation.map(|a| a.use_.clone()).unwrap_or_default(),
                note: t.annotation.and_then(|a| a.note.clone()).unwrap_or_default(),
                is_open: t.is_open,
                is_ai_text: t.response.ai_text_flag,
                reply_to: t.prompt.reply_to.clone(),
            })
            .collect();
        rows.reverse();
        let guard = entries
            .iter()
            .filter_map(|e| if let LogEntry::Guard(g) = e { Some(g) } else { None })
            .map(|g| GuardRow { ts: g.ts.clone(), tool: g.tool.clone(), input: g.input.clone(), files: g.files.clone(), refused: g.refused, turn: g.reference.clone() })
            .rev()
            .collect();
        Ok(LogView { stats: compute_stats(&entries, &today, &self.workspace.party), turns: rows, guard, words: count_words(document_text, Format::Markdown) })
    }

    /// Writes the AI register, and the prompts appendix if asked for, into the sidecar's `disclosure/`.
    pub fn export(&self, options: &ExportOptions) -> Result<Exported, String> {
        let lang = if options.language == "de" { Lang::De } else { Lang::En };
        let (format, extension) = if options.format == "tex" { (DocFormat::Tex, "tex") } else { (DocFormat::Md, "md") };
        let language = if lang == Lang::De { "de" } else { "en" };
        let entries = self.workspace.log.read().map_err(error)?;
        let (host, name, folder) = (self.workspace.host.as_ref(), &self.workspace.sidecar.name, self.workspace.sidecar.disclosure_dir());
        let register = disclosure(&entries, name, lang, format, Profile::Editor);
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
