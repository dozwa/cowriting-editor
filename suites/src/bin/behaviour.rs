//! Runs the behaviour cases against a real model, through the same code the editor uses, and
//! says per case how often the rules held.
//!
//!   cargo run -p cowriting-suites --bin behaviour -- --backend openai-compatible \
//!       --base-url http://localhost:11434/v1 --model qwen3.5:9b [--case ID]… [--repeat N] [--record]
//!
//! The API key, where one is needed, is read from COWRITING_API_KEY. `--record` writes the
//! verdict to `tested-models.json`; it is refused for a partial run.

use cowriting_app::{make_backend, BackendSettings, Session, TestedModel, EDITOR_VERSION};
use cowriting_core::disclosure::{disclosure, DocFormat, Lang, Profile};
use cowriting_core::host::{DiskHost, Host};
use cowriting_core::turn::Ask;
use cowriting_core::types::LogEntry;
use cowriting_suites::{judge, offered_dois, Case, Evidence, Rule};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const RUN_TIMEOUT: Duration = Duration::from_secs(600);

/// Some(false) only when Crossref says the DOI does not exist.
async fn doi_exists(http: &reqwest::Client, doi: &str) -> Option<bool> {
    let response =
        http.get(format!("https://api.crossref.org/works/{doi}")).header("User-Agent", "cowriting-behaviour-suite").timeout(Duration::from_secs(15)).send().await.ok()?;
    match response.status().as_u16() {
        200 => Some(true),
        404 => Some(false),
        _ => None,
    }
}

enum RunResult {
    /// The rules that did not hold, with what a person should read.
    Judged(Vec<(String, String)>),
    /// No answer to judge: the model or the connection failed.
    Empty(String),
}

async fn run_once(case: &Case, run: usize, settings: &BackendSettings, backend: &dyn cowriting_core::backend::Backend, http: &reqwest::Client) -> (RunResult, PathBuf) {
    let folder = std::env::temp_dir().join(format!("cowriting-behaviour-{}-{}-{run}", std::process::id(), case.id));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let document = folder.join("main.md");
    let text = std::fs::read_to_string(format!("{ROOT}/suites/behaviour/document.md")).unwrap();
    std::fs::write(&document, &text).unwrap();

    let (session, _) = Session::open(Arc::new(DiskHost), &document.to_string_lossy()).unwrap();
    let ask = Ask { text: case.prompt.clone(), document_text: text.clone(), model: settings.model.clone(), ..Default::default() };
    let asked = tokio::time::timeout(RUN_TIMEOUT, session.ask(backend, ask, &mut |_| {})).await;
    if std::fs::read_to_string(&document).unwrap() != text {
        return (RunResult::Judged(vec![("document".into(), "the document on disk changed during the turn".into())]), folder);
    }
    match asked {
        Err(_) => return (RunResult::Empty("took too long".into()), folder),
        Ok(Err(error)) => return (RunResult::Empty(error), folder),
        Ok(Ok(_)) => {}
    }

    // the verdict comes from what is on disk: the log, the notes and the disclosure
    let entries = session.workspace.log.read().unwrap();
    let answers: Vec<&str> = entries.iter().filter_map(|e| if let LogEntry::Response(r) = e { Some(r.answer.as_str()) } else { None }).collect();
    let answers = answers.join("\n\n");
    let register = disclosure(&entries, &session.workspace.sidecar.name, Lang::En, DocFormat::Md, Profile::Editor);
    let notes_dir = session.workspace.sidecar.notes_dir();
    let notes: Vec<String> = DiskHost.list(&notes_dir).unwrap_or_default().iter().filter_map(|name| DiskHost.read(&format!("{notes_dir}/{name}")).ok()).collect();

    let mut registry = HashMap::new();
    if case.rules.iter().any(|rule| matches!(rule, Rule::NoInventedDoi)) {
        for doi in offered_dois(&answers) {
            let exists = doi_exists(http, &doi).await;
            registry.insert(doi, exists);
        }
    }
    let evidence = Evidence { answers: &answers, register: &register, notes: &notes };
    let misses = case
        .rules
        .iter()
        .filter_map(|rule| {
            judge(rule, &evidence, &|doi| registry.get(doi).copied().flatten()).err().map(|detail| (format!("{rule:?}").split([' ', '{']).next().unwrap_or("").to_string(), detail))
        })
        .collect();
    (RunResult::Judged(misses), folder)
}

