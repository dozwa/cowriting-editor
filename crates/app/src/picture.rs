//! A picture of the author's, written into the document as markdown: where it lies as seen from
//! the document, and a copy of it beside the document when the author wants one. The picture
//! itself is only read, never moved or changed.

use serde::Serialize;
use std::path::Path;

/// What the export can put into a PDF or a Word file.
pub const KINDS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "svg", "webp", "pdf"];
/// The folder beside the document that copies go into.
pub const FOLDER: &str = "images";

/// A picture as the document would name it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Picture {
    /// The path to write between the brackets of `![description](…)`.
    pub path: String,
    /// The file's name, to show.
    pub name: String,
    /// It lies in the document's folder or below it, so it moves along with that folder.
    pub is_inside: bool,
}

fn parts(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|part| !part.is_empty() && *part != ".").collect()
}

/// A path that starts with a drive letter, as Windows has them: there, names are the same whatever their case.
fn has_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// The way from a folder to a file, with `/` between its parts on every system (markdown and pandoc
/// read that everywhere). None when there is no way: another drive.
pub fn relative(folder: &str, file: &str) -> Option<String> {
    let (from, to) = (parts(folder), parts(file));
    let is_windows = has_drive(folder) || has_drive(file);
    let same = |a: &str, b: &str| if is_windows { a.eq_ignore_ascii_case(b) } else { a == b };
    if is_windows && (has_drive(folder) != has_drive(file) || !same(from.first()?, to.first()?)) {
        return None;
    }
    let shared = from.iter().zip(&to).take_while(|(a, b)| same(a, b)).count().min(to.len().saturating_sub(1));
    let mut way: Vec<&str> = vec![".."; from.len() - shared];
    way.extend(&to[shared..]);
    Some(way.join("/"))
}

/// A path as markdown takes it: in angle brackets when it holds a space or a bracket, which would end it otherwise.
pub fn written(path: &str) -> String {
    if path.contains([' ', '(', ')', '<', '>']) {
        format!("<{}>", path.replace('<', "%3C").replace('>', "%3E"))
    } else {
        path.to_string()
    }
}

/// How the document in `folder` names the picture at `file`.
pub fn seen_from(folder: &str, file: &str) -> Picture {
    let name = parts(file).last().copied().unwrap_or(file).to_string();
    match relative(folder, file) {
        Some(way) => Picture { is_inside: !way.starts_with("../"), path: written(&way), name },
        // no way from here to there: the full path, which holds on this computer only
        None => Picture { path: written(&file.replace('\\', "/")), name, is_inside: false },
    }
}

/// Whether the file is a picture the export can use, going by its name.
pub fn check(file: &str) -> Result<(), String> {
    let kind = Path::new(file).extension().and_then(|kind| kind.to_str()).unwrap_or("").to_ascii_lowercase();
    if !KINDS.contains(&kind.as_str()) {
        return Err(format!("{file} is not a picture the export can use. It takes PNG, JPEG, GIF, SVG, WebP and PDF."));
    }
    if !Path::new(file).is_file() {
        return Err(format!("{file} is not there."));
    }
    Ok(())
}

/// Copies the picture into `images/` beside the document and returns it as the document names it then.
/// A file of that name with other content is kept: the copy gets a number. The same picture is not copied twice.
pub fn copy_beside(folder: &str, file: &str) -> Result<Picture, String> {
    check(file)?;
    let content = std::fs::read(file).map_err(|e| format!("{file} could not be read: {e}"))?;
    let source = Path::new(file);
    let (stem, kind) = (source.file_stem().and_then(|s| s.to_str()).unwrap_or("picture"), source.extension().and_then(|s| s.to_str()).unwrap_or("png"));
    let target_folder = Path::new(folder).join(FOLDER);
    std::fs::create_dir_all(&target_folder).map_err(|e| format!("The folder {FOLDER} could not be made beside the document: {e}"))?;
    for n in 1..1000 {
        let name = if n == 1 { format!("{stem}.{kind}") } else { format!("{stem}-{n}.{kind}") };
        let target = target_folder.join(&name);
        match std::fs::read(&target) {
            Ok(there) if there != content => continue,
            Ok(_) => {}
            Err(_) => std::fs::write(&target, &content).map_err(|e| format!("The picture could not be copied to {}: {e}", target.display()))?,
        }
        return Ok(Picture { path: written(&format!("{FOLDER}/{name}")), name, is_inside: true });
    }
    Err(format!("There are too many pictures named {stem} in {FOLDER}."))
}

// ------------------------------------------------------------ when the document moves

/// A picture the text names, `![description](way)`: where the way is written, and the way itself.
#[derive(Debug, Clone, PartialEq)]
struct Named {
    /// The bytes of the text that hold the way, with its angle brackets if it has them.
    at: std::ops::Range<usize>,
    way: String,
}

