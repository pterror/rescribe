//! BibTeX reader for rescribe.
//!
//! Parses BibTeX/BibLaTeX bibliography files into rescribe's document IR.
//! Each entry is converted to a structured bibliography node.
//!
//! # Example
//!
//! ```
//! use rescribe_read_bibtex::parse;
//!
//! let bibtex = r#"
//! @article{smith2020,
//!   author = {John Smith},
//!   title = {A Great Paper},
//!   journal = {Nature},
//!   year = {2020},
//! }
//! "#;
//!
//! let result = parse(bibtex).unwrap();
//! let doc = result.value;
//! ```

use biblatex::{Bibliography, ChunksExt};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse BibTeX text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let bibliography = Bibliography::parse(input)
        .map_err(|e| ParseError::Invalid(format!("BibTeX parse error: {:?}", e)))?;

    let mut warnings = Vec::new();
    let mut entries = Vec::new();

    for entry in bibliography.iter() {
        let entry_node = convert_entry(entry, &mut warnings);
        entries.push(entry_node);
    }

    // Wrap entries in a definition list for semantic structure
    let content = if entries.is_empty() {
        Node::new(node::DOCUMENT)
    } else {
        Node::new(node::DOCUMENT).child(Node::new(node::DEFINITION_LIST).children(entries))
    };

    let document = Document {
        content,
        resources: Default::default(),
        metadata: Properties::new(),
        source: None,
    };

    Ok(ConversionResult::with_warnings(document, warnings))
}

fn convert_entry(entry: &biblatex::Entry, _warnings: &mut Vec<FidelityWarning>) -> Node {
    let key = entry.key.clone();
    let entry_type = format!("{:?}", entry.entry_type).to_lowercase();

    // Create the term (citation key)
    let term = Node::new(node::DEFINITION_TERM)
        .child(Node::new(node::CODE).prop(prop::CONTENT, key.clone()));

    // Build the description content
    let mut desc_children = Vec::new();

    // Entry type badge
    let type_text = format!("[{}] ", entry_type);
    desc_children.push(
        Node::new(node::SPAN)
            .prop("html:class", "bibtex-type")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, type_text)),
    );

    // Authors
    if let Ok(authors) = entry.author() {
        let author_text = authors
            .iter()
            .map(format_person)
            .collect::<Vec<_>>()
            .join("; ");
        if !author_text.is_empty() {
            desc_children.push(
                Node::new(node::STRONG)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, author_text)),
            );
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }
    }

    // Title - get raw field and format it
    if let Ok(title_chunks) = entry.title() {
        let title_str = title_chunks.format_verbatim();
        desc_children.push(
            Node::new(node::EMPHASIS).child(Node::new(node::TEXT).prop(prop::CONTENT, title_str)),
        );
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Journal
    if let Ok(journal_chunks) = entry.journal() {
        let journal_str = journal_chunks.format_verbatim();
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, journal_str));
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    } else if let Ok(booktitle_chunks) = entry.book_title() {
        let booktitle_str = booktitle_chunks.format_verbatim();
        desc_children
            .push(Node::new(node::TEXT).prop(prop::CONTENT, format!("In: {}", booktitle_str)));
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Volume - use debug format for PermissiveType
    if let Ok(volume) = entry.volume() {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!("{:?}.", volume)));
    }

    // Year/Date - extract from PermissiveType
    if let Ok(date) = entry.date() {
        // Use debug format since Date is wrapped in PermissiveType
        let year_text = format!(" ({:?})", date);
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, year_text));
    }

    // Pages
    if let Ok(pages) = entry.pages() {
        let pages_str = format!("{:?}", pages);
        if !pages_str.is_empty() {
            desc_children
                .push(Node::new(node::TEXT).prop(prop::CONTENT, format!(", pp. {}", pages_str)));
        }
    }

    // DOI
    if let Ok(doi) = entry.doi() {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        desc_children.push(
            Node::new(node::LINK)
                .prop(prop::URL, format!("https://doi.org/{}", doi))
                .child(Node::new(node::TEXT).prop(prop::CONTENT, format!("doi:{}", doi))),
        );
    }

    // URL
    if let Ok(url) = entry.url() {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        desc_children.push(
            Node::new(node::LINK)
                .prop(prop::URL, url.clone())
                .child(Node::new(node::TEXT).prop(prop::CONTENT, url)),
        );
    }

    let desc = Node::new(node::DEFINITION_DESC)
        .prop("bibtex:key", key)
        .prop("bibtex:type", entry_type)
        .child(Node::new(node::PARAGRAPH).children(desc_children));

    // Return term and description as a pair in a wrapper
    Node::new("bibtex:entry").children(vec![term, desc])
}

fn format_person(person: &biblatex::Person) -> String {
    let mut parts = Vec::new();

    if !person.given_name.is_empty() {
        parts.push(person.given_name.clone());
    }
    if !person.prefix.is_empty() {
        parts.push(person.prefix.clone());
    }
    if !person.name.is_empty() {
        parts.push(person.name.clone());
    }
    if !person.suffix.is_empty() {
        parts.push(person.suffix.clone());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_article() {
        let bibtex = r#"
@article{smith2020,
  author = {John Smith},
  title = {A Great Paper},
  journal = {Nature},
  year = {2020},
}
"#;

        let result = parse(bibtex).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_book() {
        let bibtex = r#"
@book{knuth1984,
  author = {Donald E. Knuth},
  title = {The TeXbook},
  publisher = {Addison-Wesley},
  year = {1984},
}
"#;

        let result = parse(bibtex).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_empty() {
        let bibtex = "";
        let result = parse(bibtex).unwrap();
        let doc = result.value;
        assert!(doc.content.children.is_empty());
    }
}
