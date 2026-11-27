//! Layouts kept under a name: used again in another project, and passed on as a file.

use cowriting_app::export::{Layout, PageNumbers};
use cowriting_app::{Paper, PdfChoice, Session};
use cowriting_core::host::DiskHost;
use std::path::PathBuf;
use std::sync::Arc;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("cowriting-layouts-{name}-{}", std::process::id()));
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

fn session(folder: &Folder, document: &str, settings: &str) -> Session {
    std::fs::write(folder.path(document), "# Text\n").unwrap();
    Session::open_in(Arc::new(DiskHost), &folder.path(document), Some(&folder.path(&format!("{settings}/projects")))).unwrap().0
}

#[test]
fn a_layout_kept_under_a_name_is_there_for_every_project_and_goes_to_another_computer_as_a_file() {
    let folder = Folder::new("kept");
    let thesis = session(&folder, "thesis.md", "mine");
    assert!(thesis.layouts().is_empty());

    let layout = Layout { font: "Times New Roman".into(), font_size: Some(12), margin_cm: Some(2.5), page_numbers: Some(PageNumbers::BottomOuter), ..Layout::default() };
    let choice = PdfChoice { engine: Some("xelatex".into()), paper: Paper::Letter, layout: layout.clone(), word_template: "/somewhere/faculty.docx".into() };
    assert_eq!(thesis.keep_layout("Thesis, faculty of medicine", &choice).unwrap(), "Thesis, faculty of medicine");
    thesis.keep_layout("abstract", &PdfChoice::default()).unwrap();
    // another project on the same computer has them too, by name
    let paper = session(&folder, "paper.md", "mine");
    let kept = paper.layouts();
    assert_eq!(kept.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["abstract", "Thesis, faculty of medicine"]);
    assert_eq!((kept[1].paper, &kept[1].layout), (Paper::Letter, &layout));
    // the file holds the paper and the layout, and neither the typesetter nor the path of a template on this computer
    let file = std::fs::read_to_string(folder.path("mine/layouts/Thesis, faculty of medicine.json")).unwrap();
    assert!(file.contains("\"schema\": 1") && file.contains("\"marginCm\": 2.5") && !file.contains("xelatex") && !file.contains("faculty.docx"), "{file}");
    assert!(!std::path::Path::new(&folder.path(".cowriting")).exists(), "nothing appears beside the documents");

    // a name that could leave the folder, or is none, is refused
    for name in ["../outside", "", "   ", "a/b", ".hidden"] {
        assert!(thesis.keep_layout(name, &choice).is_err(), "{name:?}");
    }
    // the same name again replaces the layout
    thesis.keep_layout("abstract", &PdfChoice { layout: Layout { two_columns: true, ..Layout::default() }, ..PdfChoice::default() }).unwrap();
    assert!(thesis.layouts()[0].layout.two_columns);

    // passed on as a file, and taken in on a computer with a folder of its own
    thesis.export_layout("Thesis, faculty of medicine", &folder.path("Medicine.json")).unwrap();
    assert!(thesis.export_layout("Thesis, faculty of medicine", &folder.path("Medicine.txt")).is_err());
    let colleague = session(&folder, "other.md", "theirs");
    assert_eq!(colleague.import_layout(&folder.path("Medicine.json")).unwrap(), "Medicine");
    assert_eq!((colleague.layouts()[0].paper, &colleague.layouts()[0].layout), (Paper::Letter, &layout));
    // the same file again is the same layout; another layout under that name gets a number
    assert_eq!(colleague.import_layout(&folder.path("Medicine.json")).unwrap(), "Medicine");
    std::fs::write(folder.path("Medicine.json"), r#"{"layout": {"marginCm": 99, "font": "Arial\nevil: yes"}}"#).unwrap();
    assert_eq!(colleague.import_layout(&folder.path("Medicine.json")).unwrap(), "Medicine 2");
    let second = &colleague.layouts()[1];
    assert_eq!(
        (second.layout.margin_cm, second.layout.font.as_str(), second.paper),
        (Some(6.0), "Arialevil yes", Paper::A4),
        "what a file from elsewhere says is held within limits"
    );
    // a file that is no layout is said to be none, and nothing is kept
    std::fs::write(folder.path("notes.json"), r#"{"title": "not a layout"}"#).unwrap();
    std::fs::write(folder.path("broken.json"), "{").unwrap();
    for file in ["notes.json", "broken.json", "missing.json"] {
        assert!(colleague.import_layout(&folder.path(file)).is_err(), "{file}");
    }
    assert_eq!(colleague.layouts().len(), 2);

    thesis.remove_layout("abstract").unwrap();
    assert_eq!(paper.layouts().len(), 1);
    assert!(thesis.remove_layout("abstract").is_err());
    // without a folder of the editor's own there is nowhere to keep one
    let (bare, _) = Session::open(Arc::new(DiskHost), &folder.path("thesis.md")).unwrap();
    assert!(bare.layouts().is_empty() && bare.keep_layout("x", &choice).is_err());
}
