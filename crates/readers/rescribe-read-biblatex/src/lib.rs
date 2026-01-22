//! BibLaTeX reader for rescribe.
//!
//! Parses BibLaTeX bibliography files into rescribe's document IR.
//! Handles BibLaTeX-specific entry types and fields (date, journaltitle, etc.).
//!
//! # Example
//!
//! ```
//! use rescribe_read_biblatex::parse;
//!
//! let biblatex = r#"
//! @article{smith2020,
//!   author = {John Smith},
//!   title = {A Great Paper},
//!   journaltitle = {Nature},
//!   date = {2020-05-15},
//! }
//! "#;
//!
//! let result = parse(biblatex).unwrap();
//! let doc = result.value;
//! ```

use biblatex::{Bibliography, ChunksExt};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse BibLaTeX text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &Default::default())
}

/// Parse BibLaTeX text with options.
pub fn parse_with_options(
    input: &str,
    _options: &rescribe_core::ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let bibliography = Bibliography::parse(input)
        .map_err(|e| ParseError::Invalid(format!("BibLaTeX parse error: {:?}", e)))?;

    let mut warnings = Vec::new();
    let mut entries = Vec::new();

    for entry in bibliography.iter() {
        let entry_node = convert_entry(entry, &mut warnings);
        entries.push(entry_node);
    }

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
            .prop("html:class", "biblatex-type")
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

    // Title with optional subtitle
    if let Ok(title_chunks) = entry.title() {
        let title_str = title_chunks.format_verbatim();
        let mut full_title = title_str;

        // BibLaTeX subtitle field
        if let Ok(subtitle_chunks) = entry.subtitle() {
            let subtitle_str = subtitle_chunks.format_verbatim();
            full_title = format!("{}: {}", full_title, subtitle_str);
        }

        desc_children.push(
            Node::new(node::EMPHASIS).child(Node::new(node::TEXT).prop(prop::CONTENT, full_title)),
        );
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Journal (BibLaTeX uses journaltitle)
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

    // Volume and number
    if let Ok(volume) = entry.volume() {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!("{:?}", volume)));
        if let Ok(number) = entry.number() {
            desc_children
                .push(Node::new(node::TEXT).prop(prop::CONTENT, format!("({:?})", number)));
        }
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Date (BibLaTeX uses full date, not just year)
    if let Ok(date) = entry.date() {
        let date_text = format!(" ({:?})", date);
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, date_text));
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

    // eprint (for arXiv, etc.)
    if let Ok(eprint) = entry.eprint() {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        // Try to get eprinttype for proper linking
        let eprint_url = format!("https://arxiv.org/abs/{}", eprint);
        desc_children.push(
            Node::new(node::LINK)
                .prop(prop::URL, eprint_url)
                .child(Node::new(node::TEXT).prop(prop::CONTENT, format!("arXiv:{}", eprint))),
        );
    }

    let desc = Node::new(node::DEFINITION_DESC)
        .prop("biblatex:key", key)
        .prop("biblatex:type", entry_type)
        .child(Node::new(node::PARAGRAPH).children(desc_children));

    Node::new("biblatex:entry").children(vec![term, desc])
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
        let biblatex = r#"
@article{smith2020,
  author = {John Smith},
  title = {A Great Paper},
  journaltitle = {Nature},
  date = {2020-05-15},
}
"#;

        let result = parse(biblatex).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_online() {
        let biblatex = r#"
@online{website2024,
  author = {Jane Doe},
  title = {A Great Website},
  url = {https://example.com},
  date = {2024-01-15},
}
"#;

        let result = parse(biblatex).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_with_subtitle() {
        let biblatex = r#"
@book{knuth1984,
  author = {Donald E. Knuth},
  title = {The TeXbook},
  subtitle = {A Complete Guide to TeX},
  publisher = {Addison-Wesley},
  date = {1984},
}
"#;

        let result = parse(biblatex).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_empty() {
        let biblatex = "";
        let result = parse(biblatex).unwrap();
        let doc = result.value;
        assert!(doc.content.children.is_empty());
    }
}
