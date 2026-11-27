//! The type and layout of a PDF: what the author sets reaches pandoc in the words its templates
//! know, the document's own front matter comes first, and both typesetters make a PDF of it.

use cowriting_app::export::{find_program, local_tools, Layout, PageNumbers};
use cowriting_app::linked::{text_of, Kind};
use cowriting_app::{FileFormat, Paper, PdfChoice, Session};
use cowriting_core::host::DiskHost;
use std::path::PathBuf;
use std::sync::Arc;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-layout-{name}-{}", std::process::id()));
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

fn thesis_layout() -> Layout {
    Layout { font: "Times New Roman".into(), font_size: Some(12), margin_cm: Some(2.5), line_spacing: Some(1.5), indent: true, ragged: true, numbered: true, ..Layout::default() }
}

#[test]
fn what_the_author_sets_is_told_to_both_typesetters_in_their_own_words() {
    let told = thesis_layout().metadata("de");
    for line in [
        "lang: de-DE",
        "mainfont: \"Times New Roman\"",
        "fontsize: 12pt",
        // Typst's template takes the sides, LaTeX's the geometry package
        "margin:\n  x: 2.5cm\n  y: 2.5cm",
        "geometry: margin=2.5cm",
        "linestretch: 1.5",
        "section-numbering: \"1.1\"",
        "numbersections: true",
        "indent: true",
        // no name in the templates: said in each typesetter's own language, after the template's settings
        "include-before:\n  - |\n    ```{=typst}\n    #set par(first-line-indent: 1.5em, spacing: 0.975em)\n    #set par(justify: false)\n    ```\n  - |\n    ```{=latex}\n    \\raggedright\n    ```",
    ] {
        assert!(told.contains(line), "missing {line:?} in:\n{told}");
    }
}

#[test]
fn the_standard_layout_tells_pandoc_the_language_and_nothing_else() {
    assert_eq!(Layout::default().metadata("en"), "lang: en-US\n");
    assert_eq!(Layout::default().metadata("de"), "lang: de-DE\n");
    assert_eq!(PdfChoice::default().layout, Layout::default());
    // a choice kept by an older version has no layout: it reads as the standard
    let kept: PdfChoice = serde_json::from_str(r#"{"engine": null, "paper": "letter"}"#).unwrap();
    assert_eq!((kept.paper, kept.layout), (Paper::Letter, Layout::default()));
}

#[test]
fn values_the_export_cannot_set_are_left_out_and_a_font_name_cannot_carry_anything_else() {
    let odd = Layout { font: "Times\"\nheader-includes: |\n  evil: yes".into(), font_size: Some(48), margin_cm: Some(40.0), line_spacing: Some(f32::NAN), ..Layout::default() };
    let told = odd.metadata("en");
    assert_eq!(told, "lang: en-US\nmainfont: \"Timesheader-includes   evil yes\"\nmargin:\n  x: 6cm\n  y: 6cm\ngeometry: margin=6cm\n");
}

const TEXT: &str = "---\ntitle: Walking breaks\n---\n\n# Method\n\nWe recruited 48 participants from two offices and asked them to walk.\n\n## Sample\n\nThe second paragraph is here to be indented and set ragged.\n\nAnd a third one follows it.\n";

fn exported(session: &Session, folder: &Folder, name: &str, text: &str, choice: &PdfChoice) -> (String, Vec<u8>) {
    session.export_document(FileFormat::Pdf, &folder.path(name), text, None, choice).unwrap_or_else(|e| panic!("{name}: {e}"));
    (text_of(&folder.path(name), Kind::Pdf).unwrap(), std::fs::read(folder.path(name)).unwrap())
}

#[test]
fn a_pdf_follows_the_layout_with_typst_and_the_front_matter_comes_first() {
    let folder = Folder::new("typst");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    if local_tools().pdf_engines.first().map(String::as_str) != Some("typst") {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "Typst is not there: run scripts/fetch-tools.py");
        eprintln!("Typst is not there: the layout was not tried on a PDF");
        return;
    }
    // the standard: headings without numbers, in Typst's own serif
    let (plain, standard) = exported(&session, &folder, "standard.pdf", TEXT, &PdfChoice::default());
    assert!(plain.contains("Method") && !plain.contains("1 Method") && !plain.contains("1.1 Sample"), "{plain}");
    assert!(!String::from_utf8_lossy(&standard).contains("DejaVuSansMono"));

    // everything set; DejaVu Sans Mono comes with Typst, so it is there on every system
    let layout = Layout { font: "DejaVu Sans Mono".into(), ..thesis_layout() };
    let choice = PdfChoice { layout, ..PdfChoice::default() };
    let (set, bytes) = exported(&session, &folder, "set.pdf", TEXT, &choice);
    assert!(set.contains("1 Method") && set.contains("1.1 Sample"), "the headings are numbered: {set}");
    assert!(String::from_utf8_lossy(&bytes).contains("DejaVuSansMono"), "the text is set in the chosen typeface");
    // wider margins, larger type and more space between the lines: the same text needs more room
    let long = format!("{TEXT}\n{}", "A sentence to fill the page with words, again and again. ".repeat(260));
    let pages = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes).matches("/Type /Page\n").count()
            + String::from_utf8_lossy(bytes).matches("/Type/Page/").count()
            + String::from_utf8_lossy(bytes).matches("/Type /Page ").count()
    };
    let (_, tight) = exported(&session, &folder, "tight.pdf", &long, &PdfChoice::default());
    let (_, wide) = exported(
        &session,
        &folder,
        "wide.pdf",
        &long,
        &PdfChoice { layout: Layout { font_size: Some(12), margin_cm: Some(4.0), line_spacing: Some(2.0), ..Layout::default() }, ..PdfChoice::default() },
    );
    assert!(pages(&wide) > pages(&tight), "{} pages with the layout, {} without", pages(&wide), pages(&tight));

    // what the document says itself comes first: its own numbering stays
    let own = TEXT.replace("title: Walking breaks", "title: Walking breaks\nsection-numbering: \"A.1\"");
    let (kept, _) = exported(&session, &folder, "own.pdf", &own, &choice);
    assert!(kept.contains("A Method") && kept.contains("A.1 Sample") && !kept.contains("1.1 Sample"), "{kept}");
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), TEXT, "the document is not changed by its export");
}

