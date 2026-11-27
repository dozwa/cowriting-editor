//! A picture the author chooses for the document: how the document names it, and the copy beside it.

use cowriting_app::picture::{copy_beside, relative, seen_from, written, Picture};
use cowriting_app::Session;
use cowriting_core::host::DiskHost;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-picture-{name}-{}", std::process::id()));
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

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pictures/picture.png")
}

#[test]
fn a_picture_is_named_by_its_way_from_the_documents_folder() {
    let named = |folder: &str, file: &str| {
        let Picture { path, is_inside, .. } = seen_from(folder, file);
        (path, is_inside)
    };
    assert_eq!(named("/work/paper", "/work/paper/plot.png"), ("plot.png".into(), true));
    assert_eq!(named("/work/paper/", "/work/paper/figures/plot.png"), ("figures/plot.png".into(), true));
    assert_eq!(named("/work/paper", "/work/shared/plot.png"), ("../shared/plot.png".into(), false));
    assert_eq!(named("/work/paper", "/home/me/plot.png"), ("../../home/me/plot.png".into(), false));
    // a folder whose name only starts like the document's is another folder
    assert_eq!(named("/work/paper", "/work/paper-2/plot.png"), ("../paper-2/plot.png".into(), false));
    // a space or a bracket would end the path in markdown: it goes in angle brackets
    assert_eq!(named("/work/paper", "/work/paper/my figures/plot (1).png"), ("<my figures/plot (1).png>".into(), true));
    assert_eq!(seen_from("/work/paper", "/work/paper/figures/plot.png").name, "plot.png");
}

#[test]
fn on_windows_the_way_has_forward_slashes_and_another_drive_is_named_in_full() {
    assert_eq!(relative(r"C:\Users\me\paper", r"C:\Users\me\paper\figures\plot.png").as_deref(), Some("figures/plot.png"));
    // names are the same there whatever their case
    assert_eq!(relative(r"C:\Users\me\paper", r"c:\users\ME\shared\plot.png").as_deref(), Some("../shared/plot.png"));
    assert_eq!(relative(r"C:\Users\me\paper", r"D:\pictures\plot.png"), None);
    let elsewhere = seen_from(r"C:\Users\me\paper", r"D:\my pictures\plot.png");
    assert_eq!((elsewhere.path.as_str(), elsewhere.is_inside), ("<D:/my pictures/plot.png>", false));
    assert_eq!(written("a<b>.png"), "<a%3Cb%3E.png>");
}

#[test]
fn a_copy_goes_into_images_beside_the_document_and_the_picture_stays_as_it_is() {
    let (folder, elsewhere) = (Folder::new("copy"), Folder::new("copy-elsewhere"));
    std::fs::write(folder.path("thesis.md"), "# Pictures\n").unwrap();
    let source = elsewhere.path("my plot.png");
    std::fs::copy(fixture(), &source).unwrap();
    let original = std::fs::read(&source).unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();

    // seen from the document it lies outside; nothing is copied by asking
    let seen = session.picture(&source).unwrap();
    assert!(!seen.is_inside && seen.path.starts_with("<../") && seen.path.ends_with("/my plot.png>"), "{seen:?}");
    assert!(!Path::new(&folder.path("images")).exists());

    let copied = session.copy_picture(&source).unwrap();
    assert_eq!(copied, Picture { path: "<images/my plot.png>".into(), name: "my plot.png".into(), is_inside: true });
    assert_eq!(std::fs::read(folder.path("images/my plot.png")).unwrap(), original);
    // the same picture again is the same file
    assert_eq!(session.copy_picture(&source).unwrap().path, "<images/my plot.png>");
    assert_eq!(std::fs::read_dir(folder.path("images")).unwrap().count(), 1);

    // another picture of the same name gets a number, and the one that was there is kept
    let other = Folder::new("copy-other");
    std::fs::write(other.path("my plot.png"), b"another picture").unwrap();
    assert_eq!(session.copy_picture(&other.path("my plot.png")).unwrap().path, "<images/my plot-2.png>");
    assert_eq!(std::fs::read(folder.path("images/my plot.png")).unwrap(), original);
    assert_eq!(std::fs::read(folder.path("images/my plot-2.png")).unwrap(), b"another picture");

    assert_eq!(std::fs::read(&source).unwrap(), original, "the picture itself is untouched");
    assert_eq!(std::fs::read_to_string(folder.path("thesis.md")).unwrap(), "# Pictures\n");
    // copy_beside is what the session calls
    assert_eq!(copy_beside(&folder.path(""), &source).unwrap().path, "<images/my plot.png>");
}

#[test]
fn what_is_no_picture_or_not_there_is_refused_in_plain_words() {
    let folder = Folder::new("refused");
    std::fs::write(folder.path("thesis.md"), "# Pictures\n").unwrap();
    std::fs::write(folder.path("notes.txt"), "no picture").unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    assert!(session.picture(&folder.path("notes.txt")).unwrap_err().contains("is not a picture the export can use"));
    assert!(session.copy_picture(&folder.path("gone.png")).unwrap_err().contains("is not there"));
    assert!(!Path::new(&folder.path("images")).exists());
}

#[test]
fn a_copied_picture_is_in_the_export() {
    use cowriting_app::export::find_program;
    use cowriting_app::{FileFormat, PdfChoice};
    let (folder, elsewhere) = (Folder::new("export"), Folder::new("export-elsewhere"));
    std::fs::copy(fixture(), elsewhere.path("my plot (1).png")).unwrap();
    std::fs::write(folder.path("thesis.md"), "# Pictures\n").unwrap();
    let (session, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    let linked = session.picture(&elsewhere.path("my plot (1).png")).unwrap();
    let copied = session.copy_picture(&elsewhere.path("my plot (1).png")).unwrap();
    // as the editor writes them: one where it lies, one copied
    let text = format!("# Pictures\n\n![linked]({})\n\n![copied]({})\n", linked.path, copied.path);
    if find_program("pandoc").is_none() {
        assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "pandoc is not there: run scripts/fetch-tools.py");
        eprintln!("pandoc is not installed: the export of a chosen picture was not run");
        return;
    }
    let made = session.export_document(FileFormat::Docx, &folder.path("thesis.docx"), &text, None, &PdfChoice::default()).unwrap();
    assert!(made.warnings.is_empty(), "{:?}", made.warnings);
    assert!(String::from_utf8_lossy(&std::fs::read(folder.path("thesis.docx")).unwrap()).contains("word/media/"), "the Word file holds pictures");
    let made = session.export_document(FileFormat::Pdf, &folder.path("thesis.pdf"), &text, None, &PdfChoice::default());
    match made {
        Ok(made) => assert!(made.warnings.is_empty(), "{:?}", made.warnings),
        Err(why) => assert!(std::env::var_os("COWRITING_NEED_TOOLS").is_none(), "the PDF was not made: {why}"),
    }
}
