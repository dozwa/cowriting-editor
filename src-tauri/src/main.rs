// no console window beside the app on Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window's side of the editor. Every command passes straight on to `cowriting-app`; the
//! web view has no file or network access of its own, so the document, the log and the API key
//! are only ever touched here.

use cowriting_app::{make_backend, BackendSettings, ExportOptions, Exported, LogView, Opened, Session};
use cowriting_core::host::DiskHost;
use cowriting_core::turn::{Ask, TurnEvent};
use cowriting_core::types::ThreadMessage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tauri::{Manager, State};

const KEY_SERVICE: &str = "org.cowriting.editor";

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
    keyring::Entry::new(KEY_SERVICE, backend)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .filter(|key| !key.trim().is_empty())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    #[serde(flatten)]
    settings: BackendSettings,
    has_key: bool,
    /// A document to open at start (`COWRITING_OPEN`), for tests and for "open with".
    open_at_start: Option<String>,
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> SettingsView {
    let settings = read_settings(&app);
    let has_key = api_key(&settings.backend).is_some();
    SettingsView { settings, has_key, open_at_start: std::env::var("COWRITING_OPEN").ok() }
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
fn save_document(editor: State<Editor>, text: String) -> Result<(), String> {
    editor.session()?.save(&text)
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
}

#[tauri::command]
async fn ask(app: tauri::AppHandle, editor: State<'_, Editor>, input: AskInput, on_event: Channel<AskEvent>) -> Result<AskResult, String> {
    let session = editor.session()?;
    let settings = read_settings(&app);
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    let ask = Ask { text: input.text, reply_to: input.reply_to, document_text: input.document_text, model: settings.model, command: None };
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
    Ok(AskResult { id: outcome.id, answer: outcome.answer, is_ai_text: outcome.ai_text_flag, refused: outcome.refused, cut_off: outcome.cut_off })
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
fn export_disclosure(editor: State<Editor>, options: ExportOptions) -> Result<Exported, String> {
    editor.session()?.export(&options)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Editor::default())
        .invoke_handler(tauri::generate_handler![get_settings, set_settings, open_document, new_document, save_document, ask, log_view, thread, annotate, export_disclosure])
        .run(tauri::generate_context!())
        .expect("the editor's window could not be opened");
}
