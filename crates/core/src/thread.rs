//! A feedback thread: the feedback, the author's replies, the answers.
//! Ported from `threadOf` in the plugin's `plugin/hooks/register.tsx`.

use crate::log::readable;
use crate::personas::Party;
use crate::types::{LogEntry, PromptEntry, ResponseEntry, ThreadMessage};

/// The whole thread an input is part of, from its first feedback on, oldest first.
/// `assistant` is what an answer without a persona is signed with.
pub fn thread_of(entries: &[LogEntry], id: &str, party: &Party, assistant: &str) -> Vec<ThreadMessage> {
    let prompts: Vec<&PromptEntry> = entries.iter().filter_map(|e| if let LogEntry::Prompt(p) = e { Some(p) } else { None }).collect();
    let response_of = |reference: &str| -> Option<&ResponseEntry> {
        entries.iter().find_map(|e| match e {
            LogEntry::Response(r) if r.reference == reference => Some(r),
            _ => None,
        })
    };
    let Some(mut root) = prompts.iter().copied().find(|p| p.id == id) else { return Vec::new() };
    let mut seen = vec![root.id.as_str()];
    while let Some(up) = root.reply_to.as_deref().and_then(|parent| prompts.iter().copied().find(|p| p.id == parent)) {
        if seen.contains(&up.id.as_str()) {
            break; // a loop in the log is not followed
        }
        seen.push(&up.id);
        root = up;
    }
    let answer_of = |p: &PromptEntry| -> ThreadMessage {
        let reply = response_of(&p.id);
        let persona = p.agent.clone().or_else(|| reply.and_then(|r| r.personas.first().cloned())).unwrap_or_default();
        let text = if p.origin.as_deref() == Some("agent") { readable(&p.text) } else { readable(reply.map_or("", |r| r.answer.as_str())) };
        ThreadMessage {
            id: p.id.clone(),
            role: "ai".into(),
            who: if persona.is_empty() { assistant.to_string() } else { party.by_id(&persona).map_or(persona.clone(), |known| known.name.clone()) },
            persona,
            ts: p.ts.clone(),
            text: if text.is_empty() { "(no text)".into() } else { text },
        }
    };
    let mut out = vec![answer_of(root)];
    let mut walked: Vec<&str> = Vec::new();
    let mut pending = vec![root.id.as_str()];
    // depth first, children by time: a reply is followed by its own answers before the next reply
    while let Some(parent) = pending.pop() {
        if walked.contains(&parent) {
            continue;
        }
        walked.push(parent);
        if parent != root.id {
            let child = prompts.iter().copied().find(|p| p.id == parent).expect("pending ids come from prompts");
            if child.origin.as_deref() == Some("agent") {
                out.push(answer_of(child));
            } else {
                let text = readable(&child.text);
                out.push(ThreadMessage {
                    id: child.id.clone(),
                    role: "author".into(),
                    who: "You".into(),
                    persona: String::new(),
                    ts: child.ts.clone(),
                    text: if text.is_empty() { "(no text)".into() } else { text },
                });
                // a reply that only sent for a persona has no answer of its own
                if response_of(&child.id).is_some_and(|r| r.agents.is_none()) {
                    out.push(answer_of(child));
                }
            }
        }
        let mut children: Vec<&PromptEntry> = prompts.iter().copied().filter(|p| p.reply_to.as_deref() == Some(parent)).collect();
        children.sort_by(|a, b| a.ts.cmp(&b.ts));
        pending.extend(children.iter().rev().map(|c| c.id.as_str()));
    }
    out
}
