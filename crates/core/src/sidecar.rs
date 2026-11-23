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
    pub schema: u32,
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
        Sidecar { document: path.to_string(), folder: if folder.is_empty() { "/".into() } else { folder.to_string() }, name: name.to_string() }
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

    pub fn rewrites_dir(&self) -> String {
        format!("{}/log-rewrites", self.dir())
    }

    /// The author's briefing of the experts.
    pub fn party_md(&self) -> String {
        format!("{}/party.md", self.dir())
    }

    /// The document's own experts, one file each.
    pub fn party_dir(&self) -> String {
        format!("{}/party", self.dir())
    }

    /// Which experts are in the document's party.
    pub fn party_json(&self) -> String {
        format!("{}/party.json", self.dir())
    }

    pub fn doc_json(&self) -> String {
        format!("{}/doc.json", self.dir())
    }

    pub fn layout(&self) -> Layout {
        Layout::editor(&self.folder, &self.relative())
    }

    /// What the sidecar knows about its document; `None` before the first use or when it cannot be read.
    pub fn info(&self, host: &dyn Host) -> Option<DocInfo> {
        serde_json::from_str(&host.read(&self.doc_json()).ok()?).ok()
    }

    /// The sidecar's `doc.json`, created at first use and brought up to date with the document's name and content.
    pub fn ensure(&self, host: &dyn Host, document_text: &str) -> io::Result<DocInfo> {
        let mut info = self.info(host).unwrap_or_else(|| DocInfo {
            id: host.new_id(),
            file: self.name.clone(),
            created_at: host.now(),
            last_hash: String::new(),
            language: english(),
            schema: 1,
        });
        let hash = content_hash(document_text);
        if info.last_hash != hash || info.file != self.name || !host.exists(&self.doc_json()) {
            info.last_hash = hash;
            info.file = self.name.clone();
            self.write_info(host, &info)?;
        }
        Ok(info)
    }

    pub fn write_info(&self, host: &dyn Host, info: &DocInfo) -> io::Result<()> {
        host.write(&self.doc_json(), &format!("{}\n", serde_json::to_string_pretty(info).expect("doc.json is plain data")))
    }
}
