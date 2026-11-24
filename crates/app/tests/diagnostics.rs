//! The editor's technical log on a real folder: it records what was used and what failed, and
//! nothing that says who wrote, what they wrote, where it is kept or how to get into it.

use cowriting_app::diagnostics::{scrubbed, shape, Diagnostics, Seen};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-diagnostics-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Folder(path)
    }

    fn log(&self) -> Diagnostics {
        Diagnostics::at(self.0.join("config").join("diagnostics.log"), true)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn on_disk(log: &Diagnostics) -> String {
    std::fs::read_to_string(log.file()).unwrap_or_default()
}

#[test]
fn what_is_private_or_secret_never_reaches_the_file() {
    let folder = Folder::new("private");
    let log = folder.log();
    log.keep_document_private("/Users/marta.keller/Promotion/Kapitel 3 Methoden.md");

    log.note("error", "/Users/marta.keller/Promotion/Kapitel 3 Methoden.md could not be saved: Permission denied (os error 13)");
    log.note("error", "Kapitel 3 Methoden is locked by marta.keller");
    log.note("error", r"C:\Users\Jo\thesis draft.md could not be created, see notes.txt");
    log.note("error", "error sending request for url (https://llm.uni-example.de:8443/v1/chat): refused; tried 10.0.3.7:11434 and gateway.intern.example too");
    log.note("error", "401 for key sk-ant-REDACTED and Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6");
    log.note("error", "write to jo.mueller@uni-example.de about it");
    log.note("error", "invalid type: string \"Die Befragten gaben an\", expected a number; near „im zweiten Absatz“ and `secret words`");
    log.note("error", "several\nlines\tof it");

    let written = on_disk(&log);
    for gone in [
        "marta", "keller", "Promotion", "Kapitel", "Methoden", "thesis", "draft", "notes.txt", "Jo", "uni-example", "8443", "10.0.3.7", "gateway", "sk-ant", "abcdefghij", "eyJhbGci", "mueller", "Befragten",
        "zweiten", "secret words",
    ] {
        assert!(!written.contains(gone), "'{gone}' is in the log:\n{written}");
    }
    // what went wrong is still there
    for kept in ["could not be saved: Permission denied (os error 13)", "could not be created", "refused", "401 for key", "expected a number", "several lines of it"] {
        assert!(written.contains(kept), "'{kept}' is missing:\n{written}");
    }
    assert_eq!(written.lines().count(), 8, "one line each, whatever the message held");

    // a very long message is cut
    assert!(scrubbed(&"word ".repeat(500), &[]).chars().count() <= 301);
    // a model on this computer can be named
    assert_eq!(scrubbed("model gemma3:4b at localhost:11434 not found", &[]), "model gemma3:4b at localhost:11434 not found");
}

#[test]
fn the_window_names_what_happened_and_cannot_write_what_it_likes() {
    let folder = Folder::new("window");
    let log = folder.log();
    let call = |name: &str, ok: bool, ms: u64, count: u32, message: &str| Seen { kind: "call".into(), name: name.into(), ok, ms, count, message: message.into() };
    log.from_window(&[
        call("saveDocument", true, 12, 1, ""),
        call("scope", true, 8, 14, ""),
        call("ask", false, 2301, 1, "The provider at https://api.example.org/v1 answered 500"),
        call("an essay about\nmy colleague, with spaces", true, 1, 1, "a message on a call that went well is not kept"),
        Seen { kind: "view".into(), name: "tab:log".into(), ..Default::default() },
        Seen { kind: "error".into(), name: String::new(), message: "TypeError: undefined is not an object at /Users/someone/app.js:10".into(), ..Default::default() },
        Seen { kind: "diary".into(), name: "today".into(), message: "anything".into(), ..Default::default() },
    ]);
    let written = on_disk(&log);
    let lines: Vec<&str> = written.lines().map(|line| line.split_once(' ').unwrap().1).collect();
    assert_eq!(
        lines,
        [
            "call saveDocument ok 12ms",
            "call scope ok ×14 8ms",
            "call ask failed 2301ms: The provider at <address> answered 500",
            "call anessayaboutmycolleaguewithspaces ok 1ms",
            "view tab:log",
            "error window: TypeError: undefined is not an object at <path>",
        ]
    );

    // more than it may hand over at once is dropped, not written
    let many: Vec<Seen> = (0..500).map(|_| call("scope", true, 1, 1, "")).collect();
    log.from_window(&many);
    assert_eq!(on_disk(&log).lines().count(), 6 + 60);
}

#[test]
fn a_report_gets_the_last_minutes_and_the_text_only_when_the_author_says_so() {
    let folder = Folder::new("recent");
    let log = folder.log();
    let now = SystemTime::now();
    let ago = |minutes: u64| now - Duration::from_secs(minutes * 60);
    log.note_at(ago(45), "call", "openDocument ok 30ms");
    log.note_at(ago(9), "call", "ask failed 800ms: timed out");
    log.note_at(ago(1), "call", "saveDocument ok 9ms");

    let recent = log.recent_at(now, 10);
    assert!(!recent.contains("openDocument") && recent.contains("ask failed") && recent.ends_with("saveDocument ok 9ms"), "{recent}");
    assert_eq!(log.recent_at(now, 60).lines().count(), 3);

    let text = "Ein Satz, den niemand sehen soll.";
    let without = log.report("CoWriting Editor 0.9.0, macos aarch64", 10, None);
    assert!(without.contains("ask failed 800ms") && without.contains("0.9.0") && !without.contains("niemand"));
    assert!(without.contains("The document's text is not in this file."));
    assert!(log.report("x", 10, Some(text)).contains(text));
}

#[test]
fn it_can_be_turned_off_deleted_and_does_not_grow_without_end() {
    let folder = Folder::new("bounds");
    let log = folder.log();
    log.note("start", "0.9.0");
    log.set_on(false);
    log.note("call", "ask ok 1ms");
    log.note_document("opened", "text");
    log.from_window(&[Seen { kind: "view".into(), name: "tab:ask".into(), ..Default::default() }]);
    assert_eq!(on_disk(&log).lines().count(), 1, "off writes nothing, and keeps what was there");
    log.clear().unwrap();
    assert!(!log.file().exists());
    log.clear().unwrap();

    log.set_on(true);
    let line = "x".repeat(250);
    for _ in 0..4000 {
        log.note_plain("call", &line);
    }
    let size = std::fs::metadata(log.file()).unwrap().len();
    assert!(size < 450_000, "the log is {size} bytes");
    let written = on_disk(&log);
    assert!(written.lines().all(|line| line.len() > 250 && line.contains(" call ")), "cutting kept whole lines");
}

#[test]
fn a_document_is_described_by_its_measures_and_none_of_its_words() {
    let text = "\u{feff}---\ntitle: Vertraulich\n---\n# Einleitung\r\n\r\nDie Befragten 🙂 sagten [@meier2020; @kurz2019] etwas[^1].\n\n| a | b |\n|---|---|\n\n```r\nplot(x)\n\n![Bild](bild.png) und [Link](https://example.org)\n<div>\n<!-- Notiz -->\n$$\nx\n$$\n\tEnde\u{0}";
    let said = shape(text);
    for word in ["Vertraulich", "Einleitung", "Befragten", "meier", "bild", "example", "Notiz", "plot", "Ende"] {
        assert!(!said.contains(word), "'{word}' is in: {said}");
    }
    for measure in [
        "mixed line ends",
        ", byte order mark",
        "1 tabs",
        "1 control characters",
        "1 outside the basic plane",
        "; front matter",
        "1 headings",
        "1 fence lines (one not closed)",
        "2 table lines",
        "1 footnote marks",
        "2 citations",
        "1 images",
        "2 links",
        "1 lines starting with a tag",
        "1 comments",
        "1 display formulas",
    ] {
        assert!(said.contains(measure), "'{measure}' is missing in: {said}");
    }
    assert!(said.starts_with(&format!("{} bytes, {} characters, ", text.len(), text.chars().count())));
    assert!(shape("").starts_with("0 bytes, 0 characters, 0 lines (longest 0), 0 words, one line, no byte order mark"));
}
