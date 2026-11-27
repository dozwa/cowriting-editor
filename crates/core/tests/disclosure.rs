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

#[test]
fn a_model_on_the_authors_computer_is_named_by_its_program_not_by_its_address() {
    let log = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl")).unwrap();
    let served_by = |provider: &str, lang: Lang| {
        let log = log.replace("\"plugin\": \"0.1.0\", \"claude_code\": \"2.1.300\"", &format!("\"editor\": \"0.1.0\", \"provider\": \"{provider}\""));
        let text = disclosure(&parse_log(&log), "thesis.md", lang, DocFormat::Md, Profile::Editor);
        let (before, after) = if lang == Lang::De { ("die Modelle stellte ", " bereit;") } else { ("the models were served by ", "; they are named") };
        let from = text.find(before).unwrap_or_else(|| panic!("{text}")) + before.len();
        text[from..from + text[from..].find(after).unwrap()].to_string()
    };
    // what the log holds for a local server, with the address it was reached at
    assert_eq!(served_by("a local model server (localhost:11434)", Lang::En), "Ollama (local)");
    assert_eq!(served_by("a local model server (127.0.0.1:11434)", Lang::De), "Ollama (lokal)");
    assert_eq!(served_by("a local model server (localhost:1234)", Lang::En), "LM Studio (local)");
    // another port, or none: local, and no address
    assert_eq!(served_by("a local model server (localhost:8080)", Lang::En), "a local model server");
    assert_eq!(served_by("a local model server (localhost)", Lang::De), "ein lokaler Modellserver");
    // every other provider stands as the log names it
    assert_eq!(served_by("OpenAI-compatible API at llm.uni-example.de", Lang::De), "OpenAI-compatible API at llm.uni-example.de");
    assert_eq!(served_by("Anthropic", Lang::En), "Anthropic");
}

/// The register's table: its head and its rows, cell by cell.
fn table(text: &str) -> Vec<Vec<String>> {
    text.lines().filter(|line| line.starts_with("| ")).map(|line| line.trim_matches('|').split(" | ").map(|cell| cell.trim().to_string()).collect()).collect()
}

#[test]
fn the_editor_and_the_provider_are_said_once_before_the_table_and_its_column_names_the_model() {
    let log = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl")).unwrap();
    let by_editor = log.replace("\"plugin\": \"0.1.0\", \"claude_code\": \"2.1.300\"", "\"editor\": \"0.1.0\", \"provider\": \"a local model server (localhost:11434)\"");
    for (lang, sentence, head) in [
        (Lang::En, "The version used was CoWriting Editor 0.1.0, and the models were served by Ollama (local); they are named in the table.", "Model"),
        (Lang::De, "Eingesetzt wurde CoWriting Editor 0.1.0, die Modelle stellte Ollama (lokal) bereit; sie sind in der Tabelle genannt.", "Modell"),
    ] {
        let text = disclosure(&parse_log(&by_editor), "thesis.md", lang, DocFormat::Md, Profile::Editor);
        assert_eq!(text.matches("Ollama").count(), 1, "the provider is named once:\n{text}");
        assert!(text.contains(sentence), "{text}");
        let rows = table(&text);
        assert!(rows.len() > 2, "the fixture has several rows");
        assert_eq!(rows[0][1], head);
        assert!(rows[1..].iter().all(|row| !row[1].contains("CoWriting") && !row[1].contains("Ollama")), "{rows:?}");
        assert_eq!((rows[1][1].as_str(), rows[2][1].as_str()), ("claude-opus-5-5, m", "claude-opus-5-5"));
        // the LaTeX file says the same
        let tex = disclosure(&parse_log(&by_editor), "thesis.md", lang, DocFormat::Tex, Profile::Editor);
        assert!(tex.contains(sentence) && tex.matches("Ollama").count() == 1, "{tex}");
    }

    // a log without a provider (an early version did not record it): the editor's version alone
    let without = log.replace("\"plugin\": \"0.1.0\", \"claude_code\": \"2.1.300\"", "\"editor\": \"0.1.0\"");
    assert!(disclosure(&parse_log(&without), "thesis.md", Lang::En, DocFormat::Md, Profile::Editor)
        .contains("The version used was CoWriting Editor 0.1.0; the models are named in the table."));
}

#[test]
fn rows_that_went_through_different_providers_each_name_their_own() {
    let log = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/logs/full.jsonl")).unwrap();
    // every response through Anthropic, but for those of one purpose, which a local model gave
    let mut local = 0;
    let mixed: String = log
        .lines()
        .map(|line| {
            let is_first_response = line.contains("\"type\": \"response\"") && local == 0;
            local += usize::from(is_first_response);
            let provider = if is_first_response { "a local model server (localhost:11434)" } else { "Anthropic" };
            format!("{}\n", line.replace("\"plugin\": \"0.1.0\", \"claude_code\": \"2.1.300\"", &format!("\"editor\": \"0.1.0\", \"provider\": \"{provider}\"")))
        })
        .collect();
    let text = disclosure(&parse_log(&mixed), "thesis.md", Lang::En, DocFormat::Md, Profile::Editor);
    assert!(text.contains("The models and who served them are named in the table."), "{text}");
    let rows = table(&text);
    assert_eq!(rows[0][1], "Tool / model");
    assert!(rows[1..].iter().all(|row| row[1].starts_with("CoWriting Editor 0.1.0; ")), "{rows:?}");
    assert!(
        rows[1..].iter().any(|row| row[1].contains("Ollama (local)")) && rows[1..].iter().any(|row| row[1] == "CoWriting Editor 0.1.0; Anthropic (claude-opus-5-5)"),
        "{rows:?}"
    );
}
