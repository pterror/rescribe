//! RIS (Research Information Systems) reader for rescribe.
//!
//! Parses RIS bibliography files into rescribe's document IR.
//! RIS is a standardized tag format for bibliographic citations.
//!
//! # Example
//!
//! ```
//! use rescribe_read_ris::parse;
//!
//! let ris = "TY  - JOUR\nAU  - Smith, John\nTI  - A Great Paper\nER  -";
//! let result = parse(ris).unwrap();
//! let doc = result.value;
//! ```

use rescribe_core::{ConversionResult, Document, Node, ParseError, Properties};
use rescribe_std::{node, prop};
use std::collections::HashMap;

/// Parse RIS text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &Default::default())
}

/// Parse RIS text with options.
pub fn parse_with_options(
    input: &str,
    _options: &rescribe_core::ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut entries = Vec::new();
    let mut current_entry: Option<RisEntry> = None;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // RIS format: TAG  - VALUE (tag is 2-4 chars, followed by two spaces, dash, space, value)
        if line.len() >= 6 && &line[4..6] == "- " {
            let tag = line[0..2].trim();
            let value = line[6..].trim();

            match tag {
                "TY" => {
                    // Start of new entry
                    if let Some(entry) = current_entry.take() {
                        entries.push(entry.into_node());
                    }
                    current_entry = Some(RisEntry::new(value));
                }
                "ER" => {
                    // End of entry
                    if let Some(entry) = current_entry.take() {
                        entries.push(entry.into_node());
                    }
                }
                _ => {
                    // Add field to current entry
                    if let Some(ref mut entry) = current_entry {
                        entry.add_field(tag, value);
                    }
                }
            }
        }
    }

    // Handle entry without ER tag
    if let Some(entry) = current_entry {
        entries.push(entry.into_node());
    }

    let content = if entries.is_empty() {
        Node::new(node::DOCUMENT)
    } else {
        Node::new(node::DOCUMENT).child(Node::new(node::DEFINITION_LIST).children(entries))
    };

    Ok(ConversionResult::ok(Document {
        content,
        resources: Default::default(),
        metadata: Properties::new(),
        source: None,
    }))
}

/// RIS entry being parsed.
struct RisEntry {
    entry_type: String,
    fields: HashMap<String, Vec<String>>,
}

impl RisEntry {
    fn new(entry_type: &str) -> Self {
        Self {
            entry_type: entry_type.to_string(),
            fields: HashMap::new(),
        }
    }

    fn add_field(&mut self, tag: &str, value: &str) {
        self.fields
            .entry(tag.to_string())
            .or_default()
            .push(value.to_string());
    }

    fn get_first(&self, tag: &str) -> Option<&str> {
        self.fields
            .get(tag)
            .and_then(|v| v.first().map(|s| s.as_str()))
    }

    fn get_all(&self, tag: &str) -> Vec<&str> {
        self.fields
            .get(tag)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    fn into_node(self) -> Node {
        // Generate a cite key from first author and year
        let cite_key = self.generate_cite_key();
        let bibtex_type = ris_type_to_bibtex(&self.entry_type);

        // Create the term (citation key)
        let term = Node::new(node::DEFINITION_TERM)
            .child(Node::new(node::CODE).prop(prop::CONTENT, cite_key.clone()));

        // Build the description content
        let mut desc_children = Vec::new();

        // Entry type badge
        let type_text = format!("[{}] ", bibtex_type);
        desc_children.push(
            Node::new(node::SPAN)
                .prop("html:class", "ris-type")
                .child(Node::new(node::TEXT).prop(prop::CONTENT, type_text)),
        );

        // Authors (AU tag, can be multiple)
        let authors = self.get_all("AU");
        if !authors.is_empty() {
            let author_text = authors.join("; ");
            desc_children.push(
                Node::new(node::STRONG)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, author_text)),
            );
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }

