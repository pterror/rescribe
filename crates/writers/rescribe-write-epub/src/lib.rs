//! EPUB writer for rescribe.
//!
//! Emits rescribe's document IR as EPUB files.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_write_epub::emit;
//!
//! let doc = Document::new();
//! let result = emit(&doc)?;
//! // result.value contains EPUB bytes
//! ```

use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};
use rescribe_core::{ConversionResult, Document, EmitError, FidelityWarning, Node};
use rescribe_std::{node, prop};

/// Emit a document as an EPUB file.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut warnings = Vec::new();
    let mut output = Vec::new();

    // Create EPUB builder
    let zip = ZipLibrary::new().map_err(|e| {
        EmitError::Io(std::io::Error::other(format!(
            "Failed to create zip: {}",
            e
        )))
    })?;
    let mut builder = EpubBuilder::new(zip).map_err(|e| {
        EmitError::Io(std::io::Error::other(format!(
            "Failed to create builder: {}",
            e
        )))
    })?;

    // Add metadata
    if let Some(title) = doc.metadata.get_str("title") {
        builder
            .metadata("title", title)
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("Metadata error: {}", e))))?;
    } else {
        builder
            .metadata("title", "Untitled")
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("Metadata error: {}", e))))?;
    }

    if let Some(author) = doc.metadata.get_str("author") {
        builder
            .metadata("author", author)
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("Metadata error: {}", e))))?;
    }

    if let Some(language) = doc.metadata.get_str("language") {
        builder
            .metadata("lang", language)
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("Metadata error: {}", e))))?;
    } else {
        builder
            .metadata("lang", "en")
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("Metadata error: {}", e))))?;
    }

    // Split document into chapters (each h1 starts a new chapter)
    let chapters = split_into_chapters(&doc.content);

    for (i, chapter) in chapters.iter().enumerate() {
        let title = chapter
            .title
            .clone()
            .unwrap_or_else(|| format!("Chapter {}", i + 1));

        // Convert chapter content to HTML
        let html = chapter_to_html(chapter, &mut warnings)?;

        let content = EpubContent::new(format!("chapter{}.xhtml", i + 1), html.as_bytes())
            .title(title)
            .reftype(ReferenceType::Text);

        builder
            .add_content(content)
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("Content error: {}", e))))?;
    }

    // Generate the EPUB
    builder
        .generate(&mut output)
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("Generate error: {}", e))))?;

    Ok(ConversionResult::with_warnings(output, warnings))
}

struct Chapter {
    title: Option<String>,
    nodes: Vec<Node>,
}

fn split_into_chapters(root: &Node) -> Vec<Chapter> {
    let mut chapters = Vec::new();
    let mut current_nodes = Vec::new();
    let mut current_title: Option<String> = None;

    for node in &root.children {
        // Check if this is an h1 heading
        if node.kind.as_str() == node::HEADING {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            if level == 1 {
                // Start a new chapter
                if !current_nodes.is_empty() || current_title.is_some() {
                    chapters.push(Chapter {
                        title: current_title.take(),
                        nodes: std::mem::take(&mut current_nodes),
                    });
                }

                // Extract title from heading
                current_title = Some(extract_text(node));
                // Don't include the h1 itself in nodes (it will be the chapter title)
                continue;
            }
        }

        current_nodes.push(node.clone());
    }

    // Don't forget the last chapter
    if !current_nodes.is_empty() || current_title.is_some() {
        chapters.push(Chapter {
            title: current_title,
            nodes: current_nodes,
        });
    }

    // If no chapters were created (no h1 headings), create a single chapter
    if chapters.is_empty() {
        chapters.push(Chapter {
            title: None,
            nodes: root.children.clone(),
        });
    }

    chapters
}

fn extract_text(node: &Node) -> String {
    let mut text = String::new();
    extract_text_recursive(node, &mut text);
    text
}

fn extract_text_recursive(node: &Node, output: &mut String) {
    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        output.push_str(content);
    }
    for child in &node.children {
        extract_text_recursive(child, output);
    }
}

fn chapter_to_html(
    chapter: &Chapter,
    warnings: &mut Vec<FidelityWarning>,
) -> Result<String, EmitError> {
    // Create a temporary document with the chapter content
    let temp_doc =
        Document::new().with_content(Node::new(node::DOCUMENT).children(chapter.nodes.clone()));

    // Use HTML writer to convert
    let result = rescribe_write_html::emit_full_document(&temp_doc)
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("HTML emit error: {}", e))))?;

    warnings.extend(result.warnings);

    String::from_utf8(result.value)
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("UTF-8 error: {}", e))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::doc;

    #[test]
    fn test_emit_simple_epub() {
        let document = doc(|d| {
            d.heading(1, |i| i.text("Chapter 1"))
                .para(|i| i.text("Hello, world!"))
        });
        let result = emit(&document).unwrap();
        // EPUB is a ZIP file, so check for PK signature
        assert!(result.value.starts_with(b"PK"));
    }

    #[test]
    fn test_split_into_chapters() {
        let root = Node::new(node::DOCUMENT)
            .child(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 1i64)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "Ch1")),
            )
            .child(Node::new(node::PARAGRAPH))
            .child(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 1i64)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "Ch2")),
            )
            .child(Node::new(node::PARAGRAPH));

        let chapters = split_into_chapters(&root);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, Some("Ch1".to_string()));
        assert_eq!(chapters[1].title, Some("Ch2".to_string()));
    }
}
