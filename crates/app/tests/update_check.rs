//! The check for a newer version: what counts as newer, what the server's answer may be, and that
//! the request carries nothing about the author or the editor. The server here is a listener on this computer.

use cowriting_app::preferences::Preferences;
use cowriting_app::update_check::{self, Release};
use serde_json::{json, Value};
use std::io::{Read, Write};

fn answer(version: &str) -> Value {
    json!({
        "version": version,
        "notes": "### Added\n\n- Something new.",
        "pub_date": "2026-12-01T10:00:00Z",
        "platforms": {
            "darwin-aarch64": { "signature": "sig", "url": format!("https://github.com/dozwa/cowriting-editor/releases/download/v{version}/CoWriting.Editor.app.tar.gz") },
            "windows-x86_64": { "signature": "sig", "url": format!("https://github.com/dozwa/cowriting-editor/releases/download/v{version}/CoWriting.Editor_{version}_x64-setup.exe") }
        }
    })
}

/// A server on this computer that answers one request with `status` and `body`, and gives back the request it got.
fn server(status: u16, body: String) -> (String, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://127.0.0.1:{}/latest.json", listener.local_addr().unwrap().port());
    let got = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        while !String::from_utf8_lossy(&request).contains("\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "the request ended early");
            request.extend_from_slice(&buffer[..read]);
        }
        stream.write_all(format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
        String::from_utf8_lossy(&request).into_owned()
    });
    (address, got)
}

#[test]
fn a_later_alpha_and_a_release_without_a_mark_are_newer_and_nothing_else_is() {
    for (other, current) in [
        ("0.1.2-alpha", "0.1.1-alpha"),
        ("0.1.1", "0.1.1-alpha"),
        ("0.2.0-alpha", "0.1.9"),
        ("v0.1.2-alpha", "0.1.1-alpha"),
        ("0.1.1-alpha", "0.1.0"),
        ("0.1.10-alpha", "0.1.9-alpha"),
    ] {
        assert!(update_check::is_newer(other, current), "{other} is newer than {current}");
    }
    for (other, current) in
        [("0.1.1-alpha", "0.1.1-alpha"), ("0.1.0", "0.1.1-alpha"), ("0.1.1-alpha", "0.1.1"), ("newest", "0.1.1-alpha"), ("0.1.2-alpha", "what"), ("", "0.1.1-alpha")]
    {
        assert!(!update_check::is_newer(other, current), "{other} is not newer than {current}");
    }
}

