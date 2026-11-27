//! Development only: serves the editor's commands over HTTP on localhost, so the interface can
//! run in an ordinary browser (`npm run dev` in `ui/`) against the real Rust code and be driven
//! by browser automation. It serves one document, named on the command line, and is never shipped.
//!
//!   cargo run -p cowriting-suites --bin bridge -- --document /path/to/file.md \
//!       --backend openai-compatible --base-url http://localhost:11434/v1 --model gemma3:4b
//!
//! The settings live in memory; an API key is read from COWRITING_API_KEY or set in the dialog
//! for this run only. The Tauri commands in `src-tauri/src/main.rs` are the real thing; this
//! mirrors their names and shapes.

use cowriting_app::preferences::Preferences;
use cowriting_app::{list_models, make_backend, model_status, tested_models, BackendSettings, ExportOptions, Session};
use cowriting_core::host::DiskHost;
use cowriting_core::turn::{Ask, TurnEvent};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

struct Bridge {
    document: Mutex<String>,
    /// Where "Move…" takes the document, in place of a file dialog (`--move-to`).
    move_to: Option<String>,
    session: Mutex<Option<Arc<Session>>>,
    settings: Mutex<BackendSettings>,
    preferences: Mutex<Preferences>,
    key: Mutex<Option<String>>,
    is_first_run: Mutex<bool>,
    /// What opening found about stored files, kept for the run: the page opens the document more than once while it is developed.
    notices: Mutex<Vec<cowriting_core::stored::Notice>>,
}

impl Bridge {
    fn session(&self) -> Result<Arc<Session>, String> {
        self.session.lock().unwrap().clone().ok_or_else(|| "No document is open.".to_string())
    }

    fn open(&self) -> Result<Value, String> {
        let (session, mut opened) = Session::open(Arc::new(DiskHost), &self.document.lock().unwrap().clone())?;
        *self.session.lock().unwrap() = Some(Arc::new(session));
        let mut notices = self.notices.lock().unwrap();
        notices.append(&mut opened.notices);
        opened.notices = notices.clone();
        Ok(json!(opened))
    }

