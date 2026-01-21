//! CSL JSON reader for rescribe.
//!
//! Parses CSL JSON (Citation Style Language JSON) into rescribe's document IR.
//! Each citation item is converted to a structured bibliography node.
//!
//! # Example
//!
//! ```
//! use rescribe_read_csl_json::parse;
//!
//! let csl = r#"[{
//!   "id": "smith2020",
//!   "type": "article-journal",
//!   "title": "A Great Paper",
//!   "author": [{"family": "Smith", "given": "John"}],
//!   "issued": {"date-parts": [[2020]]}
//! }]"#;
//!
//! let result = parse(csl).unwrap();
//! let doc = result.value;
//! ```

use rescribe_core::{ConversionResult, Document, Node, ParseError, Properties};
use rescribe_std::{node, prop};
use serde::Deserialize;

/// Parse CSL JSON text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let items: Vec<CslItem> = serde_json::from_str(input)
        .map_err(|e| ParseError::Invalid(format!("CSL JSON parse error: {}", e)))?;

    let warnings = Vec::new();
    let mut entries = Vec::new();

    for item in &items {
        let entry_node = convert_item(item);
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

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CslItem {
    id: String,
    #[serde(rename = "type")]
    item_type: Option<String>,
    title: Option<String>,
    author: Option<Vec<CslName>>,
    editor: Option<Vec<CslName>>,
    issued: Option<CslDate>,
    #[serde(rename = "container-title")]
    container_title: Option<String>,
    #[serde(rename = "collection-title")]
    collection_title: Option<String>,
    publisher: Option<String>,
    #[serde(rename = "publisher-place")]
    publisher_place: Option<String>,
    volume: Option<StringOrInt>,
    issue: Option<StringOrInt>,
    page: Option<String>,
    #[serde(rename = "DOI")]
    doi: Option<String>,
    #[serde(rename = "URL")]
    url: Option<String>,
    #[serde(rename = "ISBN")]
    isbn: Option<String>,
    #[serde(rename = "ISSN")]
    issn: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CslName {
    family: Option<String>,
    given: Option<String>,
    literal: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CslDate {
    #[serde(rename = "date-parts")]
    date_parts: Option<Vec<Vec<i32>>>,
    literal: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrInt {
    String(String),
    Int(i64),
}

impl std::fmt::Display for StringOrInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringOrInt::String(s) => write!(f, "{}", s),
            StringOrInt::Int(i) => write!(f, "{}", i),
        }
    }
}

fn convert_item(item: &CslItem) -> Node {
    let key = item.id.clone();
    let item_type = item
        .item_type
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    // Create the term (citation key)
    let term = Node::new(node::DEFINITION_TERM)
        .child(Node::new(node::CODE).prop(prop::CONTENT, key.clone()));

    // Build the description content
    let mut desc_children = Vec::new();

    // Item type badge
    let type_text = format!("[{}] ", item_type);
    desc_children.push(
        Node::new(node::SPAN)
            .prop("html:class", "csl-type")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, type_text)),
    );

    // Authors
    if let Some(authors) = &item.author {
        let author_text = authors
            .iter()
            .map(format_name)
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

    // Title
    if let Some(title) = &item.title {
        desc_children.push(
            Node::new(node::EMPHASIS)
                .child(Node::new(node::TEXT).prop(prop::CONTENT, title.clone())),
        );
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Container title (journal, book, etc.)
    if let Some(container) = &item.container_title {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, container.clone()));
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Volume/Issue
    if let Some(volume) = &item.volume {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!("{}", volume)));
        if let Some(issue) = &item.issue {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!("({})", issue)));
        }
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
    }

    // Date
    if let Some(date) = &item.issued {
        let date_str = format_date(date);
        if !date_str.is_empty() {
            desc_children
                .push(Node::new(node::TEXT).prop(prop::CONTENT, format!(" ({})", date_str)));
        }
    }

    // Pages
    if let Some(page) = &item.page {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!(", pp. {}", page)));
    }

    // Publisher
    if let Some(publisher) = &item.publisher {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!(". {}", publisher)));
        if let Some(place) = &item.publisher_place {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, format!(", {}", place)));
        }
    }

    // DOI
    if let Some(doi) = &item.doi {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        desc_children.push(
            Node::new(node::LINK)
                .prop(prop::URL, format!("https://doi.org/{}", doi))
                .child(Node::new(node::TEXT).prop(prop::CONTENT, format!("doi:{}", doi))),
        );
    }

    // URL
    if let Some(url) = &item.url {
        desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        desc_children.push(
            Node::new(node::LINK)
                .prop(prop::URL, url.clone())
                .child(Node::new(node::TEXT).prop(prop::CONTENT, url.clone())),
        );
    }

    let desc = Node::new(node::DEFINITION_DESC)
        .prop("csl:id", key)
        .prop("csl:type", item_type)
        .child(Node::new(node::PARAGRAPH).children(desc_children));

    Node::new("csl:item").children(vec![term, desc])
}

fn format_name(name: &CslName) -> String {
    if let Some(literal) = &name.literal {
        return literal.clone();
    }

    let mut parts = Vec::new();
    if let Some(given) = &name.given {
        parts.push(given.clone());
    }
    if let Some(family) = &name.family {
        parts.push(family.clone());
    }
    parts.join(" ")
}

fn format_date(date: &CslDate) -> String {
    if let Some(literal) = &date.literal {
        return literal.clone();
    }

    if let Some(parts) = &date.date_parts
        && let Some(first) = parts.first()
    {
        return first
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("-");
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_article() {
        let csl = r#"[{
            "id": "smith2020",
            "type": "article-journal",
            "title": "A Great Paper",
            "author": [{"family": "Smith", "given": "John"}],
            "container-title": "Nature",
            "issued": {"date-parts": [[2020]]}
        }]"#;

        let result = parse(csl).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_book() {
        let csl = r#"[{
            "id": "knuth1984",
            "type": "book",
            "title": "The TeXbook",
            "author": [{"family": "Knuth", "given": "Donald E."}],
            "publisher": "Addison-Wesley",
            "issued": {"date-parts": [[1984]]}
        }]"#;

        let result = parse(csl).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_empty() {
        let csl = "[]";
        let result = parse(csl).unwrap();
        let doc = result.value;
        assert!(doc.content.children.is_empty());
    }
}
