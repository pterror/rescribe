//! OPML writer for rescribe.
//!
//! Emits rescribe's document IR as OPML (Outline Processor Markup Language).
//! Lists are converted to outline elements.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_write_opml::emit;
//!
//! let doc = Document::new();
//! let result = emit(&doc)?;
//! let opml = String::from_utf8(result.value).unwrap();
//! ```

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use rescribe_core::{ConversionResult, Document, EmitError, Node};
use rescribe_std::{node, prop};
use std::io::Cursor;

/// Emit a document as OPML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    let warnings = Vec::new();

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    // OPML root
    let mut opml = BytesStart::new("opml");
    opml.push_attribute(("version", "2.0"));
    writer
        .write_event(Event::Start(opml))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    // Head section
    writer
        .write_event(Event::Start(BytesStart::new("head")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    // Title
    if let Some(title) = doc.metadata.get_str("title") {
        writer
            .write_event(Event::Start(BytesStart::new("title")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;
        writer
            .write_event(Event::Text(BytesText::new(title)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;
        writer
            .write_event(Event::End(BytesEnd::new("title")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;
    }

    // Author as ownerName
    if let Some(author) = doc.metadata.get_str("author") {
        writer
            .write_event(Event::Start(BytesStart::new("ownerName")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;
        writer
            .write_event(Event::Text(BytesText::new(author)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;
        writer
            .write_event(Event::End(BytesEnd::new("ownerName")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("head")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    // Body section
    writer
        .write_event(Event::Start(BytesStart::new("body")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    // Convert document content to outlines
    write_outlines(&mut writer, &doc.content)?;

    writer
        .write_event(Event::End(BytesEnd::new("body")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    writer
        .write_event(Event::End(BytesEnd::new("opml")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML write error: {}", e))))?;

    let result = writer.into_inner().into_inner();
    Ok(ConversionResult::with_warnings(result, warnings))
}

fn write_outlines<W: std::io::Write>(writer: &mut Writer<W>, node: &Node) -> Result<(), EmitError> {
    match node.kind.as_str() {
        node::DOCUMENT => {
            for child in &node.children {
                write_outlines(writer, child)?;
            }
        }
        node::LIST => {
            for child in &node.children {
                write_outlines(writer, child)?;
            }
        }
        node::LIST_ITEM => {
            // Extract text and URL from the list item content
            let (text, url) = extract_outline_content(node);

            // Check if there are nested lists
            let nested_lists: Vec<&Node> = node
                .children
                .iter()
                .filter(|c| c.kind.as_str() == node::LIST)
                .collect();

            if nested_lists.is_empty() {
                // Empty element
                let mut outline = BytesStart::new("outline");
                outline.push_attribute(("text", text.as_str()));
                if let Some(url) = url {
                    outline.push_attribute(("xmlUrl", url.as_str()));
                }
                writer.write_event(Event::Empty(outline)).map_err(|e| {
                    EmitError::Io(std::io::Error::other(format!("XML write error: {}", e)))
                })?;
            } else {
                // Start element with children
                let mut outline = BytesStart::new("outline");
                outline.push_attribute(("text", text.as_str()));
                if let Some(url) = url {
                    outline.push_attribute(("xmlUrl", url.as_str()));
                }
                writer.write_event(Event::Start(outline)).map_err(|e| {
                    EmitError::Io(std::io::Error::other(format!("XML write error: {}", e)))
                })?;

                for nested in nested_lists {
                    write_outlines(writer, nested)?;
                }

                writer
                    .write_event(Event::End(BytesEnd::new("outline")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML write error: {}", e)))
                    })?;
            }
        }
        node::HEADING => {
            // Convert headings to outlines
            let text = extract_text(node);
            let mut outline = BytesStart::new("outline");
            outline.push_attribute(("text", text.as_str()));
            writer.write_event(Event::Empty(outline)).map_err(|e| {
                EmitError::Io(std::io::Error::other(format!("XML write error: {}", e)))
            })?;
        }
        node::PARAGRAPH => {
            // Convert paragraphs to outlines
            let (text, url) = extract_paragraph_content(node);
            let mut outline = BytesStart::new("outline");
            outline.push_attribute(("text", text.as_str()));
            if let Some(url) = url {
                outline.push_attribute(("xmlUrl", url.as_str()));
            }
            writer.write_event(Event::Empty(outline)).map_err(|e| {
                EmitError::Io(std::io::Error::other(format!("XML write error: {}", e)))
            })?;
        }
        _ => {
            // Try to process children
            for child in &node.children {
                write_outlines(writer, child)?;
            }
        }
    }
    Ok(())
}

fn extract_outline_content(list_item: &Node) -> (String, Option<String>) {
    let mut text = String::new();
    let mut url: Option<String> = None;

    for child in &list_item.children {
        if child.kind.as_str() == node::LIST {
            continue; // Skip nested lists
        }
        let (t, u) = extract_paragraph_content(child);
        if !t.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&t);
        }
        if url.is_none() && u.is_some() {
            url = u;
        }
    }

    (text, url)
}

fn extract_paragraph_content(node: &Node) -> (String, Option<String>) {
    let mut text = String::new();
    let mut url: Option<String> = None;

    for child in &node.children {
        match child.kind.as_str() {
            node::TEXT => {
                if let Some(content) = child.props.get_str(prop::CONTENT) {
                    text.push_str(content);
                }
            }
            node::LINK => {
                if url.is_none() {
                    url = child.props.get_str(prop::URL).map(|s| s.to_string());
                }
                // Get link text
                let link_text = extract_text(child);
                text.push_str(&link_text);
            }
            _ => {
                // Recursively extract text from other nodes
                let child_text = extract_text(child);
                text.push_str(&child_text);
            }
        }
    }

    (text, url)
}

fn extract_text(node: &Node) -> String {
    let mut result = String::new();
    extract_text_recursive(node, &mut result);
    result
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

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::doc;

    #[test]
    fn test_emit_simple_list() {
        let document =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));

        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("<opml"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
    }

    #[test]
    fn test_emit_with_metadata() {
        let mut document = doc(|d| d.para(|i| i.text("Test")));
        document.metadata.set("title", "My Outline");
        document.metadata.set("author", "John Doe");

        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("<title>My Outline</title>"));
        assert!(output.contains("<ownerName>John Doe</ownerName>"));
    }
}
