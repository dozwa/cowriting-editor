//! Feedback and bug reports: the editor sends nothing itself. It opens a new issue or a new mail,
//! filled in with what the author wrote and saw in the window, and the author sends it.
//!
//! The window gives the words; where they go is decided here, so a window that was tampered with
//! cannot make the editor open any address it likes.

/// The project's page, where the code is and a star can be given.
pub const REPOSITORY: &str = "https://github.com/dozwa/cowriting-editor";

/// Where the author of the editor can be bought a coffee.
pub const COFFEE: &str = "https://buymeacoffee.com/dzwanzig";

/// Where a new issue is written.
const ISSUES: &str = "https://github.com/dozwa/cowriting-editor/issues/new";

/// The address reports by mail go to, and the contact the About page shows. Empty would mean: no mail offered.
pub const MAIL: &str = "dorianzwanzig@googlemail.com";

/// What a report may be at most: the address that carries it has to pass through browsers and mail programs.
pub const LONGEST_TITLE: usize = 150;
pub const LONGEST_TEXT: usize = 4000;

/// Mail can be chosen at all.
pub fn has_mail() -> bool {
    !MAIL.is_empty()
}

/// Everything but letters, digits and `-._~` as `%XX`, so the text arrives as written.
fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The address that opens the report, ready to send: `way` is 'issue' or 'mail'.
pub fn address(way: &str, title: &str, text: &str) -> Result<String, String> {
    let (title, text) = (title.trim(), text.trim());
    if title.is_empty() || text.is_empty() {
        return Err("A report needs a title and a text.".into());
    }
    if title.chars().count() > LONGEST_TITLE || text.chars().count() > LONGEST_TEXT {
        return Err(format!("The report is too long to be handed over: at most {LONGEST_TITLE} characters for the title and {LONGEST_TEXT} for the text."));
    }
    match way {
        "issue" => Ok(format!("{ISSUES}?title={}&body={}", encoded(title), encoded(text))),
        // mail programs want line ends as CR LF
        "mail" if has_mail() => {
            Ok(format!("mailto:{MAIL}?subject={}&body={}", encoded(&format!("CoWriting Editor: {title}")), encoded(&text.replace("\r\n", "\n").replace('\n', "\r\n"))))
        }
        "mail" => Err("There is no address for reports by mail yet.".into()),
        _ => Err("A report goes out as an issue or by mail.".into()),
    }
}
