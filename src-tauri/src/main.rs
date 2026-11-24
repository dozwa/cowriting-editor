// no console window beside the app on Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window's side of the editor. Every command passes straight on to `cowriting-app`; the
//! web view has no file or network access of its own, so the document, the log and the API key
//! are only ever touched here.

use cowriting_app::diagnostics::{name_only, Diagnostics, Seen};
use cowriting_app::preferences::{in_export_folder, last_document, remember_document, Preferences, SettingsFile};
use cowriting_app::usage::{self, Counted, Sender};
use cowriting_app::{
    export_tools, list_models, make_backend, model_status, tested_models, AiTextRow, BackendSettings, DocumentExport, ExportOptions, ExportTools, Exported, FileFormat, LiveView,
    LogView, Opened, PartyView, PdfChoice, ScopeView, Session, SourceHits, SourcesView, VerifyFile,
};
use cowriting_core::host::{DiskHost, Host};
use cowriting_core::review::{CheckSummary, LiveOutcome};
use cowriting_core::turn::{Ask, TurnEvent};
use cowriting_core::types::ThreadMessage;
use cowriting_core::words::{count_words, Format};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::ipc::Channel;
use tauri::{Manager, State};

const KEY_SERVICE: &str = "io.github.dozwa.cowriting-editor";

#[derive(Default)]
struct Editor {
    session: Mutex<Option<Arc<Session>>>,
    /// Whether this start is one at which the editor asks for support; counted once, when it starts.
    asking: Mutex<cowriting_app::support::Asking>,
    /// What is counted while the editor is open; sent when it closes only if the author agreed to that.
    counted: Mutex<Counted>,
    /// The sign of use went out for this start.
    is_ping_sent: std::sync::atomic::AtomicBool,
    /// What was written for the author since the start; only these can be shown from the window.
    written: cowriting_app::shown::Written,
}

impl Editor {
    fn session(&self) -> Result<Arc<Session>, String> {
        self.session.lock().unwrap().clone().ok_or_else(|| "No document is open.".to_string())
    }
}

/// When the editor was started.
static STARTED: OnceLock<std::time::Instant> = OnceLock::new();

/// Where usage figures go, if anywhere: see `usage::server`. `COWRITING_USAGE_SERVER` names one on this computer, for tests and trials.
fn usage_server() -> Option<String> {
    usage::server(std::env::var("COWRITING_USAGE_SERVER").ok().as_deref())
}

/// Who sends: only while a consent stands is there an installation's number.
fn usage_sender(app: &tauri::AppHandle) -> Option<Sender> {
    let installation = usage::installation(&state_file(app)?)?;
    Some(Sender { installation, version: app.package_info().version.to_string(), system: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH) })
}

/// Sends the sign of use for this start, once, if the author agreed to it. Figures of an earlier
/// session that could not be sent when the editor closed go first, if that consent stands.
fn send_start(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    let preferences = read_file(app).preferences;
    let (Some(server), Some(sender), Some(state)) = (usage_server(), usage_sender(app), state_file(app)) else { return };
    let waiting = if preferences.usage_figures { usage::take_waiting(&state) } else { None };
    let is_first = preferences.usage_ping && !app.state::<Editor>().is_ping_sent.swap(true, Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        if let Some(figures) = waiting {
            match usage::send(&server, &figures).await {
                Ok(()) => note("usage", "figures of the last session sent"),
                Err(e) => {
                    usage::keep_waiting(&state, &figures);
                    note("usage", &format!("figures of the last session not sent: {e}"));
                }
            }
        }
        if is_first {
            match usage::send(&server, &usage::start_message(&sender)).await {
                Ok(()) => note("usage", "sign of use sent"),
                Err(e) => note("usage", &format!("sign of use not sent: {e}")),
            }
        }
    });
}

/// The editor closes: a session's figures go out if the author agreed to that; if they cannot, they wait for the next start.
fn send_session(app: &tauri::AppHandle) {
    if !read_file(app).preferences.usage_figures {
        return;
    }
    let (Some(server), Some(sender), Some(state)) = (usage_server(), usage_sender(app), state_file(app)) else { return };
    let counted = app.state::<Editor>().counted.lock().unwrap().clone();
    let figures = usage::session_message(&sender, &counted, STARTED.get().map(std::time::Instant::elapsed).unwrap_or_default());
    match tauri::async_runtime::block_on(usage::send(&server, &figures)) {
        Ok(()) => note("usage", "figures of this session sent"),
        Err(e) => {
            usage::keep_waiting(&state, &figures);
            note("usage", &format!("figures of this session not sent, they wait: {e}"));
        }
    }
}

