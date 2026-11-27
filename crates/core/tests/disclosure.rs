//! The register of a document written in the editor (`Profile::Editor`), where it says something
//! else than the plugin's: the golden tests hold the plugin's wording.

use cowriting_core::disclosure::{disclosure, DocFormat, Lang, Profile};
use cowriting_core::log::parse_log;
use cowriting_core::types::LogEntry;
use std::path::PathBuf;

fn full_log() -> Vec<LogEntry> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl");
    parse_log(&std::fs::read_to_string(path).unwrap())
}

fn register(lang: Lang, format: DocFormat) -> String {
    disclosure(&full_log(), "thesis.md", lang, format, Profile::Editor)
}

#[test]
fn requests_of_one_day_have_a_date_and_those_of_several_a_range() {
    let (english, german) = (register(Lang::En, DocFormat::Md), register(Lang::De, DocFormat::Md));
    assert!(english.contains("| 2 requests, 2026-10-07 |") && english.contains("| 2 requests, 2026-10-01 to 2026-10-07 |"), "{english}");
    assert!(german.contains("| 2 Anfragen, 2026-10-07 |") && german.contains("| 2 Anfragen, 2026-10-01 bis 2026-10-07 |"), "{german}");
    assert!(!english.contains("2026-10-07 to 2026-10-07") && !german.contains("2026-10-07 bis 2026-10-07"));
    assert!(register(Lang::De, DocFormat::Tex).contains("2 Anfragen, 2026-10-07 &"));
}

#[test]
fn a_heading_in_latex_is_set_ragged_and_the_german_one_on_the_log_is_short() {
    let german = register(Lang::De, DocFormat::Tex);
    assert!(german.contains("\\subsection*{\\raggedright Rückgängig gemachte Änderungen am KI-Protokoll}"), "{german}");
    assert!(register(Lang::De, DocFormat::Md).contains("## Rückgängig gemachte Änderungen am KI-Protokoll\n"));
    for text in [german, register(Lang::En, DocFormat::Tex)] {
        let headings: Vec<&str> = text.lines().filter(|line| line.starts_with("\\subsection")).collect();
        assert!(headings.len() >= 5 && headings.iter().all(|line| line.starts_with("\\subsection*{\\raggedright ")), "{headings:?}");
    }
}

#[test]
fn the_table_in_latex_is_as_wide_as_the_line() {
    for text in [register(Lang::De, DocFormat::Tex), register(Lang::En, DocFormat::Tex)] {
        let open = text.lines().find(|line| line.contains("\\begin{tabular}")).unwrap();
        // every column is its share of the line less the space LaTeX puts on both of its sides, and the shares are the whole line
        let columns: Vec<&str> = open.split("p{\\dimexpr").skip(1).collect();
        assert!(
            columns.len() == 5 && columns.iter().all(|column| column.ends_with("\\linewidth-2\\tabcolsep\\relax}") || column.ends_with("\\linewidth-2\\tabcolsep\\relax}}")),
            "{open}"
        );
        let shares: f64 = columns.iter().map(|column| column.split_once("\\linewidth").unwrap().0.parse::<f64>().unwrap()).sum();
        assert!((shares - 1.0).abs() < 1e-9, "{open}");
        // it starts at the margin, and no cell is justified or keeps its first word in one piece
        assert!(open.starts_with("{\\footnotesize\\noindent\\begin{tabular}"), "{open}");
        let rows: Vec<&str> = text.lines().filter(|line| line.ends_with("\\tabularnewline")).collect();
        assert_eq!(rows.len(), 8, "the head and seven purposes");
        assert!(rows.iter().all(|row| row.split(" & ").count() == 5 && row.split(" & ").all(|cell| cell.starts_with("\\raggedright\\hspace{0pt}"))), "{rows:?}");
        assert!(text.contains("\\end{tabular}}\n"));
    }
    // a model's name can be broken where it has no space
    let log = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl"))
        .unwrap()
        .replace("claude-opus-5-5", "hf.co/some_one/model:latest");
    let named = disclosure(&parse_log(&log), "thesis.md", Lang::En, DocFormat::Tex, Profile::Editor);
    assert!(named.contains("hf.co/\\allowbreak{}some\\_\\allowbreak{}one/\\allowbreak{}model:\\allowbreak{}latest"), "{named}");
}

#[test]
fn a_latex_file_says_first_which_fonts_pdflatex_needs() {
    use cowriting_core::disclosure::prompts_appendix;
    for (lang, word) in [(Lang::En, "Without lmodern"), (Lang::De, "Ohne lmodern")] {
        let appendix = prompts_appendix(&full_log(), "thesis.md", lang, DocFormat::Tex, true, Profile::Editor);
        for text in [register(lang, DocFormat::Tex), appendix] {
            let lines: Vec<&str> = text.lines().collect();
            assert!(lines[..3].iter().all(|line| line.starts_with("% ")) && lines[3].starts_with("\\section*{"), "{:?}", &lines[..4]);
            assert!(lines[0].contains("\\usepackage[T1]{fontenc}") && lines[0].contains("\\usepackage{lmodern}") && lines[1].contains(word) && lines[2].contains("XeLaTeX"));
        }
        // the markdown file has no such note
        assert!(register(lang, DocFormat::Md).starts_with("# "));
    }
}
