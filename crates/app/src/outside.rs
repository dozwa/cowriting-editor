//! What the author keeps with a statement of AI use outside the editor: chats with another tool,
//! read from its export or pasted, and other files. Everything goes into the sidecar's
//! `external/`; the log names each file with its size and its SHA-256. A chat is kept in the
//! editor's own plain form, only the conversations the author chose, so nothing else of an
//! export comes beside the document. The files the author points at are only read.

use cowriting_core::chats::{self, Chat};
use cowriting_core::disclosure::Kept;
use cowriting_core::types::Attachment;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

/// The largest file that is kept as it is.
pub const LARGEST_FILE: u64 = 25 * 1024 * 1024;
/// The largest export the editor reads to show its conversations.
pub const LARGEST_EXPORT: u64 = 1024 * 1024 * 1024;
/// The most files one statement holds.
pub const MOST: usize = 20;

/// Something the author adds to a statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NewAttachment {
    /// A file, kept unchanged.
    File { path: String },
    /// Conversations of an export or a transcript, by their place in `conversations`.
    Chats { path: String, conversations: Vec<usize> },
    /// A chat pasted as text. With the author's marks ("Prompt:", "Answer:") it is kept as a chat, without them as it is.
    Text { text: String },
}

/// One conversation of a file, for the author to choose from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub index: usize,
    pub title: String,
    pub tool: String,
    pub date: String,
    pub turns: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversations {
    /// The form the file has ('chatgpt', 'claude-ai', 'claude-code', 'cowriting'); empty when the editor cannot read it as a chat.
    pub form: String,
    /// The file's name, to show.
    pub name: String,
    pub list: Vec<Conversation>,
    /// It can be kept as a file instead: it is not too large for that.
    pub can_keep_as_file: bool,
}

pub fn sha256(content: &[u8]) -> String {
    Sha256::digest(content).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn name_of(path: &str) -> String {
    Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or("file").to_string()
}

fn read_export(path: &str) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("{path} could not be read: {e}"))?.len();
    if size > LARGEST_EXPORT {
        return Err(format!("{} is larger than the editor reads ({} MB at most).", name_of(path), LARGEST_EXPORT / 1024 / 1024));
    }
    std::fs::read(path).map_err(|e| format!("{path} could not be read: {e}"))
}

/// The conversations a file holds. A file that is no chat the editor knows has none; it can still be kept as a file.
pub fn conversations(path: &str) -> Result<Conversations, String> {
    let content = read_export(path)?;
    let read = std::str::from_utf8(&content).ok().and_then(chats::read);
    let (form, list) = read.map_or((String::new(), Vec::new()), |(form, chats)| {
        let list = chats
            .iter()
            .enumerate()
            .map(|(index, chat)| Conversation { index, title: chat.title.clone(), tool: chat.tool.clone(), date: chat.date.clone(), turns: chat.turns.len() })
            .collect();
        (form.to_string(), list)
    });
    Ok(Conversations { form, name: name_of(path), list, can_keep_as_file: content.len() as u64 <= LARGEST_FILE })
}

/// Writes a new file into `folder` and says how the log names it.
fn put(folder: &str, file: &str, content: &[u8], kind: &str, turns: usize, title: &str, from: &str) -> Result<Attachment, String> {
    std::fs::create_dir_all(folder).map_err(|e| format!("The folder for what is kept with a statement could not be made: {e}"))?;
    let target = Path::new(folder).join(file);
    if target.exists() {
        return Err(format!("{file} is already kept."));
    }
    std::fs::write(&target, content).map_err(|e| format!("{file} could not be kept: {e}"))?;
    Ok(Attachment {
        file: file.to_string(),
        kind: kind.to_string(),
        bytes: content.len() as u64,
        sha256: sha256(content),
        turns: turns as u32,
        title: title.to_string(),
        from: from.to_string(),
    })
}

/// A file's name with nothing in it that a folder or a system minds.
fn plain(name: &str) -> String {
    let kept: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' }).collect();
    let kept = kept.trim_matches(['-', '.']).to_string();
    let tail: String = kept.chars().rev().take(80).collect::<Vec<_>>().into_iter().rev().collect();
    if tail.is_empty() {
        "file".into()
    } else {
        tail
    }
}

