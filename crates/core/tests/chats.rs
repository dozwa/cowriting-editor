//! Chats from other AI tools, read from made-up samples of each form (`fixtures/chats/`).

use cowriting_core::chats::{from_text, read, Chat};

#[test]
fn a_chatgpt_export_gives_the_branch_that_was_continued_and_counts_what_it_leaves_out() {
    let (form, chats) = read(include_str!("../../../fixtures/chats/chatgpt-conversations.json")).unwrap();
    assert_eq!(form, "chatgpt");
    assert_eq!(chats.iter().map(|chat| chat.title.as_str()).collect::<Vec<_>>(), ["Mixed model in R", "Holiday plans"], "a conversation without an exchange is not offered");
    let chat = &chats[0];
    assert_eq!((chat.tool.as_str(), chat.model.as_str(), chat.date.as_str()), ("ChatGPT", "gpt-x", "2026-11-01"));
    assert_eq!(chat.other_branches, 1, "the regenerated answer");
    assert_eq!(chat.turns.len(), 2);
    assert_eq!(chat.turns[0].prompt, "How do I fit a mixed model with a random slope in R?");
    assert_eq!(chat.turns[0].answer, "Use lme4: `lmer(y ~ x + (x | subject), data = d)`.");
    assert_eq!(chat.turns[0].at, "2026-11-01T09:00:01.000Z");
    assert_eq!(chat.turns[1].prompt, "Here is my plot. Why does it not converge?");
    assert_eq!(chat.turns[1].answer, "The slope variance is near zero. Simplify the random part.");
    assert_eq!(chat.turns[1].left_out, 3, "the picture, the tool's output and the code for the tool");
    assert!(!chat.to_file().contains("regenerated"));
}

#[test]
fn a_claude_ai_export_gives_its_messages_and_counts_files_and_tool_calls() {
    let (form, chats) = read(include_str!("../../../fixtures/chats/claude-ai-conversations.json")).unwrap();
    assert_eq!((form, chats.len()), ("claude-ai", 1));
    let chat = &chats[0];
    assert_eq!((chat.tool.as_str(), chat.title.as_str(), chat.date.as_str()), ("Claude", "Explaining Bland-Altman", "2026-11-03"));
    assert_eq!(chat.turns.len(), 2);
    assert_eq!(chat.turns[1].answer, "Mean difference plus and minus 1.96 standard deviations.");
    assert_eq!((chat.turns[0].left_out, chat.turns[1].left_out, chat.left_out()), (0, 2, 2), "the attached paper and the search");
}

#[test]
fn a_claude_code_transcript_gives_what_was_asked_and_answered_without_the_tools_and_the_programs_own_lines() {
    let (form, chats) = read(include_str!("../../../fixtures/chats/claude-code-session.jsonl")).unwrap();
    assert_eq!((form, chats.len()), ("claude-code", 1));
    let chat = &chats[0];
    assert_eq!((chat.tool.as_str(), chat.model.as_str(), chat.title.as_str(), chat.date.as_str()), ("Claude Code", "claude-x", "Limits of agreement script", "2026-11-05"));
    assert_eq!(chat.turns.iter().map(|turn| turn.prompt.as_str()).collect::<Vec<_>>(), ["Write a script that computes the limits of agreement from data.csv.", "Add a test."]);
    assert_eq!(chat.turns[0].answer, "I will read the file first.\n\n`loa.py` prints the mean difference and both limits.");
    assert_eq!(chat.turns[0].left_out, 2);
    let kept = chat.to_file();
    for absent in ["helper said", "program wrote", "command-name", "id,a,b", "/work/analysis", "thinking"] {
        assert!(!kept.contains(absent), "{absent} is kept");
    }
}

#[test]
fn a_kept_chat_reads_back_as_it_was_and_other_files_are_no_chat() {
    let (_, chats) = read(include_str!("../../../fixtures/chats/claude-ai-conversations.json")).unwrap();
    let kept = chats[0].to_file();
    assert_eq!(Chat::kept(&kept).as_ref(), Some(&chats[0]));
    assert_eq!(read(&kept).map(|(form, chats)| (form, chats.len())), Some(("cowriting", 1)));
    for other in ["", "# Notes\n\nNo chat.", "{\"a\": 1}", "[1, 2]", "[]", "{\"type\":\"x\"}\n{\"type\":\"y\"}"] {
        assert!(read(other).is_none(), "{other}");
        assert!(Chat::kept(other).is_none());
    }
}

#[test]
fn pasted_text_is_a_chat_only_with_the_authors_marks() {
    let chat = from_text(
        "Prompt: Is 48 enough for a pilot?\nI have two groups.\n\nAnswer: For a pilot, yes.\n\nYou: I think so too.\nWhat about power?\n**ChatGPT:** Not the aim of a pilot.\n",
    )
    .unwrap();
    assert_eq!(chat.turns.len(), 2);
    assert_eq!((chat.turns[0].prompt.as_str(), chat.turns[0].answer.as_str()), ("Is 48 enough for a pilot?\nI have two groups.", "For a pilot, yes."));
    assert_eq!((chat.turns[1].prompt.as_str(), chat.turns[1].answer.as_str()), ("I think so too.\nWhat about power?", "Not the aim of a pilot."));
    // as ChatGPT's page copies, and in German
    assert_eq!(from_text("You said:\nWarum?\nChatGPT said:\nDarum.").unwrap().turns[0].answer, "Darum.");
    assert_eq!(from_text("Frage: Warum?\nAntwort: Darum.").unwrap().turns[0].prompt, "Warum?");
    // a sentence that begins with such a word is no mark
    for unmarked in ["You know what I mean. Answers: none.", "Ich denke: ja.\nClaude Shannon wrote that.", "Prompt: only a question"] {
        assert!(from_text(unmarked).is_none(), "{unmarked}");
    }
}