#[test]
fn a_pdf_follows_the_layout_with_a_latex_that_is_installed() {
    let Some(engine) = ["xelatex", "lualatex"].into_iter().find(|engine| find_program(engine).is_some() && find_program("pandoc").is_some()) else {
        eprintln!("no XeLaTeX or LuaLaTeX is installed: the layout was not tried with LaTeX");
        return;
    };
    let folder = Folder::new("latex");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    // no typeface by name here: which ones a LaTeX finds differs from computer to computer
    let choice = PdfChoice { engine: Some(engine.into()), layout: Layout { font: String::new(), ..thesis_layout() }, ..PdfChoice::default() };
    let (set, bytes) = exported(&session, &folder, "set.pdf", TEXT, &choice);
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(set.contains("1 Method") && set.contains("1.1 Sample"), "the headings are numbered: {set}");
    let (plain, _) = exported(&session, &folder, "standard.pdf", TEXT, &PdfChoice { engine: Some(engine.into()), ..PdfChoice::default() });
    assert!(!plain.contains("1 Method"), "{plain}");
}

#[test]
fn a_projects_layout_is_kept_with_the_project_and_not_with_the_document() {
    let folder = Folder::new("kept");
    std::fs::write(folder.path("thesis.md"), TEXT).unwrap();
    let projects = folder.path("config/projects");
    let (session, _) = Session::open_in(Arc::new(DiskHost), &folder.path("thesis.md"), Some(&projects)).unwrap();
    assert_eq!(session.export_choice(), None, "nothing was ever chosen here");

    let choice = PdfChoice { engine: Some("xelatex".into()), paper: Paper::Letter, layout: Layout { font_size: Some(48), ..thesis_layout() }, ..PdfChoice::default() };
    session.keep_export_choice(&choice).unwrap();
    // what the export could not set is not kept either
    let kept = PdfChoice { layout: Layout { font_size: None, ..thesis_layout() }, ..choice.clone() };
    assert_eq!(session.export_choice(), Some(kept.clone()));
    // it is in the project's settings, in the editor's own folder: nothing appears beside the document
    assert!(!std::path::Path::new(&folder.path(".cowriting")).exists());
    assert!(std::fs::read_to_string(session.project_view().folder + "/project.json").unwrap().contains("\"lineSpacing\": 1.5"));

    // opened again, the project still has it; the other settings of the project are not touched by it
    session.set_pictures_folder("figures").unwrap();
    let (again, _) = Session::open_in(Arc::new(DiskHost), &folder.path("thesis.md"), Some(&projects)).unwrap();
    assert_eq!(again.export_choice(), Some(kept));
    assert_eq!(again.pictures_folder(), "figures");
    // another document has none
    std::fs::write(folder.path("other.md"), TEXT).unwrap();
    let (other, _) = Session::open_in(Arc::new(DiskHost), &folder.path("other.md"), Some(&projects)).unwrap();
    assert_eq!(other.export_choice(), None);
}

/// What a thesis printed on both sides may ask for beyond the text's own type.
fn book_layout() -> Layout {
    Layout {
        heading_font: "DejaVu Sans Mono".into(),
        code_font: "DejaVu Sans Mono".into(),
        font_size: Some(14),
        margin_cm: Some(2.0),
        margin_left_cm: Some(3.5),
        two_sided: true,
        two_columns: true,
        page_numbers: Some(PageNumbers::TopOuter),
        no_number_on_first_page: true,
        ..Layout::default()
    }
}

