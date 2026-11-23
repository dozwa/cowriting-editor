//! Where the AI may write. Inside a layout's root it writes only in its own places; every other
//! file is the author's, and the log may only grow. Ported from the plugin's guard decisions in
//! `plugin/hooks/core.ts` (`authorFileOf`, `isLogPath`, `liesIn`, `changedFiles`), with the folders
//! passed in as a `Layout` instead of fixed.
//!
//! The editor offers the AI no shell and no file tool that takes a manuscript path, so the plugin's
//! shell parsing (`shellWrites`) has no counterpart here.

use std::collections::{BTreeMap, BTreeSet};

/// A document's folders: which are the AI's, where the log is. Folders and files are relative to `root`.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub root: String,
    /// Folders the AI may write in.
    pub ai_folders: Vec<String>,
    /// Single files the AI may write.
    pub ai_files: Vec<String>,
    /// The log's folder: append-only, written by the app alone.
    pub log_folder: String,
}

impl Layout {
    /// A paper folder as the plugin lays it out (its `CLAUDE_WRITES`).
    pub fn plugin(root: &str) -> Layout {
        Layout {
            root: root.to_string(),
            ai_folders: ["notes/ai", "ai-log", "disclosure", "party", ".cowriting", "build"].map(String::from).to_vec(),
            ai_files: vec!["scope.md".into()],
            log_folder: "ai-log".into(),
        }
    }

    /// One markdown file with its sidecar: the root is the file's folder, and the AI's only place
    /// is the sidecar's `notes/`.
    pub fn editor(folder: &str, sidecar_in_folder: &str) -> Layout {
        Layout {
            root: folder.to_string(),
            ai_folders: vec![format!("{sidecar_in_folder}/notes")],
            ai_files: Vec::new(),
            log_folder: format!("{sidecar_in_folder}/ai-log"),
        }
    }
}

/// The path's folders and file after `.` and `..` are resolved.
pub fn segments_of(path: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for segment in path.split(['/', '\\']) {
        match segment {
            ".." => {
                out.pop();
            }
            "." | "" => {}
            other => out.push(other),
        }
    }
    out
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Does this path lie below `root`? `..` is resolved and case ignored.
pub fn lies_in(path: &str, root: &str) -> bool {
    let (base, segments) = (segments_of(root), segments_of(path));
    segments.len() > base.len() && base.iter().zip(&segments).all(|(a, b)| same(a, b))
}

fn is_ai_place(relative: &[&str], layout: &Layout) -> bool {
    // a glob never names one of the AI's places
    if relative.iter().any(|s| s.contains(['*', '?'])) {
        return false;
    }
    if relative.len() == 1 && layout.ai_files.iter().any(|file| same(file, relative[0])) {
        return true;
    }
    layout.ai_folders.iter().any(|folder| {
        let parts = segments_of(folder);
        relative.len() >= parts.len() && parts.iter().zip(relative).all(|(a, b)| same(a, b))
    })
}

/// The author's file this path names, relative to the layout's root; `None` when the path lies
/// outside the root or in one of the AI's places. `..` is resolved and case ignored.
pub fn author_file_of(path: &str, layout: &Layout) -> Option<String> {
    if !lies_in(path, &layout.root) {
        return None;
    }
    let segments = segments_of(path);
    let relative = &segments[segments_of(&layout.root).len()..];
    (!is_ai_place(relative, layout)).then(|| relative.join("/"))
}

/// Does this path name the log's folder, or lie in it?
pub fn is_log_path(path: &str, layout: &Layout) -> bool {
    let log = format!("{}/{}", layout.root, layout.log_folder);
    let (base, segments) = (segments_of(&log), segments_of(path));
    segments.len() >= base.len() && base.iter().zip(&segments).all(|(a, b)| same(a, b))
}

/// What the guard says to a write of the AI's.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// One of the AI's places.
    Allowed,
    /// The log: only the app appends to it.
    Log,
    /// A file of the author's, relative to the root.
    Authors(String),
    /// Not below the root at all. The plugin leaves such a path alone; the editor refuses it.
    Outside,
}

pub fn judge_write(path: &str, layout: &Layout) -> Verdict {
    if is_log_path(path, layout) {
        Verdict::Log
    } else if let Some(file) = author_file_of(path, layout) {
        Verdict::Authors(file)
    } else if lies_in(path, &layout.root) {
        Verdict::Allowed
    } else {
        Verdict::Outside
    }
}

/// Files by relative path → what identifies their state (content hash, or `mtime:size`).
pub type Fingerprint = BTreeMap<String, String>;

/// The files that differ between two fingerprints: changed, added or removed.
pub fn changed_files(before: &Fingerprint, after: &Fingerprint) -> Vec<String> {
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    names.into_iter().filter(|name| before.get(*name) != after.get(*name)).cloned().collect()
}

/// What a guard entry records of a call's input: one line, at most 200 UTF-16 units.
pub fn guard_input(input: &str) -> String {
    crate::text::slice_utf16(&crate::text::squash(input), 200).to_string()
}
