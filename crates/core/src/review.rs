//! The two model calls the author does not type: the live review of what they just wrote, and
//! the check of their planned points. Each is one call without tools, logged like every AI
//! input (`origin: watch` / `scope`). Follows the plugin's `reviewChanges` and `checkPoints` in
//! `plugin/hooks/register.tsx`.

use crate::backend::{Backend, Block, Message, ModelRequest};
use crate::briefs::parse_briefs;
use crate::scope::{day_number, numbered_text, parse_check, parse_scope, scope_check_system, Checked, PointStatus, ScopePlan};
use crate::turn::{TurnError, Workspace};
use crate::types::{LogEntry, PromptEntry, ResponseEntry};
use crate::watch::{parse_hints, watch_excerpt, watch_system, Pace, KEEP_HINTS, WATCH_DEFAULT};
use crate::words::{changed_paragraphs, Format};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Milliseconds since 1970 of an ISO timestamp (UTC), as the host's clock gives it.
pub fn ms_of(iso: &str) -> u64 {
    let field = |from: usize, to: usize| iso.get(from..to).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let days = day_number(iso).unwrap_or(0).max(0) as u64;
    let millis = if iso.as_bytes().get(19) == Some(&b'.') { field(20, 23) } else { 0 };
    ((days * 24 + field(11, 13)) * 60 + field(14, 16)) * 60_000 + field(17, 19) * 1000 + millis
}

/// One model call without tools; the text of the answer and the model that gave it.
async fn ask_once(backend: &dyn Backend, system: String, prompt: String, model: &str) -> Result<(String, String), TurnError> {
    let request = ModelRequest { system, messages: vec![Message::user(prompt)], tools: Vec::new(), model: model.to_string() };
    let reply = backend.complete(&request, &mut |_| {}).await.map_err(|e| TurnError::Backend(e.message))?;
    let text: String = reply.blocks.iter().filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None }).collect();
    Ok((text, reply.model.unwrap_or_else(|| model.to_string())))
}

/// A call the author did not type, as the log records it.
struct Call<'a> {
    /// 'watch' or 'scope'.
    origin: &'a str,
    category: &'a str,
    /// What was looked at.
    asked: String,
    model: String,
    personas: Vec<String>,
    answer: String,
    /// The document as it was sent.
    document: &'a str,
}

/// Logs a call the author did not type; its id and time.
fn log_call(workspace: &Workspace, backend: &dyn Backend, call: Call) -> Result<(String, String), TurnError> {
    let host = workspace.host.as_ref();
    workspace.sidecar.ensure(host, call.document).map_err(TurnError::Io)?;
    let (id, ts) = (host.new_id(), host.now());
    let prompt = PromptEntry {
        id: id.clone(),
        ts: ts.clone(),
        session: workspace.session.clone(),
        paper: workspace.sidecar.name.clone(),
        category: call.category.to_string(),
        origin: Some(call.origin.to_string()),
        text: call.asked,
        ..Default::default()
    };
    workspace.log.append(&LogEntry::Prompt(prompt)).map_err(TurnError::Io)?;
    host.write(&format!("{}/{id}.md", workspace.sidecar.snapshots_dir()), call.document).map_err(TurnError::Io)?;
    let response = ResponseEntry {
        reference: id.clone(),
        ts: ts.clone(),
        model: Some(call.model),
        editor: Some(workspace.editor.clone()),
        backend: Some(backend.id().to_string()),
        provider: Some(backend.provider()),
        personas: call.personas,
        answer: call.answer,
        ..Default::default()
    };
    workspace.log.append(&LogEntry::Response(response)).map_err(TurnError::Io)?;
    Ok((id, ts))
}

// ------------------------------------------------------------ live review

/// Whether live review is on for a document, and who watches (`live.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSettings {
    #[serde(default)]
    pub is_on: bool,
    #[serde(default)]
    pub personas: Vec<String>,
}

impl LiveSettings {
    /// Off, with the experts that make sense sentence by sentence, as far as they are in the party.
    pub fn load(workspace: &Workspace) -> LiveSettings {
        let party = workspace.party();
        let saved: Option<LiveSettings> = workspace.host.read(&workspace.sidecar.live_json()).ok().and_then(|text| serde_json::from_str(&text).ok());
        let known = |ids: Vec<String>| ids.into_iter().filter(|id| party.by_id(id).is_some()).collect::<Vec<_>>();
        match saved {
            Some(saved) => LiveSettings { is_on: saved.is_on, personas: known(saved.personas) },
            None => LiveSettings { is_on: false, personas: known(WATCH_DEFAULT.iter().filter(|id| party.is_member(id)).map(|id| id.to_string()).collect()) },
        }
    }