/// The editor's technical log, once the folder for it is known.
static DIAGNOSTICS: OnceLock<Diagnostics> = OnceLock::new();

/// A line in the technical log; the log takes out whatever is private.
fn note(kind: &str, message: &str) {
    if let Some(log) = DIAGNOSTICS.get() {
        log.note(kind, message);
    }
}

/// Something failed with the document as it was: its measures go into the technical log, never its words.
fn noting_failure<T>(command: &str, document_text: &str, result: Result<T, String>) -> Result<T, String> {
    if let (Err(_), Some(log)) = (&result, DIAGNOSTICS.get()) {
        log.note_document(&format!("at-failed-{command}"), document_text);
    }
    result
}

/// The folder the settings are in: the system's place for them, or `COWRITING_SETTINGS` (for tests and trials).
fn settings_folder(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    match std::env::var("COWRITING_SETTINGS") {
        Ok(folder) if !folder.trim().is_empty() => Ok(PathBuf::from(folder)),
        _ => app.path().app_config_dir().map_err(|e| e.to_string()),
    }
}

fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(settings_folder(app)?.join("settings.json"))
}

/// What the editor remembers that nobody set: the document that was open last.
fn state_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    Some(settings_folder(app).ok()?.join("state.json"))
}

fn read_file(app: &tauri::AppHandle) -> SettingsFile {
    match settings_file(app) {
        Ok(file) => SettingsFile::read(&file),
        Err(_) => SettingsFile { backend: None, preferences: Preferences::default() },
    }
}

/// The settings on disk; `COWRITING_BACKEND` (for tests and trials) overrides the backend.
fn read_settings(app: &tauri::AppHandle) -> BackendSettings {
    let mut settings = read_file(app).backend.unwrap_or_default();
    if let Ok(backend) = std::env::var("COWRITING_BACKEND") {
        settings.backend = backend;
    }
    settings
}

/// The AI's settings as they hold now: the open document's own if it has them, else the general
/// ones. `COWRITING_BACKEND` (for tests and trials) overrides the backend in both.
fn settings_here(app: &tauri::AppHandle, session: Option<&Session>) -> BackendSettings {
    let general = read_file(app).backend.unwrap_or_default();
    let mut settings = session.map_or(general.clone(), |session| session.settings_here(general));
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
    /// The AI as it holds for the open document: its own, or the general one.
    #[serde(flatten)]
    settings: BackendSettings,
    /// The general settings, and whether the open document has its own instead.
    general: BackendSettings,
    is_ai_own: bool,
    /// A key for this backend is in the key store.
    has_key: bool,
    /// The AI can be asked with these settings.
    is_ready: bool,
    /// 'tested', 'failed' or 'untested': whether the model kept to the assist-only rules in the behaviour suite.
    model_status: &'static str,
    /// Nothing was ever set up: the window offers to do it.
    is_first_run: bool,
    /// A document to open at start: `COWRITING_OPEN` (for tests and for "open with"), or the last one if the author wants that.
    open_at_start: Option<String>,
    preferences: Preferences,
    /// The editor's version, and the folder its settings are in: for "About".
    version: String,
    settings_folder: String,
    /// The system the editor runs on ('macos aarch64'): for a report, if the author adds it.
    system: String,
    /// Reports can go out by mail.
    feedback_by_mail: bool,
    /// The contact and the project's page, for "About".
    contact: &'static str,
    repository: &'static str,
    /// This start is one at which the editor asks for a coffee, feedback or a star.
    asking: cowriting_app::support::Asking,
    /// The tour of the editor was not seen yet: the window starts with it.
    show_tour: bool,
    usage: UsageView,
}

