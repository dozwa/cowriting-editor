//! Where a document's co-writing files live: for `/dir/thesis.md`, the folder `/dir/.cowriting/thesis.md/`.
//! Nothing is written into the document itself.

use crate::guard::Layout;
use crate::host::Host;
use serde::{Deserialize, Serialize};
use std::io;

/// The folder beside a document that holds every document's sidecar.
pub const SIDECAR_FOLDER: &str = ".cowriting";

#[derive(Debug, Clone, PartialEq)]
pub struct Sidecar {
    /// The document, as the author opened it.
    pub document: String,
    /// The folder the document lies in.
    pub folder: String,
    /// The document's file name.
    pub name: String,
    /// Where the project's settings are kept, when that is not beside the document: a folder of the editor's own.
    pub project: Option<String>,
    /// For a document that is one file of a project of several: the folder of the settings all
    /// its files share, and the project's file that lists them.
    pub shared: Option<Shared>,
}

/// What a file of a project of several files knows about that project.
#[derive(Debug, Clone, PartialEq)]
pub struct Shared {
    /// Where the experts, the linked files, the sources and the other shared settings are kept.
    pub folder: String,
    /// The project's file (`*.cowriting.json`).
    pub file: String,
}

/// What the sidecar remembers about its document: `doc.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocInfo {
    pub id: String,
    /// The file name when the sidecar was last used: how a renamed document is recognised.
    pub file: String,
    pub created_at: String,
    /// The document's content hash at its last save (see `content_hash`).
    pub last_hash: String,
    /// The language the document is written in: 'en' or 'de'.
    #[serde(default = "english")]
    pub language: String,
    /// What a newer version of the editor wrote here and this one does not know: kept as it is.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn english() -> String {
    "en".into()
}

/// A short fingerprint of a text (FNV-1a, 64 bit): enough to tell one document from another, not a signature.
pub fn content_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

impl Sidecar {
    pub fn for_document(path: &str) -> Sidecar {
        let cut = path.rfind(['/', '\\']);
        let (folder, name) = match cut {
            Some(at) => (&path[..at], &path[at + 1..]),
            None => (".", path),
        };
        Sidecar { document: path.to_string(), folder: if folder.is_empty() { "/".into() } else { folder.to_string() }, name: name.to_string(), project: None, shared: None }
    }

    /// The same document with its project's settings in `folder`, away from the document.
    pub fn with_project(mut self, folder: &str) -> Sidecar {
        self.project = Some(folder.trim_end_matches(['/', '\\']).to_string());
        self
    }

    /// The same document as one file of a project of several: what the project's files share is in `folder`.
    pub fn with_shared(mut self, folder: &str, file: &str) -> Sidecar {
        self.shared = Some(Shared { folder: folder.trim_end_matches(['/', '\\']).to_string(), file: file.to_string() });
        self
    }