    fn settings_view(&self) -> Value {
        let settings = self.settings.lock().unwrap().clone();
        let key = self.key.lock().unwrap().clone();
        let mut view = json!(settings);
        view["hasKey"] = json!(key.is_some());
        view["isReady"] = json!(make_backend(&settings, key.as_deref()).is_ok());
        view["modelStatus"] = json!(model_status(&tested_models(), &settings.backend, &settings.model));
        view["isFirstRun"] = json!(*self.is_first_run.lock().unwrap());
        view["openAtStart"] = json!(*self.document.lock().unwrap());
        view["preferences"] = json!(*self.preferences.lock().unwrap());
        view["version"] = json!(env!("CARGO_PKG_VERSION"));
        view["system"] = json!(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
        view["contact"] = json!(cowriting_app::feedback::MAIL);
        view["repository"] = json!(cowriting_app::feedback::REPOSITORY);
        view["showTour"] = json!(false);
        view["notices"] = json!([]);
        // the bridge keeps general settings only, and sends no usage figures
        view["general"] = json!(settings);
        view["isAiOwn"] = json!(false);
        view["usage"] = json!({ "isAvailable": false, "isAsked": false, "installation": null });
        view["asking"] = json!(cowriting_app::support::Asking::default());
        view["feedbackByMail"] = json!(cowriting_app::feedback::has_mail());
        view["settingsFolder"] = json!("(the bridge keeps its settings in memory)");
        view
    }

    /// Every command but `ask`, which streams.
    fn command(&self, name: &str, args: &Value, runtime: &tokio::runtime::Runtime) -> Result<Value, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();
        match name {
            "get_settings" => Ok(self.settings_view()),
            "set_settings" => {
                *self.settings.lock().unwrap() = serde_json::from_value(args["settings"].clone()).map_err(|e| e.to_string())?;
                if let Some(key) = args["apiKey"].as_str().filter(|k| !k.trim().is_empty()) {
                    *self.key.lock().unwrap() = Some(key.trim().to_string());
                }
                *self.is_first_run.lock().unwrap() = false;
                Ok(Value::Null)
            }
            "set_preferences" => {
                let saved = serde_json::from_value::<Preferences>(args["preferences"].clone()).map_err(|e| e.to_string())?.within_limits();
                if let Ok(session) = self.session() {
                    session.set_pace(saved.pace());
                    session.set_languages(saved.languages());
                }
                *self.preferences.lock().unwrap() = saved.clone();
                Ok(json!(saved))
            }
            "test_connection" => {
                let settings: BackendSettings = serde_json::from_value(args["settings"].clone()).map_err(|e| e.to_string())?;
                let key = args["apiKey"].as_str().filter(|k| !k.trim().is_empty()).map(String::from).or_else(|| self.key.lock().unwrap().clone());
                let tested = tested_models();
                let models = runtime.block_on(list_models(&settings, key.as_deref()))?;
                Ok(models.into_iter().map(|(id, name)| json!({ "status": model_status(&tested, &settings.backend, &id), "id": id, "name": name })).collect())
            }
            "pick_document" => Ok(json!(*self.document.lock().unwrap())),
            "open_document" | "new_document" => self.open(),
            "save_document" => self.session()?.save(&text("text")).map(|found| json!(found)),
            "report_paste" => self.session()?.report_paste(&text("pasted"), args["line"].as_u64().unwrap_or(0) as usize, &text("documentText")).map(|found| json!(found)),
            "state_external" => {
                serde_json::from_value(args["stated"].clone()).map_err(|e| e.to_string()).and_then(|stated| self.session()?.state_external(&stated)).map(|_| Value::Null)
            }
            "withdraw_external" => self.session()?.withdraw_external(&text("id")).map(|_| Value::Null),
            "comment_ai_text" => self.session()?.comment_ai_text(&text("id"), &text("note")).map(|_| Value::Null),
            "log_view" => self.session()?.log_view(&text("documentText")).map(|view| json!(view)),
            "thread" => self.session()?.thread(&text("id")).map(|thread| json!(thread)),
            "hint_marks" => Ok(json!(self.session()?.hint_marks())),
            "set_hint_marks" => {
                let done: Vec<usize> = serde_json::from_value(args["done"].clone()).map_err(|e| e.to_string())?;
                self.session()?.set_hint_marks(&text("turn"), &done).map(|_| Value::Null)
            }
            "annotate" => self.session()?.annotate(&text("reference"), &text("use"), &text("note")).map(|_| Value::Null),
            "party" => Ok(json!(self.session()?.party_view())),
            "project" => Ok(json!(self.session()?.project_view())),
            "link_file" => {
                let personas: Vec<String> = serde_json::from_value(args["personas"].clone()).unwrap_or_default();
                self.session()?.link_file(&text("path"), &personas).map(|_| json!(self.session().unwrap().project_view()))
            }
            "count_selection" => {
                Ok(json!(cowriting_core::words::count_selection(&text("text"), args["from"].as_u64().unwrap_or(0) as usize, args["to"].as_u64().unwrap_or(0) as usize)))
            }
            "picture" => self.session()?.picture(&text("path")).map(|picture| json!(picture)),
            "copy_picture" => {
                let session = self.session()?;
                if let Some(into) = args["into"].as_str().filter(|into| *into != session.pictures_folder()) {
                    session.set_pictures_folder(into)?;
                }
                session.copy_picture(&text("path")).map(|picture| json!(picture))
            }
            "paste_picture" => {
                let content: Vec<u8> = serde_json::from_value(args["content"].clone()).map_err(|e| e.to_string())?;
                self.session()?.paste_picture(&text("kind"), &content).map(|picture| json!(picture))
            }
            "pictures_folder" => Ok(json!(self.session()?.pictures_folder())),
            "unlink_file" => self.session()?.unlink_file(&text("path")).map(|_| json!(self.session().unwrap().project_view())),
            "set_prompt" => self.session()?.set_prompt(&text("id"), &text("prompt")).map(|_| Value::Null),
            "set_brief" => self.session()?.set_brief(&text("key"), &text("body")).map(|_| Value::Null),
            "set_member" => self.session()?.set_member(&text("id"), args["isMember"].as_bool().unwrap_or(false)).map(|_| Value::Null),
            "add_expert" => self.session()?.add_expert(&text("name"), &text("role"), &text("looksAt")).map(|id| json!(id)),
            "live" => Ok(json!(self.session()?.live_view())),
            "set_live" => {
                let personas: Vec<String> = serde_json::from_value(args["personas"].clone()).map_err(|e| e.to_string())?;
                self.session()?.set_live(args["isOn"].as_bool().unwrap_or(false), &personas, &text("documentText")).map(|_| Value::Null)
            }
            "live_review" | "check_points" => {
                let session = self.session()?;
                let settings = self.settings.lock().unwrap().clone();
                let backend = make_backend(&settings, self.key.lock().unwrap().as_deref())?;
                if name == "live_review" {
                    runtime.block_on(session.live_review(backend.as_ref(), &settings.model, &text("documentText"))).map(|outcome| json!(outcome))
                } else {
                    runtime.block_on(session.check_points(backend.as_ref(), &settings.model, &text("documentText"))).map(|summary| json!(summary))
                }
            }
            "scope" => Ok(json!(self.session()?.scope_view(&text("documentText")))),
            "set_scope" => self.session()?.set_scope(&text("text"), &text("documentText")).map(|_| Value::Null),
            "adopt_log" => {
                self.session()?.adopt(&text("name"))?;
                self.open()
            }
            // no dialog here: the place named when the bridge was started, if one was
            "pick_move" => Ok(json!(self.move_to)),
            "pictures_to_move" => Ok(json!(self.session()?.pictures_to_move(&text("target"), &text("text")))),
            "move_document" => {
                let session = self.session()?;
                match args["pictures"].as_str() {
                    Some("copy") => session.move_with_pictures(&text("target"), &text("text"), cowriting_app::picture::Along::Copy).map(|_| ())?,
                    Some("leave") => session.move_with_pictures(&text("target"), &text("text"), cowriting_app::picture::Along::Leave).map(|_| ())?,
                    _ => session.move_document(&text("target"), &text("text"))?,
                }
                *self.document.lock().unwrap() = text("target");
                self.open()
            }
            "sources" => Ok(json!(self.session()?.sources_view(&text("documentText")))),
            // no dialog here: the file beside the document, if there is one
            "pick_bibliography" => {
                let beside = std::path::Path::new(&*self.document.lock().unwrap()).with_file_name("refs.bib");
                Ok(if beside.exists() { json!(beside.to_string_lossy()) } else { Value::Null })
            }
            "add_bibliography" => self.session()?.add_bibliography(&text("path"), &text("documentText")).map(|_| Value::Null),
            "set_style" => self.session()?.set_style(&text("path"), &text("documentText")).map(|_| Value::Null),
            "remove_style" => self.session()?.remove_style().map(|_| Value::Null),
            "remove_bibliography" => self.session()?.remove_bibliography(&text("path")).map(|_| Value::Null),
            "search_sources" => Ok(json!(runtime.block_on(self.session()?.search_sources(&text("query"), &text("documentText"))))),
            "check_references" => runtime.block_on(self.session()?.check_references_online(&text("documentText"))).map(|found| json!(found)),
            "export_setup" => {
                let session = self.session()?;
                Ok(json!([
                    runtime.block_on(cowriting_app::export_tools()),
                    session.export_target(cowriting_app::FileFormat::Pdf),
                    session.export_target(cowriting_app::FileFormat::Docx)
                ]))
            }
            "pick_export" => Ok(args["suggested"].clone()),
            "export_document" => {
                let format = serde_json::from_value(args["format"].clone()).map_err(|e| e.to_string())?;
                let pdf = serde_json::from_value(args["pdf"].clone()).unwrap_or_default();
                runtime.block_on(self.session()?.export_document_online(format, &text("target"), &text("documentText"), &pdf)).map(|made| json!(made))
            }
            "export_disclosure" => {
                let options: ExportOptions = serde_json::from_value(args["options"].clone()).map_err(|e| e.to_string())?;
                self.session()?.export(&options).map(|exported| json!(exported))
            }
            other => Err(format!("The bridge has no command {other}.")),
        }
    }

