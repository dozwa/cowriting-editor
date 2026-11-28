//! A project of several markdown files as the window uses it: finding the project a document
//! belongs to, making one, adding, removing, ordering and renaming its files, and the overview of
//! all of them. The project's file itself is `cowriting_core::project_file`.
//!
//! Nothing here reads or writes a manuscript, with two exceptions the author asks for by name: a
//! file added under a name that does not exist yet is created empty, and a renamed file is moved.

use crate::picture::{joined, relative, same_file};
use cowriting_core::host::Host;
use cowriting_core::project_file::{is_project_file, ProjectFile, ENDING};
use cowriting_core::review::CheckFile;
use cowriting_core::scope::{is_empty_plan, parse_scope, scope_progress, PointStatus};
use cowriting_core::sidecar::Sidecar;
use cowriting_core::stored;
use cowriting_core::words::{count_chars, count_words, Format};
use serde::Serialize;

/// How many folders above a document a project's file is looked for: chapters may lie in folders of their own.
const FOLDERS_UP: usize = 3;

/// What of a single document's settings becomes the project's when the document is its first file.
const SHARED: [&str; 8] = ["project.json", "party.md", "party", "party.json", "prompts", "live.json", "sources.json", "linked"];

fn folder_of(path: &str) -> String {
    Sidecar::for_document(path).folder
}

fn is_markdown(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// A file of the project, by the way the project's file names it.
fn member(project: &str, entry: &str) -> String {
    joined(&folder_of(project), entry)
}

/// The way the project's file names a document by.
fn entry_for(project: &str, document: &str) -> Result<String, String> {
    relative(&folder_of(project), document).ok_or_else(|| format!("{document} is on another drive than the project. A project's files are named by their way from its folder."))
}

fn load(host: &dyn Host, project: &str) -> Result<ProjectFile, String> {
    ProjectFile::load(host, project).ok_or_else(|| format!("{project} could not be read as a project."))
}

fn save(host: &dyn Host, project: &str, file: &ProjectFile) -> Result<(), String> {
    file.save(host, project).map_err(|e| format!("{project} could not be written: {e}"))
}

/// The project that lists this document: looked for in the document's folder and a few above it.
pub fn find(host: &dyn Host, document: &str) -> Option<(String, ProjectFile)> {
    let mut folder = folder_of(document);
    for _ in 0..=FOLDERS_UP {
        let mut names: Vec<String> = host.list(&folder).unwrap_or_default().into_iter().filter(|name| is_project_file(name)).collect();
        names.sort();
        for name in names {
            let path = format!("{}/{name}", folder.trim_end_matches(['/', '\\']));
            if let Some(file) = ProjectFile::load(host, &path).filter(|file| file.files.iter().any(|entry| same_file(&member(&path, entry), document))) {
                return Some((path, file));
            }
        }
        let above = folder.trim_end_matches(['/', '\\']).rfind(['/', '\\'])?;
        if above == 0 {
            return None;
        }
        folder.truncate(above);
    }
    None
}

/// Where the settings are that the project's files share: in the editor's folder for projects
/// under the project's id, or beside the project's file when the editor has no such folder.
pub fn shared_folder(projects: Option<&str>, project: &str, file: &ProjectFile) -> String {
    let id: String = file.id.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).take(60).collect();
    match projects {
        Some(projects) if !id.is_empty() => format!("{}/project-{id}", projects.trim_end_matches(['/', '\\'])),
        Some(projects) => format!("{}/{}", projects.trim_end_matches(['/', '\\']), Sidecar::project_key(project)),
        None => Sidecar::for_document(project).dir(),
    }
}

/// A document's sidecar with its own settings where the editor keeps them.
fn sidecar(projects: Option<&str>, document: &str) -> Sidecar {
    let sidecar = Sidecar::for_document(document);
    match projects {
        Some(projects) => sidecar.with_project(&format!("{}/{}", projects.trim_end_matches(['/', '\\']), Sidecar::project_key(document))),
        None => sidecar,
    }
}

/// What "Open" opens for this path: the document itself, or the first file of a project that is still there.
pub fn document_to_open(host: &dyn Host, path: &str) -> Result<String, String> {
    if !is_project_file(path) {
        // the open dialog lets .json through for the projects' files; no other one is a document
        return if path.to_lowercase().ends_with(".json") {
            Err(format!("{path} is not a project of the editor's (its name would end in {ENDING})."))
        } else {
            Ok(path.to_string())
        };
    }
    let file = load(host, path)?;
    file.files.iter().map(|entry| member(path, entry)).find(|document| host.exists(document)).ok_or_else(|| format!("None of the files {path} lists is there any more."))
}