#[test]
fn headings_code_sides_columns_and_page_numbers_are_told_to_both_typesetters() {
    let told = book_layout().metadata("en");
    for line in [
        "codefont: \"DejaVu Sans Mono\"\nmonofont: \"DejaVu Sans Mono\"",
        "fontsize: 14pt",
        // the left margin is the inner one on two-sided pages: Typst names it so, the geometry package swaps by itself
        "margin:\n  top: 2cm\n  bottom: 2cm\n  inside: 3.5cm\n  outside: 2cm",
        "geometry:\n  - top=2cm\n  - bottom=2cm\n  - left=3.5cm\n  - right=2cm",
        "classoption:\n  - twoside\n  - twocolumn",
        "columns: 2",
        "header-includes:\n  - |\n    ```{=typst}\n    #show heading: set text(font: \"DejaVu Sans Mono\")\n    #set page(header: context {\n      let n = counter(page).get().first()\n      if n > 1 { align(if calc.odd(n) { right } else { left }, [#n]) }\n    })\n    ```",
        "\\newfontfamily\\cwheadingfont{DejaVu Sans Mono}",
        "\\usepackage[fontsize=14pt]{scrextend}",
        "\\def\\ps@cowriting{\\def\\@oddhead{\\hfil\\thepage}\\def\\@evenhead{\\thepage\\hfil}\\def\\@oddfoot{}\\def\\@evenfoot{}}",
        "\\thispagestyle{empty}",
    ] {
        assert!(told.contains(line), "missing {line:?} in:\n{told}");
    }
    // only one side set, one-sided: the others stay the typesetter's
    let one = Layout { margin_top_cm: Some(4.0), ..Layout::default() }.metadata("en");
    assert_eq!(one, "lang: en-US\nmargin:\n  top: 4cm\ngeometry:\n  - top=4cm\n");
    // no numbers: Typst's template is given an empty pattern; LaTeX is told, the title's page included
    let off = Layout { page_numbers: Some(PageNumbers::Off), no_number_on_first_page: true, ..Layout::default() }.metadata("en");
    assert_eq!(
        off,
        "lang: en-US\npage-numbering: \"\"\nheader-includes:\n  - |\n    ```{=latex}\n    \\makeatletter\\let\\ps@plain\\ps@empty\\makeatother\n    \\pagestyle{empty}\n    ```\n"
    );
    // sizes LaTeX's own classes have need no package
    assert!(!Layout { font_size: Some(11), ..Layout::default() }.metadata("en").contains("scrextend"));
    assert_eq!(book_layout().fonts(), ["DejaVu Sans Mono"]);
    assert_eq!(Layout { font: "Arial".into(), heading_font: "arial".into(), code_font: "Menlo".into(), ..Layout::default() }.fonts(), ["Arial", "Menlo"]);
}

const LONG: &str = "A sentence to fill the page with words, again and again. ";

