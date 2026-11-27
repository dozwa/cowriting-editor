//! The document as a PDF or a Word file, made by pandoc on the author's computer, with the
//! references behind its citekeys. No AI is involved, and nothing here writes the document.

use cowriting_core::sources::{cited_keys, load, load_bibliography};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Pdf,
    Docx,
}

impl FileFormat {
    pub fn extension(self) -> &'static str {
        match self {
            FileFormat::Pdf => "pdf",
            FileFormat::Docx => "docx",
        }
    }
}

/// The programs that turn pandoc's output into a PDF, in the order they are tried.
/// Typst comes with the editor and is the standard; a LaTeX found on the computer can be chosen instead.
const PDF_ENGINES: [&str; 5] = ["typst", "xelatex", "lualatex", "pdflatex", "tectonic"];

/// Where the editor's own pandoc and Typst are, once the window knows (the installed app's resources).
static TOOLS_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Tells the export where the programs that came with the editor are.
pub fn set_tools_dir(dir: PathBuf) {
    let _ = TOOLS_DIR.set(dir);
}

/// The folders with the editor's own programs, first choice before anything installed on the computer.
fn own_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("COWRITING_TOOLS").map(PathBuf::from).into_iter().chain(TOOLS_DIR.get().cloned()).collect();
    if cfg!(debug_assertions) {
        // a development build: what scripts/fetch-tools.py put into the repository's folder
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tools"));
    }
    dirs
}

/// The paper a PDF is set on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

/// What the author chose for a PDF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PdfChoice {
    /// One of the engines `ExportTools` lists; None for the standard one.
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub paper: Paper,
    /// How the text is set, where the author wants it other than the standard.
    #[serde(default)]
    pub layout: Layout,
}

/// How the text of a PDF is set: what a call for papers or a faculty usually prescribes. Whatever
/// is left at its standard is not passed on, and pandoc's template decides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Layout {
    /// The typeface of the text, by the name the system knows it under; empty for the standard.
    pub font: String,
    /// In points: 10, 11 or 12, which is what LaTeX's standard class has.
    pub font_size: Option<u8>,
    /// The margin on all four sides, in centimetres.
    pub margin_cm: Option<f32>,
    /// 1 is single, 1.5 one and a half, 2 double.
    pub line_spacing: Option<f32>,
    /// Paragraphs begin with an indent and have no space between them; the standard is a space and no indent.
    pub indent: bool,
    /// Lines end where the words end; the standard is justified.
    pub ragged: bool,
    /// Headings are numbered: 1, 1.1, 1.1.1.
    pub numbered: bool,
}

impl Layout {
    /// The same layout with every value inside what the export can set.
    pub fn within_limits(&self) -> Layout {
        // a font's name is letters, digits and a few signs: nothing that could end the line it is written on
        let font: String = self.font.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '\'' | '+' | '&')).take(80).collect();
        Layout {
            font: font.trim().to_string(),
            font_size: self.font_size.filter(|size| (10..=12).contains(size)),
            margin_cm: self.margin_cm.filter(|margin| margin.is_finite()).map(|margin| (margin.clamp(0.5, 6.0) * 100.0).round() / 100.0),
            line_spacing: self.line_spacing.filter(|spacing| spacing.is_finite()).map(|spacing| (spacing.clamp(0.8, 3.0) * 100.0).round() / 100.0),
            ..self.clone()
        }
    }

    /// What pandoc is told about the document besides its own front matter, as a metadata file
    /// (`--metadata-file`): the language, and the layout where it is not the standard. What the
    /// front matter sets itself comes first; pandoc sees to that. The names are those of pandoc's
    /// templates, for Typst and for LaTeX; each reads its own and passes over the other's.
    pub fn metadata(&self, language: &str) -> String {
        let layout = self.within_limits();
        let mut lines = vec![format!("lang: {}", if language == "de" { "de-DE" } else { "en-US" })];
        if !layout.font.is_empty() {
            lines.push(format!("mainfont: \"{}\"", layout.font));
        }
        if let Some(size) = layout.font_size {
            lines.push(format!("fontsize: {size}pt"));
        }
        if let Some(margin) = layout.margin_cm {
            // Typst takes the sides one by one, LaTeX the geometry package's words
            lines.extend(["margin:".to_string(), format!("  x: {margin}cm"), format!("  y: {margin}cm"), format!("geometry: margin={margin}cm")]);
        }
        if let Some(spacing) = layout.line_spacing {
            lines.push(format!("linestretch: {spacing}"));
        }
        if layout.numbered {
            lines.extend(["section-numbering: \"1.1\"".to_string(), "numbersections: true".to_string()]);
        }
        if layout.indent {
            lines.push("indent: true".into());
        }
        // what the templates have no name for is said in the typesetter's own words, after the template's settings
        let mut typst: Vec<String> = Vec::new();
        let mut latex: Vec<&str> = Vec::new();
        if layout.indent {
            // no space between paragraphs: the distance of two lines, which the template sets as 0.65em times the line spacing
            typst.push(format!("#set par(first-line-indent: 1.5em, spacing: {}em)", ((0.65 * layout.line_spacing.unwrap_or(1.0)) * 1000.0).round() / 1000.0));
        }
        if layout.ragged {
            typst.push("#set par(justify: false)".into());
            latex.push("\\raggedright");
        }
        if !typst.is_empty() || !latex.is_empty() {
            lines.push("include-before:".into());
            for (format, code) in [("typst", typst.join("\n")), ("latex", latex.join("\n"))] {
                if !code.is_empty() {
                    lines.push("  - |".into());
                    lines.push(format!("    ```{{={format}}}"));
                    lines.extend(code.lines().map(|line| format!("    {line}")));
                    lines.push("    ```".into());
                }
            }
        }
        format!("{}\n", lines.join("\n"))
    }
}

