//! The review party's catalogue (the plugin's `plugin/hooks/personas.ts`, without the sprites,
//! which the interface draws). Experts the author made are added to a `Party` per document; there
//! is no global list.

#[derive(Debug, Clone, PartialEq)]
pub struct Persona {
    pub id: String,
    pub name: String,
    /// What a compact card shows, at most 13 characters.
    pub short: String,
    pub role: String,
    pub category: String,
    pub is_custom: bool,
}

const CATALOG: &[(&str, &str, &str, &str, &str)] = &[
    ("grammar-guy", "Grammar Guy", "Grammar Guy", "Grammar, spelling, punctuation (EN/DE)", "language"),
    ("statistics-nerd", "Statistics Nerd", "Stats Nerd", "Design, tests, effect sizes, reporting", "methods"),
    ("reviewer-2", "Reviewer #2", "Reviewer #2", "Weaknesses, limitations, tough questions", "feedback"),
    ("librarian", "The Librarian", "Librarian", "Sources, Zotero, Obsidian, DOI checks", "literature"),
    ("architect", "The Architect", "Architect", "Structure, argument flow, red thread", "structure"),
    ("integrity-guardian", "Integrity Guardian", "Integrity", "Good scientific practice, plagiarism, disclosure", "integrity"),
    ("lay-reader", "The Lay Reader", "Lay Reader", "Clarity for non-experts", "feedback"),
    ("style-coach", "The Style Coach", "Style Coach", "Writing style, compared with papers you admire", "style"),
    ("terminologist", "The Terminologist", "Terminologist", "Field terminology, usual vs. unusual wording", "terminology"),
    ("clinician", "The Clinician", "Clinician", "Clinical relevance, patient perspective", "content"),
];

/// The personas that ship with CoWriting.
pub fn catalog() -> Vec<Persona> {
    CATALOG
        .iter()
        .map(|&(id, name, short, role, category)| Persona { id: id.into(), name: name.into(), short: short.into(), role: role.into(), category: category.into(), is_custom: false })
        .collect()
}

/// An id as the log may write it: `cowrite:reviewer-2` names `reviewer-2`.
pub fn bare_id(id: &str) -> &str {
    ["cowriting:", "cowrite:", "cw:"].iter().find_map(|prefix| id.strip_prefix(prefix)).unwrap_or(id)
}

/// The personas one document knows: the catalogue and the author's own experts.
#[derive(Debug, Clone)]
pub struct Party {
    personas: Vec<Persona>,
}

impl Default for Party {
    fn default() -> Self {
        Party { personas: catalog() }
    }
}

impl Party {
    pub fn with_custom(custom: Vec<Persona>) -> Self {
        let mut personas = catalog();
        // a catalogue id is never replaced by a file of the same name
        personas.extend(custom.into_iter().filter(|c| !CATALOG.iter().any(|entry| entry.0 == c.id)));
        Party { personas }
    }

    pub fn by_id(&self, id: &str) -> Option<&Persona> {
        let key = bare_id(id);
        self.personas.iter().find(|p| p.id == key)
    }

    pub fn all(&self) -> &[Persona] {
        &self.personas
    }
}