/// Whether a way leads from the document's folder: not a full path, and not an address (`https:`, `data:`).
fn is_from_the_document(way: &str) -> bool {
    let has_scheme = way.split_once(':').is_some_and(|(scheme, _)| !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')));
    !(way.is_empty() || way.starts_with(['/', '\\', '#']) || has_scheme)
}

/// The pictures a markdown text names by a way from its own folder, in the order they are written.
fn named(text: &str) -> Vec<Named> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find("![").map(|at| at + from) {
        from = start + 2;
        let Some(open) = text[from..].find("](").map(|at| at + from + 2) else { break };
        // the description is on one line and holds no bracket of its own
        if text[from..open - 2].contains(['\n', '[', ']']) {
            continue;
        }
        let rest = &text[open..];
        let length = if rest.starts_with('<') { rest.find('>').map(|end| end + 1) } else { rest.find([')', ' ', '\n']) };
        let Some(length) = length.filter(|length| *length > 0) else { continue };
        let written = &rest[..length];
        let way = written.strip_prefix('<').and_then(|w| w.strip_suffix('>')).map(|w| w.replace("%3C", "<").replace("%3E", ">")).unwrap_or_else(|| written.to_string());
        if is_from_the_document(&way) {
            found.push(Named { at: open..open + length, way });
        }
        from = open + length;
    }
    found
}

/// A way from a folder, as a path of its own: `..` goes up, `/` is between the parts.
fn joined(folder: &str, way: &str) -> String {
    let mut all: Vec<&str> = parts(folder);
    for part in parts(way) {
        if part == ".." {
            all.pop();
        } else {
            all.push(part);
        }
    }
    let lead = if folder.starts_with('/') {
        "/"
    } else if folder.starts_with("\\\\") {
        "//"
    } else {
        ""
    };
    format!("{lead}{}", all.join("/"))
}

/// What a document's move does with the pictures it names by a way from its folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    /// Pictures in the document's folder or below it are copied to the same place beside the new
    /// file; the others are named from the new place.
    Copy,
    /// Every picture stays where it is and is named from the new place.
    Leave,
}

/// The pictures a move would leave behind: those the text names from its folder, and that are there.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ToMove {
    /// In the document's folder or below it, by the way the text names them.
    pub inside: Vec<String>,
    /// Elsewhere (the way starts with `..`).
    pub outside: Vec<String>,
}

fn same_folder(a: &str, b: &str) -> bool {
    relative(a, &format!("{}/x", b.trim_end_matches(['/', '\\']))).as_deref() == Some("x")
}

/// The pictures of `text` that would no longer be found when the document goes from `from` to the folder `to`.
pub fn to_move(text: &str, from: &str, to: &str) -> ToMove {
    let mut found = ToMove::default();
    if same_folder(from, to) {
        return found;
    }
    for picture in named(text) {
        if !Path::new(&joined(from, &picture.way)).is_file() {
            continue;
        }
        let list = if picture.way.starts_with("../") { &mut found.outside } else { &mut found.inside };
        if !list.contains(&picture.way) {
            list.push(picture.way);
        }
    }
    found
}

/// The text for the document's new folder, and the copies to make for it (from, to). Nothing is
/// written here. A way that leads to no file is left as it is: it was broken before.
pub fn carried(text: &str, from: &str, to: &str, along: Along) -> (String, Vec<(String, String)>) {
    if same_folder(from, to) {
        return (text.to_string(), Vec::new());
    }
    let (mut moved, mut copies, mut done) = (String::with_capacity(text.len()), Vec::new(), 0);
    for picture in named(text) {
        let file = joined(from, &picture.way);
        if !Path::new(&file).is_file() {
            continue;
        }
        let copy = joined(to, &picture.way);
        // a copy goes where the way leads from the new folder; a different file that is there already is not replaced
        let is_copied = along == Along::Copy && !picture.way.starts_with("../") && std::fs::read(&copy).map_or(true, |there| std::fs::read(&file).is_ok_and(|here| here == there));
        if is_copied {
            if !copies.iter().any(|(_, to)| *to == copy) {
                copies.push((file, copy));
            }
            continue;
        }
        moved.push_str(&text[done..picture.at.start]);
        moved.push_str(&seen_from(to, &file).path);
        done = picture.at.end;
    }
    moved.push_str(&text[done..]);
    (moved, copies)
}

/// Makes the copies `carried` named. The pictures themselves stay where they are.
pub fn copy_all(copies: &[(String, String)]) -> Result<(), String> {
    for (from, to) in copies {
        if Path::new(to).is_file() {
            continue;
        }
        if let Some(folder) = Path::new(to).parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("The folder for {to} could not be made: {e}"))?;
        }
        std::fs::copy(from, to).map_err(|e| format!("The picture {from} could not be copied to {to}: {e}"))?;
    }
    Ok(())
}
