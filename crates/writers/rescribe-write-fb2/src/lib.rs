//! FictionBook 2 (FB2) writer for rescribe.
//!
//! Serializes rescribe's document IR to FB2 XML.
//!
//! # Example
//!
//! ```
//! use rescribe_write_fb2::emit;
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
//! let xml = String::from_utf8(result.value).unwrap();
//! ```

use base64::Engine;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};
use std::io::Cursor;

/// Emit a document to FB2 XML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to FB2 XML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let warnings = Vec::new();
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Start FictionBook element
    let mut fb = BytesStart::new("FictionBook");
    fb.push_attribute(("xmlns", "http://www.gribuser.ru/xml/fictionbook/2.0"));
    fb.push_attribute(("xmlns:l", "http://www.w3.org/1999/xlink"));
    writer
        .write_event(Event::Start(fb))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Write description
    write_description(&mut writer, doc)?;

    // Write body
    write_body(&mut writer, doc)?;

    // Write binary resources (images)
    write_binaries(&mut writer, doc)?;

    // End FictionBook
    writer
        .write_event(Event::End(BytesEnd::new("FictionBook")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    let result = writer.into_inner().into_inner();
    Ok(ConversionResult::with_warnings(result, warnings))
}

fn write_description(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    doc: &Document,
) -> Result<(), EmitError> {
    writer
        .write_event(Event::Start(BytesStart::new("description")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // title-info
    writer
        .write_event(Event::Start(BytesStart::new("title-info")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Genre (default to prose)
    write_simple_element(writer, "genre", "prose")?;

    // Author
    if let Some(author) = doc.metadata.get_str("author") {
        writer
            .write_event(Event::Start(BytesStart::new("author")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Try to split name
        let parts: Vec<&str> = author.split_whitespace().collect();
        if parts.len() >= 2 {
            write_simple_element(writer, "first-name", parts[0])?;
            write_simple_element(writer, "last-name", parts[parts.len() - 1])?;
        } else if !parts.is_empty() {
            write_simple_element(writer, "nickname", parts[0])?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("author")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
    }

    // Book title
    if let Some(title) = doc.metadata.get_str("title") {
        write_simple_element(writer, "book-title", title)?;
    } else {
        write_simple_element(writer, "book-title", "Untitled")?;
    }

    // Language
    if let Some(lang) = doc.metadata.get_str("lang") {
        write_simple_element(writer, "lang", lang)?;
    } else {
        write_simple_element(writer, "lang", "en")?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("title-info")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // document-info
    writer
        .write_event(Event::Start(BytesStart::new("document-info")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    write_simple_element(writer, "program-used", "rescribe")?;

    writer
        .write_event(Event::End(BytesEnd::new("document-info")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    writer
        .write_event(Event::End(BytesEnd::new("description")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    Ok(())
}

fn write_body(writer: &mut Writer<Cursor<Vec<u8>>>, doc: &Document) -> Result<(), EmitError> {
    writer
        .write_event(Event::Start(BytesStart::new("body")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Wrap content in a section
    writer
        .write_event(Event::Start(BytesStart::new("section")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    for child in &doc.content.children {
        write_node(writer, child)?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("section")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    writer
        .write_event(Event::End(BytesEnd::new("body")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    Ok(())
}

fn write_binaries(writer: &mut Writer<Cursor<Vec<u8>>>, doc: &Document) -> Result<(), EmitError> {
    for (id, resource) in &doc.resources {
        let mut binary = BytesStart::new("binary");
        binary.push_attribute(("id", id.as_str()));
        if !resource.mime_type.is_empty() {
            binary.push_attribute(("content-type", resource.mime_type.as_str()));
        }

        writer
            .write_event(Event::Start(binary))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        let encoded = base64::engine::general_purpose::STANDARD.encode(&resource.data);
        writer
            .write_event(Event::Text(BytesText::new(&encoded)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        writer
            .write_event(Event::End(BytesEnd::new("binary")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
    }

    Ok(())
}

fn write_simple_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    text: &str,
) -> Result<(), EmitError> {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
    writer
        .write_event(Event::Text(BytesText::new(text)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
    Ok(())
}

fn write_node(writer: &mut Writer<Cursor<Vec<u8>>>, node: &Node) -> Result<(), EmitError> {
    match node.kind.as_str() {
        node::DOCUMENT | node::DIV | node::SPAN => {
            for child in &node.children {
                write_node(writer, child)?;
            }
        }

        node::HEADING => {
            // Write as title (wrapped in p elements)
            writer
                .write_event(Event::Start(BytesStart::new("title")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::Start(BytesStart::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            for child in &node.children {
                write_inline(writer, child)?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::End(BytesEnd::new("title")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::PARAGRAPH => {
            writer
                .write_event(Event::Start(BytesStart::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::BLOCKQUOTE => {
            writer
                .write_event(Event::Start(BytesStart::new("cite")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("cite")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LIST => {
            // FB2 doesn't have native lists - render as paragraphs with markers
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let mut num = 1;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    writer
                        .write_event(Event::Start(BytesStart::new("p")))
                        .map_err(|e| {
                            EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                        })?;

                    // Write marker
                    let marker = if ordered {
                        let m = format!("{}. ", num);
                        num += 1;
                        m
                    } else {
                        "• ".to_string()
                    };
                    writer
                        .write_event(Event::Text(BytesText::new(&marker)))
                        .map_err(|e| {
                            EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                        })?;

                    // Write item content
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            for inline in &item_child.children {
                                write_inline(writer, inline)?;
                            }
                        } else {
                            write_inline(writer, item_child)?;
                        }
                    }

                    writer
                        .write_event(Event::End(BytesEnd::new("p")))
                        .map_err(|e| {
                            EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                        })?;
                }
            }
        }

        node::LIST_ITEM => {
            // Should be handled by LIST
            for child in &node.children {
                write_node(writer, child)?;
            }
        }

        node::CODE_BLOCK => {
            // FB2 doesn't have code blocks - render as paragraphs with code element
            writer
                .write_event(Event::Start(BytesStart::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::Start(BytesStart::new("code")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            if let Some(content) = node.props.get_str(prop::CONTENT) {
                writer
                    .write_event(Event::Text(BytesText::new(content)))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("code")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::End(BytesEnd::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::HORIZONTAL_RULE => {
            // Use empty-line
            writer
                .write_event(Event::Empty(BytesStart::new("empty-line")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE => {
            writer
                .write_event(Event::Start(BytesStart::new("table")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("table")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_ROW => {
            writer
                .write_event(Event::Start(BytesStart::new("tr")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("tr")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_CELL => {
            writer
                .write_event(Event::Start(BytesStart::new("td")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("td")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_HEADER => {
            writer
                .write_event(Event::Start(BytesStart::new("th")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("th")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::IMAGE => {
            let mut img = BytesStart::new("image");
            if let Some(url) = node.props.get_str(prop::URL) {
                // Add # prefix for internal references
                let href = if url.starts_with('#') || url.contains("://") {
                    url.to_string()
                } else {
                    format!("#{}", url)
                };
                img.push_attribute(("l:href", href.as_str()));
            }
            writer
                .write_event(Event::Empty(img))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::FIGURE => {
            for child in &node.children {
                write_node(writer, child)?;
            }
        }

        // Inline nodes at block level - wrap in p
        node::TEXT | node::EMPHASIS | node::STRONG | node::CODE | node::LINK => {
            writer
                .write_event(Event::Start(BytesStart::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            write_inline(writer, node)?;
            writer
                .write_event(Event::End(BytesEnd::new("p")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        _ => {
            for child in &node.children {
                write_node(writer, child)?;
            }
        }
    }

    Ok(())
}

fn write_inline(writer: &mut Writer<Cursor<Vec<u8>>>, node: &Node) -> Result<(), EmitError> {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                writer
                    .write_event(Event::Text(BytesText::new(content)))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }
        }

        node::EMPHASIS => {
            writer
                .write_event(Event::Start(BytesStart::new("emphasis")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("emphasis")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::STRONG => {
            writer
                .write_event(Event::Start(BytesStart::new("strong")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("strong")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::STRIKEOUT => {
            writer
                .write_event(Event::Start(BytesStart::new("strikethrough")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("strikethrough")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::CODE => {
            writer
                .write_event(Event::Start(BytesStart::new("code")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                writer
                    .write_event(Event::Text(BytesText::new(content)))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("code")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SUBSCRIPT => {
            writer
                .write_event(Event::Start(BytesStart::new("sub")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("sub")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SUPERSCRIPT => {
            writer
                .write_event(Event::Start(BytesStart::new("sup")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("sup")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LINK => {
            let mut link = BytesStart::new("a");
            if let Some(url) = node.props.get_str(prop::URL) {
                link.push_attribute(("l:href", url));
            }
            writer
                .write_event(Event::Start(link))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("a")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::IMAGE => {
            let mut img = BytesStart::new("image");
            if let Some(url) = node.props.get_str(prop::URL) {
                let href = if url.starts_with('#') || url.contains("://") {
                    url.to_string()
                } else {
                    format!("#{}", url)
                };
                img.push_attribute(("l:href", href.as_str()));
            }
            writer
                .write_event(Event::Empty(img))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LINE_BREAK => {
            // FB2 doesn't have inline line break - close and reopen p would break context
            writer
                .write_event(Event::Text(BytesText::new("\n")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SOFT_BREAK => {
            writer
                .write_event(Event::Text(BytesText::new(" ")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        _ => {
            for child in &node.children {
                write_inline(writer, child)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::Properties;
    use rescribe_std::builder::*;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_empty() {
        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata: Properties::new(),
            source: None,
        };

        let xml = emit_str(&doc);
        assert!(xml.contains("<FictionBook"));
        assert!(xml.contains("</FictionBook>"));
        assert!(xml.contains("<book-title>Untitled</book-title>"));
    }

    #[test]
    fn test_emit_with_title() {
        let mut metadata = Properties::new();
        metadata.set("title", "Test Book".to_string());

        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata,
            source: None,
        };

        let xml = emit_str(&doc);
        assert!(xml.contains("<book-title>Test Book</book-title>"));
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = doc(|d| d.para(|p| p.text("Hello, world!")));
        let xml = emit_str(&doc);
        assert!(xml.contains("<p>Hello, world!</p>"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(1, |h| h.text("Chapter Title")));
        let xml = emit_str(&doc);
        assert!(xml.contains("<title><p>Chapter Title</p></title>"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let xml = emit_str(&doc);
        assert!(xml.contains("<emphasis>italic</emphasis>"));
    }

    #[test]
    fn test_emit_strong() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let xml = emit_str(&doc);
        assert!(xml.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("http://example.com", |l| l.text("click"))));
        let xml = emit_str(&doc);
        assert!(xml.contains("<a l:href=\"http://example.com\">click</a>"));
    }

    #[test]
    fn test_emit_blockquote() {
        let doc = doc(|d| d.blockquote(|b| b.para(|p| p.text("Quoted text"))));
        let xml = emit_str(&doc);
        assert!(xml.contains("<cite>"));
        assert!(xml.contains("Quoted text"));
        assert!(xml.contains("</cite>"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let xml = emit_str(&doc);
        assert!(xml.contains("• one"));
        assert!(xml.contains("• two"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc =
            doc(|d| d.ordered_list(|l| l.item(|i| i.text("first")).item(|i| i.text("second"))));
        let xml = emit_str(&doc);
        assert!(xml.contains("1. first"));
        assert!(xml.contains("2. second"));
    }
}