/// What the window needs to ask for consent to usage figures, and to show what stands.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageView {
    /// There is a server and a privacy statement in force: only then is anything asked or shown.
    is_available: bool,
    /// The author answered once; they are not asked by a dialog again.
    is_asked: bool,
    /// The installation's number, while a consent stands.
    installation: Option<String>,
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle, editor: State<Editor>) -> SettingsView {
    let file = read_file(&app);
    let session = editor.session().ok();
    let settings = settings_here(&app, session.as_deref());
    let general = read_settings(&app);
    let key = api_key(&settings.backend);
    let is_ready = make_backend(&settings, key.as_deref()).is_ok();
    let model_status = model_status(&tested_models(), &settings.backend, &settings.model);
    let is_first_run = file.backend.is_none() && std::env::var("COWRITING_BACKEND").is_err();
    let last = || if file.preferences.reopen_last { state_file(&app).and_then(|state| last_document(&state)) } else { None };
    SettingsView {
        is_ai_own: session.as_deref().is_some_and(|session| session.project_view().overrides.ai.is_some()),
        general,
        settings,
        has_key: key.is_some(),
        is_ready,
        model_status,
        is_first_run,
        open_at_start: std::env::var("COWRITING_OPEN").ok().or_else(last),
        preferences: file.preferences,
        version: app.package_info().version.to_string(),
        settings_folder: settings_folder(&app).map(|folder| folder.to_string_lossy().into_owned()).unwrap_or_default(),
        system: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        feedback_by_mail: cowriting_app::feedback::has_mail(),
        contact: cowriting_app::feedback::MAIL,
        repository: cowriting_app::feedback::REPOSITORY,
        asking: *editor.asking.lock().unwrap(),
        // a scripted run (tests) is not interrupted by it
        show_tour: std::env::var("COWRITING_BACKEND").is_err() && state_file(&app).is_some_and(|state| cowriting_app::preferences::is_tour_due(&state)),
        usage: UsageView {
            is_available: usage_server().is_some(),
            is_asked: state_file(&app).is_some_and(|state| usage::was_asked(&state)),
            installation: state_file(&app).and_then(|state| usage::installation(&state)),
        },
    }
}

/// Saves the settings; a key that is given goes to the system's key store and nowhere else.
#[tauri::command]
fn set_settings(app: tauri::AppHandle, settings: BackendSettings, api_key: Option<String>) -> Result<(), String> {
    SettingsFile::save_backend(&settings_file(&app)?, &settings)?;
    if let Some(key) = api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        keyring::Entry::new(KEY_SERVICE, &settings.backend)
            .and_then(|entry| entry.set_password(&key))
            .map_err(|e| format!("The key could not be put into the system's key store: {e}"))?;
    }
    Ok(())
}

/// Puts a key for one provider into the system's key store: for a document whose own AI is not the general one.
#[tauri::command]
fn set_api_key(backend: String, api_key: String) -> Result<(), String> {
    if !["anthropic", "openai-compatible"].contains(&backend.as_str()) || api_key.trim().is_empty() {
        return Err("A key needs a provider that takes one.".into());
    }
    keyring::Entry::new(KEY_SERVICE, &backend)
        .and_then(|entry| entry.set_password(api_key.trim()))
        .map_err(|e| format!("The key could not be put into the system's key store: {e}"))
}

/// Saves the preferences and returns them as saved, within their limits. An open document's live review keeps the new pace at once.
#[tauri::command]
fn set_preferences(app: tauri::AppHandle, editor: State<Editor>, preferences: Preferences) -> Result<Preferences, String> {
    let before = read_file(&app).preferences;
    let saved = SettingsFile::save_preferences(&settings_file(&app)?, preferences)?;
    if let Some(log) = DIAGNOSTICS.get() {
        log.set_on(saved.diagnostics);
    }
    // a consent to usage figures given or taken back in the settings holds at once
    if (before.usage_ping, before.usage_figures) != (saved.usage_ping, saved.usage_figures) {
        usage_decided(&app, &saved);
    }
    if let Ok(session) = editor.session() {
        apply_here(&session, &saved);
    }
    Ok(saved)
}

/// What the author decided about usage figures: the installation's number is made or deleted, and a sign of use they just agreed to goes out for this start.
fn usage_decided(app: &tauri::AppHandle, preferences: &Preferences) {
    if let Some(state) = state_file(app) {
        usage::decided(&state, preferences, &|| (0..3).map(|_| DiskHost.new_id()).collect());
        note("usage", &format!("decided: sign of use {}, figures {}", if preferences.usage_ping { "yes" } else { "no" }, if preferences.usage_figures { "yes" } else { "no" }));
        send_start(app);
    }
}

