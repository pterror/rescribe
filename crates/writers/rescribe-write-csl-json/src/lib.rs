//! CSL JSON writer for rescribe.
//!
//! Serializes rescribe's document IR to CSL JSON (Citation Style Language JSON).
//! Extracts bibliography entries from definition lists.
//!
//! # Example
//!
//! ```
//! use rescribe_write_csl_json::emit;
//! use rescribe_core::{Document, Node, Properties};
//!
//! let doc = Document {
//!     content: Node::new("document"),
//!     resources: Default::default(),
//!     metadata: Properties::new(),
//!     source: None,
//! };
//!
//! let result = emit(&doc).unwrap();
//! let json = String::from_utf8(result.value).unwrap();
//! ```

use rescribe_core::{ConversionResult, Document, EmitError, Node};
use rescribe_std::{node, prop};
use serde::Serialize;

/// Emit a document to CSL JSON.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut items = Vec::new();
    let warnings = Vec::new();

    collect_items(&doc.content, &mut items);

    let json = serde_json::to_string_pretty(&items)
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("CSL JSON error: {}", e))))?;

    Ok(ConversionResult::with_warnings(json.into_bytes(), warnings))
}

fn collect_items(node: &Node, items: &mut Vec<CslItem>) {
    // Check for csl:item nodes
    if node.kind.as_str() == "csl:item"
        && let Some(item) = extract_csl_item(node)
    {
        items.push(item);
        return;
    }

    // Check for bibtex:entry nodes (convert to CSL)
    if node.kind.as_str() == "bibtex:entry"
        && let Some(item) = extract_bibtex_item(node)
    {
        items.push(item);
        return;
    }

    // Check definition_desc nodes with csl:id property
    if node.kind.as_str() == node::DEFINITION_DESC {
        if let Some(id) = node.props.get_str("csl:id") {
            let item = extract_from_definition(node, id);
            items.push(item);
            return;
        }
        // Also check for bibtex:key
        if let Some(key) = node.props.get_str("bibtex:key") {
            let item = extract_from_definition(node, key);
            items.push(item);
            return;
        }
    }

    // Recurse into children
    for child in &node.children {
        collect_items(child, items);
    }
}

fn extract_csl_item(node: &Node) -> Option<CslItem> {
    // Find the definition_desc child with csl:id
    for child in &node.children {
        if child.kind.as_str() == node::DEFINITION_DESC
            && let Some(id) = child.props.get_str("csl:id")
        {
            return Some(extract_from_definition(child, id));
        }
    }
    None
}

fn extract_bibtex_item(node: &Node) -> Option<CslItem> {
    // Find the definition_desc child with bibtex:key
    for child in &node.children {
        if child.kind.as_str() == node::DEFINITION_DESC
            && let Some(key) = child.props.get_str("bibtex:key")
        {
            return Some(extract_from_definition(child, key));
        }
    }
    None
}

fn extract_from_definition(node: &Node, id: &str) -> CslItem {
    let item_type = node
        .props
        .get_str("csl:type")
        .or_else(|| node.props.get_str("bibtex:type"))
        .map(map_type_to_csl)
        .unwrap_or_else(|| "article".to_string());

    // Extract text content from the paragraph children
    let mut title = None;
    let mut authors = Vec::new();
    let mut container_title = None;
    let mut doi = None;
    let mut url = None;

    // Walk through looking for specific node types
    extract_content(
        node,
        &mut title,
        &mut authors,
        &mut container_title,
        &mut doi,
        &mut url,
    );

    CslItem {
        id: id.to_string(),
        item_type: Some(item_type),
        title,
        author: if authors.is_empty() {
            None
        } else {
            Some(authors)
        },
        container_title,
        doi,
        url,
        // These would need more sophisticated extraction
        editor: None,
        issued: None,
        collection_title: None,
        publisher: None,
        publisher_place: None,
        volume: None,
        issue: None,
        page: None,
        isbn: None,
        issn: None,
        abstract_text: None,
    }
}

