//! A report is handed to the browser or the mail program as an address; this is what is in it.

use cowriting_app::feedback::{address, has_mail, LONGEST_TEXT, LONGEST_TITLE};

#[test]
fn a_report_arrives_as_written_and_only_where_the_editor_says() {
    let url = address("issue", " Export fails & stops ", "Schöne Grüße\n1 + 1 = 2?  #1").unwrap();
    assert!(url.starts_with("https://github.com/dozwa/cowriting-editor/issues/new?title="), "{url}");
    assert_eq!(url.split_once("?title=").unwrap().1, "Export%20fails%20%26%20stops&body=Sch%C3%B6ne%20Gr%C3%BC%C3%9Fe%0A1%20%2B%201%20%3D%202%3F%20%20%231");
    // nothing the author wrote can add a field or change where it goes
    assert_eq!(url.matches('&').count(), 1);
    assert_eq!(url.matches('?').count(), 1);

    // no other way, nothing empty, nothing too long for an address
    assert!(address("https://elsewhere.example", "t", "x").is_err());
    assert!(address("issue", "  ", "x").is_err());
    assert!(address("issue", "t", "\n").is_err());
    assert!(address("issue", &"t".repeat(LONGEST_TITLE), &"ß".repeat(LONGEST_TEXT)).is_ok());
    assert!(address("issue", "t", &"x".repeat(LONGEST_TEXT + 1)).is_err());
    assert!(address("issue", &"t".repeat(LONGEST_TITLE + 1), "x").is_err());

    // mail only once there is an address to write to
    match address("mail", "t", "one\ntwo") {
        Ok(mail) => assert!(has_mail() && mail.starts_with("mailto:") && mail.ends_with("body=one%0D%0Atwo"), "{mail}"),
        Err(_) => assert!(!has_mail()),
    }
}
