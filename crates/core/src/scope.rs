//! Scope and progress: what the author plans (`scope.md`: goal, targets, sections, points) and
//! how far the text has come. Ported from the plugin's `plugin/hooks/register.tsx` (`parseScope`,
//! `isEmptyPlan`, `scopeProgress`, `numberedText`, `scopeCheckSystem`, `parseCheck`), for
//! markdown only: the editor has no LaTeX documents.

use crate::disclosure::Lang;
use crate::text::slice_utf16;
use crate::words::{count_words, Format};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

/// Words on a page when the author gives pages, not words.
pub const WORDS_PER_PAGE: f64 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Outline,
    Draft,
    Revised,
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointStatus {
    Covered,
    Partial,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanSection {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<Stage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPoint {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    pub is_ticked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct ScopePlan {
    pub goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub figures: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tables: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    pub sections: Vec<PlanSection>,
    pub points: Vec<PlanPoint>,
}

const STAGES: [(&str, Stage); 12] = [
    ("outline", Stage::Outline),
    ("gliederung", Stage::Outline),
    ("skizze", Stage::Outline),
    ("draft", Stage::Draft),
    ("entwurf", Stage::Draft),
    ("rohfassung", Stage::Draft),
    ("revised", Stage::Revised),
    ("überarbeitet", Stage::Revised),
    ("ueberarbeitet", Stage::Revised),
    ("final", Stage::Final),
    ("fertig", Stage::Final),
    ("done", Stage::Final),
];

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
static SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^## +(.+)$").unwrap());
static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[-*+]\s+").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d[\d.,' ]*").unwrap());
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap());
static TICK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[([ xX✓])\]\s*").unwrap());
static TAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\((?:→\s*|in\s+)?([^()]+)\)\s*$").unwrap());
static STAGE_WORDS: LazyLock<Vec<(Regex, Stage)>> =
    LazyLock::new(|| STAGES.iter().map(|(word, stage)| (Regex::new(&format!(r"(^|[^\p{{L}}]){word}([^\p{{L}}]|$)")).unwrap(), *stage)).collect());

/// The first number in a text, with thousands separators and a decimal comma understood; only a positive one counts.
fn number(text: &str) -> Option<f64> {
    let found: Vec<char> = NUMBER.find(text)?.as_str().chars().collect();
    let mut plain = String::new();
    for (i, c) in found.iter().enumerate() {
        // a separator before exactly three digits groups thousands
        let groups = matches!(c, '.' | ',' | '\'' | ' ')
            && found.len() >= i + 4
            && found[i + 1..i + 4].iter().all(char::is_ascii_digit)
            && !found.get(i + 4).is_some_and(char::is_ascii_digit);
        if !groups {
            plain.push(*c);
        }
    }
    let n: f64 = plain.replacen(',', ".", 1).trim().parse().ok()?;
    (n.is_finite() && n > 0.0).then_some(n)
}

fn section_kind(heading: &str) -> Option<&'static str> {
    let lower = heading.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["goal", "ziel"]) {
        Some("goal")
    } else if has(&["target", "umfang", "vorgabe"]) {
        Some("targets")
    } else if has(&["section", "kapitel", "abschnitt", "gliederung", "outline"]) {
        Some("sections")
    } else if has(&["point", "kernaussage", "aussage", "punkt"]) {
        Some("points")
    } else {
        None
    }
}