/// The author answered the question about usage figures; both parts are their own yes or no.
#[tauri::command]
fn usage_answered(app: tauri::AppHandle, ping: bool, figures: bool) -> Result<Preferences, String> {
    if usage_server().is_none() {
        return Err("There is nowhere to send usage figures to.".into());
    }
    let saved = SettingsFile::save_preferences(&settings_file(&app)?, Preferences { usage_ping: ping, usage_figures: figures, ..read_file(&app).preferences })?;
    usage_decided(&app, &saved);
    Ok(saved)
}

/// The two messages as they would be sent now, for the author to read: the sign of use, and this session's figures so far.
#[tauri::command]
fn usage_preview(app: tauri::AppHandle, editor: State<Editor>) -> (String, String) {
    let sender = usage_sender(&app).unwrap_or_else(|| Sender {
        installation: "(a random number, made when you agree)".into(),
        version: app.package_info().version.to_string(),
        system: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    });
    let counted = editor.counted.lock().unwrap().clone();
    let pretty = |message: &serde_json::Value| serde_json::to_string_pretty(message).unwrap_or_default();
    (pretty(&usage::start_message(&sender)), pretty(&usage::session_message(&sender, &counted, STARTED.get().map(std::time::Instant::elapsed).unwrap_or_default())))
}

/// The privacy statement for usage figures, in the language shown.
#[tauri::command]
fn privacy_statement(language: String) -> &'static str {
    usage::statement(&language)
}

/// The pace of live review and the languages for an open document: the general ones, with what its project sets for itself.
fn apply_here(session: &Session, general: &Preferences) {
    let here = session.preferences_here(general);
    session.set_pace(here.pace());
    session.set_languages(here.languages());
}

/// The editor's folder for what belongs to single projects, one folder each.
fn projects_folder(app: &tauri::AppHandle) -> Option<String> {
    Some(settings_folder(app).ok()?.join("projects").to_string_lossy().into_owned())
}

/// What belongs to the open project besides its text and log: linked files, and its own settings.
#[tauri::command]
fn project(editor: State<Editor>) -> Result<cowriting_app::ProjectView, String> {
    Ok(editor.session()?.project_view())
}

#[tauri::command]
fn link_file(editor: State<Editor>, path: String, personas: Vec<String>) -> Result<cowriting_app::ProjectView, String> {
    let session = editor.session()?;
    session.link_file(&path, &personas)?;
    Ok(session.project_view())
}

#[tauri::command]
fn unlink_file(editor: State<Editor>, path: String) -> Result<cowriting_app::ProjectView, String> {
    let session = editor.session()?;
    session.unlink_file(&path)?;
    Ok(session.project_view())
}

/// What this project sets differently from the general settings; it holds at once.
#[tauri::command]
fn set_project(app: tauri::AppHandle, editor: State<Editor>, overrides: cowriting_core::project::Project) -> Result<cowriting_app::ProjectView, String> {
    let session = editor.session()?;
    session.set_overrides(overrides)?;
    apply_here(&session, &read_file(&app).preferences);
    Ok(session.project_view())
}

#[tauri::command]
fn set_prompt(editor: State<Editor>, id: String, prompt: String) -> Result<(), String> {
    editor.session()?.set_prompt(&id, &prompt)
}

/// The author answered the editor's asking for support ('coffee', 'feedback', 'star', or 'later'): it is quiet for a while.
#[tauri::command]
fn support_answered(app: tauri::AppHandle, editor: State<Editor>, what: String) {
    if let Some(state) = state_file(&app) {
        cowriting_app::support::answered(&state, &what);
    }
    editor.asking.lock().unwrap().is_due = false;
}

/// The author went through the tour of the editor, or closed it: it does not start by itself again.
#[tauri::command]
fn tour_seen(app: tauri::AppHandle) {
    if let Some(state) = state_file(&app) {
        cowriting_app::preferences::tour_seen(&state);
    }
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
fn open_document(app: tauri::AppHandle, editor: State<Editor>, path: String) -> Result<Opened, String> {
    let (session, opened) = Session::open_in(Arc::new(DiskHost), &path, projects_folder(&app).as_deref())?;
    apply_here(&session, &read_file(&app).preferences);
    *editor.session.lock().unwrap() = Some(Arc::new(session));
    editor.counted.lock().unwrap().opened(&opened.path, count_words(&opened.text, Format::Markdown));
    if let Some(log) = DIAGNOSTICS.get() {
        log.keep_document_private(&opened.path);
        log.note_document("opened", &opened.text);
    }
    if let Some(state) = state_file(&app) {
        remember_document(&state, &opened.path);
    }
    Ok(opened)
}

/// A new, empty document at a path the author chose in the save dialog.
#[tauri::command]
fn new_document(app: tauri::AppHandle, editor: State<Editor>, path: String) -> Result<Opened, String> {
    if !std::path::Path::new(&path).exists() {
        std::fs::write(&path, "").map_err(|e| format!("{path} could not be created: {e}"))?;
    }
    open_document(app, editor, path)
}

/// Opens a page or a folder with the program the system has for it.
fn show(target: &str) -> std::io::Result<()> {
    let mut opener = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        std::process::Command::new("xdg-open")
    };
    opener.arg(target).spawn().map(|_| ())
}

