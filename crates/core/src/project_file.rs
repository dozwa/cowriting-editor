//! A project of several markdown files: a thesis with its chapters, a book, a long report. One
//! file beside them, `<name>.cowriting.json`, lists them in their order. Every file keeps its own
//! log, notes and plan as a single document does; the experts, the linked files, the sources and
//! the other settings are the project's and hold in every file (see `Sidecar::settings_dir`).
//!
//! A markdown file that no such file lists is a project of its own, as before.

use crate::host::Host;
use serde::{Deserialize, Serialize};

/// How the name of a project's file ends.
pub const ENDING: &str = ".cowriting.json";

/// `<name>.cowriting.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectFile {
    /// What the editor keeps the project's shared settings under, so that they stay with it when
    /// the project's file is renamed or its folder moved.
    pub id: String,
    /// The project's files in their order, each by its way from the folder of this file, with `/`.
    pub files: Vec<String>,
    /// What the whole project is to say or reach, in the author's words.
    pub goal: String,
    /// How many words the whole project is to have.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<u32>,
    /// Whether the AI, asked in one file, may read the project's other files. Off unless the
    /// author switches it on: asked about a chapter, the AI stays with that chapter.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ai_reads_files: bool,
    /// What a newer version of the editor wrote here and this one does not know: kept as it is.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Whether a path names a project's file and not a document.
pub fn is_project_file(path: &str) -> bool {
    path.to_lowercase().ends_with(ENDING)
}

impl ProjectFile {
    /// The project's file at `path`; `None` when there is none or it cannot be read.
    pub fn load(host: &dyn Host, path: &str) -> Option<ProjectFile> {
        crate::stored::read_as(host, path, &crate::stored::PROJECT_FILE)
    }

    pub fn save(&self, host: &dyn Host, path: &str) -> std::io::Result<()> {
        crate::stored::write(host, path, &crate::stored::PROJECT_FILE, self)
    }

    /// The project's name: its file's name without the ending.
    pub fn name_of(path: &str) -> &str {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        if is_project_file(name) {
            &name[..name.len() - ENDING.len()]
        } else {
            name
        }
    }
}
