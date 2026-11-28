//! What the log adds up to: turns, what waits for the author, what each persona was asked.
//! Ported from `computeStats` in the plugin's `plugin/hooks/core.ts`.

use crate::log::flatten;
use crate::personas::Party;
use crate::text::slice_utf16;
use crate::types::{AnnotationEntry, LogEntry, PromptEntry, RecentTurn, ResponseEntry, Stats};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Turns the editor starts on its own: AI input, disclosed, but the author owes them no classification.
pub fn is_auto_origin(origin: Option<&str>) -> bool {
    matches!(origin, Some("watch" | "scope" | "summary" | "brief"))
}

/// `today` is the current day in UTC, `YYYY-MM-DD`.
pub fn compute_stats(entries: &[LogEntry], today: &str, party: &Party) -> Stats {
    // prompts in the order their id first appeared, each with its latest text
    let mut order: Vec<&str> = Vec::new();
    let mut prompts: HashMap<&str, &PromptEntry> = HashMap::new();
    let mut responses: HashMap<&str, &ResponseEntry> = HashMap::new();
    let mut annotated: HashMap<&str, &AnnotationEntry> = HashMap::new();
    let mut stats = Stats::default();
    for entry in entries {
        match entry {
            LogEntry::Prompt(p) => {
                if prompts.insert(&p.id, p).is_none() {
                    order.push(&p.id);
                }
            }
            LogEntry::Response(r) => {
                responses.insert(&r.reference, r);
            }
            // what the author did with a kept chat of another tool is no turn of the editor's: it is counted apart
            LogEntry::Annotation(a) if a.reference.starts_with(crate::types::KEPT) => {}
            LogEntry::Annotation(a) => {
                annotated.insert(&a.reference, a);
            }
            // a refusal changed nothing
            LogEntry::Guard(g) if !g.refused => stats.guard_events += 1,
            _ => {}
        }
    }

    let mut turns: Vec<RecentTurn> = Vec::new();
    for id in order {
        let p = prompts[id];
        if p.category == "meta" {
            continue;
        }
        let Some(r) = responses.get(id) else { continue };
        let a = annotated.get(id);
        let category = a.and_then(|a| a.category.clone()).unwrap_or_else(|| p.category.clone());
        let origin = p.origin.as_deref();
        stats.turns += 1;
        if p.ts.starts_with(today) {
            stats.today += 1;
        }
        *stats.by_category.entry(category.clone()).or_default() += 1;
        // a turn that only launched a reviewer has nothing to classify: its report does
        let is_launch_only = r.agents.as_ref().is_some_and(|agents| !agents.is_empty());
        let is_open = a.is_none() && !is_auto_origin(origin) && !is_launch_only;
        let who = p.agent.clone().or_else(|| r.personas.first().cloned());
        if is_open {
            stats.unannotated += 1;
            if let Some(who) = &who {
                *stats.open_by_persona.entry(who.clone()).or_default() += 1;
            }
        }
        if r.ai_text_flag {
            stats.ai_text += 1;
        }
        for persona in &r.personas {
            // a consult is one the author asked for; a hand-back or a live hint is not
            if origin.is_none() {
                *stats.persona_consults.entry(persona.clone()).or_default() += 1;
            }
            if a.is_some_and(|a| a.use_ == "adopted" || a.use_ == "adapted") {
                *stats.persona_adopted.entry(persona.clone()).or_default() += 1;
            }
        }
        let preview = if origin == Some("agent") {
            let from = party.by_id(p.agent.as_deref().unwrap_or("")).map_or("a subagent", |persona| persona.name.as_str());
            format!("{} from {from}", if p.reply_to.is_some() { "↳ answer" } else { "↩ report" })
        } else {
            slice_utf16(&WHITESPACE.replace_all(&p.text, " "), 60).to_string()
        };
        turns.push(RecentTurn {
            id: id.to_string(),
            ts: p.ts.clone(),
            category,
            preview,
            answer: slice_utf16(&flatten(&r.answer), 240).to_string(),
            persona: who,
            origin: p.origin.clone(),
            reply_to: p.reply_to.clone(),
            is_open,
            personas: r.personas.clone(),
            is_annotated: a.is_some(),
            is_ai_text: r.ai_text_flag,
        });
    }
    stats.annotations = annotated.len();
    stats.rejected = annotated.values().filter(|a| a.use_ == "rejected").count();
    stats.recent = turns.iter().rev().take(6).cloned().collect();
    stats.open = turns.iter().rev().filter(|t| t.is_open).take(5).cloned().collect();
    stats
}
