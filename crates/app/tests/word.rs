//! The layout of a Word file: it is in the styles, the page and the footer of the file itself,
//! taken from the project's layout or from a Word file the author chose as a template.

use cowriting_app::export::{find_program, Layout, PageNumbers};
use cowriting_app::{FileFormat, Paper, PdfChoice, Session};
use cowriting_core::host::DiskHost;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-word-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One part of a Word file, as text; empty when the file has no such part.
fn part(file: &str, name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(file).unwrap()).unwrap();
    let mut text = String::new();
    if let Ok(mut found) = archive.by_name(name) {
        found.read_to_string(&mut text).unwrap();
    }
    text
}

const TEXT: &str = "---\ntitle: Walking breaks\n---\n\n# Method\n\nWe recruited 48 participants and asked them to walk.\n\nA second paragraph with `code` in it.\n\n## Sample\n\nAnd a third one.\n";

fn has_pandoc() -> bool {
    if find_program("pandoc").is_some() {
        return true;
    }
    assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "pandoc is not there: run scripts/fetch-tools.py");
    eprintln!("pandoc is not installed: the Word file's layout was not tried");
    false
}

#[test]
fn a_word_file_has_the_projects_layout_in_its_styles_its_page_and_its_footer() {
    if !has_pandoc() {
        return;
    }
    let folder = Folder::new("layout");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    let export = |name: &str, choice: &PdfChoice| {
        session.export_document(FileFormat::Docx, &folder.path(name), TEXT, None, choice).unwrap_or_else(|e| panic!("{name}: {e}"));
        folder.path(name)
    };

    // the standard: pandoc's styles, on the chosen paper with Word's own margins, no page numbers
    let plain = export("plain.docx", &PdfChoice::default());
    assert!(part(&plain, "word/styles.xml").contains("w:asciiTheme=\"minorHAnsi\""));
    let page = part(&plain, "word/document.xml");
    assert!(page.contains("w:w=\"11906\"") && page.contains("w:h=\"16838\"") && page.contains("w:top=\"1440\""), "A4 with an inch of margin");
    assert!(!page.contains("footerReference") && !page.contains("<w:t xml:space=\"preserve\">1 </w:t>"), "no numbers, on the pages or before the headings");
    assert!(export("letter.docx", &PdfChoice { paper: Paper::Letter, ..PdfChoice::default() }).ends_with("letter.docx"));
    assert!(part(&folder.path("letter.docx"), "word/document.xml").contains("w:w=\"12240\""));

    let layout = Layout {
        font: "Times New Roman".into(),
        heading_font: "Arial".into(),
        code_font: "Courier New".into(),
        font_size: Some(12),
        margin_cm: Some(2.5),
        margin_left_cm: Some(3.5),
        line_spacing: Some(1.5),
        two_columns: true,
        page_numbers: Some(PageNumbers::BottomCentre),
        no_number_on_first_page: true,
        indent: true,
        numbered: true,
        ..Layout::default()
    };
    let set = export("set.docx", &PdfChoice { layout: layout.clone(), ..PdfChoice::default() });
    let styles = part(&set, "word/styles.xml");
    for expected in [
        // the text's typeface and size, for every paragraph
        "<w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\" w:eastAsia=\"Times New Roman\" w:cs=\"Times New Roman\" />",
        "<w:sz w:val=\"24\" />",
        "w:line=\"360\" w:lineRule=\"auto\"",
        "<w:rFonts w:ascii=\"Arial\"",
        "<w:rFonts w:ascii=\"Courier New\"",
        "<w:ind w:firstLine=\"340\" />",
        "<w:ind w:firstLine=\"0\" />",
    ] {
        assert!(styles.contains(expected), "missing {expected} in the styles");
    }
    assert!(!styles.contains("majorHAnsi"), "no heading is left in the theme's typeface");
    let page = part(&set, "word/document.xml");
    // 2.5 cm all round, 3.5 cm on the left; two columns; a footer, and none on the first page
    for expected in ["w:top=\"1417\"", "w:right=\"1417\"", "w:bottom=\"1417\"", "w:left=\"1984\"", "<w:cols w:num=\"2\"", "<w:titlePg", "footerReference"] {
        assert!(page.contains(expected), "missing {expected} in the page: {}", &page[page.find("<w:sectPr").unwrap_or(0)..]);
    }
    // the headings are numbered in the text itself
    assert!(page.contains("1.1"), "the second heading carries its number");
    let rels = part(&set, "word/_rels/document.xml.rels");
    let footer = rels.split("Target=\"").map(|rest| rest.split('"').next().unwrap_or("")).find(|target| target.starts_with("footer")).expect("a footer among the file's parts");
    let footer = part(&set, &format!("word/{footer}"));
    assert!(footer.contains(" PAGE ") && footer.contains("w:val=\"center\""), "{footer}");

    // two-sided, the number at the top outside: a header for the right pages and one for the left, and mirrored margins
    let book = export("book.docx", &PdfChoice { layout: Layout { two_sided: true, page_numbers: Some(PageNumbers::TopOuter), ..Layout::default() }, ..PdfChoice::default() });
    let page = part(&book, "word/document.xml");
    assert!(page.matches("headerReference").count() == 2 && page.contains("w:type=\"even\""), "{}", &page[page.find("<w:sectPr").unwrap_or(0)..]);
    let settings = part(&book, "word/settings.xml");
    assert!(settings.contains("mirrorMargins") && settings.contains("evenAndOddHeaders"), "{settings}");
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), TEXT, "the document is not changed by its export");
    let left: Vec<String> = std::fs::read_dir(&folder.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.ends_with(".docx") && name != "thesis.md")
        .collect();
    assert!(left.is_empty(), "nothing else is left beside the document: {left:?}");
}