fn today() -> String {
    cowriting_core::host::DiskHost.now().chars().take(10).collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|at| args.get(at + 1)).cloned();
    let wanted: Vec<String> = args.windows(2).filter(|pair| pair[0] == "--case").map(|pair| pair[1].clone()).collect();
    let repeat: Option<usize> = value("--repeat").and_then(|n| n.parse().ok());
    let record = args.iter().any(|a| a == "--record");
    let (Some(backend_id), Some(model)) = (value("--backend"), value("--model")) else {
        eprintln!("usage: behaviour --backend anthropic|openai-compatible --model MODEL [--base-url URL] [--case ID]… [--repeat N] [--record]");
        std::process::exit(2);
    };
    let settings = BackendSettings { backend: backend_id.clone(), model: model.clone(), base_url: value("--base-url").unwrap_or_default() };
    let backend = match make_backend(&settings, std::env::var("COWRITING_API_KEY").ok().as_deref()) {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("{error} (the suite reads the key from COWRITING_API_KEY)");
            std::process::exit(2);
        }
    };
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(format!("{ROOT}/suites/behaviour/cases.json")).unwrap()).unwrap();
    let chosen: Vec<&Case> = cases.iter().filter(|c| wanted.is_empty() || wanted.contains(&c.id)).collect();
    let http = reqwest::Client::new();
    println!("{model} through {} ({})", backend.provider(), backend.id());

    let (mut failed, mut inconclusive, mut summary) = (0, 0, serde_json::Map::new());
    for case in &chosen {
        if case.needs_tools && !backend.uses_tools() {
            println!("{:<13}{:<30}the model cannot call tools", "left out", case.id);
            continue;
        }
        let (mut held, mut tested, mut lines) = (0, 0, Vec::new());
        for run in 0..repeat.unwrap_or(case.runs) {
            match run_once(case, run, &settings, backend.as_ref(), &http).await {
                (RunResult::Judged(misses), folder) => {
                    tested += 1;
                    held += misses.is_empty() as usize;
                    lines.extend(misses.into_iter().map(|(kind, detail)| format!("    {kind}: {detail:?}\n      {}", folder.display())));
                }
                (RunResult::Empty(why), _) => lines.push(format!("    no answer: {why}")),
            }
        }
        let state = if tested == 0 {
            inconclusive += 1;
            "inconclusive"
        } else if held == tested {
            "pass"
        } else {
            failed += 1;
            "fail"
        };
        println!("{state:<13}{:<30}{held}/{tested}", case.id);
        lines.iter().for_each(|line| println!("{line}"));
        summary.insert(case.id.clone(), format!("{held}/{tested}").into());
    }
    if !backend.uses_tools() {
        println!("The model cannot call tools: it was sent the document with each question.");
    }

    if record {
        if !wanted.is_empty() || repeat.is_some() || inconclusive > 0 {
            eprintln!("Not recorded: a verdict needs every case, with its own number of runs, and an answer in each.");
            std::process::exit(2);
        }
        let path = format!("{ROOT}/tested-models.json");
        let mut file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let entry =
            TestedModel { backend: backend_id, model, verdict: if failed == 0 { "passed" } else { "failed" }.into(), date: today(), editor: EDITOR_VERSION.into(), cases: summary };
        let models = file["models"].as_array_mut().unwrap();
        // one entry per backend and model: the latest run
        models.retain(|m| !(m["backend"] == entry.backend.as_str() && m["model"] == entry.model.as_str()));
        models.push(serde_json::to_value(&entry).unwrap());
        std::fs::write(&path, serde_json::to_string_pretty(&file).unwrap() + "\n").unwrap();
        println!("Recorded as {} in tested-models.json.", entry.verdict);
    }
    std::process::exit(if failed > 0 || inconclusive > 0 { 1 } else { 0 });
}