/// Where programs are looked for. An app started from the Dock or the Start menu does not get the shell's PATH.
fn program_dirs() -> Vec<PathBuf> {
    let mut dirs = own_dirs();
    dirs.extend(std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect::<Vec<_>>()).unwrap_or_default());
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    if cfg!(windows) {
        dirs.extend(["C:/Program Files/Pandoc", "C:/texlive/2026/bin/windows", "C:/texlive/2025/bin/windows"].iter().map(PathBuf::from));
        dirs.extend(home.iter().flat_map(|home| ["AppData/Local/Pandoc", "AppData/Local/Programs/MiKTeX/miktex/bin/x64"].iter().map(|below| home.join(below))));
    } else {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/Library/TeX/texbin", "/usr/bin", "/opt/local/bin"].iter().map(PathBuf::from));
        dirs.extend(home.iter().flat_map(|home| [".local/bin", ".cargo/bin"].iter().map(|below| home.join(below))));
    }
    dirs
}

pub fn find_program(name: &str) -> Option<PathBuf> {
    let names = [name.to_string(), format!("{name}.exe")];
    program_dirs().iter().flat_map(|dir| names.iter().map(move |name| dir.join(name))).find(|path| path.is_file())
}

/// What is there to export with.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportTools {
    pub has_pandoc: bool,
    /// The programs that can typeset the PDF, the standard one first; empty when there is none (Word export does not need one).
    pub pdf_engines: Vec<String>,
    /// pandoc and Typst are the editor's own copies, not ones installed on the computer.
    pub is_own: bool,
    /// Zotero is running with Better BibTeX: references come from the library by citekey.
    pub has_zotero: bool,
    /// The typefaces on this computer that a PDF can be set in, as Typst names them; empty when it cannot say.
    pub fonts: Vec<String>,
}

/// The typefaces Typst finds on this computer (`typst fonts`), sorted.
fn fonts() -> Vec<String> {
    let Some(typst) = find_program("typst") else { return Vec::new() };
    let mut command = Command::new(typst);
    command.arg("fonts").stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    let listed = command.output().map(|output| String::from_utf8_lossy(&output.stdout).into_owned()).unwrap_or_default();
    // a family whose name starts with a dot is the system's own and cannot be chosen by name
    let mut fonts: Vec<String> = listed.lines().map(str::trim).filter(|name| !name.is_empty() && !name.starts_with('.')).map(String::from).collect();
    fonts.sort_by_key(|name| name.to_lowercase());
    fonts.dedup();
    fonts
}

pub fn local_tools() -> ExportTools {
    ExportTools {
        has_pandoc: find_program("pandoc").is_some(),
        pdf_engines: PDF_ENGINES.iter().filter(|engine| find_program(engine).is_some()).map(|engine| engine.to_string()).collect(),
        is_own: ["pandoc", "typst"].iter().all(|name| find_program(name).is_some_and(|found| own_dirs().iter().any(|dir| found.starts_with(dir)))),
        has_zotero: false,
        fonts: Vec::new(),
    }
}

/// What is there to export with, and the typefaces to choose from: for the export dialog.
pub fn local_tools_with_fonts() -> ExportTools {
    ExportTools { fonts: fonts(), ..local_tools() }
}

/// What an export made.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentExport {
    pub path: String,
    /// How many different keys the document cites.
    pub cited: usize,
    /// How many of them came from the Zotero library.
    pub from_zotero: usize,
    /// Cited keys that neither Zotero nor a bibliography file holds: they are in the export as they are in the text.
    pub missing: Vec<String>,
    /// What pandoc remarked.
    pub warnings: Vec<String>,
}

/// The references an export uses and the keys nobody holds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct References {
    /// Bibliography files for pandoc, in order.
    pub files: Vec<String>,
    pub from_zotero: usize,
    pub missing: Vec<String>,
}

/// The ids of the items in a CSL JSON text.
fn csl_ids(csl: &str) -> BTreeSet<String> {
    load("zotero.json", csl).unwrap_or_default().into_iter().map(|entry| entry.key).collect()
}