    /// A folder name for a document's project: its file name, and a fingerprint of where it lies.
    pub fn project_key(document: &str) -> String {
        let name: String =
            document.rsplit(['/', '\\']).next().unwrap_or("document").chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).take(60).collect();
        format!("{name}-{}", content_hash(document))
    }

    /// Where the project's settings are: the experts and what they were told, the sources, live
    /// review, linked files. For a file of a project of several files that is the folder they
    /// share; else the document's own (`own_dir`).
    pub fn settings_dir(&self) -> String {
        self.shared.as_ref().map_or_else(|| self.own_dir(), |shared| shared.folder.clone())
    }

    /// Where this one document's settings are, which it shares with no other file: its plan. In
    /// the editor's own folder when one was given, else with the log.
    pub fn own_dir(&self) -> String {
        self.project.clone().unwrap_or_else(|| self.dir())
    }

    /// Linked files, and what this project sets differently from the general settings.
    pub fn project_json(&self) -> String {
        format!("{}/project.json", self.settings_dir())
    }

    /// The author's own wording of the experts that come with the editor, one file each.
    pub fn prompts_dir(&self) -> String {
        format!("{}/prompts", self.settings_dir())
    }

    /// Settings that an earlier version kept beside the document move to the project's folder, once.
    /// Nothing that is already there is replaced.
    pub fn bring_settings(&self, host: &dyn Host) {
        if self.project.is_none() {
            return;
        }
        for name in ["party.md", "party", "party.json", "scope.md", "live.json", "sources.json"] {
            let (old, new) = (format!("{}/{name}", self.dir()), format!("{}/{name}", self.own_dir()));
            if host.exists(&old) && !host.exists(&new) {
                let _ = host.rename(&old, &new);
            }
        }
    }

    /// The settings of a project that was at `from` come along to this one, when the document was moved or renamed.
    pub fn take_settings(&self, host: &dyn Host, from: &Sidecar) {
        if let (Some(old), Some(new)) = (&from.project, &self.project) {
            if old != new && host.exists(old) && !host.exists(new) {
                let _ = host.rename(old, new);
            }
        }
    }

    /// The sidecar's folder, relative to the document's folder.
    pub fn relative(&self) -> String {
        format!("{SIDECAR_FOLDER}/{}", self.name)
    }

    pub fn dir(&self) -> String {
        format!("{}/{}", self.folder.trim_end_matches(['/', '\\']), self.relative())
    }

    pub fn log_dir(&self) -> String {
        format!("{}/ai-log", self.dir())
    }

    /// The only place the AI writes.
    pub fn notes_dir(&self) -> String {
        format!("{}/notes", self.dir())
    }

    /// The document as it was when each question was asked, by prompt id.
    pub fn snapshots_dir(&self) -> String {
        format!("{}/snapshots", self.dir())
    }

    pub fn disclosure_dir(&self) -> String {
        format!("{}/disclosure", self.dir())
    }

    /// What the author kept with statements of AI use outside the editor: chats and other files.
    /// Neither the AI nor a turn writes here.
    pub fn external_dir(&self) -> String {
        format!("{}/external", self.dir())
    }

    pub fn rewrites_dir(&self) -> String {
        format!("{}/log-rewrites", self.dir())
    }

    /// The author's briefing of the experts.
    pub fn party_md(&self) -> String {
        format!("{}/party.md", self.settings_dir())
    }

    /// The document's own experts, one file each.
    pub fn party_dir(&self) -> String {
        format!("{}/party", self.settings_dir())
    }

    /// Which experts are in the document's party.
    pub fn party_json(&self) -> String {
        format!("{}/party.json", self.settings_dir())
    }

    /// The author's plan: goal, targets, sections, points. Every file of a project has its own.
    pub fn scope_md(&self) -> String {
        format!("{}/scope.md", self.own_dir())
    }

    /// What the last point check found.
    pub fn check_json(&self) -> String {
        format!("{}/check.json", self.dir())
    }

    /// Whether live review is on, and who watches.
    pub fn live_json(&self) -> String {
        format!("{}/live.json", self.settings_dir())
    }

    /// The bibliography files the author added in the editor.
    pub fn sources_json(&self) -> String {
        format!("{}/sources.json", self.settings_dir())
    }

    /// What the last reference check found.
    pub fn verify_json(&self) -> String {
        format!("{}/verify.json", self.dir())
    }

    /// Which hints of which answers the author marked as done: a working aid, not part of the log.
    pub fn marks_json(&self) -> String {
        format!("{}/marks.json", self.dir())
    }

    pub fn doc_json(&self) -> String {
        format!("{}/doc.json", self.dir())
    }

    pub fn layout(&self) -> Layout {
        Layout::editor(&self.folder, &self.relative())
    }

    /// Looks at this document's stored files before they are used: one that cannot be read is set
    /// aside and one from a newer editor is named, so that neither is written over (see `stored`).
    pub fn look_over(&self, host: &dyn Host) -> Vec<crate::stored::Notice> {
        use crate::stored::{look_at, CHECK, DOC, LIVE, MARKS, PARTY, PROJECT, SOURCES};
        type Any = serde_json::Map<String, serde_json::Value>;
        [
            look_at::<DocInfo>(host, &self.doc_json(), &DOC),
            look_at::<crate::project::Project>(host, &self.project_json(), &PROJECT),
            look_at::<crate::review::LiveSettings>(host, &self.live_json(), &LIVE),
            look_at::<crate::review::CheckFile>(host, &self.check_json(), &CHECK),
            look_at::<Any>(host, &self.sources_json(), &SOURCES),
            look_at::<Any>(host, &self.party_json(), &PARTY),
            look_at::<Any>(host, &self.marks_json(), &MARKS),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// What the sidecar knows about its document; `None` before the first use or when it cannot be read.
    pub fn info(&self, host: &dyn Host) -> Option<DocInfo> {
        crate::stored::read_as(host, &self.doc_json(), &crate::stored::DOC)
    }

    /// The sidecar's `doc.json`, created at first use and brought up to date with the document's name and content.
    pub fn ensure(&self, host: &dyn Host, document_text: &str) -> io::Result<DocInfo> {
        let mut info = self.info(host).unwrap_or_else(|| DocInfo {
            id: host.new_id(),
            file: self.name.clone(),
            created_at: host.now(),
            last_hash: String::new(),
            language: english(),
            extra: Default::default(),
        });
        let hash = content_hash(document_text);
        if info.last_hash != hash || info.file != self.name || !host.exists(&self.doc_json()) {
            info.last_hash = hash;
            info.file = self.name.clone();
            self.write_info(host, &info)?;
        }
        Ok(info)
    }

    /// Takes over another document's sidecar: the AI log, notes and everything else move here.
    /// Refused when this document already has one, so no log is ever overwritten.
    pub fn adopt(&self, host: &dyn Host, from: &Sidecar) -> io::Result<()> {
        if host.exists(&self.dir()) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already has an AI log of its own", self.name)));
        }
        host.rename(&from.dir(), &self.dir())
    }

    /// Sidecars in this document's folder whose document is no longer there: a renamed or deleted file's log.
    pub fn orphans(&self, host: &dyn Host) -> Vec<(Sidecar, DocInfo)> {
        let mut names = host.folders(&format!("{}/{SIDECAR_FOLDER}", self.folder)).unwrap_or_default();
        names.sort();
        let beside = |name: &String| Sidecar::for_document(&format!("{}/{name}", self.folder));
        names.iter().map(beside).filter(|other| !host.exists(&other.document)).filter_map(|other| other.info(host).map(|info| (other, info))).collect()
    }

    pub fn write_info(&self, host: &dyn Host, info: &DocInfo) -> io::Result<()> {
        crate::stored::write(host, &self.doc_json(), &crate::stored::DOC, info)
    }
}
