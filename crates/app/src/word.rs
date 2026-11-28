//! How a Word file is set. pandoc gives a Word file the styles, the page and the header and
//! footer of another Word file it is shown (`--reference-doc`). The export makes that file from
//! the layout the author chose for the project: pandoc's own reference file, with the typefaces,
//! the size, the spacing, the page and the page numbers written into it. Nothing here reads or
//! writes the author's document.

use crate::export::{find_program, Layout, PageNumbers, Paper};
use std::io::{Cursor, Read, Write};
use std::process::{Command, Stdio};

const MAIN: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// The Word file pandoc takes its styles from when it is shown no other.
fn pandocs_own() -> Result<Vec<u8>, String> {
    let pandoc = find_program("pandoc").ok_or("pandoc is not installed. Install it from pandoc.org, then export again.")?;
    let mut command = Command::new(pandoc);
    command.args(["--print-default-data-file", "reference.docx"]).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // no console window
    }
    let output = command.output().map_err(|e| format!("pandoc could not be started: {e}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err("pandoc did not give its Word styles, so the layout could not be written into them.".into());
    }
    Ok(output.stdout)
}

/// A Word file whose styles and page are this layout's, for pandoc to take them from.
pub fn reference_doc(layout: &Layout, paper: Paper) -> Result<Vec<u8>, String> {
    styled(&pandocs_own()?, layout, paper)
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Centimetres in the twentieths of a point Word counts in.
fn twips(cm: f32) -> i64 {
    (cm * 566.93).round() as i64
}

/// One typeface for every script, in place of the theme's.
fn fonts(name: &str) -> String {
    let name = escaped(name);
    format!("<w:rFonts w:ascii=\"{name}\" w:hAnsi=\"{name}\" w:eastAsia=\"{name}\" w:cs=\"{name}\" />")
}

/// Replaces every `<w:rFonts … />` that carries `mark` between `from` and `to`.
fn set_fonts(xml: &str, mark: &str, font: &str, from: usize, to: usize) -> String {
    let (mut out, mut at) = (xml[..from].to_string(), from);
    while let Some(start) = xml[at..to].find("<w:rFonts").map(|found| at + found) {
        let end = start + xml[start..].find("/>").map_or(0, |found| found + 2);
        out.push_str(&xml[at..start]);
        out.push_str(&if xml[start..end].contains(mark) { fonts(font) } else { xml[start..end].to_string() });
        at = end;
    }
    out + &xml[at..]
}

/// `word/styles.xml` with the layout's typefaces, size, line spacing and paragraphs.
fn styles(xml: &str, layout: &Layout) -> String {
    // pandoc for Windows has the file with Windows line ends, and one of the places below spans lines
    let mut xml = xml.replace("\r\n", "\n");
    let defaults_end = xml.find("</w:docDefaults>").unwrap_or(0);
    if !layout.font.is_empty() {
        xml = set_fonts(&xml, "minorHAnsi", &layout.font, 0, defaults_end);
    }
    // the headings are in their own typeface, or in the text's as in a PDF
    let headings = if layout.heading_font.is_empty() { &layout.font } else { &layout.heading_font };
    if !headings.is_empty() {
        let end = xml.len();
        xml = set_fonts(&xml, "majorHAnsi", headings, 0, end);
    }
    if !layout.code_font.is_empty() {
        xml = xml.replace("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" />", &fonts(&layout.code_font));
    }
    if let Some(size) = layout.font_size {
        // Word counts in half points; the first two are the text's, the headings keep their own
        let half = u32::from(size) * 2;
        xml = xml.replacen("<w:sz w:val=\"24\" />", &format!("<w:sz w:val=\"{half}\" />"), 1).replacen("<w:szCs w:val=\"24\" />", &format!("<w:szCs w:val=\"{half}\" />"), 1);
    }
    if let Some(spacing) = layout.line_spacing {
        // for every paragraph: 240 is a single line
        xml = xml.replacen("<w:spacing w:after=\"200\" />", &format!("<w:spacing w:after=\"200\" w:line=\"{}\" w:lineRule=\"auto\" />", (spacing * 240.0).round() as i64), 1);
    }
    if layout.indent {
        // the text's paragraphs begin with an indent and stand close; the first one after a heading has none
        xml = xml.replacen("<w:spacing w:before=\"180\" w:after=\"180\" />", "<w:spacing w:before=\"0\" w:after=\"0\" />\n      <w:ind w:firstLine=\"340\" />", 1);
        xml = xml.replacen(
            "<w:next w:val=\"BodyText\" />\n    <w:qFormat />\n  </w:style>",
            "<w:next w:val=\"BodyText\" />\n    <w:qFormat />\n    <w:pPr>\n      <w:ind w:firstLine=\"0\" />\n    </w:pPr>\n  </w:style>",
            1,
        );
    }
    xml
}

/// A header or a footer that holds the page's number.
fn numbered_part(tag: &str, align: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:{tag} xmlns:w=\"{MAIN}\"><w:p><w:pPr><w:jc w:val=\"{align}\" /></w:pPr>\
         <w:r><w:fldChar w:fldCharType=\"begin\" /></w:r><w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\" /></w:r>\
         <w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\" /></w:r></w:p></w:{tag}>"
    )
}

/// pandoc's reference file with the layout written into it.
pub fn styled(reference: &[u8], layout: &Layout, paper: Paper) -> Result<Vec<u8>, String> {
    let layout = layout.within_limits();
    let broken = |e: zip::result::ZipError| format!("pandoc's Word styles could not be read: {e}");
    let mut archive = zip::ZipArchive::new(Cursor::new(reference)).map_err(broken)?;
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(broken)?;
        let mut content = Vec::new();
        file.read_to_end(&mut content).map_err(|e| format!("pandoc's Word styles could not be read: {e}"))?;
        parts.push((file.name().to_string(), content));
    }
    let mut change = |name: &str, with: &dyn Fn(&str) -> String| {
        if let Some((_, content)) = parts.iter_mut().find(|(part, _)| part == name) {
            *content = with(&String::from_utf8_lossy(content)).into_bytes();
        }
    };
    change("word/styles.xml", &|xml| styles(xml, &layout));

    // the page: its size, its margins (Word's own inch where none is set), its columns
    let (width, height) = match paper {
        Paper::A4 => (11906, 16838),
        Paper::Letter => (12240, 15840),
    };
    let side = |own: Option<f32>| own.or(layout.margin_cm).map_or(1440, twips);
    let mut page = format!(
        "<w:pgSz w:w=\"{width}\" w:h=\"{height}\" /><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\" />",
        side(layout.margin_top_cm),
        side(layout.margin_right_cm),
        side(layout.margin_bottom_cm),
        side(layout.margin_left_cm)
    );
    if layout.two_columns {
        page.push_str("<w:cols w:num=\"2\" w:space=\"708\" />");
    }
    // page numbers: a header or footer of their own, a second one for the left pages when the number stands outside
    let (mut references, mut relations, mut types, mut added): (String, String, String, Vec<(String, Vec<u8>)>) = Default::default();
    let placed = layout.page_numbers.filter(|place| *place != PageNumbers::Off);
    let has_even = placed.is_some_and(|place| place != PageNumbers::BottomCentre) && layout.two_sided;
    if let Some(place) = placed {
        let (tag, kind) = if place == PageNumbers::TopOuter { ("hdr", "header") } else { ("ftr", "footer") };
        let align = if place == PageNumbers::BottomCentre { "center" } else { "right" };
        let sides: &[(&str, &str)] = if has_even { &[("default", align), ("even", "left")] } else { &[("default", align)] };
        for (which, align) in sides {
            let (id, file) = (format!("rIdCowriting{which}"), format!("{kind}-cowriting-{which}.xml"));
            references.push_str(&format!("<w:{kind}Reference w:type=\"{which}\" r:id=\"{id}\" />"));
            relations.push_str(&format!("<Relationship Type=\"{RELATIONSHIPS}/{kind}\" Id=\"{id}\" Target=\"{file}\" />"));
            types.push_str(&format!("<Override PartName=\"/word/{file}\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.{kind}+xml\" />"));
            added.push((format!("word/{file}"), numbered_part(tag, align).into_bytes()));
        }
        if layout.no_number_on_first_page {
            // the first page has a header and footer of its own, and there is none
            page.push_str("<w:titlePg />");
        }
    }
    change("word/document.xml", &|xml| {
        let xml = if xml.contains("xmlns:r=") { xml.to_string() } else { xml.replacen("<w:document ", &format!("<w:document xmlns:r=\"{RELATIONSHIPS}\" "), 1) };
        match xml.rfind("</w:footnotePr>").filter(|at| xml[..*at].contains("<w:sectPr>")) {
            Some(at) => xml[..at + 15].replacen("<w:sectPr>", &format!("<w:sectPr>{references}"), 1) + &page + &xml[at + 15..],
            None => xml.replacen("<w:sectPr>", &format!("<w:sectPr>{references}{page}"), 1),
        }
    });
    change("word/_rels/document.xml.rels", &|xml| xml.replacen("</Relationships>", &format!("{relations}</Relationships>"), 1));
    change("[Content_Types].xml", &|xml| xml.replacen("</Types>", &format!("{types}</Types>"), 1));
    change("word/settings.xml", &|xml| {
        let mut xml = xml.to_string();
        if layout.two_sided {
            xml = xml.replacen("<w:embedSystemFonts />", "<w:embedSystemFonts />\n  <w:mirrorMargins />", 1);
        }
        if has_even {
            xml = xml.replacen("<w:defaultTabStop w:val=\"720\" />", "<w:defaultTabStop w:val=\"720\" />\n  <w:evenAndOddHeaders />", 1);
        }
        xml
    });
    parts.extend(added);

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in &parts {
        writer.start_file(name.as_str(), options).map_err(|e| format!("The Word styles could not be written: {e}"))?;
        writer.write_all(content).map_err(|e| format!("The Word styles could not be written: {e}"))?;
    }
    Ok(writer.finish().map_err(|e| format!("The Word styles could not be written: {e}"))?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_paragraph_after_a_heading_has_no_indent_whatever_line_ends_the_styles_have() {
        let style = "<w:style w:type=\"paragraph\" w:styleId=\"FirstParagraph\">\n    <w:next w:val=\"BodyText\" />\n    <w:qFormat />\n  </w:style>";
        let layout = Layout { indent: true, ..Layout::default() };
        for xml in [style.to_string(), style.replace('\n', "\r\n")] {
            assert!(styles(&xml, &layout).contains("<w:ind w:firstLine=\"0\" />"), "{xml:?}");
        }
    }
}