/// Makes a project at `path` with one document as its first file. That document's experts, linked
/// files, sources and other settings become the project's; its plan, log and notes stay its own.
/// Returns the project's file as it was named.
pub fn create(host: &dyn Host, projects: Option<&str>, path: &str, first: &str) -> Result<String, String> {
    let path = if is_project_file(path) { path.to_string() } else { format!("{}{ENDING}", path.trim_end_matches(".json")) };
    if ProjectFile::name_of(&path).trim().is_empty() {
        return Err("The project needs a name.".into());
    }
    if host.exists(&path) {
        return Err(format!("{path} is there already. Choose another name, or add the document to that project."));
    }
    if let Some((other, _)) = find(host, first) {
        return Err(format!("This document is a file of the project {} already.", ProjectFile::name_of(&other)));
    }
    let file = ProjectFile { id: host.new_id(), files: vec![entry_for(&path, first)?], ..Default::default() };
    save(host, &path, &file)?;
    let (own, shared) = (sidecar(projects, first).own_dir(), shared_folder(projects, &path, &file));
    for name in SHARED {
        let (old, new) = (format!("{own}/{name}"), format!("{shared}/{name}"));
        if host.exists(&old) && !host.exists(&new) {
            host.rename(&old, &new).map_err(|e| format!("The project is made, but {name} of the document's settings could not be moved to it: {e}"))?;
        }
    }
    Ok(path)
}

/// Adds a markdown file to the end of the project. A name that is not there yet becomes an empty file.
pub fn add(host: &dyn Host, project: &str, document: &str) -> Result<(), String> {
    let mut file = load(host, project)?;
    if !is_markdown(document) {
        return Err("A project's files are markdown files: the name ends in .md.".into());
    }
    if file.files.iter().any(|entry| same_file(&member(project, entry), document)) {
        return Err("That file is in the project already.".into());
    }
    if let Some((other, _)) = find(host, document) {
        return Err(format!("That file belongs to the project {}. Remove it there first.", ProjectFile::name_of(&other)));
    }
    let entry = entry_for(project, document)?;
    if !host.exists(document) {
        host.write(document, "").map_err(|e| format!("{document} could not be created: {e}"))?;
    }
    file.files.push(entry);
    save(host, project, &file)
}

/// Takes a file out of the project. The file, its log and its plan stay where they are.
pub fn remove(host: &dyn Host, project: &str, document: &str) -> Result<(), String> {
    let mut file = load(host, project)?;
    let before = file.files.len();
    file.files.retain(|entry| !same_file(&member(project, entry), document));
    if file.files.len() == before {
        return Err("That file is not in the project.".into());
    }
    if file.files.is_empty() {
        return Err("A project keeps at least one file.".into());
    }
    save(host, project, &file)
}

/// Puts the project's files into this order. It must name every file once.
pub fn order(host: &dyn Host, project: &str, documents: &[String]) -> Result<(), String> {
    let mut file = load(host, project)?;
    let mut left = file.files.clone();
    let mut ordered = Vec::new();
    for document in documents {
        let at = left.iter().position(|entry| same_file(&member(project, entry), document)).ok_or("The new order names a file that is not in the project, or one twice.")?;
        ordered.push(left.remove(at));
    }
    if !left.is_empty() {
        return Err("The new order leaves out a file of the project.".into());
    }
    file.files = ordered;
    save(host, project, &file)
}

/// The project names a file by its new place, after the file was moved there.
pub fn replace(host: &dyn Host, project: &str, from: &str, to: &str) -> Result<(), String> {
    let mut file = load(host, project)?;
    let new = entry_for(project, to)?;
    for entry in file.files.iter_mut().filter(|entry| same_file(&member(project, entry), from)) {
        *entry = new.clone();
    }
    save(host, project, &file)
}

