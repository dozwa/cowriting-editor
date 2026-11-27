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
