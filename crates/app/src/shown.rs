//! Showing the author a file the editor wrote, in the system's file manager or with the program
//! the system has for it. The window can only name a file from this list: it cannot have any
//! other file opened.

use std::path::Path;
use std::sync::Mutex;

/// The files the editor wrote for the author since it was started: exports, disclosures, report files.
#[derive(Default)]
pub struct Written(Mutex<Vec<String>>);

impl Written {
    pub fn add(&self, path: &str) {
        let mut paths = self.0.lock().unwrap();
        if !paths.iter().any(|known| known == path) {
            paths.push(path.to_string());
        }
    }

    /// The command that shows `path`, or why not: it has to be a file from this list that is still there.
    pub fn command(&self, path: &str, how: How) -> Result<(String, Vec<String>), String> {
        if !self.0.lock().unwrap().iter().any(|known| known == path) {
            return Err("The editor shows only files it has just written.".into());
        }
        if !Path::new(path).is_file() {
            return Err(format!("{path} is no longer there."));
        }
        Ok(command_for(std::env::consts::OS, path, how))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    /// Marked in its folder, in the file manager.
    Folder,
    /// With the program the system opens such a file with.
    Open,
}

/// The program and its arguments on the system named; the path is always one argument of its own.
pub fn command_for(system: &str, path: &str, how: How) -> (String, Vec<String>) {
    let folder = Path::new(path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let one = |program: &str, args: &[&str]| (program.to_string(), args.iter().map(|a| a.to_string()).collect());
    // The editor joins its own folders on with "/", so a path on Windows mixes both separators. Windows itself
    // takes that, but Explorer reads the path on its own and then shows nothing: it gets backslashes only.
    let native = path.replace('/', "\\");
    let path = if system == "windows" { native.as_str() } else { path };
    match (system, how) {
        ("macos", How::Folder) => one("open", &["-R", path]),
        ("macos", How::Open) => one("open", &[path]),
        ("windows", How::Folder) => one("explorer", &[&format!("/select,{path}")]),
        ("windows", How::Open) => one("rundll32", &["url.dll,FileProtocolHandler", path]),
        // no common way to mark a file there: its folder is opened
        (_, How::Folder) => one("xdg-open", &[&folder]),
        (_, How::Open) => one("xdg-open", &[path]),
    }
}