/// Keeps what the author adds, in `folder`. `new_id` gives each file a name of its own.
pub fn keep(folder: &str, new: &NewAttachment, new_id: &dyn Fn() -> String) -> Result<Vec<Attachment>, String> {
    match new {
        NewAttachment::File { path } => {
            let size = std::fs::metadata(path).map_err(|e| format!("{path} could not be read: {e}"))?.len();
            if size > LARGEST_FILE {
                return Err(format!("{} is larger than what is kept as a file ({} MB at most).", name_of(path), LARGEST_FILE / 1024 / 1024));
            }
            let content = std::fs::read(path).map_err(|e| format!("{path} could not be read: {e}"))?;
            let name = name_of(path);
            Ok(vec![put(folder, &format!("{}-{}", new_id(), plain(&name)), &content, "file", 0, &name, "")?])
        }
        NewAttachment::Chats { path, conversations } => {
            let content = read_export(path)?;
            let (form, chats) = std::str::from_utf8(&content).ok().and_then(chats::read).ok_or_else(|| format!("{} is not a chat the editor can read.", name_of(path)))?;
            let whole = sha256(&content);
            let mut kept = Vec::new();
            for index in conversations {
                let mut chat = chats.get(*index).cloned().ok_or_else(|| format!("{} has no conversation {}.", name_of(path), index + 1))?;
                (chat.from, chat.from_sha256) = (name_of(path), whole.clone());
                kept.push(put(folder, &format!("chat-{}.json", new_id()), chat.to_file().as_bytes(), "chat", chat.turns.len(), &chat.title, form)?);
            }
            Ok(kept)
        }
        NewAttachment::Text { text } => {
            let text = text.replace("\r\n", "\n");
            if text.trim().is_empty() {
                return Err("There is no text to keep.".into());
            }
            Ok(vec![match chats::from_text(&text) {
                Some(chat) => put(folder, &format!("chat-{}.json", new_id()), chat.to_file().as_bytes(), "chat", chat.turns.len(), "", chats::PASTED)?,
                None => put(folder, &format!("text-{}.md", new_id()), text.as_bytes(), "text", 0, "", "")?,
            }])
        }
    }
}

/// Takes a kept file away. The log still names it.
pub fn remove(folder: &str, attachment: &Attachment) {
    if attachment.file == plain(&attachment.file) {
        let _ = std::fs::remove_file(Path::new(folder).join(&attachment.file));
    }
}

/// Checksums by a file's path, the time it was last written and its size.
type Seen = HashMap<(String, SystemTime, u64), String>;

/// A file's checksum, read again only when the file has changed on disk.
fn checksum(path: &Path) -> Option<String> {
    static SEEN: Mutex<Option<Seen>> = Mutex::new(None);
    let about = std::fs::metadata(path).ok()?;
    let key = (path.to_string_lossy().into_owned(), about.modified().ok()?, about.len());
    if let Some(known) = SEEN.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return Some(known.clone());
    }
    let sum = sha256(&std::fs::read(path).ok()?);
    let mut seen = SEEN.lock().unwrap();
    let seen = seen.get_or_insert_with(HashMap::new);
    if seen.len() > 500 {
        seen.clear();
    }
    seen.insert(key, sum.clone());
    Some(sum)
}

/// 'intact', 'changed' (it is not what the log says was kept) or 'missing'.
pub fn state(folder: &str, attachment: &Attachment) -> &'static str {
    // a name out of a log someone edited must not lead out of the folder
    if attachment.file != plain(&attachment.file) {
        return "missing";
    }
    match checksum(&Path::new(folder).join(&attachment.file)) {
        None => "missing",
        Some(sum) if sum == attachment.sha256 => "intact",
        Some(_) => "changed",
    }
}

/// A kept file as it is now, for the appendix and for comparing pasted text.
pub fn found(folder: &str, attachment: &Attachment) -> Kept {
    match state(folder, attachment) {
        "missing" => Kept::Missing,
        "changed" => Kept::Changed,
        _ => {
            let text = || std::fs::read_to_string(Path::new(folder).join(&attachment.file)).ok();
            match attachment.kind.as_str() {
                "chat" => text().and_then(|text| Chat::kept(&text)).map_or(Kept::Changed, Kept::Chat),
                "text" => text().map_or(Kept::Changed, Kept::Text),
                _ => Kept::File,
            }
        }
    }
}