#[test]
fn the_answer_gives_the_version_its_notes_and_its_day_only_when_it_is_newer() {
    let newer = update_check::read(&answer("0.1.2-alpha").to_string(), "0.1.1-alpha").unwrap().expect("a newer release");
    assert_eq!((newer.version.as_str(), newer.notes.as_str(), newer.date.as_str()), ("0.1.2-alpha", "### Added\n\n- Something new.", "2026-12-01"));
    assert_eq!(newer.answer["platforms"]["darwin-aarch64"]["signature"], "sig", "the answer is kept as it came, for the updater");
    assert_eq!(update_check::read(&answer("0.1.1-alpha").to_string(), "0.1.1-alpha"), Ok(None), "the same version is not newer");
    assert_eq!(update_check::read(&answer("0.1.0").to_string(), "0.1.1-alpha"), Ok(None), "an older one neither");
    // an answer without notes or day is still an answer
    let bare = update_check::read(r#"{"version":"0.3.0"}"#, "0.1.1-alpha").unwrap().unwrap();
    assert_eq!(bare, Release { version: "0.3.0".into(), notes: String::new(), date: String::new(), answer: json!({"version": "0.3.0"}) });
}

#[test]
fn an_answer_that_is_no_release_is_an_error_and_never_a_newer_version() {
    for nonsense in ["", "<html>503</html>", "[]", "{}", r#"{"version":"the newest"}"#, r#"{"version":7}"#] {
        assert!(update_check::read(nonsense, "0.1.1-alpha").is_err(), "{nonsense:?} is no release");
    }
}

#[test]
fn only_files_of_the_projects_releases_on_github_are_ours() {
    assert!(update_check::files_are_ours(&answer("0.1.2-alpha")));
    let mut elsewhere = answer("0.1.2-alpha");
    elsewhere["platforms"]["windows-x86_64"]["url"] = json!("https://example.org/CoWriting.Editor_x64-setup.exe");
    assert!(!update_check::files_are_ours(&elsewhere), "one file from another place is enough to refuse");
    let mut other_project = answer("0.1.2-alpha");
    other_project["platforms"]["darwin-aarch64"]["url"] =
        json!("https://github.com/dozwa/cowriting-editor/releases/download/../../../other/project/releases/download/v1/app.tar.gz");
    assert!(!update_check::files_are_ours(&other_project));
    assert!(!update_check::files_are_ours(&json!({"version": "0.3.0"})), "an answer that names no file has none of ours");
    assert!(!update_check::files_are_ours(&json!({"version": "0.3.0", "platforms": {}})));
    assert!(update_check::files_are_ours(&json!({"version": "0.3.0", "url": "https://github.com/dozwa/cowriting-editor/releases/download/v0.3.0/app.tar.gz", "signature": "sig"})));
}

#[test]
fn a_release_asks_at_the_file_on_github_and_a_dev_version_does_not_ask() {
    // the README and the privacy statement name this address; a change here changes them too
    assert_eq!(update_check::SERVER, "https://raw.githubusercontent.com/dozwa/cowriting-editor/updates/latest.json");
    assert_eq!(update_check::server(None, "0.1.1-alpha").as_deref(), Some(update_check::SERVER));
    assert_eq!(update_check::server(None, "0.1.1-dev"), None);
    // for a trial, only an address on this computer
    assert_eq!(update_check::server(Some("http://127.0.0.1:4010/latest.json"), "0.1.1-dev").as_deref(), Some("http://127.0.0.1:4010/latest.json"));
    assert_eq!(update_check::server(Some("https://example.org/latest.json"), "0.1.1-alpha"), None);
}

#[test]
fn the_check_at_start_is_on_until_the_author_switches_it_off() {
    assert!(Preferences::default().update_check);
    let kept: Preferences = serde_json::from_value(json!({ "updateCheck": false })).unwrap();
    assert!(!kept.within_limits().update_check);
}

#[tokio::test]
async fn the_request_is_a_plain_get_that_says_nothing_about_the_editor_or_the_author() {
    let (address, got) = server(200, answer("0.1.2-alpha").to_string());
    let found = update_check::check(&address, "0.1.1-alpha").await.unwrap().expect("a newer release");
    assert_eq!(found.version, "0.1.2-alpha");
    let request = got.join().unwrap().to_lowercase();
    let mut lines = request.lines();
    assert_eq!(lines.next(), Some("get /latest.json http/1.1"), "no version and no number in the address");
    let headers: Vec<&str> = lines.filter(|line| !line.is_empty()).map(|line| line.split(':').next().unwrap()).collect();
    assert!(headers.iter().all(|name| ["accept", "host"].contains(name)), "nothing goes with the request but what a GET needs: {headers:?}");
    assert!(!request.contains("0.1.1") && !request.contains("user-agent") && !request.contains("cookie"));
}

#[tokio::test]
async fn a_server_that_fails_or_cannot_be_reached_gives_a_reason_without_its_address() {
    let (address, got) = server(503, String::new());
    assert_eq!(update_check::check(&address, "0.1.1-alpha").await, Err("The server answered 503.".into()));
    got.join().unwrap();
    // nobody listens there
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let failed = update_check::check(&format!("http://127.0.0.1:{free}/latest.json"), "0.1.1-alpha").await.unwrap_err();
    assert!(failed.starts_with("The server could not be reached") && !failed.contains("127.0.0.1"), "{failed}");
    let (address, got) = server(200, "<html>not json</html>".into());
    assert_eq!(update_check::check(&address, "0.1.1-alpha").await, Err("The server's answer could not be read.".into()));
    got.join().unwrap();
}

#[test]
fn the_author_is_told_about_a_version_by_itself_once() {
    let folder = std::env::temp_dir().join(format!("cowriting-update-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let state = folder.join("state.json");
    std::fs::write(&state, r#"{"lastDocument":"/somewhere/thesis.md"}"#).unwrap();
    assert!(!update_check::was_told(&state, "0.1.2-alpha"));
    update_check::told(&state, "0.1.2-alpha");
    assert!(update_check::was_told(&state, "0.1.2-alpha"));
    assert!(!update_check::was_told(&state, "0.1.3-alpha"), "the next version is news again");
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    assert_eq!(kept["lastDocument"], "/somewhere/thesis.md", "what else the file holds stays");
    let _ = std::fs::remove_dir_all(&folder);
}
