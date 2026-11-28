//! AI use outside the editor is the author's own statement: judged by the log and the disclosure on disk.

use cowriting_app::{ExportOptions, ExternalUse, Session};
use cowriting_core::host::DiskHost;
use std::sync::Arc;

#[test]
fn the_author_states_outside_ai_use_corrects_it_and_takes_it_back_and_the_disclosure_follows() {
    let folder = std::env::temp_dir().join(format!("cowriting-external-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let document = folder.join("thesis.md").to_string_lossy().into_owned();
    std::fs::write(&document, "# Walking breaks\n").unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &document).unwrap();
    let export = |language: &str| {
        let options: ExportOptions = serde_json::from_value(serde_json::json!({ "language": language, "format": "md", "withPrompts": false, "withAnswers": false })).unwrap();
        session.export(&options).unwrap().register
    };

    // nothing stated: the disclosure has no such section
    assert!(session.state_external(&ExternalUse { tool: "ChatGPT".into(), ..Default::default() }).is_err(), "what it was used for is needed");
    assert!(session.state_external(&ExternalUse { purpose: "code".into(), ..Default::default() }).is_err(), "the tool is needed");

    let stated = ExternalUse {
        tool: "ChatGPT".into(),
        model: "GPT-x".into(),
        date: "2026-11-02".into(),
        purpose: "Writing R code for\nthe mixed model".into(),
        material: "The trial data (z)".into(),
        ..Default::default()
    };
    session.state_external(&stated).unwrap();
    session.state_external(&ExternalUse { tool: "DeepL".into(), purpose: "Translating the abstract".into(), ..Default::default() }).unwrap();
    let view = session.log_view("").unwrap();
    assert_eq!(view.external.iter().map(|e| e.tool.as_str()).collect::<Vec<_>>(), ["ChatGPT", "DeepL"]);
    let register = export("en");
    assert!(register.contains("## AI use outside the editor, as stated by the author"), "{register}");
    assert!(register.contains("- 2026-11-02: ChatGPT (GPT-x). Used for: Writing R code for the mixed model. Worked on: The trial data (z)."), "{register}");
    assert!(register.contains("- DeepL. Used for: Translating the abstract."));
    assert!(register.contains("These statements are the author's own."));

    // a correction takes the statement's place; a withdrawal removes it; the log keeps every line
    let first = view.external[0].id.clone();
    session.state_external(&ExternalUse { id: first.clone(), checked: "Every result recomputed by hand".into(), ..stated.clone() }).unwrap();
    assert!(session.state_external(&ExternalUse { id: first.clone(), ..stated.clone() }).is_err(), "a replaced statement cannot be replaced again");
    let view = session.log_view("").unwrap();
    assert_eq!(view.external.len(), 2);
    assert_eq!(view.external[0].checked, "Every result recomputed by hand");
    session.withdraw_external(&view.external[1].id).unwrap();
    assert!(session.withdraw_external("nothing").is_err());
    let german = export("de");
    assert!(german.contains("Geprüft durch: Every result recomputed by hand.") && !german.contains("DeepL"), "{german}");
    let log_folder = folder.join(".cowriting/thesis.md/ai-log");
    let log = std::fs::read_to_string(std::fs::read_dir(log_folder).unwrap().next().unwrap().unwrap().path()).unwrap();
    assert_eq!(log.lines().filter(|line| line.contains(r#""type":"external""#)).count(), 4);
    assert_eq!(std::fs::read_to_string(&document).unwrap(), "# Walking breaks\n");
    let _ = std::fs::remove_dir_all(&folder);
}

/// A document in a folder of its own, and its open session.
fn opened(name: &str) -> (std::path::PathBuf, Session) {
    let folder = std::env::temp_dir().join(format!("cowriting-external-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let document = folder.join("thesis.md").to_string_lossy().into_owned();
    std::fs::write(&document, "# Walking breaks\n").unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &document).unwrap();
    (folder, session)
}

fn fixture(name: &str) -> String {
    format!("{}/../../fixtures/chats/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn exported(session: &Session, language: &str, format: &str) -> (String, String) {
    let options: ExportOptions =
        serde_json::from_value(serde_json::json!({ "language": language, "format": format, "withPrompts": false, "withAnswers": false, "withOutside": true })).unwrap();
    let written = session.export(&options).unwrap();
    assert!(written.paths[1].ends_with(&format!("ai-outside.{language}.{format}")), "{:?}", written.paths);
    (written.register, std::fs::read_to_string(&written.paths[1]).unwrap())
}

#[test]
fn chats_chosen_from_an_export_are_kept_with_a_statement_and_printed_in_the_appendix() {
    use cowriting_app::outside::{conversations, NewAttachment};
    let (folder, session) = opened("chats");
    let export = fixture("chatgpt-conversations.json");

    // the author sees what the export holds, and chooses
    let offered = conversations(&export).unwrap();
    assert_eq!(offered.form, "chatgpt");
    assert_eq!(
        offered.list.iter().map(|c| (c.title.as_str(), c.date.as_str(), c.turns)).collect::<Vec<_>>(),
        [("Mixed model in R", "2026-11-01", 2), ("Holiday plans", "2026-11-02", 1)]
    );
    assert_eq!(conversations(&folder.join("thesis.md").to_string_lossy()).unwrap().form, "", "a file that is no chat can only be kept as a file");

    let stated = ExternalUse {
        tool: "ChatGPT".into(),
        purpose: "R code for the mixed model".into(),
        add: vec![
            NewAttachment::Chats { path: export.clone(), conversations: vec![0] },
            NewAttachment::Chats { path: fixture("claude-code-session.jsonl"), conversations: vec![0] },
            NewAttachment::Text { text: "Frage: Reichen 48?\nAntwort: Für eine Pilotstudie ja.".into() },
            NewAttachment::Text { text: "Notes I took while using the tool.".into() },
            NewAttachment::File { path: folder.join("thesis.md").to_string_lossy().into_owned() },
        ],
        ..Default::default()
    };
    session.state_external(&stated).unwrap();
    let view = session.log_view("").unwrap();
    let kept = &view.external[0].attachments;
    assert_eq!(
        kept.iter().map(|a| (a.kind.as_str(), a.turns, a.from.as_str(), a.state.as_str())).collect::<Vec<_>>(),
        [("chat", 2, "chatgpt", "intact"), ("chat", 2, "claude-code", "intact"), ("chat", 1, "pasted", "intact"), ("text", 0, "", "intact"), ("file", 0, "", "intact"),]
    );
    assert_eq!(kept[4].title, "thesis.md");
    let external = folder.join(".cowriting/thesis.md/external");
    assert_eq!(std::fs::read_dir(&external).unwrap().count(), 5);
    // only what was chosen is beside the document: not the other conversation, not the branch left behind, not the tools' output
    let everything: String = std::fs::read_dir(&external).unwrap().map(|file| std::fs::read_to_string(file.unwrap().path()).unwrap()).collect();
    for absent in ["Lisbon", "regenerated", "exit code 0", "id,a,b", "/work/analysis"] {
        assert!(!everything.contains(absent), "{absent} was kept");
    }
    let chat = std::fs::read_to_string(external.join(&kept[0].file)).unwrap();
    assert!(chat.contains("\"from\": \"chatgpt-conversations.json\"") && chat.contains("\"fromSha256\""), "{chat}");
    assert_eq!(kept[0].sha256.len(), 64);

    // the register names each file, the appendix prints the chats
    let (register, appendix) = exported(&session, "en", "md");
    let short: String = kept[0].sha256.chars().take(12).collect();
    assert!(
        register.contains(&format!(
            "Kept with it: chat “Mixed model in R” (ChatGPT export, 2 exchanges, file {}, SHA-256 {short}…), use: not annotated; chat “Limits of agreement script”",
            kept[0].file
        )),
        "{register}"
    );
    assert!(register.contains("file “thesis.md”") && register.contains("pasted text (file text-"), "{register}");
    assert!(appendix.starts_with("# Appendix: AI use outside the editor"), "{appendix}");
    assert!(appendix.contains("The editor did not see this use.") && appendix.contains(&kept[0].sha256), "{appendix}");
    assert!(appendix.contains("## ChatGPT\n\nUsed for: R code for the mixed model"), "{appendix}");
    assert!(appendix.contains("Model: gpt-x · Begun: 2026-11-01 · Tool calls, files and pictures left out: 3 · Messages on branches not continued, left out: 1"), "{appendix}");
    assert!(appendix.contains("**1. Asked · 2026-11-01 09:00:**\n\n> How do I fit a mixed model with a random slope in R?\n\n**Answered:**\n\n> Use lme4:"), "{appendix}");
    assert!(
        appendix.contains("> Für eine Pilotstudie ja.") && appendix.contains("> Notes I took while using the tool.") && appendix.contains("Kept as a file; not printed here."),
        "{appendix}"
    );
    assert!(appendix.contains("### chat (pasted text, 1 exchange, file chat-"), "{appendix}");
    let (german_register, _) = exported(&session, "de", "md");
    assert!(
        german_register.contains("Dazu abgelegt: Chat „Mixed model in R“ (ChatGPT-Export, 2 Wechsel, Datei ")
            && german_register.contains("Chat (eingefügter Text, 1 Wechsel, Datei "),
        "{german_register}"
    );
    let (_, german) = exported(&session, "de", "tex");
    assert!(german.contains("\\section*{Anhang: KI-Nutzung außerhalb des Editors}") && german.contains("\\item \\textit{Gefragt (2026-11-01 09:00):} How do I fit"), "{german}");
    assert!(german.contains("\\allowbreak{}"), "the checksum can break: {german}");

    // a file changed after it was kept is said to be changed, and is not printed
    std::fs::write(external.join(&kept[0].file), chat.replace("lme4", "nlme")).unwrap();
    assert_eq!(session.log_view("").unwrap().external[0].attachments[0].state, "changed");
    let (_, appendix) = exported(&session, "en", "md");
    assert!(appendix.contains("This file was changed after it was kept; it is not printed.") && !appendix.contains("nlme") && !appendix.contains("random slope"), "{appendix}");

    // a correction that leaves a file out takes it away; a withdrawal takes all of them
    let mut corrected = view.external[0].clone();
    corrected.attachments.remove(0);
    corrected.add = vec![NewAttachment::Chats { path: export, conversations: vec![7] }];
    assert!(session.state_external(&corrected).is_err(), "there is no eighth conversation");
    assert_eq!(std::fs::read_dir(&external).unwrap().count(), 5, "a statement that did not come about leaves the files as they were");
    corrected.add = Vec::new();
    session.state_external(&corrected).unwrap();
    assert_eq!(std::fs::read_dir(&external).unwrap().count(), 4);
    let view = session.log_view("").unwrap();
    assert_eq!(view.external[0].attachments.len(), 4);
    session.withdraw_external(&view.external[0].id).unwrap();
    assert_eq!(std::fs::read_dir(&external).unwrap().count(), 0);
    assert_eq!(std::fs::read_to_string(folder.join("thesis.md")).unwrap(), "# Walking breaks\n");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn text_pasted_from_a_kept_chat_is_found_and_the_disclosure_says_where_it_is_from() {
    use cowriting_app::outside::NewAttachment;
    let (folder, session) = opened("paste");
    let answer = "A random slope lets the effect of walking breaks differ between participants, which a random intercept alone cannot express in the model.";
    session
        .state_external(&ExternalUse {
            tool: "ChatGPT".into(),
            purpose: "Explaining the model".into(),
            add: vec![NewAttachment::Text { text: format!("Prompt: Why a random slope?\nAnswer: {answer}") }],
            ..Default::default()
        })
        .unwrap();
    let text = format!("# Walking breaks\n\n{answer}\n");
    let found = session.report_paste(answer, 3, &text).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].outside.as_str(), found[0].asked.as_str(), found[0].turn.as_str()), ("ChatGPT", "Why a random slope?", ""));
    assert!(session.report_paste(answer, 3, &text).unwrap().is_empty(), "found once");
    let (register, _) = exported(&session, "en", "md");
    assert!(register.contains("from a kept chat with ChatGPT, answer to “Why a random slope?”"), "{register}");
    // what the author wrote in the question is theirs
    assert!(session.report_paste("Why a random slope?", 5, &text).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_kept_chat_is_classified_like_an_answer_and_the_disclosure_says_what_was_done_with_it() {
    use cowriting_app::outside::NewAttachment;
    let (folder, session) = opened("classified");
    let stated = ExternalUse {
        tool: "ChatGPT".into(),
        purpose: "R code for the mixed model".into(),
        add: vec![
            NewAttachment::Text { text: "Prompt: Are 48 enough?\nAnswer: For a pilot study, yes.".into() },
            NewAttachment::File { path: folder.join("thesis.md").to_string_lossy().into_owned() },
        ],
        ..Default::default()
    };
    session.state_external(&stated).unwrap();
    let kept = session.log_view("").unwrap().external[0].attachments.clone();
    let (chat, file) = (&kept[0], &kept[1]);
    assert_eq!((chat.use_.as_str(), chat.note.as_str()), ("", ""), "nothing said yet");
    let (register, appendix) = exported(&session, "en", "md");
    assert!(register.contains("SHA-256") && register.contains("), use: not annotated; file “thesis.md”"), "an unclassified chat is said to be: {register}");
    assert!(appendix.contains("\n\nUse: not annotated.\n\n"), "{appendix}");

    // the reference names the kept file; anything else is refused
    let reference = format!("external:{}", chat.file);
    assert!(session.annotate(&reference, "copied", "").is_err(), "one of the four choices");
    assert!(session.annotate("external:not-kept.json", "adopted", "").is_err());
    assert!(session.annotate(&format!("external:{}", file.file), "adopted", "").is_err(), "a file that is no chat is not an answer to classify");
    session.annotate(&reference, "adopted", "").unwrap();
    // a later choice counts, as for a turn; the log keeps both
    session.annotate(&reference, "adapted", "  Took the model, wrote the code myself  ").unwrap();
    let view = session.log_view("").unwrap();
    let chat = &view.external[0].attachments[0];
    assert_eq!((chat.use_.as_str(), chat.note.as_str()), ("adapted", "Took the model, wrote the code myself"));
    assert_eq!((view.stats.turns, view.stats.annotations, view.stats.unannotated), (0, 0, 0), "the editor's own turns are counted apart");

    let (register, appendix) = exported(&session, "en", "md");
    assert!(register.contains("), use: adapted (Took the model, wrote the code myself); file “thesis.md” ("), "{register}");
    assert!(appendix.contains("\n\nUse: adapted (Took the model, wrote the code myself).\n\n"), "{appendix}");
    let (german, german_appendix) = exported(&session, "de", "md");
    assert!(german.contains("), Verwendung: angepasst (Took the model, wrote the code myself)"), "{german}");
    assert!(german_appendix.contains("Verwendung: angepasst (Took the model, wrote the code myself)."), "{german_appendix}");
    let (_, tex) = exported(&session, "en", "tex");
    assert!(tex.contains("Use: adapted (Took the model, wrote the code myself).\\par"), "{tex}");

    // a corrected statement that still keeps the chat keeps its classification
    let id = view.external[0].id.clone();
    session.state_external(&ExternalUse { id, checked: "Recomputed by hand".into(), ..view.external[0].clone() }).unwrap();
    assert_eq!(session.log_view("").unwrap().external[0].attachments[0].use_, "adapted");
}

#[test]
fn the_ai_log_of_the_plugin_is_read_as_chats_that_can_be_kept_with_a_statement() {
    use cowriting_app::outside::{conversations, NewAttachment};
    let (_folder, session) = opened("plugin-log");
    let log = fixture("cowriting-plugin-log.jsonl");
    let found = conversations(&log).unwrap();
    // one conversation for each session; the plugin's housekeeping and a question without an answer are not exchanges
    assert_eq!(found.form, "cowriting-log");
    assert_eq!(
        found.list.iter().map(|chat| (chat.tool.as_str(), chat.title.as_str(), chat.date.as_str(), chat.turns)).collect::<Vec<_>>(),
        [("Claude Code with the CoWriting plugin", "Walking breaks", "2026-10-01", 2), ("Claude Code with the CoWriting plugin", "Walking breaks", "2026-10-03", 1)]
    );
    // the plugin's own fixture log is read too
    let own = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl");
    assert_eq!(conversations(&own.to_string_lossy()).unwrap().form, "cowriting-log");

    let stated = ExternalUse {
        tool: "Claude Code with the CoWriting plugin".into(),
        purpose: "Feedback on the first draft".into(),
        add: vec![NewAttachment::Chats { path: log, conversations: vec![0] }],
        ..Default::default()
    };
    session.state_external(&stated).unwrap();
    let view = session.log_view("").unwrap();
    let kept = &view.external[0].attachments[0];
    assert_eq!((kept.kind.as_str(), kept.turns, kept.from.as_str(), kept.title.as_str()), ("chat", 2, "cowriting-log", "Walking breaks"));
    assert_eq!(view.stats.turns, 0, "nothing of it became a turn of this document's own log");
    let (register, appendix) = exported(&session, "en", "md");
    assert!(register.contains("chat “Walking breaks” (AI log of the CoWriting plugin or editor, 2 exchanges, file "), "{register}");
    assert!(appendix.contains("> Give me feedback on the introduction.") && appendix.contains("> Most journals now ask for 'participants'."), "{appendix}");
    assert!(appendix.contains("Tool calls, files and pictures left out: 2") && !appendix.contains("/cowrite:stats") && !appendix.contains("48 participants"), "{appendix}");
}