/// Shows the folder the editor's settings are in, in the system's file manager.
#[tauri::command]
fn show_settings_folder(app: tauri::AppHandle) -> Result<(), String> {
    let folder = settings_folder(&app)?;
    std::fs::create_dir_all(&folder).map_err(|e| format!("{} could not be created: {e}", folder.display()))?;
    show(&folder.to_string_lossy()).map_err(|e| format!("The folder could not be shown ({e}). It is {}", folder.display()))
}

/// Shows one of a few pages in the author's browser. The window names the guide; it cannot name an address.
#[tauri::command]
fn open_guide(which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "markdown" => "https://commonmark.org/help/",
        "pandoc" => "https://pandoc.org/MANUAL.html#pandocs-markdown",
        "styles" => "https://www.zotero.org/styles",
        "repository" => cowriting_app::feedback::REPOSITORY,
        "coffee" => cowriting_app::feedback::COFFEE,
        _ => return Err("There is no such guide.".into()),
    };
    show(url).map_err(|e| format!("The page could not be opened in your browser ({e}). Its address is {url}"))
}

/// Opens a report in the author's browser (a new issue) or mail program, filled in and not sent:
/// sending is theirs. The window gives the words it showed; where they go is not the window's to say.
#[tauri::command]
fn send_feedback(app: tauri::AppHandle, editor: State<Editor>, way: String, title: String, text: String) -> Result<(), String> {
    let address = cowriting_app::feedback::address(&way, &title, &text)?;
    support_answered(app, editor, "feedback".into());
    show(&address).map_err(|e| format!("The report could not be opened in your {} ({e}). Copy it instead.", if way == "mail" { "mail program" } else { "browser" }))
}

/// What the window saw since it last said so: commands and how they ended, views, its own errors.
#[tauri::command]
fn record_seen(seen: Vec<Seen>) {
    if let Some(log) = DIAGNOSTICS.get() {
        log.from_window(&seen);
    }
}

/// The technical log's last minutes, for the author to read and to add to a report if they want.
#[tauri::command]
fn recent_diagnostics(minutes: u64) -> String {
    DIAGNOSTICS.get().map(|log| log.recent(minutes.clamp(1, 120))).unwrap_or_default()
}

#[tauri::command]
fn clear_diagnostics() -> Result<(), String> {
    DIAGNOSTICS.get().map_or(Ok(()), Diagnostics::clear)
}

/// Writes a file for the author to attach to a report, where they chose: the log's last minutes
/// and, only when they agreed in the dialog, the text of the document as it is in the window.
#[tauri::command]
fn write_report_file(app: tauri::AppHandle, target: String, minutes: u64, document_text: Option<String>) -> Result<(), String> {
    let log = DIAGNOSTICS.get().ok_or("The editor keeps no log here.")?;
    let about = format!("CoWriting Editor {}, {} {}", app.package_info().version, std::env::consts::OS, std::env::consts::ARCH);
    std::fs::write(&target, log.report(&about, minutes.clamp(1, 120), document_text.as_deref())).map_err(|e| format!("The file could not be written: {e}"))?;
    app.state::<Editor>().written.add(&target);
    log.note_plain("report", &format!("file written, {}", if document_text.is_some() { "with the document's text" } else { "without the document's text" }));
    Ok(())
}

/// The author says this document is the renamed one a log in its folder was kept for.
#[tauri::command]
fn adopt_log(app: tauri::AppHandle, editor: State<Editor>, name: String) -> Result<Opened, String> {
    let session = editor.session()?;
    session.adopt(&name)?;
    let path = session.workspace.sidecar.document.clone();
    open_document(app, editor, path)
}

/// Moves or renames the open document, with its log.
#[tauri::command]
fn move_document(app: tauri::AppHandle, editor: State<Editor>, target: String, text: String) -> Result<Opened, String> {
    editor.session()?.move_document(&target, &text)?;
    open_document(app, editor, target)
}

