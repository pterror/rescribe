//! PDF reader for rescribe.
//!
//! Parses PDF files into rescribe's document IR.
//!
//! # Limitations
//!
//! PDF is fundamentally a visual/layout format, not a semantic format.
//! This reader extracts text content but cannot reliably determine:
//! - Headings vs regular text (all text looks the same in PDF)
//! - List structure
//! - Table structure
//! - Emphasis/bold/italic (font changes, not semantic markup)
//!
//! The extracted content is organized by page, with text split into
//! paragraphs based on blank lines.
//!
//! For better structure extraction from PDFs, consider using an OCR-based
//! approach or a specialized PDF analysis tool that can infer document structure.

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, Node, ParseError, ParseOptions, Properties,
    Severity, SourceInfo, WarningKind,
};
use rescribe_std::{node, prop};

/// Parse PDF bytes into a rescribe Document.
pub fn parse(input: &[u8]) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse PDF with custom options.
pub fn parse_with_options(
    input: &[u8],
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut warnings = Vec::new();

    // Extract text by pages
    let pages = pdf_extract::extract_text_from_mem_by_pages(input)
        .map_err(|e| ParseError::Invalid(format!("PDF extraction failed: {}", e)))?;

    let mut doc_children = Vec::new();

    for (page_num, page_text) in pages.into_iter().enumerate() {
        // Add a page break between pages (except before the first)
        if page_num > 0 {
            let page_break = Node::new(node::HORIZONTAL_RULE).prop(prop::LAYOUT_PAGE_BREAK, true);
            doc_children.push(page_break);
        }

        // Split text into paragraphs based on blank lines
        let paragraphs = split_into_paragraphs(&page_text);

        if paragraphs.is_empty() && !page_text.trim().is_empty() {
            // If we couldn't split but there's text, add it as a single paragraph
            let para = Node::new(node::PARAGRAPH).child(text_node(page_text.trim()));
            doc_children.push(para);
        } else {
            for para_text in paragraphs {
                if !para_text.is_empty() {
                    let para = Node::new(node::PARAGRAPH).child(text_node(&para_text));
                    doc_children.push(para);
                }
            }
        }
    }

    // Add warning about structural loss
    warnings.push(FidelityWarning::new(
        Severity::Major,
        WarningKind::FeatureLost("PDF structure".into()),
        "PDF is a visual format; semantic structure (headings, lists, tables) cannot be reliably extracted",
    ));

    let doc_node = Node::new(node::DOCUMENT).children(doc_children);

    let document = Document {
        content: doc_node,
        resources: Default::default(),
        metadata: Properties::new(),
        source: Some(SourceInfo {
            format: "pdf".to_string(),
            metadata: Properties::new(),
        }),
    };

    Ok(ConversionResult::with_warnings(document, warnings))
}

/// Split text into paragraphs based on blank lines.
fn split_into_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut blank_line_count = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_line_count += 1;
            // Two or more blank lines indicate a paragraph break
            if blank_line_count >= 1 && !current.trim().is_empty() {
                paragraphs.push(normalize_paragraph(&current));
                current.clear();
            }
        } else {
            blank_line_count = 0;
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(trimmed);
        }
    }

    // Don't forget the last paragraph
    if !current.trim().is_empty() {
        paragraphs.push(normalize_paragraph(&current));
    }

    paragraphs
}

/// Normalize a paragraph's whitespace.
fn normalize_paragraph(text: &str) -> String {
    // Collapse multiple spaces into one
    let mut result = String::with_capacity(text.len());
    let mut prev_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }

    result.trim().to_string()
}

/// Create a text node with the given content.
fn text_node(content: &str) -> Node {
    Node::new(node::TEXT).prop(prop::CONTENT, content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.\nContinued.\n\nThird.";
        let paragraphs = split_into_paragraphs(text);
        assert_eq!(paragraphs.len(), 3);
        assert_eq!(paragraphs[0], "First paragraph.");
        assert_eq!(paragraphs[1], "Second paragraph. Continued.");
        assert_eq!(paragraphs[2], "Third.");
    }

    #[test]
    fn test_normalize_paragraph() {
        let text = "  Hello   world  ";
        assert_eq!(normalize_paragraph(text), "Hello world");
    }

    #[test]
    fn test_split_paragraphs_empty() {
        let text = "";
        let paragraphs = split_into_paragraphs(text);
        assert!(paragraphs.is_empty());
    }

    #[test]
    fn test_split_paragraphs_only_whitespace() {
        let text = "   \n\n   \n";
        let paragraphs = split_into_paragraphs(text);
        assert!(paragraphs.is_empty());
    }
}