    /// Writes one JSON line per event, then the result or the error.
    fn ask(&self, args: &Value, runtime: &tokio::runtime::Runtime, stream: &mut TcpStream) {
        let line = |stream: &mut TcpStream, value: Value| {
            let _ = stream.write_all(format!("{value}\n").as_bytes());
            let _ = stream.flush();
        };
        let outcome = (|| {
            let session = self.session()?;
            let settings = self.settings.lock().unwrap().clone();
            let backend = make_backend(&settings, self.key.lock().unwrap().as_deref())?;
            let input = &args["input"];
            let ask = Ask {
                text: input["text"].as_str().unwrap_or_default().to_string(),
                reply_to: input["replyTo"].as_str().map(String::from),
                document_text: input["documentText"].as_str().unwrap_or_default().to_string(),
                model: settings.model,
                persona: input["persona"].as_str().map(String::from),
                command: input["persona"].as_str().map(|_| "ask".to_string()).or_else(|| input["command"].as_str().map(String::from)),
            };
            let mut events = stream.try_clone().map_err(|e| e.to_string())?;
            runtime.block_on(session.ask(backend.as_ref(), ask, &mut |event| {
                let event = match event {
                    TurnEvent::Started { id } => json!({ "kind": "started", "id": id }),
                    TurnEvent::Text(text) => json!({ "kind": "text", "text": text }),
                    TurnEvent::Tool { name } => json!({ "kind": "tool", "name": name }),
                    TurnEvent::Refused { tool, input } => json!({ "kind": "refused", "tool": tool, "input": input }),
                };
                line(&mut events, json!({ "event": event }));
            }))
        })();
        match outcome {
            Ok(o) => line(
                stream,
                json!({ "result": { "id": o.id, "answer": o.answer, "isAiText": o.ai_text_flag, "refused": o.refused, "cutOff": o.cut_off, "withoutTools": o.without_tools } }),
            ),
            Err(error) => line(stream, json!({ "error": error })),
        }
    }
}

