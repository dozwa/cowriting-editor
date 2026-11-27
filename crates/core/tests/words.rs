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

// ------------------------------------------------------------ the count of a selection

use cowriting_core::words::{count_selection, Counted};

/// The count of the part of `marked` between `[[` and `]]`, which are not part of the text.
fn selected(marked: &str) -> (usize, usize, usize) {
    let (from, to) = (marked.find("[[").unwrap(), marked.find("]]").unwrap() - 2);
    let text = marked.replacen("[[", "", 1).replacen("]]", "", 1);
    let units = |bytes: usize| text[..bytes].encode_utf16().count();
    let Counted { words, chars } = count_selection(&text, units(from), units(to));
    (words, chars.with_spaces, chars.without_spaces)
}

#[test]
fn a_selection_of_plain_text_counts_its_words_and_characters() {
    assert_eq!(selected("The [[sample was small]]. More follows."), (3, 16, 14));
    // the whole text selected is the count of the document
    let text = "---\ntitle: Try\n---\n\n# Heading\n\nA first **bold** line with a [link](https://example.org/long/address).\n\n<!-- aside -->\n\nA third line.\n";
    let all = count_selection(text, 0, text.encode_utf16().count());
    assert_eq!((all.words, all.chars), (count_words(text, Format::Markdown), count_chars(text, Format::Markdown)));
    // nothing selected, or the ends the other way round
    assert_eq!(selected("The [[]]sample."), (0, 0, 0));
    assert_eq!(count_selection("The sample was small.", 10, 4), count_selection("The sample was small.", 4, 10));
    // positions are counted as the editor counts them: an emoji is two units there, and one character here
    assert_eq!(selected("Größe 🙂 [[zählt 🙂 hier]] nicht."), (2, 12, 10));
}

#[test]
fn what_the_document_does_not_count_a_selection_does_not_count_either() {
    // inside a comment, inside code, and the front matter
    assert_eq!(selected("Text. <!-- a [[note to]] myself --> More."), (0, 0, 0));
    assert_eq!(selected("Text.\n\n```\nlet [[x = 1]];\n```\n\nMore."), (0, 0, 0));
    assert_eq!(selected("[[---\ntitle: My thesis\n---\n\nOne]] word."), (1, 3, 3));
    // a comment inside the selection is passed over, and marks are no characters
    assert_eq!(selected("[[Two <!-- hidden words --> **words**]] and more."), (2, 9, 8));
    assert_eq!(selected("## [[A heading]]\n\n- [[]]item"), (2, 9, 8));
    // a citation's key is not a word
    assert_eq!(selected("[[Breaks help [@smith2020] a lot]]."), (4, 17, 14));
}

#[test]
fn a_selection_that_cuts_through_a_link_counts_the_part_of_its_text_it_holds() {
    // half of the link's text
    assert_eq!(selected("See [the [[long]] report](https://example.org/a/b) here."), (1, 4, 4));
    // from inside the text into the address: of the characters, the text's part only
    let (_, with_spaces, without_spaces) = selected("See [the long [[report](https://exam]]ple.org/a/b) here.");
    assert_eq!((with_spaces, without_spaces), (6, 6));
    // the address alone has no characters; from the address on into what follows, only what follows
    let (_, with_spaces, _) = selected("See [the long report](https://[[example.org]]/a/b) here.");
    assert_eq!(with_spaces, 0);
    let (_, with_spaces, without_spaces) = selected("See [the long report](https://exam[[ple.org/a/b) here]].");
    assert_eq!((with_spaces, without_spaces), (4, 4));
    // a picture is not text
    let (_, with_spaces, without_spaces) = selected("[[![a plot](images/plot.png) shows it]].");
    assert_eq!((with_spaces, without_spaces), (8, 7));
}

#[test]
fn the_words_of_a_selection_are_counted_as_the_documents_are_so_the_two_numbers_agree() {
    // the count of words is the plugin's and takes an address for a word; a selection does the same,
    // or the parts of a text would not add up to the whole
    let text = "See [the long report](https://example.org/a/b) here.";
    let whole = count_words(text, Format::Markdown);
    let units = text.encode_utf16().count();
    // "report](https://example.org/a/b)" is one word there: five in all, cut at a space
    let (first, second) = (count_selection(text, 0, 13).words, count_selection(text, 13, units).words);
    assert_eq!((whole, first, second), (5, 3, 2));
    // the address alone is that word's rest: a word that is cut counts on both sides of the cut
    assert_eq!(count_selection(text, 30, 41).words, 1);
}