#[test]
fn a_pdf_with_typst_has_the_headings_type_two_columns_and_page_numbers_where_asked() {
    let folder = Folder::new("typst-more");
    let text = format!("# Method\n\n{}\n\n`code`\n\n## Sample\n\n{}\n", LONG.repeat(150), LONG.repeat(150));
    std::fs::write(folder.path("thesis.md"), &text).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    if local_tools().pdf_engines.first().map(String::as_str) != Some("typst") {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "Typst is not there: run scripts/fetch-tools.py");
        return;
    }
    let pages = |name: &str| pdf_extract::extract_text_by_pages(folder.path(name)).unwrap();
    // how each page begins and ends: where a number would stand
    let edges =
        |name: &str| pages(name).iter().map(|page| (page.trim().chars().take(6).collect::<String>(), page.trim().chars().rev().take(3).collect::<String>())).collect::<Vec<_>>();
    let with = |layout: Layout| PdfChoice { layout, ..PdfChoice::default() };
    // the standard: one column, a number at the bottom of every page
    exported(&session, &folder, "standard.pdf", &text, &PdfChoice::default());
    let standard = edges("standard.pdf");
    assert!(standard.len() >= 2 && standard[0].1.starts_with('1') && standard[1].1.starts_with('2'), "{standard:?}");
    exported(&session, &folder, "off.pdf", &text, &with(Layout { page_numbers: Some(PageNumbers::Off), ..Layout::default() }));
    assert!(edges("off.pdf").iter().all(|(_, end)| !end.starts_with(|c: char| c.is_ascii_digit())), "{:?}", edges("off.pdf"));

    // numbers at the bottom, none on the first page
    exported(&session, &folder, "numbers.pdf", &text, &with(Layout { page_numbers: Some(PageNumbers::BottomCentre), no_number_on_first_page: true, ..Layout::default() }));
    let numbered = edges("numbers.pdf");
    assert!(!numbered[0].1.starts_with('1') && numbered[1].1.starts_with('2'), "{numbered:?}");
    // at the top, from the first page on, and not at the bottom as well
    exported(&session, &folder, "top.pdf", &text, &with(Layout { page_numbers: Some(PageNumbers::TopOuter), ..Layout::default() }));
    let top = edges("top.pdf");
    assert!(top[0].0.starts_with('1') && top[1].0.starts_with('2') && !top[1].1.starts_with('2'), "{top:?}");

    // the headings and the code in a typeface of their own, the text in the standard one
    let (_, bytes) = exported(&session, &folder, "headings.pdf", &text, &with(Layout { heading_font: "DejaVu Sans Mono".into(), ..Layout::default() }));
    let fonts = String::from_utf8_lossy(&bytes).into_owned();
    assert!(fonts.contains("DejaVuSansMono-Bold") && fonts.contains("Libertinus"), "the headings in theirs, the text in the standard");
    // a typeface this computer does not have: Typst sets the text in another one, and the export names what is missing
    let made = session
        .export_document(
            FileFormat::Pdf,
            &folder.path("lacking.pdf"),
            &text,
            None,
            &with(Layout { font: "No Such Typeface 4711".into(), code_font: "DejaVu Sans Mono".into(), ..Layout::default() }),
        )
        .unwrap();
    assert_eq!(made.missing_fonts, ["No Such Typeface 4711"]);
    assert!(session.export_document(FileFormat::Pdf, &folder.path("standard.pdf"), &text, None, &PdfChoice::default()).unwrap().missing_fonts.is_empty());

    // two columns, 14 pt and a wide inner margin: more pages for the same text, and it still is a PDF
    let (_, wide) = exported(&session, &folder, "book.pdf", &text, &with(book_layout()));
    assert!(wide.starts_with(b"%PDF-"));
    assert!(pages("book.pdf").len() > standard.len());
    let (_, columns) = exported(&session, &folder, "columns.pdf", &text, &with(Layout { two_columns: true, font_size: Some(8), ..Layout::default() }));
    assert!(columns.starts_with(b"%PDF-"));
}

#[test]
fn a_pdf_with_latex_has_page_numbers_where_asked_a_size_of_its_own_and_the_headings_type() {
    let Some(engine) = ["xelatex", "lualatex"].into_iter().find(|engine| find_program(engine).is_some() && find_program("pandoc").is_some()) else {
        eprintln!("no XeLaTeX or LuaLaTeX is installed: the layout was not tried with LaTeX");
        return;
    };
    let folder = Folder::new("latex-more");
    let text = format!("---\ntitle: Walking breaks\n---\n\n# Method\n\n{}\n\n## Sample\n\n{}\n", LONG.repeat(150), LONG.repeat(150));
    std::fs::write(folder.path("thesis.md"), &text).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    let pages = |name: &str| pdf_extract::extract_text_by_pages(folder.path(name)).unwrap();
    let edges =
        |name: &str| pages(name).iter().map(|page| (page.trim().chars().take(6).collect::<String>(), page.trim().chars().rev().take(3).collect::<String>())).collect::<Vec<_>>();
    let with = |layout: Layout| PdfChoice { engine: Some(engine.into()), layout, ..PdfChoice::default() };
    // the standard: a number at the bottom of every page
    exported(&session, &folder, "standard.pdf", &text, &with(Layout::default()));
    let standard = edges("standard.pdf");
    assert!(standard[0].1.starts_with('1') && standard[1].1.starts_with('2'), "{standard:?}");

    exported(&session, &folder, "off.pdf", &text, &with(Layout { page_numbers: Some(PageNumbers::Off), ..Layout::default() }));
    assert!(edges("off.pdf").iter().all(|(_, end)| !end.starts_with(|c: char| c.is_ascii_digit())), "{:?}", edges("off.pdf"));
    // at the top, and the page with the title has none
    exported(&session, &folder, "top.pdf", &text, &with(Layout { page_numbers: Some(PageNumbers::TopOuter), no_number_on_first_page: true, ..Layout::default() }));
    let top = edges("top.pdf");
    assert!(top[0].0.starts_with("Walkin") && !top[0].1.starts_with('1') && top[1].0.starts_with('2'), "{top:?}");

    // 14 pt, which the standard class does not have, two-sided in two columns, the headings in the system's own typeface
    let (_, big) = exported(&session, &folder, "book.pdf", &text, &with(Layout { heading_font: "Courier New".into(), ..book_layout() }));
    assert!(big.starts_with(b"%PDF-"));
    // (that the headings are in their typeface is not read from the PDF here: XeLaTeX packs the fonts' names)
    assert!(pages("book.pdf").len() > standard.len());
}