    pub fn save(&self, workspace: &Workspace) -> std::io::Result<()> {
        workspace.host.write(&workspace.sidecar.live_json(), &format!("{}\n", serde_json::to_string_pretty(self).expect("plain data")))
    }
}

/// A hint an expert left on a line the author just wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveHint {
    /// The logged review it came from.
    pub turn: String,
    pub persona: String,
    /// The expert's name.
    pub who: String,
    pub line: usize,
    pub text: String,
    pub ts: String,
}

/// What live review remembers while the document is open. Nothing of it is saved.
#[derive(Debug, Clone, Default)]
pub struct LiveState {
    /// The document as it was at the last review, or when watching began.
    pub baseline: Option<String>,
    pub last_review_at: u64,
    pub review_times: Vec<u64>,
    /// Newest first.
    pub hints: Vec<LiveHint>,
    /// The gap between reviews and the hourly limit that are kept.
    pub pace: Pace,
}

/// What a call for a live review came to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LiveOutcome {
    /// Live review is off, or nobody watches.
    Off,
    /// Nothing with prose in it changed since the last review.
    Unchanged,
    /// The last review is too recent; ask again in this many milliseconds.
    Wait {
        ms: u64,
    },
    /// The hourly limit is reached.
    Paused,
    /// The model gave no answer; it will be tried again after the next change.
    Failed {
        message: String,
    },
    Reviewed {
        hints: usize,
        ts: String,
    },
}

/// Reviews what changed in the document since the last review, if it is time. The caller says
/// when the author has paused; this keeps the gap between reviews and the hourly limit.
pub async fn live_review(
    workspace: &Workspace,
    backend: &dyn Backend,
    model: &str,
    settings: &LiveSettings,
    state: &mut LiveState,
    document: &str,
) -> Result<LiveOutcome, TurnError> {
    let party = workspace.party();
    let personas: Vec<_> = settings.personas.iter().filter_map(|id| party.by_id(id)).collect();
    if !settings.is_on || personas.is_empty() {
        return Ok(LiveOutcome::Off);
    }
    let Some(baseline) = state.baseline.clone() else {
        // the baseline: what was there when watching began
        state.baseline = Some(document.to_string());
        return Ok(LiveOutcome::Unchanged);
    };
    let changed = changed_paragraphs(&baseline, document, Format::Markdown);
    if changed.is_empty() {
        return Ok(LiveOutcome::Unchanged);
    }
    let now = ms_of(&workspace.host.now());
    if state.last_review_at > 0 && now.saturating_sub(state.last_review_at) < state.pace.gap_ms {
        return Ok(LiveOutcome::Wait { ms: state.pace.gap_ms - now.saturating_sub(state.last_review_at) });
    }
    state.review_times.retain(|t| now.saturating_sub(*t) < 3_600_000);
    if state.review_times.len() >= state.pace.per_hour {
        return Ok(LiveOutcome::Paused);
    }

    state.baseline = Some(document.to_string());
    state.last_review_at = now;
    state.review_times.push(now);
    let host = workspace.host.as_ref();
    let briefs = parse_briefs(&host.read(&workspace.sidecar.party_md()).unwrap_or_default(), &party);
    let file = workspace.sidecar.name.as_str();
    let watching: Vec<&str> = personas.iter().map(|p| p.id.as_str()).collect();
    let knowledge = crate::project::knowledge_for(host, &workspace.sidecar, Some(&watching));
    let answer = ask_once(backend, watch_system(&personas, workspace.answer_language(), "", &briefs) + &knowledge.section, watch_excerpt(&[(file, &changed)], 4000), model).await;
    let (answer, answered_by) = match answer {
        Ok(answer) => answer,
        Err(error) => return Ok(LiveOutcome::Failed { message: error.to_string() }),
    };
    let ids: Vec<&str> = personas.iter().map(|p| p.id.as_str()).collect();
    let hints = parse_hints(&answer, &ids);

    let asked = format!("Live review ({}) of {file} l.{}–{}", ids.join(", "), changed[0].start, changed[changed.len() - 1].end);
    let logged = if hints.is_empty() { "(no hints)".to_string() } else { hints.iter().map(|h| format!("{} l.{}: {}", h.persona, h.line, h.hint)).collect::<Vec<_>>().join("\n") };
    let mut who: Vec<String> = Vec::new();
    for hint in &hints {
        if !who.contains(&hint.persona) {
            who.push(hint.persona.clone());
        }
    }
    let category = hints.first().and_then(|h| party.by_id(&h.persona)).map_or("feedback", |p| p.category.as_str());
    let (id, ts) = log_call(workspace, backend, Call { origin: "watch", category, asked, model: answered_by, personas: who, answer: logged, document })?;

    let fresh = hints.iter().map(|h| LiveHint {
        turn: id.clone(),
        persona: h.persona.clone(),
        who: party.by_id(&h.persona).map_or(h.persona.clone(), |p| p.name.clone()),
        line: h.line,
        text: h.hint.clone(),
        ts: ts.clone(),
    });
    state.hints = fresh.chain(state.hints.drain(..)).take(KEEP_HINTS).collect();
    Ok(LiveOutcome::Reviewed { hints: hints.len(), ts })
}

