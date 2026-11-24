//! Usage figures: nothing without consent, nothing but counts with it, and what arrives at the
//! server is what the editor showed. The server here is a listener on this computer.

use cowriting_app::preferences::Preferences;
use cowriting_app::usage::{self, Counted, Sender};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-usage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn on_disk(file: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

/// A server on this computer that takes one message, answers with `status` and gives back what it got.
fn server(status: u16) -> (String, std::thread::JoinHandle<Value>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://127.0.0.1:{}/usage", listener.local_addr().unwrap().port());
    let got = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        let body = loop {
            let read = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&request).into_owned();
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length: usize = head.lines().find_map(|line| line.to_lowercase().strip_prefix("content-length:").map(|n| n.trim().parse().unwrap())).unwrap_or(0);
                if body.len() >= length {
                    break body.to_string();
                }
            }
            assert!(read > 0, "the request ended early");
        };
        stream.write_all(format!("HTTP/1.1 {status} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").as_bytes()).unwrap();
        serde_json::from_str(&body).unwrap()
    });
    (address, got)
}

fn sender() -> Sender {
    Sender { installation: "0123456789abcdef".into(), version: "0.9.0".into(), system: "macos aarch64".into() }
}

#[test]
fn without_a_server_or_with_a_draft_statement_nobody_is_asked() {
    // as the editor is built now: no address, and the statement still has parts to fill in
    assert_eq!(usage::server(None), None);
    assert!(!usage::is_complete(usage::statement("en")) && !usage::is_complete(usage::statement("de")));
    assert!(usage::is_complete("Everything is said."));
    // a trial address is this computer or nothing
    assert_eq!(usage::server(Some("http://127.0.0.1:4000/usage")).as_deref(), Some("http://127.0.0.1:4000/usage"));
    assert_eq!(usage::server(Some("https://collect.example.org/usage")), None);
    // both statements say what is sent and that it needs consent
    assert!(usage::statement("en").contains("only with your separate consent") && usage::statement("de").contains("nur mit Ihrer gesonderten Einwilligung"));
    // the standard is no
    assert!(!Preferences::default().usage_ping && !Preferences::default().usage_figures);
}

#[test]
fn consent_makes_a_random_number_and_taking_it_back_deletes_it() {
    let folder = Folder::new("consent");
    let state = folder.0.join("state.json");
    std::fs::write(&state, r#"{ "lastDocument": "/work/thesis.md" }"#).unwrap();
    let ids = std::cell::Cell::new(0);
    let new_id = || {
        ids.set(ids.get() + 1);
        format!("random-{}", ids.get())
    };
    assert!(!usage::was_asked(&state));
    assert_eq!(usage::installation(&state), None);

    // a no is remembered as an answer, and makes no number
    usage::decided(&state, &Preferences::default(), &new_id);
    assert!(usage::was_asked(&state));
    assert_eq!(usage::installation(&state), None);
    assert_eq!(on_disk(&state)["lastDocument"], "/work/thesis.md", "what else is remembered stays");

    // either consent makes one, and it stays the same while one stands
    usage::decided(&state, &Preferences { usage_ping: true, ..Default::default() }, &new_id);
    assert_eq!(usage::installation(&state).as_deref(), Some("random-1"));
    let both = Preferences { usage_ping: true, usage_figures: true, ..Default::default() };
    usage::decided(&state, &both, &new_id);
    assert_eq!(usage::installation(&state).as_deref(), Some("random-1"));

    // figures that wait are dropped when that consent is taken back; the number goes with the last consent
    usage::keep_waiting(&state, &json!({ "kind": "session" }));
    usage::decided(&state, &Preferences { usage_ping: true, ..Default::default() }, &new_id);
    assert_eq!(usage::take_waiting(&state), None);
    assert_eq!(usage::installation(&state).as_deref(), Some("random-1"));
    usage::decided(&state, &Preferences::default(), &new_id);
    assert_eq!(usage::installation(&state), None);
    assert!(usage::was_asked(&state));
    // agreeing again later is a new installation, not the old one
    usage::decided(&state, &both, &new_id);
    assert_eq!(usage::installation(&state).as_deref(), Some("random-2"));

    // waiting figures are handed out once
    usage::keep_waiting(&state, &json!({ "kind": "session", "minutes": 3 }));
    assert_eq!(usage::take_waiting(&state), Some(json!({ "kind": "session", "minutes": 3 })));
    assert_eq!(usage::take_waiting(&state), None);
}

#[test]
fn what_is_sent_is_counts_and_the_editors_own_names() {
    let mut counted = Counted::default();
    counted.opened("/Users/marta/Promotion/Kapitel 3.md", 1000);
    counted.saved(1040);
    counted.saved(1010); // deleting words takes nothing back
    counted.saved(1025);
    counted.opened("/Users/marta/Promotion/Kapitel 4.md", 200);
    counted.saved(260);
    counted.opened("/Users/marta/Promotion/Kapitel 3.md", 1025);
    counted.request("anthropic", "claude-opus-5-5");
    counted.request("anthropic", "claude-opus-5-5");
    counted.request("openai-compatible", "gemma3:4b");
    counted.request("fake:/Users/marta/scripts/answer.json", "my model \"private\"");

    assert_eq!(usage::start_message(&sender()), json!({ "kind": "start", "installation": "0123456789abcdef", "version": "0.9.0", "system": "macos aarch64" }));
    let session = usage::session_message(&sender(), &counted, Duration::from_secs(47 * 60 + 30));
    assert_eq!(
        session,
        json!({
            "kind": "session", "installation": "0123456789abcdef", "version": "0.9.0", "system": "macos aarch64",
            "minutes": 47, "aiRequests": 4,
            "aiRequestsBy": { "anthropic": { "claude-opus-5-5": 2 }, "openai-compatible": { "gemma3:4b": 1 }, "scripted": { "mymodelprivate": 1 } },
            "documents": 2, "wordsAdded": 115
        })
    );
    let sent = session.to_string();
    for gone in ["marta", "Promotion", "Kapitel", "/Users", "answer.json"] {
        assert!(!sent.contains(gone), "'{gone}' would be sent: {sent}");
    }
}

#[tokio::test]
async fn the_server_gets_exactly_the_message_and_a_failure_is_said() {
    let (address, got) = server(204);
    let message = usage::start_message(&sender());
    usage::send(&address, &message).await.unwrap();
    assert_eq!(got.join().unwrap(), message);

    let (address, got) = server(500);
    assert_eq!(usage::send(&address, &message).await.unwrap_err(), "The server answered 500.");
    got.join().unwrap();

    // nobody there: said without the address
    let gone = usage::send("http://127.0.0.1:9/usage", &message).await.unwrap_err();
    assert!(gone.starts_with("The figures could not be sent") && !gone.contains("127.0.0.1"), "{gone}");
}
