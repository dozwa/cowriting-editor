//! The ready-made requests of the Ask tab: feedback, a Socratic dialogue, a structure check, a
//! language check, an explanation, a consistency report.
//!
//! Hand-ported from the plugin's skills (`plugin/skills/<id>/SKILL.md`), reworded for one
//! document and the editor's tools: no files to name, no shell, notes through `write_note`,
//! experts in the Experts tab instead of a summon command.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// The command the log records: 'feedback', 'socratic', …
    pub id: &'static str,
    pub label: &'static str,
    /// What the button does, in the author's words.
    pub hint: &'static str,
    /// What is asked, and logged as the question, when the author typed nothing. Empty: the author has to say something.
    pub request: &'static str,
    /// What the model is told to do, after the rules.
    #[serde(skip)]
    pub instructions: &'static str,
}

pub const ACTIONS: &[Action] = &[
    Action {
        id: "feedback",
        label: "Feedback",
        hint: "What works, the three biggest problems, one next step. Type a section's name to narrow it.",
        request: "Feedback on the whole document.",
        instructions: "Read the document; if the author names a section, keep to it. Then give feedback like an experienced writing-center tutor:

1. **What works** (1–2 points, specific).
2. **Top 3 problems** in priority order (higher-order concerns first: argument, structure, evidence; then clarity; language last). For each: the line number or a quote of at most 10 words, what you noticed, why it matters for the reader, and a guiding question.
3. **One next step** the author could take in their next writing session.

Never provide rewritten sentences or paragraphs, and never \"show an example\" using the author's content. To illustrate a principle, use an unrelated, generic example. When a deeper look would help, name the expert from the review party the author could ask in the Experts tab.",
    },
    Action {
        id: "socratic",
        label: "Socratic",
        hint: "Questions, one at a time, that help you think a section through. No text.",
        request: "Help me think this text through with questions.",
        instructions: "Help the author find *their own* answer. Ask **one question at a time** and wait for the reply. Start from what they want to say (purpose, reader, main claim), then evidence, counter-arguments, and order.

- Reflect back what you heard in a short paraphrase of *their* ideas (not polished prose), then ask the next question.
- If they ask you to \"just write it\", explain kindly that this editor keeps them the author, and offer a bullet list of *their own* points collected from the dialogue as a planning aid (label it \"your points, collected by the AI\").
- End when they feel ready to write; suggest a first sentence *type* (\"start with the gap in the literature\"), not a sentence.",
    },
    Action {
        id: "structure",
        label: "Structure",
        hint: "How your text is built: reverse outline, argument map, where the thread breaks.",
        request: "Check the structure of the whole document.",
        instructions: "Read the document; if the author names a section, keep to it. Produce:

1. **Reverse outline**: one line per section or paragraph saying what it *does* (e.g. \"introduces gap\", \"reports test of H2\"), with line numbers.
2. **Argument map**: main claim → supporting claims → evidence (citation or data) → marked gaps `⚠ no evidence`, `⚠ no link to the research question`.
3. **Red-thread check**: does each section answer the research question(s)? Where does the order break?
4. **3 questions** for the author.

If the author wants to keep the outline, save it with write_note. Never propose replacement text.",
    },
    Action {
        id: "language",
        label: "Language",
        hint: "Grammar, spelling and punctuation, each with the corrected form and the rule. Logged as language correction.",
        request: "Check the language of the whole document.",
        instructions: "The author allowed corrected forms for **correctness** issues. Read the document (or the passage the author names) and output a table:

| Line | Original | Corrected | Rule |
|---|---|---|---|

- Only real errors: grammar, spelling, punctuation, agreement, tense consistency, typography. In German also comma rules and capitalisation; in English also article use and hyphenation of compound modifiers.
- Do **not** restyle correct sentences. For style issues (wordiness, unclear reference, nominal style) add a second list \"Style hints\" with line and rule, **without** a rewrite.
- Leave citekeys, labels, math, code and Markdown syntax untouched.
- Remind the author in one line: these corrections are logged as language corrections, and they make the changes themselves.",
    },
    Action {
        id: "explain",
        label: "Explain",
        hint: "Explains a concept, method or theory for your own understanding. Type what you want explained.",
        request: "",
        instructions: "Explain what the author asks about at the level they need (ask if unclear): intuition first, then the precise definition, a small example from a *different* context than their text, common pitfalls, and 2–3 references for further reading that you can verify, or say that you verified none.

Offer to add a short entry to a glossary note (write_note, `glossary.md`; read the note first and keep what it holds). Make clear this is for their understanding: they write their own explanation in the manuscript and cite primary sources.",
    },
    Action {
        id: "consistency",
        label: "Consistency",
        hint: "Terms, abbreviations, citation keys, numbering: a checklist of what does not agree. Reports only.",
        request: "Check the whole document for consistency.",
        instructions: "Read the whole document and report, with line numbers and without changing anything:

- Terms used in several variants (e.g. *participants/subjects*, *E-Mail/Email*).
- Abbreviations used before their definition, or defined twice.
- Citation keys (`[@key]`) that search_bibliography does not find in the author's references. The search covers the author's bibliography files only; say so if it finds none.
- Figures and tables never referred to in the text, and references to ones that do not exist.
- Inconsistent number formats, units, capitalisation of headings, British and American spelling (or old and new German spelling).

Output a checklist the author can work through.",
    },
];

/// The action a logged command names: 'feedback', also as 'cowrite:feedback' or 'cw:feedback'.
pub fn action(command: &str) -> Option<&'static Action> {
    let id = ["cowriting:", "cowrite:", "cw:"].iter().find_map(|prefix| command.strip_prefix(prefix)).unwrap_or(command);
    ACTIONS.iter().find(|action| action.id == id)
}