#[tauri::command]
fn save_document(editor: State<Editor>, text: String) -> Result<Vec<AiTextRow>, String> {
    let found = noting_failure("save", &text, editor.session()?.save(&text))?;
    editor.counted.lock().unwrap().saved(count_words(&text, Format::Markdown));
    Ok(found)
}

#[tauri::command]
fn report_paste(editor: State<Editor>, pasted: String, line: usize, document_text: String) -> Result<Vec<AiTextRow>, String> {
    editor.session()?.report_paste(&pasted, line, &document_text)
}

#[tauri::command]
fn comment_ai_text(editor: State<Editor>, id: String, note: String) -> Result<(), String> {
    editor.session()?.comment_ai_text(&id, &note)
}

/// The author states AI use outside the editor, or corrects a statement.
#[tauri::command]
fn state_external(editor: State<Editor>, stated: cowriting_app::ExternalUse) -> Result<(), String> {
    editor.session()?.state_external(&stated)
}

#[tauri::command]
fn withdraw_external(editor: State<Editor>, id: String) -> Result<(), String> {
    editor.session()?.withdraw_external(&id)
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
    /// The action button that started this ('feedback', …), if one did.
    command: Option<String>,
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

/// The author ends the answer under way.
#[tauri::command]
fn stop_answer(editor: State<Editor>) -> Result<(), String> {
    editor.session()?.stop_answer();
    note("ask", "stopped by the author");
    Ok(())
}

#[tauri::command]
async fn ask(app: tauri::AppHandle, editor: State<'_, Editor>, input: AskInput, on_event: Channel<AskEvent>) -> Result<AskResult, String> {
    let session = editor.session()?;
    let settings = settings_here(&app, Some(&session));
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    let command = input.persona.as_ref().map(|_| "ask".to_string()).or(input.command);
    let about = format!(
        "{} {}, {}, question of {} characters, document of {} characters",
        if settings.backend.starts_with("fake:") { "scripted" } else { settings.backend.as_str() },
        name_only(&settings.model),
        if input.persona.is_some() { "to an expert" } else { "to the assistant" },
        input.text.chars().count(),
        input.document_text.chars().count()
    );
    let document_text = input.document_text.clone();
    editor.counted.lock().unwrap().request(&settings.backend, &settings.model);
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
        .await;
    let outcome = noting_failure("ask", &document_text, outcome)?;
    if let Some(log) = DIAGNOSTICS.get() {
        log.note_plain(
            "ask",
            &format!(
                "{about}: answer of {} characters, {} tool calls refused{}{}",
                outcome.answer.chars().count(),
                outcome.refused,
                if outcome.cut_off.is_some() { ", cut off" } else { "" },
                if outcome.without_tools { ", without tools" } else { "" }
            ),
        );
    }
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
    let settings = settings_here(&app, Some(&session));
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    let outcome = noting_failure("live-review", &document_text, session.live_review(backend.as_ref(), &settings.model, &document_text).await)?;
    // only a review that reached the model counts as a request
    if !matches!(outcome, LiveOutcome::Off | LiveOutcome::Unchanged | LiveOutcome::Wait { .. } | LiveOutcome::Paused) {
        editor.counted.lock().unwrap().request(&settings.backend, &settings.model);
    }
    Ok(outcome)
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
    let settings = settings_here(&app, Some(&session));
    let backend = make_backend(&settings, api_key(&settings.backend).as_deref())?;
    editor.counted.lock().unwrap().request(&settings.backend, &settings.model);
    noting_failure("check-points", &document_text, session.check_points(backend.as_ref(), &settings.model, &document_text).await)
}

#[tauri::command]
fn sources(editor: State<Editor>, document_text: String) -> Result<SourcesView, String> {
    Ok(editor.session()?.sources_view(&document_text))
}

#[tauri::command]
fn add_bibliography(editor: State<Editor>, path: String, document_text: String) -> Result<(), String> {
    editor.session()?.add_bibliography(&path, &document_text)
}

#[tauri::command]
fn remove_bibliography(editor: State<Editor>, path: String) -> Result<(), String> {
    editor.session()?.remove_bibliography(&path)
}

/// The reference style of this document's exports: a CSL file of the author's.
#[tauri::command]
fn set_style(editor: State<Editor>, path: String, document_text: String) -> Result<(), String> {
    editor.session()?.set_style(&path, &document_text)
}

#[tauri::command]
fn remove_style(editor: State<Editor>) -> Result<(), String> {
    editor.session()?.remove_style()
}

#[tauri::command]
async fn search_sources(editor: State<'_, Editor>, query: String, document_text: String) -> Result<SourceHits, String> {
    let session = editor.session()?;
    Ok(session.search_sources(&query, &document_text).await)
}

#[tauri::command]
async fn check_references(editor: State<'_, Editor>, document_text: String) -> Result<VerifyFile, String> {
    let session = editor.session()?;
    session.check_references_online(&document_text).await
}

/// What there is to export with, and where the export would go.
#[tauri::command]
async fn export_setup(app: tauri::AppHandle, editor: State<'_, Editor>) -> Result<(ExportTools, String, String), String> {
    let session = editor.session()?;
    // beside the document, or in the folder the author chose for exports
    let folder = read_file(&app).preferences.export_folder;
    let target = |format| in_export_folder(&session.export_target(format), &folder);
    Ok((export_tools().await, target(FileFormat::Pdf), target(FileFormat::Docx)))
}

#[tauri::command]
async fn export_document(editor: State<'_, Editor>, format: FileFormat, target: String, document_text: String, pdf: PdfChoice) -> Result<DocumentExport, String> {
    let session = editor.session()?;
    let made = noting_failure("export", &document_text, session.export_document_online(format, &target, &document_text, &pdf).await)?;
    editor.written.add(&made.path);
    Ok(made)
}

/// Shows a file the editor wrote since it started: marked in its folder, or opened with the system's program for it.
#[tauri::command]
fn show_written(editor: State<Editor>, path: String, how: cowriting_app::shown::How) -> Result<(), String> {
    let (program, args) = editor.written.command(&path, how)?;
    std::process::Command::new(program).args(args).spawn().map(|_| ()).map_err(|e| format!("The file could not be shown ({e}). It is {path}"))
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
    let exported = editor.session()?.export(&options)?;
    exported.paths.iter().for_each(|path| editor.written.add(path));
    Ok(exported)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Editor::default())
        .setup(|app| {
            // pandoc and Typst ship in the app's resources
            if let Ok(resources) = tauri::Manager::path(app).resource_dir() {
                cowriting_app::export::set_tools_dir(resources.join("tools"));
            }
            if let Ok(folder) = settings_folder(app.handle()) {
                let log = DIAGNOSTICS.get_or_init(|| Diagnostics::at(folder.join("diagnostics.log"), read_file(app.handle()).preferences.diagnostics));
                log.note_plain("start", &format!("CoWriting Editor {}, {} {}", app.package_info().version, std::env::consts::OS, std::env::consts::ARCH));
                // a crash on the Rust side says where it was before the editor goes down
                let before = std::panic::take_hook();
                std::panic::set_hook(Box::new(move |panic| {
                    let said = panic.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
                    let place = panic.location().map(|at| format!("{}:{}", at.file().rsplit(['/', '\\']).next().unwrap_or(""), at.line())).unwrap_or_default();
                    note("panic", &format!("{said} (in {})", name_only(&place)));
                    before(panic);
                }));
            }
            let _ = STARTED.set(std::time::Instant::now());
            send_start(app.handle());
            if let Some(state) = state_file(app.handle()) {
                *app.state::<Editor>().asking.lock().unwrap() = cowriting_app::support::started(&state);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            set_preferences,
            set_api_key,
            show_settings_folder,
            test_connection,
            open_document,
            new_document,
            save_document,
            adopt_log,
            open_guide,
            project,
            link_file,
            unlink_file,
            set_project,
            set_prompt,
            support_answered,
            tour_seen,
            send_feedback,
            record_seen,
            recent_diagnostics,
            clear_diagnostics,
            write_report_file,
            usage_answered,
            usage_preview,
            privacy_statement,
            move_document,
            ask,
            stop_answer,
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
            state_external,
            withdraw_external,
            live,
            set_live,
            live_review,
            scope,
            set_scope,
            check_points,
            sources,
            add_bibliography,
            remove_bibliography,
            set_style,
            remove_style,
            search_sources,
            check_references,
            export_setup,
            export_document,
            show_written
        ])
        .build(tauri::generate_context!())
        .expect("the editor's window could not be opened")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                send_session(app);
            }
        });
}
