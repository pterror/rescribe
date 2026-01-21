//! DocBook writer for rescribe.
//!
//! Serializes rescribe's document IR to DocBook 5 XML.
//!
//! # Example
//!
//! ```
//! use rescribe_write_docbook::emit;
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

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use rescribe_core::{ConversionResult, Document, EmitError, Node};
use rescribe_std::{node, prop};
use std::io::Cursor;

/// Emit a document to DocBook XML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let warnings = Vec::new();
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Start article element
    let mut article = BytesStart::new("article");
    article.push_attribute(("xmlns", "http://docbook.org/ns/docbook"));
    article.push_attribute(("version", "5.0"));
    writer
        .write_event(Event::Start(article))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Write title from metadata if present
    if let Some(title) = doc.metadata.get_str("title") {
        write_element(&mut writer, "title", title)?;
    }

    // Write content
    for child in &doc.content.children {
        write_node(&mut writer, child)?;
    }

    // End article
    writer
        .write_event(Event::End(BytesEnd::new("article")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    let result = writer.into_inner().into_inner();
    Ok(ConversionResult::with_warnings(result, warnings))
}

fn write_element(
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
        node::DOCUMENT | node::DIV => {
            for child in &node.children {
                write_node(writer, child)?;
            }
        }

        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as u32;

            // Write as section with title
            let section_tag = match level {
                1 => "section",
                2 => "sect1",
                3 => "sect2",
                4 => "sect3",
                5 => "sect4",
                _ => "sect5",
            };

            writer
                .write_event(Event::Start(BytesStart::new(section_tag)))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::Start(BytesStart::new("title")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            for child in &node.children {
                write_inline(writer, child)?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("title")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::End(BytesEnd::new(section_tag)))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::PARAGRAPH => {
            writer
                .write_event(Event::Start(BytesStart::new("para")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("para")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::BLOCKQUOTE => {
            writer
                .write_event(Event::Start(BytesStart::new("blockquote")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("blockquote")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let tag = if ordered {
                "orderedlist"
            } else {
                "itemizedlist"
            };

            writer
                .write_event(Event::Start(BytesStart::new(tag)))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new(tag)))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LIST_ITEM => {
            writer
                .write_event(Event::Start(BytesStart::new("listitem")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("listitem")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::DEFINITION_LIST => {
            writer
                .write_event(Event::Start(BytesStart::new("variablelist")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            // Process children in pairs (term, desc)
            let mut i = 0;
            while i < node.children.len() {
                writer
                    .write_event(Event::Start(BytesStart::new("varlistentry")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;

                if i < node.children.len() {
                    write_node(writer, &node.children[i])?;
                }
                if i + 1 < node.children.len() {
                    write_node(writer, &node.children[i + 1])?;
                }

                writer
                    .write_event(Event::End(BytesEnd::new("varlistentry")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;

                i += 2;
            }

            writer
                .write_event(Event::End(BytesEnd::new("variablelist")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::DEFINITION_TERM => {
            writer
                .write_event(Event::Start(BytesStart::new("term")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("term")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::DEFINITION_DESC => {
            writer
                .write_event(Event::Start(BytesStart::new("listitem")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("listitem")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::CODE_BLOCK => {
            let mut start = BytesStart::new("programlisting");
            if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
                start.push_attribute(("language", lang));
            }
            writer
                .write_event(Event::Start(start))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            if let Some(content) = node.props.get_str(prop::CONTENT) {
                writer
                    .write_event(Event::Text(BytesText::new(content)))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("programlisting")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE => {
            writer
                .write_event(Event::Start(BytesStart::new("informaltable")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::Start(BytesStart::new("tgroup")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::Start(BytesStart::new("tbody")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            for child in &node.children {
                write_node(writer, child)?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("tbody")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::End(BytesEnd::new("tgroup")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::End(BytesEnd::new("informaltable")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_ROW => {
            writer
                .write_event(Event::Start(BytesStart::new("row")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("row")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_CELL | node::TABLE_HEADER => {
            writer
                .write_event(Event::Start(BytesStart::new("entry")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("entry")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::FIGURE => {
            writer
                .write_event(Event::Start(BytesStart::new("figure")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("figure")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::IMAGE => {
            writer
                .write_event(Event::Start(BytesStart::new("mediaobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::Start(BytesStart::new("imageobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            let mut imagedata = BytesStart::new("imagedata");
            if let Some(url) = node.props.get_str(prop::URL) {
                imagedata.push_attribute(("fileref", url));
            }
            writer
                .write_event(Event::Empty(imagedata))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::End(BytesEnd::new("imageobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::End(BytesEnd::new("mediaobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::HORIZONTAL_RULE => {
            // DocBook doesn't have HR, skip
        }

        node::FOOTNOTE_DEF => {
            writer
                .write_event(Event::Start(BytesStart::new("footnote")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("footnote")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        // Inline nodes that appear at block level
        node::TEXT | node::EMPHASIS | node::STRONG | node::CODE | node::LINK => {
            // Wrap in para
            writer
                .write_event(Event::Start(BytesStart::new("para")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            write_inline(writer, node)?;
            writer
                .write_event(Event::End(BytesEnd::new("para")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        _ => {
            // Unknown block - recurse into children
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
            let mut emphasis = BytesStart::new("emphasis");
            emphasis.push_attribute(("role", "strong"));
            writer
                .write_event(Event::Start(emphasis))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("emphasis")))
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

        node::LINK => {
            let mut link = BytesStart::new("link");
            if let Some(url) = node.props.get_str(prop::URL) {
                link.push_attribute(("xlink:href", url));
            }
            writer
                .write_event(Event::Start(link))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("link")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SUBSCRIPT => {
            writer
                .write_event(Event::Start(BytesStart::new("subscript")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("subscript")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SUPERSCRIPT => {
            writer
                .write_event(Event::Start(BytesStart::new("superscript")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("superscript")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LINE_BREAK => {
            writer
                .write_event(Event::Empty(BytesStart::new("sbr")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SOFT_BREAK => {
            writer
                .write_event(Event::Text(BytesText::new(" ")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::IMAGE => {
            writer
                .write_event(Event::Start(BytesStart::new("inlinemediaobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::Start(BytesStart::new("imageobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            let mut img = BytesStart::new("imagedata");
            if let Some(url) = node.props.get_str(prop::URL) {
                img.push_attribute(("fileref", url));
            }
            writer
                .write_event(Event::Empty(img))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            writer
                .write_event(Event::End(BytesEnd::new("imageobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::End(BytesEnd::new("inlinemediaobject")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        _ => {
            // Unknown inline - recurse
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

    #[test]
    fn test_emit_empty() {
        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata: Properties::new(),
            source: None,
        };

        let result = emit(&doc).unwrap();
        let xml = String::from_utf8(result.value).unwrap();
        assert!(xml.contains("<article"));
        assert!(xml.contains("</article>"));
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = Document {
            content: Node::new(node::DOCUMENT).child(
                Node::new(node::PARAGRAPH)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "Hello, world!")),
            ),
            resources: Default::default(),
            metadata: Properties::new(),
            source: None,
        };

        let result = emit(&doc).unwrap();
        let xml = String::from_utf8(result.value).unwrap();
        assert!(xml.contains("<para>Hello, world!</para>"));
    }

    #[test]
    fn test_emit_with_title() {
        let mut metadata = Properties::new();
        metadata.set("title", "Test Document".to_string());

        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata,
            source: None,
        };

        let result = emit(&doc).unwrap();
        let xml = String::from_utf8(result.value).unwrap();
        assert!(xml.contains("<title>Test Document</title>"));
    }
}
