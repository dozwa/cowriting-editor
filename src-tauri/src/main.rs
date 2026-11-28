// no console window beside the app on Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window's side of the editor. Every command passes straight on to `cowriting-app`; the
//! web view has no file or network access of its own, so the document, the log and the API key
//! are only ever touched here.

use cowriting_app::diagnostics::{name_only, Diagnostics, Seen};
use cowriting_app::preferences::{in_export_folder, last_document, remember_document, Preferences, SettingsFile};
use cowriting_app::update_check;
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

/// The name the keys are kept under in the system's key store: the app's identifier, so the dev app
/// (`….cowriting-editor.dev`) and a released one each have their own. Set when the app starts.
static KEY_SERVICE_NAME: OnceLock<String> = OnceLock::new();

fn key_service() -> &'static str {
    KEY_SERVICE_NAME.get().map_or("io.github.dozwa.cowriting-editor", String::as_str)
}

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
    /// What the start found about the editor's own files: one set aside, one from a newer version.
    notices: Mutex<Vec<cowriting_core::stored::Notice>>,
    /// The newer version the last check found, with the server's answer: the update is installed from it.
    newer: Mutex<Option<update_check::Release>>,
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

/// Where the editor asks for a newer version, if it asks: see `update_check::server`. `COWRITING_UPDATE_SERVER` names one on this computer, for tests and trials.
fn update_server(app: &tauri::AppHandle) -> Option<String> {
    update_check::server(std::env::var("COWRITING_UPDATE_SERVER").ok().as_deref(), &app.package_info().version.to_string())
}

/// What a check for a newer version found.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
enum UpdateFound {
    /// There is a newer version. `is_news`: the author was not told about it before.
    #[serde(rename_all = "camelCase")]
    Newer { version: String, notes: String, date: String, is_news: bool, can_install: bool },
    /// The editor is the newest version.
    Newest,
}

/// Asks the server and keeps what it found for the update. The technical log says that the check ran and how it ended, never where it asked.
async fn look_for_update(app: &tauri::AppHandle) -> Result<UpdateFound, String> {
    let server = update_server(app).ok_or("This version does not check for updates.")?;
    let found = update_check::check(&server, &app.package_info().version.to_string()).await;
    note(
        "update",
        &match &found {
            Ok(Some(_)) => "checked: there is a newer version".to_string(),
            Ok(None) => "checked: this is the newest version".to_string(),
            Err(e) => format!("check failed: {e}"),
        },
    );
    let found = found?;
    *app.state::<Editor>().newer.lock().unwrap() = found.clone();
    Ok(match found {
        Some(release) => {
            let is_news = state_file(app).is_some_and(|state| !update_check::was_told(&state, &release.version));
            if let (true, Some(state)) = (is_news, state_file(app)) {
                update_check::told(&state, &release.version);
            }
            UpdateFound::Newer { version: release.version, notes: release.notes, date: release.date, is_news, can_install: can_update_itself(app) }
        }
        None => UpdateFound::Newest,
    })
}

/// Whether this copy can fetch and install an update by itself: only a release is built with the updater and the key its files are signed with.
fn can_update_itself(app: &tauri::AppHandle) -> bool {
    app.config().plugins.0.contains_key("updater")
}

/// Fetches the newer version the last check found from the project's releases on GitHub, installs
/// it and starts the editor again. Only the author starts this. The updater installs a file only
/// if it carries the signature of the editor's own key, for exactly the version announced.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let by_hand = format!("The installer is on {}.", update_check::RELEASES);
    if !can_update_itself(&app) {
        return Err(format!("This copy of the editor cannot update itself. {by_hand}"));
    }
    let release = app.state::<Editor>().newer.lock().unwrap().clone().ok_or("No newer version is known yet. Check for updates first.")?;
    if !update_check::files_are_ours(&release.answer) {
        note("update", "refused: the answer names files outside the project's releases");
        return Err(format!("The server names files that are not among the project's releases on GitHub, so nothing was fetched. {by_hand}"));
    }
    let failed = |e: tauri_plugin_updater::Error| {
        note("update", &format!("update failed: {e}"));
        format!("The update could not be installed: {e}. {by_hand}")
    };
    let update = app.updater().and_then(|updater| updater.restore_update(release.answer.clone())).map_err(failed)?.ok_or("The version found is not newer than this one.")?;
    note("update", "download started");
    update.download_and_install(|_, _| {}, || {}).await.map_err(failed)?;
    note("update", "installed, starting again");
    // on Windows the installer has taken over by now and this is not reached
    app.restart()
}