fn extract_content(
    node: &Node,
    title: &mut Option<String>,
    authors: &mut Vec<CslName>,
    _container_title: &mut Option<String>,
    doi: &mut Option<String>,
    url: &mut Option<String>,
) {
    // Extract title from emphasis nodes
    if node.kind.as_str() == node::EMPHASIS && title.is_none() {
        *title = Some(collect_text(node));
    }

    // Extract authors from strong nodes
    if node.kind.as_str() == node::STRONG {
        let text = collect_text(node);
        for author in text.split(';') {
            let author = author.trim();
            if !author.is_empty() {
                authors.push(CslName::from_string(author));
            }
        }
    }

    // Extract DOI/URL from links
    if node.kind.as_str() == node::LINK
        && let Some(link_url) = node.props.get_str(prop::URL)
    {
        if link_url.contains("doi.org") {
            // Extract DOI from URL
            if let Some(d) = link_url.strip_prefix("https://doi.org/") {
                *doi = Some(d.to_string());
            }
        } else if url.is_none() {
            *url = Some(link_url.to_string());
        }
    }

    for child in &node.children {
        extract_content(child, title, authors, _container_title, doi, url);
    }
}

fn collect_text(node: &Node) -> String {
    let mut text = String::new();
    collect_text_recursive(node, &mut text);
    text
}

fn collect_text_recursive(node: &Node, text: &mut String) {
    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        text.push_str(content);
    }
    for child in &node.children {
        collect_text_recursive(child, text);
    }
}

fn map_type_to_csl(bibtex_type: &str) -> String {
    match bibtex_type {
        "article" => "article-journal",
        "book" => "book",
        "inbook" => "chapter",
        "incollection" => "chapter",
        "inproceedings" => "paper-conference",
        "conference" => "paper-conference",
        "phdthesis" => "thesis",
        "mastersthesis" => "thesis",
        "techreport" => "report",
        "manual" => "book",
        "misc" => "article",
        "unpublished" => "manuscript",
        "online" => "webpage",
        other => other,
    }
    .to_string()
}

#[derive(Debug, Serialize)]
struct CslItem {
    id: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    item_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<Vec<CslName>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    editor: Option<Vec<CslName>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issued: Option<CslDate>,
    #[serde(rename = "container-title", skip_serializing_if = "Option::is_none")]
    container_title: Option<String>,
    #[serde(rename = "collection-title", skip_serializing_if = "Option::is_none")]
    collection_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publisher: Option<String>,
    #[serde(rename = "publisher-place", skip_serializing_if = "Option::is_none")]
    publisher_place: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<String>,
    #[serde(rename = "DOI", skip_serializing_if = "Option::is_none")]
    doi: Option<String>,
    #[serde(rename = "URL", skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(rename = "ISBN", skip_serializing_if = "Option::is_none")]
    isbn: Option<String>,
    #[serde(rename = "ISSN", skip_serializing_if = "Option::is_none")]
    issn: Option<String>,
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    abstract_text: Option<String>,
}

#[derive(Debug, Serialize)]
struct CslName {
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    given: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    literal: Option<String>,
}

impl CslName {
    fn from_string(s: &str) -> Self {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() >= 2 {
            // Assume "Given Family" format
            let (given, family) = parts.split_at(parts.len() - 1);
            CslName {
                given: Some(given.join(" ")),
                family: Some(family[0].to_string()),
                literal: None,
            }
        } else if parts.len() == 1 {
            CslName {
                family: Some(parts[0].to_string()),
                given: None,
                literal: None,
            }
        } else {
            CslName {
                literal: Some(s.to_string()),
                family: None,
                given: None,
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct CslDate {
    #[serde(rename = "date-parts", skip_serializing_if = "Option::is_none")]
    date_parts: Option<Vec<Vec<i32>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    literal: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::Properties;

    #[test]
    fn test_emit_empty() {
        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata: Properties::new(),
            source: None,
        };

        let result = emit(&doc).unwrap();
        let json = String::from_utf8(result.value).unwrap();
        assert_eq!(json.trim(), "[]");
    }
}
