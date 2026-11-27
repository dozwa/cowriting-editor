//! Counting characters: what a call for papers means by "20,000 characters including spaces".

use cowriting_core::words::{count_chars, count_words, Chars, Format};

fn chars(text: &str) -> (usize, usize) {
    let Chars { with_spaces, without_spaces } = count_chars(text, Format::Markdown);
    (with_spaces, without_spaces)
}

#[test]
fn plain_prose_is_counted_with_and_without_spaces() {
    assert_eq!(chars("One two."), (8, 7));
    assert_eq!(chars(""), (0, 0));
    assert_eq!(chars("  \n\n "), (0, 0));
}

#[test]
fn a_run_of_spaces_and_line_ends_is_one_space() {
    assert_eq!(chars("One   two.\n\nThree\tfour.  \n"), chars("One two. Three four."));
    assert_eq!(chars("One two. Three four."), (20, 17));
}

#[test]
fn front_matter_code_comments_and_citation_keys_are_left_out_as_for_words() {
    let text = "---\ntitle: A long title\n---\n\nOne <!-- not this --> two [@smith2020; @lee2019].\n\n```\ncode here\n```\n";
    assert_eq!(chars(text), chars("One two ."));
    assert_eq!(count_words(text, Format::Markdown), 2);
}

#[test]
fn marks_are_left_out_where_they_are_marks() {
    assert_eq!(chars("# Title\n\n## Heading"), chars("Title Heading"));
    assert_eq!(chars("**bold**, *italic*, _also_ and __strong__, `code` and ~~gone~~"), chars("bold, italic, also and strong, code and gone"));
    assert_eq!(chars("- one\n- two\n\n1. three\n2) four\n\n> quoted\n> > twice"), chars("one two three four quoted twice"));
    assert_eq!(chars("| Group | Mean |\n|-------|:----:|\n| A     | 4.2  |"), chars("Group Mean A 4.2"));
    assert_eq!(chars("Before.\n\n---\n\nAfter.\n\n* * *"), chars("Before. After."));
    assert_eq!(chars("A line<br>break and <span class=\"x\">a span</span>."), chars("A line break and a span ."));
}

#[test]
fn the_authors_own_hyphens_stars_and_numbers_stay() {
    assert_eq!(chars("A well-known snake_case name, 3 - 2 = 1."), (40, 32));
    assert_eq!(chars("In 2020. A year."), (16, 13));
    // escaped, a star and a bar are characters of the text; the backslash is not
    assert_eq!(chars(r"A star \* and a bar \| stay."), (26, 19));
}

#[test]
fn of_a_link_the_text_counts_and_of_a_picture_nothing() {
    assert_eq!(chars("See [the report](https://example.org/a/very/long/address) here."), chars("See the report here."));
    assert_eq!(chars("Before. ![A description](figures/plot.png) After."), chars("Before. After."));
    assert_eq!(chars("A claim.[^1] More.\n\n[^1]: The note."), chars("A claim. More. The note."));
    assert_eq!(chars("A claim.^[The note.] More."), chars("A claim. The note. More."));
}

#[test]
fn a_letter_is_one_character_however_it_is_stored() {
    // "ä" as one code point and as "a" with a combining mark
    assert_eq!(chars("M\u{e4}dchen"), (7, 7));
    assert_eq!(chars("Ma\u{308}dchen"), (7, 7));
    // a family of four people, joined
    assert_eq!(chars("Hi \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"), (4, 3));
}

#[test]
fn latex_is_counted_without_its_commands() {
    let Chars { with_spaces, without_spaces } = count_chars("\\section{Intro}\nSome \\emph{text} here. % not this\n", Format::Latex);
    assert_eq!((with_spaces, without_spaces), (21, 18));
}
