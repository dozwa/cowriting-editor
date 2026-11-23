// no console window beside the app on Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window's side of the editor. Every command passes straight on to `cowriting-app`; the
//! web view has no file or network access of its own, so the document, the log and the API key
//! are only ever touched here.

use cowriting_app::{
    list_models, make_backend, model_status, tested_models, AiTextRow, BackendSettings, ExportOptions, Exported, LiveView, LogView, Opened, PartyView, ScopeView, Session,
};
use cowriting_core::host::DiskHost;
use cowriting_core::review::{CheckSummary, LiveOutcome};
use cowriting_core::turn::{Ask, TurnEvent};
use cowriting_core::types::ThreadMessage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tauri::{Manager, State};

const KEY_SERVICE: &str = "io.github.dozwa.cowriting-editor";

#[derive(Default)]
struct Editor {
    session: Mutex<Option<Arc<Session>>>,
}

impl Editor {
    fn session(&self) -> Result<Arc<Session>, String> {
        self.session.lock().unwrap().clone().ok_or_else(|| "No document is open.".to_string())
    }
}

fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json"))
}

/// The settings on disk; `COWRITING_BACKEND` (for tests and trials) overrides the backend.
fn read_settings(app: &tauri::AppHandle) -> BackendSettings {
    let mut settings: BackendSettings =
        settings_file(app).ok().and_then(|file| std::fs::read_to_string(file).ok()).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    if let Ok(backend) = std::env::var("COWRITING_BACKEND") {
        settings.backend = backend;
    }
    settings
}