        // Title (TI or T1)
        let title = self.get_first("TI").or(self.get_first("T1"));
        if let Some(t) = title {
            desc_children.push(
                Node::new(node::EMPHASIS)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, t.to_string())),
            );
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }

        // Journal/Publication (JO, JF, or T2)
        let journal = self
            .get_first("JO")
            .or(self.get_first("JF"))
            .or(self.get_first("T2"));
        if let Some(j) = journal {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, j.to_string()));
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }

        // Volume (VL)
        if let Some(vol) = self.get_first("VL") {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!("{}.", vol)));
        }

        // Year (PY or Y1)
        let year = self.get_first("PY").or(self.get_first("Y1"));
        if let Some(y) = year {
            // Extract just the year part (format might be YYYY/MM/DD)
            let year_str = y.split('/').next().unwrap_or(y);
            desc_children
                .push(Node::new(node::TEXT).prop(prop::CONTENT, format!(" ({})", year_str)));
        }

        // Pages (SP - start page, EP - end page)
        if let Some(sp) = self.get_first("SP") {
            let pages = if let Some(ep) = self.get_first("EP") {
                format!(", pp. {}-{}", sp, ep)
            } else {
                format!(", p. {}", sp)
            };
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, pages));
        }

        // DOI (DO)
        if let Some(doi) = self.get_first("DO") {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
            desc_children.push(
                Node::new(node::LINK)
                    .prop(prop::URL, format!("https://doi.org/{}", doi))
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, format!("doi:{}", doi))),
            );
        }

        // URL (UR)
        if let Some(url) = self.get_first("UR") {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
            desc_children.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url.to_string())
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, url.to_string())),
            );
        }

        let desc = Node::new(node::DEFINITION_DESC)
            .prop("ris:type", self.entry_type.clone())
            .prop("ris:key", cite_key)
            .child(Node::new(node::PARAGRAPH).children(desc_children));

        Node::new("ris:entry").children(vec![term, desc])
    }

    fn generate_cite_key(&self) -> String {
        let author_part = self
            .get_first("AU")
            .map(|a| {
                // Get last name (before comma) and take first 8 chars
                a.split(',')
                    .next()
                    .unwrap_or(a)
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(8)
                    .collect::<String>()
                    .to_lowercase()
            })
            .unwrap_or_else(|| "unknown".to_string());

        let year_part = self
            .get_first("PY")
            .or(self.get_first("Y1"))
            .map(|y| y.split('/').next().unwrap_or(y).to_string())
            .unwrap_or_default();

        format!("{}{}", author_part, year_part)
    }
}

/// Map RIS reference types to BibTeX types.
fn ris_type_to_bibtex(ris_type: &str) -> &'static str {
    match ris_type {
        "JOUR" => "article",
        "BOOK" => "book",
        "CHAP" | "SECT" => "incollection",
        "CONF" | "CPAPER" => "inproceedings",
        "THES" => "phdthesis",
        "RPRT" => "techreport",
        "MGZN" | "NEWS" => "article",
        "ELEC" | "WEB" => "online",
        "COMP" => "software",
        "DATA" => "dataset",
        "ABST" | "INPR" | "JFULL" => "article",
        "EDBOOK" => "book",
        "GEN" | "CTLG" | "ENCYC" | "DICT" => "misc",
        "MANSCPT" | "UNPB" => "unpublished",
        "PAMP" => "booklet",
        "PAT" => "misc",
        "SER" => "book",
        "SLIDE" | "VIDEO" | "SOUND" | "MAP" | "ADVS" | "ART" => "misc",
        _ => "misc",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_article() {
        let ris = r#"TY  - JOUR
AU  - Smith, John
AU  - Doe, Jane
TI  - A Great Paper
JO  - Nature
PY  - 2020
VL  - 123
SP  - 45
EP  - 67
DO  - 10.1234/nature.2020
ER  -"#;

        let result = parse(ris).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_book() {
        let ris = r#"TY  - BOOK
AU  - Knuth, Donald E.
TI  - The Art of Computer Programming
PY  - 1997
ER  -"#;

        let result = parse(ris).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_multiple() {
        let ris = r#"TY  - JOUR
AU  - First, Author
TI  - First Paper
ER  -
TY  - JOUR
AU  - Second, Author
TI  - Second Paper
ER  -"#;

        let result = parse(ris).unwrap();
        let doc = result.value;
        let def_list = &doc.content.children[0];
        assert_eq!(def_list.children.len(), 2);
    }

    #[test]
    fn test_parse_empty() {
        let ris = "";
        let result = parse(ris).unwrap();
        let doc = result.value;
        assert!(doc.content.children.is_empty());
    }
}