/// scope.md as the author wrote it; comments and placeholders ignored.
pub fn parse_scope(text: &str) -> ScopePlan {
    let mut plan = ScopePlan::default();
    let (mut pages, mut per_page) = (None, WORDS_PER_PAGE);
    let text = COMMENT.replace_all(text, "");
    let headings: Vec<regex::Captures> = SECTION.captures_iter(&text).collect();
    for (i, heading) in headings.iter().enumerate() {
        let kind = section_kind(&heading[1]);
        let end = headings.get(i + 1).map_or(text.len(), |next| next.get(0).unwrap().start());
        let body = text[heading.get(0).unwrap().end()..end].trim();
        let items: Vec<String> = body.split('\n').map(str::trim).filter(|l| BULLET.is_match(l)).map(|l| BULLET.replace(l, "").into_owned()).collect();
        let key_and_rest = |item: &str| -> (String, String) {
            let (key, rest) = item.split_once(':').unwrap_or((item, ""));
            (key.to_string(), rest.to_string())
        };
        match kind {
            Some("goal") => plan.goal = body.to_string(),
            Some("targets") => {
                for item in &items {
                    let (key, rest) = key_and_rest(item);
                    let value = rest.trim();
                    if value.is_empty() {
                        continue;
                    }
                    match key.trim().to_lowercase().as_str() {
                        "words" | "wörter" | "woerter" => plan.words = number(value),
                        "pages" | "seiten" => pages = number(value),
                        "words per page" | "wörter pro seite" => per_page = number(value).unwrap_or(per_page),
                        "figures" | "abbildungen" => plan.figures = number(value),
                        "tables" | "tabellen" => plan.tables = number(value),
                        "references" | "quellen" | "literatur" => plan.references = number(value),
                        "deadline" | "abgabe" | "frist" => {
                            if let Some(date) = DATE.find(value) {
                                plan.deadline = Some(date.as_str().to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("sections") => {
                for item in &items {
                    let (name, rest) = key_and_rest(item);
                    let detail = rest.to_lowercase();
                    let stage = STAGE_WORDS.iter().find(|(word, _)| word.is_match(&detail)).map(|(_, stage)| *stage);
                    if !name.trim().is_empty() {
                        plan.sections.push(PlanSection { name: name.trim().to_string(), target: number(&detail), stage });
                    }
                }
            }
            Some("points") => {
                for item in &items {
                    let tick = TICK.captures(item);
                    let mut text = tick.as_ref().map_or(item.as_str(), |t| &item[t[0].len()..]);
                    let mut section = None;
                    if let Some(tail) = TAIL.captures(text) {
                        let named = tail[1].trim();
                        if plan.sections.iter().any(|s| s.name.to_lowercase() == named.to_lowercase()) {
                            section = Some(named.to_string());
                            text = &text[..tail.get(0).unwrap().start()];
                        }
                    }
                    if !text.trim().is_empty() {
                        plan.points.push(PlanPoint { text: text.trim().to_string(), section, is_ticked: tick.is_some_and(|t| &t[1] != " ") });
                    }
                }
            }
            _ => {}
        }
    }
    if plan.words.is_none() {
        if let Some(pages) = pages {
            plan.words = Some((pages * per_page).round());
        }
    }
    if plan.words.is_none() && plan.sections.iter().any(|s| s.target.is_some()) {
        plan.words = Some(plan.sections.iter().fold(0.0, |sum, s| sum + s.target.unwrap_or(0.0)));
    }
    plan
}

pub fn is_empty_plan(plan: &ScopePlan) -> bool {
    plan.goal.is_empty() && plan.points.is_empty() && plan.sections.is_empty() && plan.words.is_none()
}

// ------------------------------------------------------------ the text

static MARKERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u:\b)TODO(?-u:\b)|(?-u:\b)FIXME(?-u:\b)|(?-u:\b)XXX(?-u:\b)|\\todo(?-u:\b)|\?\?|\[citation needed\]|(?-u:\b)UNVERIFIED(?-u:\b)").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,2}\s+(.+?)\s*#*$").unwrap());
static FIGURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap());
static TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$").unwrap());
static LATEX_CITE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\[A-Za-z]*cite[a-z]*\*?(?:\[[^\]]*\]){0,2}\{([^}]*)\}").unwrap());
static CITEKEY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?-u)(?:^|[\s\[;-])@([\w:.-]*\w)").unwrap());

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Count {
    pub done: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
}

/// What a check found for one point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checked {
    pub status: PointStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#where: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A point the author plans to make, with what the last check found.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopePoint {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Ticked by the author in scope.md: done, whatever a check says.
    pub is_ticked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PointStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#where: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSection {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<Stage>,
    pub words: usize,
    /// TODO, FIXME, ??, UNVERIFIED … left in it.
    pub markers: usize,
    /// A heading of that name exists in the document.
    pub is_found: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeProgress {
    pub goal: String,
    pub words: Count,
    pub figures: Count,
    pub tables: Count,
    pub references: Count,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_left: Option<i64>,
    /// Words a day still needed to reach the target by the deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words_per_day: Option<f64>,
    pub points: Vec<ScopePoint>,
    /// When the points were last checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
    pub sections: Vec<ScopeSection>,
    pub markers: usize,
    /// 0–1: points made and quantities reached.
    pub content: f64,
    /// 0–1: an estimate from the sections' stages and open markers; None: no stages set.
    pub finish: Option<f64>,
    /// 0–1: content, and finish where known.
    pub overall: f64,
}

/// The text cut at its top headings (# and ##): heading, then what is under it.
fn chunks(text: &str) -> Vec<(String, String)> {
    let mut out = vec![(String::new(), String::new())];
    for line in text.split('\n') {
        match HEADING.captures(line) {
            Some(heading) => out.push((heading[1].trim().to_string(), String::new())),
            None => {
                let current = &mut out.last_mut().unwrap().1;
                current.push_str(line);
                current.push('\n');
            }
        }
    }
    out
}

fn citekeys(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for found in LATEX_CITE.captures_iter(text) {
        keys.extend(found[1].split(',').map(str::trim).filter(|k| !k.is_empty()).map(String::from));
    }
    keys.extend(CITEKEY.captures_iter(text).map(|found| found[1].to_string()));
    keys
}

fn names_match(heading: &str, name: &str) -> bool {
    let (lower, n) = (heading.to_lowercase(), name.to_lowercase());
    !lower.is_empty() && (lower == n || lower.contains(&n) || n.contains(&lower))
}

/// Days since 1970-01-01 of an ISO date (y-m-d).
pub fn day_number(date: &str) -> Option<i64> {
    let mut parts = date.get(..10)?.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    Some(era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468)
}

fn stage_value(stage: Stage) -> f64 {
    match stage {
        Stage::Outline => 0.0,
        Stage::Draft => 0.4,
        Stage::Revised => 0.75,
        Stage::Final => 1.0,
    }
}

/// Each open marker in a section takes this much off its finish.
const MARKER_COST: f64 = 0.05;

/// How far the text has come against the plan; points as the author ticked them or the last check found.
pub fn scope_progress(plan: &ScopePlan, text: &str, checked: &BTreeMap<String, Checked>, checked_at: Option<&str>, today: &str) -> ScopeProgress {
    let parts = chunks(text);
    let count = |text: &str| count_words(text, Format::Markdown);
    let words: usize = parts.iter().map(|(_, text)| count(text)).sum();
    let sections: Vec<ScopeSection> = plan
        .sections
        .iter()
        .map(|s| {
            let own: Vec<&(String, String)> = parts.iter().filter(|(heading, _)| names_match(heading, &s.name)).collect();
            ScopeSection {
                name: s.name.clone(),
                target: s.target,
                stage: s.stage,
                words: own.iter().map(|(_, text)| count(text)).sum(),
                markers: own.iter().map(|(_, text)| MARKERS.find_iter(text).count()).sum(),
                is_found: !own.is_empty(),
            }
        })
        .collect();
    let points: Vec<ScopePoint> = plan
        .points
        .iter()
        .map(|p| {
            let found = checked.get(&p.text);
            ScopePoint {
                text: p.text.clone(),
                section: p.section.clone(),
                is_ticked: p.is_ticked,
                status: found.map(|c| c.status),
                r#where: found.and_then(|c| c.r#where.clone()),
                note: found.and_then(|c| c.note.clone()),
            }
        })
        .collect();
    let counts = [
        Count { done: words, target: plan.words },
        Count { done: FIGURE.find_iter(text).count(), target: plan.figures },
        Count { done: TABLE.find_iter(text).count(), target: plan.tables },
        Count { done: citekeys(text).len(), target: plan.references },
    ];
    let ratio = |c: &Count| c.target.map(|target| (c.done as f64 / target).min(1.0));

    // content: the points count double, each quantity once (figures, tables, references half)
    let made = (!points.is_empty()).then(|| {
        points.iter().fold(0.0, |sum, p| {
            sum + if p.is_ticked || p.status == Some(PointStatus::Covered) {
                1.0
            } else if p.status == Some(PointStatus::Partial) {
                0.5
            } else {
                0.0
            }
        }) / points.len() as f64
    });
    let weighted = [(made, 2.0), (ratio(&counts[0]), 1.0), (ratio(&counts[1]), 0.5), (ratio(&counts[2]), 0.5), (ratio(&counts[3]), 0.5)];
    let known: Vec<(f64, f64)> = weighted.iter().filter_map(|(value, weight)| value.map(|v| (v, *weight))).collect();
    let content = if known.is_empty() { 0.0 } else { known.iter().fold(0.0, |s, (v, w)| s + v * w) / known.iter().fold(0.0, |s, (_, w)| s + w) };

    // finish: the sections' stages, weighted by their targets, less the open markers
    let staged: Vec<&ScopeSection> = sections.iter().filter(|s| s.stage.is_some()).collect();
    let finish = (!staged.is_empty()).then(|| {
        staged.iter().fold(0.0, |sum, s| sum + (stage_value(s.stage.unwrap()) - s.markers as f64 * MARKER_COST).max(0.0) * s.target.unwrap_or(1.0))
            / staged.iter().fold(0.0, |sum, s| sum + s.target.unwrap_or(1.0))
    });

    let days_left = plan.deadline.as_deref().and_then(|deadline| Some(day_number(deadline)? - day_number(today)?));
    let words_per_day = match (plan.words, days_left) {
        (Some(target), Some(days)) if days > 0 => Some(((target - words as f64) / days as f64).ceil().max(0.0)),
        _ => None,
    };
    let [words, figures, tables, references] = counts;
    ScopeProgress {
        goal: plan.goal.clone(),
        words,
        figures,
        tables,
        references,
        deadline: plan.deadline.clone(),
        days_left,
        words_per_day,
        points,
        checked_at: checked_at.map(String::from),
        sections,
        markers: MARKERS.find_iter(text).count(),
        content,
        finish,
        overall: finish.map_or(content, |finish| 0.7 * content + 0.3 * finish),
    }
}

// ------------------------------------------------------------ the check

/// The document's lines with their numbers, cut at `max_chars`.
pub fn numbered_text(files: &[(&str, &str)], max_chars: usize) -> String {
    let len = |s: &str| s.encode_utf16().count();
    let mut out = String::new();
    for (file, text) in files {
        let block = format!("### {file}\n{}\n\n", text.split('\n').enumerate().map(|(i, line)| format!("{}: {line}", i + 1)).collect::<Vec<_>>().join("\n"));
        if len(&out) + len(&block) > max_chars {
            return format!("{out}{}\n[… cut]", slice_utf16(&block, max_chars.saturating_sub(len(&out))));
        }
        out.push_str(&block);
    }
    out
}

/// What the point check is told. `language` as in `watch_system`: None asks for the manuscript's.
/// `file` is the document's name for the example of a place; the plugin's example is a LaTeX file.
pub fn scope_check_system(language: Option<Lang>, file: Option<&str>) -> String {
    let example = format!("Answer with a JSON array only: [{{\"n\": 1, \"status\": \"partial\", \"where\": \"{}:42\", \"note\": \"…\"}}].", file.unwrap_or("manuscript/main.tex"));
    let language = match language {
        Some(Lang::De) => "German",
        Some(Lang::En) => "English",
        None => "the language of the manuscript",
    };
    [
        "You check which of an author's planned points their manuscript already makes. You never write, rephrase or suggest text:",
        "the author is the sole author (DFG good scientific practice). You only assess.",
        "For each numbered point answer: status \"covered\" (made and supported), \"partial\" (mentioned but not developed or not",
        "supported) or \"missing\"; where = \"file:line\" of the best evidence (omit if missing); note = at most 15 words on what is",
        "there or what is lacking, never wording for it.",
        &format!("Write the notes in {language}."),
        &example,
    ]
    .join(" ")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckResult {
    pub n: usize,
    #[serde(flatten)]
    pub found: Checked,
}

static JSON_ARRAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)\[.*\]").unwrap());

/// The results in a point check's answer, for points 1 to `count`. An answer that is no list of results gives none.
pub fn parse_check(text: &str, count: usize) -> Vec<CheckResult> {
    let Some(json) = JSON_ARRAY.find(text) else { return Vec::new() };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json.as_str()) else { return Vec::new() };
    // the plugin gives up on the whole answer when an item is null
    if items.iter().any(Value::is_null) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for item in &items {
        let n = match item.get("n") {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
            Some(Value::String(s)) if s.trim().is_empty() => 0.0,
            Some(Value::String(s)) => s.trim().parse().unwrap_or(f64::NAN),
            Some(Value::Bool(b)) => f64::from(u8::from(*b)),
            Some(Value::Null) => 0.0,
            _ => f64::NAN,
        };
        if !n.is_finite() || n.fract() != 0.0 || n < 1.0 || n > count as f64 {
            continue;
        }
        let status = match item.get("status").and_then(Value::as_str) {
            Some("covered") => PointStatus::Covered,
            Some("partial") => PointStatus::Partial,
            Some("missing") => PointStatus::Missing,
            _ => continue,
        };
        let text_of = |key: &str| item.get(key).and_then(Value::as_str).filter(|s| !s.is_empty());
        out.push(CheckResult {
            n: n as usize,
            found: Checked { status, r#where: text_of("where").map(String::from), note: text_of("note").map(|note| slice_utf16(note, 160).to_string()) },
        });
    }
    out
}
