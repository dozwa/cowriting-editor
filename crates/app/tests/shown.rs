//! Showing a written file: only what the editor wrote, only while it is there.

use cowriting_app::shown::{command_for, How, Written};

#[test]
fn only_a_file_the_editor_wrote_and_that_is_still_there_is_shown() {
    let folder = std::env::temp_dir().join(format!("cowriting-shown-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join(".cowriting")).unwrap();
    let written = folder.join(".cowriting").join("ai register.en.md");
    let other = folder.join("someone else's.txt");
    std::fs::write(&written, "x").unwrap();
    std::fs::write(&other, "x").unwrap();
    let (written, other) = (written.to_string_lossy().into_owned(), other.to_string_lossy().into_owned());

    let list = Written::default();
    assert!(list.command(&written, How::Folder).is_err(), "nothing is shown before it was written");
    list.add(&written);
    list.add(&written);
    let (_, args) = list.command(&written, How::Folder).unwrap();
    assert!(args.iter().any(|arg| arg.contains("ai register.en.md") || arg.ends_with(".cowriting")), "{args:?}");
    assert!(list.command(&other, How::Open).is_err(), "a file the editor did not write is not opened");
    assert!(list.command("/bin/sh", How::Open).is_err());

    std::fs::remove_file(&written).unwrap();
    assert!(list.command(&written, How::Open).unwrap_err().contains("no longer there"));
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn each_system_gets_the_path_as_one_argument() {
    let path = "/home/a b/.cowriting/t.md/disclosure/reg -R.md";
    assert_eq!(command_for("macos", path, How::Folder), ("open".to_string(), vec!["-R".to_string(), path.to_string()]));
    assert_eq!(command_for("macos", path, How::Open).1, [path]);
    assert_eq!(command_for("linux", path, How::Folder), ("xdg-open".to_string(), vec!["/home/a b/.cowriting/t.md/disclosure".to_string()]));
    assert_eq!(command_for("linux", path, How::Open).1, [path]);
    assert_eq!(command_for("windows", r"C:\a b\x.pdf", How::Folder).1, [r"/select,C:\a b\x.pdf"]);
    assert_eq!(command_for("windows", r"C:\a b\x.pdf", How::Open).1.last().unwrap(), r"C:\a b\x.pdf");
    // the editor's own folders are joined on with "/": Explorer finds the file only when every separator is Windows' own
    let mixed = r"C:\Users\a b\proben/.cowriting/beispiel.md/disclosure/ai-register.de.tex";
    let native = r"C:\Users\a b\proben\.cowriting\beispiel.md\disclosure\ai-register.de.tex";
    assert_eq!(command_for("windows", mixed, How::Folder), ("explorer".to_string(), vec![format!("/select,{native}")]));
    assert_eq!(command_for("windows", mixed, How::Open).1, ["url.dll,FileProtocolHandler", native]);
    // elsewhere a backslash is a character of the name like any other
    assert_eq!(command_for("macos", "/home/a\\b/x.pdf", How::Open).1, ["/home/a\\b/x.pdf"]);
}