/// The key the system's key store holds for a backend. An environment variable serves where there is no key store.
fn api_key(backend: &str) -> Option<String> {
    let from_environment = || if backend == "anthropic" { std::env::var("ANTHROPIC_API_KEY").ok() } else { None };
    keyring::Entry::new(KEY_SERVICE, backend).ok().and_then(|entry| entry.get_password().ok()).or_else(from_environment).filter(|key| !key.trim().is_empty())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    #[serde(flatten)]
    settings: BackendSettings,
    /// A key for this backend is in the key store.
    has_key: bool,
    /// The AI can be asked with these settings.
    is_ready: bool,
    /// 'tested', 'failed' or 'untested': whether the model kept to the assist-only rules in the behaviour suite.
    model_status: &'static str,
    /// Nothing was ever set up: the window offers to do it.
    is_first_run: bool,
    /// A document to open at start (`COWRITING_OPEN`), for tests and for "open with".
    open_at_start: Option<String>,
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> SettingsView {
    let settings = read_settings(&app);
    let key = api_key(&settings.backend);
    let is_ready = make_backend(&settings, key.as_deref()).is_ok();
    let model_status = model_status(&tested_models(), &settings.backend, &settings.model);
    let is_first_run = !settings_file(&app).is_ok_and(|file| file.exists()) && std::env::var("COWRITING_BACKEND").is_err();
    SettingsView { settings, has_key: key.is_some(), is_ready, model_status, is_first_run, open_at_start: std::env::var("COWRITING_OPEN").ok() }
}

/// Saves the settings; a key that is given goes to the system's key store and nowhere else.
#[tauri::command]
fn set_settings(app: tauri::AppHandle, settings: BackendSettings, api_key: Option<String>) -> Result<(), String> {
    let file = settings_file(&app)?;
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&file, serde_json::to_string_pretty(&settings).unwrap()).map_err(|e| format!("The settings could not be saved: {e}"))?;
    if let Some(key) = api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        keyring::Entry::new(KEY_SERVICE, &settings.backend)
            .and_then(|entry| entry.set_password(&key))
            .map_err(|e| format!("The key could not be put into the system's key store: {e}"))?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelChoice {
    id: String,
    name: String,
    status: &'static str,
}

/// Asks the provider in these settings (not yet saved) for its models: the connection test.
/// A key that is given is used; otherwise the stored one.
#[tauri::command]
async fn test_connection(settings: BackendSettings, api_key: Option<String>) -> Result<Vec<ModelChoice>, String> {
    let key = api_key.filter(|k| !k.trim().is_empty()).or_else(|| self::api_key(&settings.backend));
    let tested = tested_models();
    let models = list_models(&settings, key.as_deref()).await?;
    Ok(models.into_iter().map(|(id, name)| ModelChoice { status: model_status(&tested, &settings.backend, &id), id, name }).collect())
}

#[tauri::command]
fn open_document(editor: State<Editor>, path: String) -> Result<Opened, String> {
    let (session, opened) = Session::open(Arc::new(DiskHost), &path)?;
    *editor.session.lock().unwrap() = Some(Arc::new(session));
    Ok(opened)
}

/// A new, empty document at a path the author chose in the save dialog.
#[tauri::command]
fn new_document(editor: State<Editor>, path: String) -> Result<Opened, String> {
    if !std::path::Path::new(&path).exists() {
        std::fs::write(&path, "").map_err(|e| format!("{path} could not be created: {e}"))?;
    }
    open_document(editor, path)
}

#[tauri::command]
fn save_document(editor: State<Editor>, text: String) -> Result<Vec<AiTextRow>, String> {
    editor.session()?.save(&text)
}

#[tauri::command]
fn report_paste(editor: State<Editor>, pasted: String, line: usize, document_text: String) -> Result<Vec<AiTextRow>, String> {
    editor.session()?.report_paste(&pasted, line, &document_text)
}

#[tauri::command]
fn comment_ai_text(editor: State<Editor>, id: String, note: String) -> Result<(), String> {
    editor.session()?.comment_ai_text(&id, &note)
}

/// What the window hears while an answer arrives.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum AskEvent {
    Started { id: String },
    Text { text: String },
    Tool { name: String },
    Refused { tool: String, input: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskInput {
    text: String,
    reply_to: Option<String>,
    /// The expert to ask; none for the assistant itself.
    persona: Option<String>,
    /// The editor's text as it is now, saved or not.
    document_text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AskResult {
    id: String,
    answer: String,
    is_ai_text: bool,
    refused: usize,
    cut_off: Option<String>,
    without_tools: bool,
}

#[tauri::command]
async fn ask(app: tauri::AppHandle, editor: State<'_, Editor>, input: AskInput, on_event: Channel<AskEvent>) -> Result<AskResult, String> {
    let session = editor.session()?;
    let settings = read_settings(&app);
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    let command = input.persona.as_ref().map(|_| "ask".to_string());
    let ask = Ask { text: input.text, reply_to: input.reply_to, persona: input.persona, document_text: input.document_text, model: settings.model, command };
    let outcome = session
        .ask(backend.as_ref(), ask, &mut |event| {
            let _ = on_event.send(match event {
                TurnEvent::Started { id } => AskEvent::Started { id },
                TurnEvent::Text(text) => AskEvent::Text { text },
                TurnEvent::Tool { name } => AskEvent::Tool { name },
                TurnEvent::Refused { tool, input } => AskEvent::Refused { tool, input },
            });
        })
        .await?;
    Ok(AskResult {
        id: outcome.id,
        answer: outcome.answer,
        is_ai_text: outcome.ai_text_flag,
        refused: outcome.refused,
        cut_off: outcome.cut_off,
        without_tools: outcome.without_tools,
    })
}

#[tauri::command]
fn live(editor: State<Editor>) -> Result<LiveView, String> {
    Ok(editor.session()?.live_view())
}

#[tauri::command]
fn set_live(editor: State<Editor>, is_on: bool, personas: Vec<String>, document_text: String) -> Result<(), String> {
    editor.session()?.set_live(is_on, &personas, &document_text)
}

#[tauri::command]
async fn live_review(app: tauri::AppHandle, editor: State<'_, Editor>, document_text: String) -> Result<LiveOutcome, String> {
    let session = editor.session()?;
    let settings = read_settings(&app);
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    session.live_review(backend.as_ref(), &settings.model, &document_text).await
}

#[tauri::command]
fn scope(editor: State<Editor>, document_text: String) -> Result<ScopeView, String> {
    Ok(editor.session()?.scope_view(&document_text))
}

#[tauri::command]
fn set_scope(editor: State<Editor>, text: String, document_text: String) -> Result<(), String> {
    editor.session()?.set_scope(&text, &document_text)
}

#[tauri::command]
async fn check_points(app: tauri::AppHandle, editor: State<'_, Editor>, document_text: String) -> Result<CheckSummary, String> {
    let session = editor.session()?;
    let settings = read_settings(&app);
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    session.check_points(backend.as_ref(), &settings.model, &document_text).await
}

#[tauri::command]
fn log_view(editor: State<Editor>, document_text: String) -> Result<LogView, String> {
    editor.session()?.log_view(&document_text)
}

#[tauri::command]
fn thread(editor: State<Editor>, id: String) -> Result<Vec<ThreadMessage>, String> {
    editor.session()?.thread(&id)
}

#[tauri::command]
fn annotate(editor: State<Editor>, reference: String, r#use: String, note: String) -> Result<(), String> {
    editor.session()?.annotate(&reference, &r#use, &note)
}

#[tauri::command]
fn party(editor: State<Editor>) -> Result<PartyView, String> {
    Ok(editor.session()?.party_view())
}

#[tauri::command]
fn set_brief(editor: State<Editor>, key: String, body: String) -> Result<(), String> {
    editor.session()?.set_brief(&key, &body)
}

#[tauri::command]
fn set_member(editor: State<Editor>, id: String, is_member: bool) -> Result<(), String> {
    editor.session()?.set_member(&id, is_member)
}

#[tauri::command]
fn add_expert(editor: State<Editor>, name: String, role: String, looks_at: String) -> Result<String, String> {
    editor.session()?.add_expert(&name, &role, &looks_at)
}

#[tauri::command]
fn export_disclosure(editor: State<Editor>, options: ExportOptions) -> Result<Exported, String> {
    editor.session()?.export(&options)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Editor::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            test_connection,
            open_document,
            new_document,
            save_document,
            ask,
            log_view,
            thread,
            annotate,
            export_disclosure,
            party,
            set_brief,
            set_member,
            add_expert,
            report_paste,
            comment_ai_text,
            live,
            set_live,
            live_review,
            scope,
            set_scope,
            check_points
        ])
        .run(tauri::generate_context!())
        .expect("the editor's window could not be opened");
}