// ------------------------------------------------------------ the point check

/// What the last point check found (`check.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CheckFile {
    pub ts: String,
    pub model: String,
    pub points: BTreeMap<String, Checked>,
}

pub fn read_check(workspace: &Workspace) -> Option<CheckFile> {
    serde_json::from_str(&workspace.host.read(&workspace.sidecar.check_json()).ok()?).ok()
}

/// The author's plan, or None when they have not set one.
pub fn read_plan(workspace: &Workspace) -> Option<ScopePlan> {
    let plan = parse_scope(&workspace.host.read(&workspace.sidecar.scope_md()).ok()?);
    (!crate::scope::is_empty_plan(&plan)).then_some(plan)
}

/// How many points a check found covered, partly covered and missing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckSummary {
    pub covered: usize,
    pub partial: usize,
    pub missing: usize,
}

/// Asks which of the planned points the document makes. Logged; the result is kept in `check.json`.
pub async fn check_points(workspace: &Workspace, backend: &dyn Backend, model: &str, document: &str) -> Result<CheckSummary, TurnError> {
    let plan = read_plan(workspace).filter(|plan| !plan.points.is_empty()).ok_or_else(|| TurnError::Invalid("Your plan has no points yet: add them under “## Points”.".into()))?;
    if document.trim().is_empty() {
        return Err(TurnError::Invalid("The document is still empty: nothing to check yet.".into()));
    }
    let list = plan.points.iter().enumerate().map(|(i, p)| format!("{}. {}{}", i + 1, p.text, p.section.as_ref().map(|s| format!(" (planned for: {s})")).unwrap_or_default()));
    let goal = if plan.goal.is_empty() { "(not given)" } else { &plan.goal };
    let prompt = format!(
        "Planned points:\n{}\n\nGoal of the paper: {goal}\n\nManuscript:\n{}",
        list.collect::<Vec<_>>().join("\n"),
        numbered_text(&[(&workspace.sidecar.name, document)], 80_000)
    );
    let (answer, answered_by) = ask_once(backend, scope_check_system(workspace.answer_language(), Some(&workspace.sidecar.name)), prompt, model).await?;
    let results = parse_check(&answer, plan.points.len());

    let asked = format!("Scope check: which of {} planned points the manuscript makes", plan.points.len());
    let line = |r: &crate::scope::CheckResult| {
        let status = serde_json::to_value(r.found.status).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        format!(
            "{}. {status}{}{}",
            r.n,
            r.found.r#where.as_ref().map(|w| format!(" @ {w}")).unwrap_or_default(),
            r.found.note.as_ref().map(|n| format!(": {n}")).unwrap_or_default()
        )
    };
    let logged = if results.is_empty() { "(no result)".to_string() } else { results.iter().map(line).collect::<Vec<_>>().join("\n") };
    let (_, ts) = log_call(workspace, backend, Call { origin: "scope", category: "structure", asked, model: answered_by.clone(), personas: Vec::new(), answer: logged, document })?;

    let points: BTreeMap<String, Checked> = results.iter().map(|r| (plan.points[r.n - 1].text.clone(), r.found.clone())).collect();
    let file = CheckFile { ts, model: answered_by, points };
    workspace.host.write(&workspace.sidecar.check_json(), &format!("{}\n", serde_json::to_string_pretty(&file).expect("plain data"))).map_err(TurnError::Io)?;
    let count = |status: PointStatus| results.iter().filter(|r| r.found.status == status).count();
    let (covered, partial) = (count(PointStatus::Covered), count(PointStatus::Partial));
    Ok(CheckSummary { covered, partial, missing: plan.points.len() - covered - partial })
}
