//! Linked files that are not plain text: a Word file is read with pandoc, a PDF with a library.
//! The editor keeps the text it found as a copy in the project's folder and passes that on; the
//! author's file is only ever read.

use crate::export::find_program;
use std::process::{Command, Stdio};

/// How a linked file's text is got at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Read as it is.
    Text,
    /// Read by pandoc, as this format of its.
    Pandoc(&'static str),
    Pdf,
}

pub fn kind_of(path: &str) -> Kind {
    let lower = path.to_lowercase();
    let ending = lower.rsplit('.').next().unwrap_or("");
    match ending {
        "docx" => Kind::Pandoc("docx"),
        "odt" => Kind::Pandoc("odt"),
        "rtf" => Kind::Pandoc("rtf"),
        "epub" => Kind::Pandoc("epub"),
        "pdf" => Kind::Pdf,
        _ => Kind::Text,
    }
}

/// The text in a Word file or a PDF. An error says what the author can do about it.
pub fn text_of(path: &str, kind: Kind) -> Result<String, String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let text = match kind {
        Kind::Text => std::fs::read_to_string(path).map_err(|e| format!("{name} could not be read: {e}"))?,
        Kind::Pandoc(format) => with_pandoc(path, format).map_err(|e| format!("{name} could not be read: {e}"))?,
        Kind::Pdf => {
            let bytes = std::fs::read(path).map_err(|e| format!("{name} could not be read: {e}"))?;
            // the library gives up loudly on some files; that is an unreadable file, not a crash of the editor
            let found = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes));
            match found {
                Ok(Ok(text)) => text,
                _ => return Err(format!("{name} could not be read as a PDF. If it opens in a PDF reader, save its text as a .txt file and link that.")),
            }
        }
    };
    if text.trim().is_empty() {
        return Err(match kind {
            Kind::Pdf => format!("{name} has no text in it: it may be a scan. Linked files need text the editor can read."),
            _ => format!("{name} has no text in it."),
        });
    }
    Ok(tidy(&text))
}

/// Runs of empty lines and spaces at line ends say nothing and cost room.
fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut empty = 0;
    for line in text.lines().map(str::trim_end) {
        empty = if line.is_empty() { empty + 1 } else { 0 };
        if empty < 2 {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string() + "\n"
}

fn with_pandoc(path: &str, format: &str) -> Result<String, String> {
    let pandoc = find_program("pandoc").ok_or("pandoc, which reads such files, was not found. Save the file as text and link that.")?;
    let mut command = Command::new(pandoc);
    command.args(["--from", format, "--to", "plain", "--wrap=none", "--sandbox", path]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    let output = command.output().map_err(|e| format!("pandoc could not be started: {e}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(said.lines().map(str::trim).rfind(|line| !line.is_empty()).unwrap_or("pandoc could not read it").to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