fn serve(bridge: &Bridge, mut stream: TcpStream) {
    let (mut request, mut buffer) = (Vec::new(), [0u8; 8192]);
    let (head_end, length) = loop {
        let Ok(n) = stream.read(&mut buffer) else { return };
        request.extend_from_slice(&buffer[..n]);
        let head = String::from_utf8_lossy(&request).to_string();
        if let Some(at) = head.find("\r\n\r\n") {
            let length: usize = head[..at].to_lowercase().split("content-length:").nth(1).and_then(|rest| rest.lines().next()).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            if request.len() >= at + 4 + length {
                break (at + 4, length);
            }
        }
        if n == 0 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&request[..head_end]).to_string();
    let path = head.split_whitespace().nth(1).unwrap_or("");
    let name = path.rsplit('/').next().unwrap_or("");
    let args: Value = serde_json::from_slice(&request[head_end..head_end + length]).unwrap_or(Value::Null);
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    if name == "ask" {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n");
        bridge.ask(&args, &runtime, &mut stream);
        return;
    }
    let body = match bridge.command(name, &args, &runtime) {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "error": error }),
    }
    .to_string();
    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|at| args.get(at + 1)).cloned();
    let Some(document) = value("--document") else {
        eprintln!("usage: bridge --document FILE.md [--backend B] [--model M] [--base-url URL] [--port 1421] [--first-run]");
        std::process::exit(2);
    };
    let defaults = BackendSettings::default();
    let settings = BackendSettings {
        backend: value("--backend").unwrap_or(defaults.backend),
        model: value("--model").unwrap_or(defaults.model),
        base_url: value("--base-url").unwrap_or_default(),
        max_tokens: 0,
    };
    let bridge = Arc::new(Bridge {
        document: Mutex::new(document),
        move_to: value("--move-to"),
        session: Mutex::default(),
        settings: Mutex::new(settings),
        preferences: Mutex::default(),
        key: Mutex::new(std::env::var("COWRITING_API_KEY").ok().filter(|k| !k.trim().is_empty())),
        is_first_run: Mutex::new(args.iter().any(|a| a == "--first-run")),
        notices: Mutex::default(),
    });
    // this computer only
    let listener = TcpListener::bind(("127.0.0.1", value("--port").and_then(|p| p.parse().ok()).unwrap_or(1421))).expect("the bridge's port is taken");
    println!("bridge ready on {}", listener.local_addr().unwrap());
    for stream in listener.incoming().flatten() {
        let bridge = bridge.clone();
        std::thread::spawn(move || serve(&bridge, stream));
    }
}
