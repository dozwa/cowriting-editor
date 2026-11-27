//! Layouts kept under a name, to use again in another project and to hand to someone else: "Thesis,
//! faculty of medicine", "Conference abstract". Each is one small file in the editor's own folder
//! (`layouts/<name>.json`): the paper and how the text is set. The typesetter and a Word template
//! are not in it; those belong to the computer and the project.

use crate::export::{Layout, Paper, PdfChoice};
use crate::Session;
use cowriting_core::stored;
use serde::{Deserialize, Serialize};

/// The most a layout's file is read of: it is a few hundred characters.
const LONGEST: u64 = 100_000;

/// A layout as its file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct Kept {
    #[serde(default)]
    paper: Paper,
    layout: Layout,
}

/// A layout the author kept under a name.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptLayout {
    pub name: String,
    pub paper: Paper,
    pub layout: Layout,
}

/// A name that can be a file's name on every system: letters, digits, spaces and a few signs.
fn file_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | ',' | '(' | ')' | '+' | '&')).take(60).collect();
    kept.trim().trim_start_matches('.').trim().to_string()
}

fn checked(name: &str) -> Result<String, String> {
    let kept = file_name(name);
    if kept.is_empty() || kept != name.trim() {
        return Err("A layout's name is made of letters, digits, spaces and - _ . , ( ) + &, and is at most 60 characters long.".into());
    }
    Ok(kept)
}

impl Session {
    /// Where the kept layouts are: beside the projects' folder, in the editor's own.
    fn layouts_dir(&self) -> Result<String, String> {
        let projects = self.projects.as_deref().ok_or("This editor has no folder of its own to keep layouts in.")?;
        let own = std::path::Path::new(projects).parent().ok_or("This editor has no folder of its own to keep layouts in.")?;
        Ok(own.join("layouts").to_string_lossy().into_owned())
    }

    /// The layouts kept on this computer, by name. A file that is no layout is passed over.
    pub fn layouts(&self) -> Vec<KeptLayout> {
        let (host, Ok(dir)) = (self.workspace.host.as_ref(), self.layouts_dir()) else { return Vec::new() };
        let mut names: Vec<String> = host.list(&dir).unwrap_or_default().into_iter().filter_map(|file| file.strip_suffix(".json").map(String::from)).collect();
        names.sort_by_key(|name| name.to_lowercase());
        names
            .into_iter()
            .filter_map(|name| {
                let kept: Kept = stored::read_as(host, &format!("{dir}/{name}.json"), &stored::LAYOUT)?;
                Some(KeptLayout { name, paper: kept.paper, layout: kept.layout.within_limits() })
            })
            .collect()
    }

    /// Keeps the paper and the layout of a choice under a name; a layout of that name is replaced.
    pub fn keep_layout(&self, name: &str, choice: &PdfChoice) -> Result<String, String> {
        let (name, dir) = (checked(name)?, self.layouts_dir()?);
        let kept = Kept { paper: choice.paper, layout: choice.layout.within_limits() };
        stored::write(self.workspace.host.as_ref(), &format!("{dir}/{name}.json"), &stored::LAYOUT, &kept).map_err(|e| format!("The layout could not be kept: {e}"))?;
        Ok(name)
    }

    pub fn remove_layout(&self, name: &str) -> Result<(), String> {
        let (name, dir) = (checked(name)?, self.layouts_dir()?);
        self.workspace.host.remove(&format!("{dir}/{name}.json")).map_err(|e| format!("The layout could not be removed: {e}"))
    }

    /// Takes a layout from a file somebody passed on. It is kept under the file's name; a layout
    /// of that name which is another one stays, and this one gets a number.
    pub fn import_layout(&self, path: &str) -> Result<String, String> {
        let file = std::path::Path::new(path);
        if std::fs::metadata(file).map_err(|e| format!("{path} could not be read: {e}"))?.len() > LONGEST {
            return Err(format!("{path} is not a layout of this editor: it is far too long for one."));
        }
        let text = std::fs::read_to_string(file).map_err(|e| format!("{path} could not be read: {e}"))?;
        let not_one = || format!("{path} is not a layout of this editor.");
        let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| not_one())?;
        if !json.get("layout").is_some_and(serde_json::Value::is_object) {
            return Err(not_one());
        }
        let kept: Kept = serde_json::from_value(json).map_err(|_| not_one())?;
        let kept = Kept { paper: kept.paper, layout: kept.layout.within_limits() };
        let stem = file_name(&file.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default());
        let stem = if stem.is_empty() { "Layout".to_string() } else { stem };
        let here = self.layouts();
        let same = |name: &str| here.iter().find(|other| other.name.eq_ignore_ascii_case(name)).map(|other| (other.paper, &other.layout) == (kept.paper, &kept.layout));
        let name = (1..).map(|n| if n == 1 { stem.clone() } else { format!("{stem} {n}") }).find(|name| same(name) != Some(false)).expect("some number is free");
        self.keep_layout(&name, &PdfChoice { paper: kept.paper, layout: kept.layout, ..PdfChoice::default() })
    }

    /// Writes a kept layout to a file of the author's choice, to pass it on.
    pub fn export_layout(&self, name: &str, target: &str) -> Result<(), String> {
        let (name, dir) = (checked(name)?, self.layouts_dir()?);
        if !target.to_lowercase().ends_with(".json") {
            return Err("A layout is saved as a file whose name ends in .json.".into());
        }
        let text = self.workspace.host.read(&format!("{dir}/{name}.json")).map_err(|e| format!("The layout {name} could not be read: {e}"))?;
        std::fs::write(target, text).map_err(|e| format!("{target} could not be written: {e}"))
    }
}