/// Which references the export uses: what Zotero gave for the cited keys (already written to `zotero_file`), then the document's own files.
pub fn references(bibliography: &cowriting_core::sources::Bibliography, document_text: &str, zotero: Option<(&str, &str)>) -> References {
    let cited = cited_keys(document_text);
    let from_zotero = zotero.map(|(_, csl)| csl_ids(csl)).unwrap_or_default();
    // pandoc reads BibTeX, CSL JSON and RIS; a PubMed export has no citekeys to cite by anyway
    let readable = bibliography.files.iter().filter(|file| file.error.is_none() && file.format != "MEDLINE");
    let mut files: Vec<String> = zotero.filter(|_| !from_zotero.is_empty()).map(|(path, _)| path.to_string()).into_iter().collect();
    files.extend(readable.map(|file| file.path.clone()));
    let in_files: BTreeSet<&str> = bibliography.entries.iter().filter(|entry| !entry.derived).map(|entry| entry.key.as_str()).collect();
    let missing = cited.iter().filter(|key| !from_zotero.contains(*key) && !in_files.contains(key.as_str())).cloned().collect();
    References { files, from_zotero: cited.iter().filter(|key| from_zotero.contains(*key)).count(), missing }
}

/// What pandoc is called with. The document comes on standard input, so what is exported is what the editor shows.
/// `pdf_engine` is the program's name and where it is.
/// `style` is a CSL file the author chose in the editor; a style the document names itself (`csl:`) pandoc reads on its own.
pub fn pandoc_args(format: FileFormat, output: &str, folder: &str, references: &[String], pdf_engine: Option<(&str, &str)>, paper: Paper, style: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = ["--from", "markdown", "--standalone", "--output", output, "--resource-path", folder].iter().map(|arg| arg.to_string()).collect();
    args.push("--citeproc".into());
    for file in references {
        args.extend(["--bibliography".to_string(), file.clone()]);
    }
    if let Some(style) = style {
        args.extend(["--csl".to_string(), style.to_string()]);
    }
    if let (FileFormat::Pdf, Some((engine, path))) = (format, pdf_engine) {
        // Typst and LaTeX name the same paper differently
        let size = match (paper, engine) {
            (Paper::A4, _) => "a4",
            (Paper::Letter, "typst") => "us-letter",
            (Paper::Letter, _) => "letter",
        };
        args.extend(["--pdf-engine".to_string(), path.to_string(), "--variable".to_string(), format!("papersize={size}")]);
        if engine == "typst" {
            args.push(format!("--pdf-engine-opt=--root={}", typst_root(folder)));
        }
    }
    args
}

/// The folder Typst may read files below, for a document in `folder`: the top of its disk.
///
/// pandoc copies every picture into a temporary folder of its own and names the copy to Typst by its full path.
/// Typst reads a full path as one below its root, which is the document's folder unless it is told otherwise, and
/// reads nothing outside the root: no picture was ever found. From the top of the disk the copies are found.
/// It also means Typst code in the document can read any file of the author's; it is the author's own document.
/// On Windows the root is the document's drive, so pictures are not found while the temporary folder is on another drive.
fn typst_root(folder: &str) -> String {
    Path::new(folder).ancestors().last().filter(|top| top.has_root()).map_or_else(|| "/".to_string(), |top| top.to_string_lossy().into_owned())
}

/// Refuses a target that is not a file of the chosen kind, or that is the document itself.
pub fn check_target(format: FileFormat, target: &str, document: &str) -> Result<(), String> {
    if !target.to_lowercase().ends_with(&format!(".{}", format.extension())) {
        return Err(format!("The export needs a file name that ends in .{}.", format.extension()));
    }
    if Path::new(target) == Path::new(document) {
        return Err("The export cannot replace the document itself.".into());
    }
    Ok(())
}

/// Runs pandoc. Returns what it remarked; an error says what pandoc said.
pub fn run_pandoc(document_text: &str, folder: &str, args: &[String]) -> Result<Vec<String>, String> {
    let pandoc = find_program("pandoc").ok_or("pandoc is not installed. Install it from pandoc.org, then export again.")?;
    let path = std::env::join_paths(program_dirs()).map_err(|e| e.to_string())?;
    let mut command = Command::new(pandoc);
    command.args(args).current_dir(folder).env("PATH", path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    let mut child = command.spawn().map_err(|e| format!("pandoc could not be started: {e}"))?;
    let mut stdin = child.stdin.take().expect("piped");
    let text = document_text.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let output = child.wait_with_output().map_err(|e| format!("pandoc did not finish: {e}"))?;
    let _ = writer.join();
    let said = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<String> = said.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from).collect();
    if !output.status.success() {
        let shown: Vec<&str> = lines.iter().rev().take(12).rev().map(String::as_str).collect();
        return Err(format!("pandoc could not make the file:\n{}", shown.join("\n")));
    }
    Ok(lines.into_iter().filter(|line| line.starts_with("[WARNING]")).collect())
}

/// The document's bibliography, for `references`.
pub fn bibliography_of(workspace: &cowriting_core::turn::Workspace, document_text: &str) -> cowriting_core::sources::Bibliography {
    load_bibliography(workspace.host.as_ref(), &workspace.sidecar, document_text)
}