/// Gives a file of the project another name in its folder; its log, notes and plan go with it.
/// Not for the document that is open: that one is moved with the text as it is in the window.
pub fn rename(host: &dyn Host, projects: Option<&str>, project: &str, document: &str, name: &str) -> Result<String, String> {
    let file = load(host, project)?;
    if !file.files.iter().any(|entry| same_file(&member(project, entry), document)) {
        return Err("That file is not in the project.".into());
    }
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '\\']) || !is_markdown(name) {
        return Err("The file needs a name that ends in .md, without a folder.".into());
    }
    let target = format!("{}/{name}", folder_of(document).trim_end_matches(['/', '\\']));
    let (from, to) = (sidecar(projects, document), sidecar(projects, &target));
    if host.exists(&target) {
        return Err(format!("{name} is there already."));
    }
    if host.exists(&to.dir()) {
        return Err(format!("{name} has an AI log of its own there. Choose another name, so that neither log is lost."));
    }
    if !host.exists(document) {
        return Err(format!("{document} is not there any more."));
    }
    host.rename(document, &target).map_err(|e| format!("The file could not be renamed: {e}"))?;
    if host.exists(&from.dir()) {
        to.adopt(host, &from).map_err(|e| format!("The file has its new name, but its AI log could not be moved from {}: {e}", from.dir()))?;
    }
    to.take_settings(host, &from);
    replace(host, project, document, &target)?;
    Ok(target)
}

/// Sets what the whole project is to reach: its goal in the author's words, and its words.
pub fn set_goal(host: &dyn Host, project: &str, goal: &str, words: Option<u32>) -> Result<(), String> {
    let mut file = load(host, project)?;
    file.goal = goal.trim().to_string();
    file.words = words.filter(|words| *words > 0);
    save(host, project, &file)
}

/// One file of the project in the overview.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub path: String,
    /// As the project's file names it: its way from the project's folder.
    pub name: String,
    pub words: usize,
    /// The words its own plan sets for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
    /// The points its plan lists, and how many of them are made: ticked by the author or found by the last check.
    pub points: usize,
    pub points_made: usize,
    /// 0–1, as the Plan tab shows it; None without a plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    /// The file is not where the project names it.
    pub is_missing: bool,
    pub is_open: bool,
}

/// The project with all its files: what the Project tab shows.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub path: String,
    pub name: String,
    pub goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words_target: Option<u32>,
    /// The words and characters (with spaces) of all files together.
    pub words: usize,
    pub chars: usize,
    pub files: Vec<FileRow>,
}

/// The overview of a project. `open` is the document in the window with its text as it is there,
/// saved or not; the other files are read from disk.
pub fn overview(host: &dyn Host, projects: Option<&str>, project: &str, open: Option<(&str, &str)>) -> Result<Overview, String> {
    let file = load(host, project)?;
    let today = host.now();
    let (mut words, mut chars) = (0, 0);
    let mut files = Vec::new();
    for entry in &file.files {
        let path = member(project, entry);
        let in_window = open.filter(|(document, _)| same_file(&path, document)).map(|(_, text)| text.to_string());
        let is_open = in_window.is_some();
        let text = in_window.or_else(|| host.read(&path).ok());
        let mut row = FileRow { path: path.clone(), name: entry.clone(), words: 0, target: None, points: 0, points_made: 0, progress: None, is_missing: text.is_none(), is_open };
        if let Some(text) = text {
            row.words = count_words(&text, Format::Markdown);
            words += row.words;
            chars += count_chars(&text, Format::Markdown).with_spaces;
            let sidecar = sidecar(projects, &path);
            let plan = host.read(&sidecar.scope_md()).ok().map(|plan| parse_scope(&plan)).filter(|plan| !is_empty_plan(plan));
            if let Some(plan) = plan {
                let check: Option<CheckFile> = stored::read_as(host, &sidecar.check_json(), &stored::CHECK);
                let (points, at) = check.map(|check| (check.points, Some(check.ts))).unwrap_or_default();
                let progress = scope_progress(&plan, &text, &points, at.as_deref(), today.get(..10).unwrap_or(&today));
                row.target = progress.words.target;
                row.points = progress.points.len();
                row.points_made = progress.points.iter().filter(|point| point.is_ticked || point.status == Some(PointStatus::Covered)).count();
                row.progress = Some(progress.overall);
            }
        }
        files.push(row);
    }
    Ok(Overview { path: project.to_string(), name: ProjectFile::name_of(project).to_string(), goal: file.goal, words_target: file.words, words, chars, files })
}