#[test]
fn a_word_file_of_the_authors_can_be_the_template_and_comes_before_the_layout() {
    if !has_pandoc() {
        return;
    }
    let folder = Folder::new("template");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    // a faculty's template: here, a Word file set in Georgia with wide margins
    let faculty = Layout { font: "Georgia".into(), margin_cm: Some(4.0), ..Layout::default() };
    std::fs::write(folder.path("faculty.docx"), cowriting_app::word::reference_doc(&faculty, Paper::Letter).unwrap()).unwrap();

    let choice = PdfChoice {
        layout: Layout { font: "Arial".into(), margin_cm: Some(2.0), numbered: true, ..Layout::default() },
        word_template: folder.path("faculty.docx"),
        ..PdfChoice::default()
    };
    session.export_document(FileFormat::Docx, &folder.path("thesis.docx"), TEXT, None, &choice).unwrap();
    let (styles, page) = (part(&folder.path("thesis.docx"), "word/styles.xml"), part(&folder.path("thesis.docx"), "word/document.xml"));
    assert!(styles.contains("w:ascii=\"Georgia\"") && !styles.contains("w:ascii=\"Arial\""), "the template's typeface, not the layout's");
    assert!(page.contains("w:w=\"12240\"") && page.contains("w:top=\"2268\""), "the template's page");
    // what is no style stays the layout's: the numbers before the headings
    assert!(page.contains("1.1"));

    // a PDF does not care about the Word template
    assert!(
        session.export_document(FileFormat::Pdf, &folder.path("thesis.pdf"), TEXT, None, &PdfChoice { layout: Layout::default(), ..choice.clone() }).is_ok()
            || find_program("typst").is_none()
    );
    // a template that is gone is said, and no file is made
    std::fs::remove_file(folder.path("faculty.docx")).unwrap();
    let refused = session.export_document(FileFormat::Docx, &folder.path("second.docx"), TEXT, None, &choice).unwrap_err();
    assert!(refused.contains("faculty.docx") && refused.contains("not there any more"), "{refused}");
    assert!(!std::path::Path::new(&folder.path("second.docx")).exists());
}