/// The check at start: only if the author left it on, and silent when it fails. Nothing is fetched or installed by it.
#[tauri::command]
async fn update_at_start(app: tauri::AppHandle) -> Result<Option<UpdateFound>, String> {
    if !read_file(&app).preferences.update_check || update_server(&app).is_none() {
        return Ok(None);
    }
    Ok(look_for_update(&app).await.ok())
}

/// The check the author asks for: it says what it found, or why it could not ask.
#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateFound, String> {
    look_for_update(&app).await
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
    keyring::Entry::new(key_service(), backend).ok().and_then(|entry| entry.get_password().ok()).or_else(from_environment).filter(|key| !key.trim().is_empty())
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
    /// This version can ask whether a newer one exists: a released version with a server to ask.
    can_check_for_update: bool,
    /// The editor's own files that could not be read and were set aside, or that a newer version wrote.
    notices: Vec<cowriting_core::stored::Notice>,
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
        can_check_for_update: update_server(&app).is_some(),
        notices: editor.notices.lock().unwrap().clone(),
    }
}

/// Saves the settings; a key that is given goes to the system's key store and nowhere else.
#[tauri::command]
fn set_settings(app: tauri::AppHandle, settings: BackendSettings, api_key: Option<String>) -> Result<(), String> {
    SettingsFile::save_backend(&settings_file(&app)?, &settings)?;
    if let Some(key) = api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        keyring::Entry::new(key_service(), &settings.backend)
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
    keyring::Entry::new(key_service(), &backend)
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

/// The words and characters of the selected part of the text, by the document's own rules. `from` and `to` are the editor's positions.
#[tauri::command]
fn count_selection(text: String, from: usize, to: usize) -> cowriting_core::words::Counted {
    cowriting_core::words::count_selection(&text, from, to)
}

/// How the open document would name a picture the author chose.
#[tauri::command]
fn picture(editor: State<Editor>, path: String) -> Result<cowriting_app::picture::Picture, String> {
    editor.session()?.picture(&path)
}

/// Copies a picture the author chose into `images/` beside the open document.
#[tauri::command]
fn copy_picture(editor: State<Editor>, path: String, into: Option<String>) -> Result<cowriting_app::picture::Picture, String> {
    let session = editor.session()?;
    // the author named the folder in the dialog: it is this project's from now on
    if let Some(into) = into.filter(|into| *into != session.pictures_folder()) {
        session.set_pictures_folder(&into)?;
    }
    session.copy_picture(&path)
}

/// Saves a picture from the clipboard into the project's pictures folder. `kind` is its file ending.
#[tauri::command]
fn paste_picture(editor: State<Editor>, kind: String, content: Vec<u8>) -> Result<cowriting_app::picture::Picture, String> {
    editor.session()?.paste_picture(&kind, &content)
}

/// The folder beside the document that this project's pictures are copied into.
#[tauri::command]
fn pictures_folder(editor: State<Editor>) -> Result<String, String> {
    Ok(editor.session()?.pictures_folder())
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
        "releases" => update_check::RELEASES,
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
fn move_document(app: tauri::AppHandle, editor: State<Editor>, target: String, text: String, pictures: Option<String>) -> Result<Opened, String> {
    match pictures.as_deref() {
        Some("copy") => editor.session()?.move_with_pictures(&target, &text, cowriting_app::picture::Along::Copy).map(|_| ())?,
        Some("leave") => editor.session()?.move_with_pictures(&target, &text, cowriting_app::picture::Along::Leave).map(|_| ())?,
        _ => editor.session()?.move_document(&target, &text)?,
    }
    open_document(app, editor, target)
}

/// The pictures a move to `target` would leave behind.
#[tauri::command]
fn pictures_to_move(editor: State<Editor>, target: String, text: String) -> Result<cowriting_app::picture::ToMove, String> {
    Ok(editor.session()?.pictures_to_move(&target, &text))
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

/// The conversations a file of another AI tool holds, for the author to choose from. The file is only read.
#[tauri::command]
fn read_chats(path: String) -> Result<cowriting_app::outside::Conversations, String> {
    cowriting_app::outside::conversations(&path)
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

/// The project of several files the open document is one of, with all its files; null for a document on its own.
#[tauri::command]
fn project_files(editor: State<Editor>, document_text: String) -> Result<Option<cowriting_app::project_files::Overview>, String> {
    editor.session()?.project_overview(&document_text)
}

/// Makes a project of several files with the open document as its first. The window opens the document again afterwards.
#[tauri::command]
fn make_project(editor: State<Editor>, path: String) -> Result<String, String> {
    editor.session()?.make_project(&path)
}

#[tauri::command]
fn add_project_file(editor: State<Editor>, path: String) -> Result<(), String> {
    editor.session()?.add_project_file(&path)
}

#[tauri::command]
fn remove_project_file(editor: State<Editor>, path: String) -> Result<(), String> {
    editor.session()?.remove_project_file(&path)
}

#[tauri::command]
fn order_project_files(editor: State<Editor>, paths: Vec<String>) -> Result<(), String> {
    editor.session()?.order_project_files(&paths)
}

#[tauri::command]
fn rename_project_file(editor: State<Editor>, path: String, name: String) -> Result<String, String> {
    editor.session()?.rename_project_file(&path, &name)
}

#[tauri::command]
fn set_project_goal(editor: State<Editor>, goal: String, words: Option<u32>) -> Result<(), String> {
    editor.session()?.set_project_goal(&goal, words)
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
async fn export_setup(app: tauri::AppHandle, editor: State<'_, Editor>) -> Result<(ExportTools, String, String, Option<PdfChoice>), String> {
    let session = editor.session()?;
    // beside the document, or in the folder the author chose for exports
    let folder = read_file(&app).preferences.export_folder;
    let target = |format| in_export_folder(&session.export_target(format), &folder);
    Ok((export_tools().await, target(FileFormat::Pdf), target(FileFormat::Docx), session.export_choice()))
}

/// The layouts kept under a name on this computer.
#[tauri::command]
fn layouts(editor: State<Editor>) -> Result<Vec<cowriting_app::layouts::KeptLayout>, String> {
    Ok(editor.session()?.layouts())
}

/// Keeps the paper and the layout of a choice under a name, and gives back all that are kept.
#[tauri::command]
fn keep_layout(editor: State<Editor>, name: String, pdf: PdfChoice) -> Result<Vec<cowriting_app::layouts::KeptLayout>, String> {
    let session = editor.session()?;
    session.keep_layout(&name, &pdf)?;
    Ok(session.layouts())
}

#[tauri::command]
fn remove_layout(editor: State<Editor>, name: String) -> Result<Vec<cowriting_app::layouts::KeptLayout>, String> {
    let session = editor.session()?;
    session.remove_layout(&name)?;
    Ok(session.layouts())
}

/// Takes a layout from a file somebody passed on; gives back the name it is kept under, and all that are kept.
#[tauri::command]
fn import_layout(editor: State<Editor>, path: String) -> Result<(String, Vec<cowriting_app::layouts::KeptLayout>), String> {
    let session = editor.session()?;
    let name = session.import_layout(&path)?;
    Ok((name, session.layouts()))
}

/// Writes a kept layout to a file the author chose, to pass it on.
#[tauri::command]
fn export_layout(editor: State<Editor>, name: String, target: String) -> Result<(), String> {
    editor.session()?.export_layout(&name, &target)?;
    editor.written.add(&target);
    Ok(())
}

/// Keeps how the open project's exports are set.
#[tauri::command]
fn keep_export_choice(editor: State<Editor>, pdf: PdfChoice) -> Result<(), String> {
    editor.session()?.keep_export_choice(&pdf)
}

#[tauri::command]
async fn export_document(editor: State<'_, Editor>, format: FileFormat, target: String, document_text: String, pdf: PdfChoice) -> Result<DocumentExport, String> {
    let session = editor.session()?;
    let made = noting_failure("export", &document_text, session.export_document_online(format, &target, &document_text, &pdf).await)?;
    editor.written.add(&made.path);
    Ok(made)
}

/// Makes the export in a temporary file, for the author to look at; nothing is written beside the document.
#[tauri::command]
async fn preview_document(editor: State<'_, Editor>, format: FileFormat, document_text: String, pdf: PdfChoice) -> Result<DocumentExport, String> {
    let session = editor.session()?;
    let made = noting_failure("preview", &document_text, session.preview_document_online(format, &document_text, &pdf).await)?;
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

/// The hints the author marked as done, by answer.
#[tauri::command]
fn hint_marks(editor: State<Editor>) -> Result<std::collections::BTreeMap<String, Vec<usize>>, String> {
    Ok(editor.session()?.hint_marks())
}

#[tauri::command]
fn set_hint_marks(editor: State<Editor>, turn: String, done: Vec<usize>) -> Result<(), String> {
    editor.session()?.set_hint_marks(&turn, &done)
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

/// macOS: lets the window mark misspelled words, and keeps it from correcting them.
///
/// The window is WebKit's, and WebKit underlines only while "Check Spelling While Typing" is on for the app
/// (`WebContinuousSpellCheckingEnabled` in the app's defaults). That is off until an app's Edit menu switches it
/// on, and this app has no such item: the editor asked for the check and nothing was marked. Whether a text is
/// checked is still the "Spelling check" setting, which the editor puts on its text as an attribute.
/// Correcting while typing is switched off the same way, so that the system never changes the author's words.
/// Both are set only while the app's defaults say nothing: what the author chose in the context menu stays.
/// It has to happen before the window is made, since WebKit reads the defaults once.
#[cfg(target_os = "macos")]
fn let_spelling_be_marked() {
    use objc2_foundation::{ns_string, NSUserDefaults};
    let defaults = NSUserDefaults::standardUserDefaults();
    for (key, value) in [(ns_string!("WebContinuousSpellCheckingEnabled"), true), (ns_string!("WebAutomaticSpellingCorrectionEnabled"), false)] {
        if defaults.objectForKey(key).is_none() {
            defaults.setBool_forKey(value, key);
        }
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    let_spelling_be_marked();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Editor::default())
        .setup(|app| {
            let _ = KEY_SERVICE_NAME.set(app.config().identifier.clone());
            // pandoc and Typst ship in the app's resources
            if let Ok(resources) = tauri::Manager::path(app).resource_dir() {
                cowriting_app::export::set_tools_dir(resources.join("tools"));
            }
            // before anything reads or writes the editor's own files: one that cannot be read is set aside, not written over
            if let (Ok(settings), Some(state)) = (settings_file(app.handle()), state_file(app.handle())) {
                *app.state::<Editor>().notices.lock().unwrap() = cowriting_app::preferences::look_over(&settings, &state);
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
            // only a release names the updater and its key in its configuration; the dev app has neither and never updates itself
            if can_update_itself(app.handle()) {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            let _ = STARTED.set(std::time::Instant::now());
            send_start(app.handle());
            if let Some(state) = state_file(app.handle()) {
                *app.state::<Editor>().asking.lock().unwrap() = cowriting_app::support::started(&state);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            project_files,
            make_project,
            add_project_file,
            remove_project_file,
            order_project_files,
            rename_project_file,
            set_project_goal,
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
            picture,
            copy_picture,
            paste_picture,
            pictures_folder,
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
            update_at_start,
            check_for_update,
            install_update,
            usage_answered,
            usage_preview,
            privacy_statement,
            move_document,
            pictures_to_move,
            count_selection,
            ask,
            stop_answer,
            log_view,
            thread,
            annotate,
            hint_marks,
            set_hint_marks,
            export_disclosure,
            party,
            set_brief,
            set_member,
            add_expert,
            report_paste,
            comment_ai_text,
            state_external,
            withdraw_external,
            read_chats,
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
            keep_export_choice,
            layouts,
            keep_layout,
            remove_layout,
            import_layout,
            export_layout,
            export_document,
            preview_document,
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use objc2_foundation::{ns_string, NSUserDefaults};

    #[test]
    fn spelling_is_marked_and_not_corrected_unless_the_author_chose_otherwise() {
        let defaults = NSUserDefaults::standardUserDefaults();
        let (marking, correcting) = (ns_string!("WebContinuousSpellCheckingEnabled"), ns_string!("WebAutomaticSpellingCorrectionEnabled"));
        defaults.removeObjectForKey(marking);
        defaults.removeObjectForKey(correcting);
        super::let_spelling_be_marked();
        assert!(defaults.boolForKey(marking), "WebKit is told to mark spelling");
        assert!(defaults.objectForKey(correcting).is_some() && !defaults.boolForKey(correcting), "and not to correct it");
        // the author switched marking off and correcting on, in the context menu: the next start leaves both
        defaults.setBool_forKey(false, marking);
        defaults.setBool_forKey(true, correcting);
        super::let_spelling_be_marked();
        assert!(!defaults.boolForKey(marking) && defaults.boolForKey(correcting));
        // the test program has defaults of its own, under its file's name: nothing of them is left behind
        let own = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
        defaults.removePersistentDomainForName(&objc2_foundation::NSString::from_str(&own));
        defaults.synchronize();
        if let Some(home) = std::env::var_os("HOME") {
            let _ = std::fs::remove_file(std::path::Path::new(&home).join("Library/Preferences").join(format!("{own}.plist")));
        }
    }
}

#[cfg(test)]
mod update_tests {
    use cowriting_app::update_check;

    /// The release workflow's script writes `latest.json` from the signatures of a release's files.
    /// The editor's check and the updater both have to read what it writes.
    #[test]
    fn what_the_release_writes_is_read_by_the_check_and_by_the_updater() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let folder = std::env::temp_dir().join(format!("cowriting-latest-json-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("signatures")).unwrap();
        // the names the files have on GitHub, where a space became a dot; a file that is no installer is left out
        for name in [
            "CoWriting.Editor.app.tar.gz",
            "CoWriting.Editor_0.1.2-alpha_x64-setup.exe",
            "CoWriting.Editor_0.1.2-alpha_amd64.AppImage",
            "CoWriting.Editor_0.1.2-alpha_amd64.deb",
            "notes.txt",
        ] {
            std::fs::write(folder.join("signatures").join(format!("{name}.sig")), format!("signature of {name}\n")).unwrap();
        }
        std::fs::write(folder.join("notes.md"), "### Added\n\n- Something „new“.\n").unwrap();
        let out = folder.join("latest.json");
        let python = if cfg!(windows) { "python" } else { "python3" };
        let ran = std::process::Command::new(python)
            .arg(root.join("scripts/latest-json.py"))
            .args(["write", "--tag", "v0.1.2-alpha", "--signatures"])
            .arg(folder.join("signatures"))
            .arg("--notes")
            .arg(folder.join("notes.md"))
            .arg("--out")
            .arg(&out)
            .output()
            .expect("python runs the script");
        assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
        let written = std::fs::read_to_string(&out).unwrap();

        let release = update_check::read(&written, "0.1.1-alpha").unwrap().expect("0.1.2-alpha is newer than 0.1.1-alpha");
        assert_eq!((release.version.as_str(), release.notes.as_str()), ("0.1.2-alpha", "### Added\n\n- Something „new“."));
        assert_eq!(release.date.len(), 10, "the day of the release");
        assert!(update_check::files_are_ours(&release.answer), "every file is named where the release has it on GitHub");
        let platforms = release.answer["platforms"].as_object().unwrap();
        assert_eq!(platforms.keys().map(String::as_str).collect::<Vec<_>>(), ["darwin-aarch64", "linux-x86_64", "linux-x86_64-deb", "windows-x86_64"]);
        assert_eq!(platforms["windows-x86_64"]["url"], "https://github.com/dozwa/cowriting-editor/releases/download/v0.1.2-alpha/CoWriting.Editor_0.1.2-alpha_x64-setup.exe");
        assert_eq!(platforms["windows-x86_64"]["signature"], "signature of CoWriting.Editor_0.1.2-alpha_x64-setup.exe");

        let for_updater: tauri_plugin_updater::RemoteRelease = serde_json::from_value(release.answer.clone()).expect("the updater reads it");
        assert_eq!(for_updater.version.to_string(), "0.1.2-alpha");
        assert_eq!(for_updater.signature("darwin-aarch64").unwrap(), "signature of CoWriting.Editor.app.tar.gz");
        assert!(for_updater.pub_date.is_some());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The updater's part of the configuration is laid over a release only: the dev app has none, and without a key a release has none either.
    #[test]
    fn the_updater_is_configured_in_a_file_of_its_own_that_the_plugin_reads() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |name: &str| serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap();
        for name in ["tauri.conf.json", "tauri.release.conf.json"] {
            assert!(read(name)["plugins"]["updater"].is_null(), "{name} does not name the updater");
        }
        let updater = read("tauri.updater.conf.json");
        assert_eq!(updater["bundle"]["createUpdaterArtifacts"], true);
        assert_eq!(updater["plugins"]["updater"]["requireSignedVersion"], true, "a file is installed only as the version it was signed as");
        let config: tauri_plugin_updater::Config = serde_json::from_value(updater["plugins"]["updater"].clone()).expect("the plugin reads its configuration");
        assert!(config.endpoints.is_empty(), "the editor asks its server itself; the updater asks nobody");
    }
}
