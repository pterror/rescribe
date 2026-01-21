//! JATS XML writer for rescribe.
//!
//! Serializes rescribe's document IR to JATS (Journal Article Tag Suite) XML.

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use rescribe_core::{ConversionResult, Document, EmitError, Node};
use rescribe_std::{node, prop};
use std::io::Cursor;

/// Emit a document to JATS XML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let warnings = Vec::new();
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Start article element
    let mut article = BytesStart::new("article");
    article.push_attribute(("xmlns:xlink", "http://www.w3.org/1999/xlink"));
    article.push_attribute(("article-type", "research-article"));
    writer
        .write_event(Event::Start(article))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    // Write front matter if we have metadata
    let has_title = doc.metadata.get_str("title").is_some();
    if has_title {
        writer
            .write_event(Event::Start(BytesStart::new("front")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        writer
            .write_event(Event::Start(BytesStart::new("article-meta")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        if let Some(title) = doc.metadata.get_str("title") {
            writer
                .write_event(Event::Start(BytesStart::new("title-group")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            write_element(&mut writer, "article-title", title)?;
            writer
                .write_event(Event::End(BytesEnd::new("title-group")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("article-meta")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        writer
            .write_event(Event::End(BytesEnd::new("front")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
    }

    // Write body
    writer
        .write_event(Event::Start(BytesStart::new("body")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

    for child in &doc.content.children {
        write_node(&mut writer, child)?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("body")))
        .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

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
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);

            // Level 1 headings become sections, others become sec with title
            if level == 1 {
                writer
                    .write_event(Event::Start(BytesStart::new("sec")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::Start(BytesStart::new("title")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                for child in &node.children {
                    write_inline(writer, child)?;
                }
                writer
                    .write_event(Event::End(BytesEnd::new("title")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::End(BytesEnd::new("sec")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            } else {
                writer
                    .write_event(Event::Start(BytesStart::new("sec")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::Start(BytesStart::new("title")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                for child in &node.children {
                    write_inline(writer, child)?;
                }
                writer
                    .write_event(Event::End(BytesEnd::new("title")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::End(BytesEnd::new("sec")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }
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
                .write_event(Event::Start(BytesStart::new("disp-quote")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("disp-quote")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let mut list = BytesStart::new("list");
            if ordered {
                list.push_attribute(("list-type", "order"));
            } else {
                list.push_attribute(("list-type", "bullet"));
            }

            writer
                .write_event(Event::Start(list))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("list")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LIST_ITEM => {
            writer
                .write_event(Event::Start(BytesStart::new("list-item")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("list-item")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::DEFINITION_LIST => {
            writer
                .write_event(Event::Start(BytesStart::new("def-list")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            // Process children in pairs (term, desc)
            let mut i = 0;
            while i < node.children.len() {
                writer
                    .write_event(Event::Start(BytesStart::new("def-item")))
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
                    .write_event(Event::End(BytesEnd::new("def-item")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;

                i += 2;
            }

            writer
                .write_event(Event::End(BytesEnd::new("def-list")))
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
                .write_event(Event::Start(BytesStart::new("def")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("def")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::CODE_BLOCK => {
            let mut code = BytesStart::new("code");
            if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
                code.push_attribute(("language", lang));
            }
            writer
                .write_event(Event::Start(code))
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
        }

        node::TABLE => {
            writer
                .write_event(Event::Start(BytesStart::new("table-wrap")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::Start(BytesStart::new("table")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

            // Check if we have thead/tbody structure
            let has_structure = node.children.iter().any(|c| {
                c.kind.as_str() == node::TABLE_HEAD || c.kind.as_str() == node::TABLE_BODY
            });

            if has_structure {
                for child in &node.children {
                    write_node(writer, child)?;
                }
            } else {
                // Wrap in tbody
                writer
                    .write_event(Event::Start(BytesStart::new("tbody")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                for child in &node.children {
                    write_node(writer, child)?;
                }
                writer
                    .write_event(Event::End(BytesEnd::new("tbody")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("table")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            writer
                .write_event(Event::End(BytesEnd::new("table-wrap")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_HEAD => {
            writer
                .write_event(Event::Start(BytesStart::new("thead")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("thead")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::TABLE_BODY => {
            writer
                .write_event(Event::Start(BytesStart::new("tbody")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("tbody")))
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

        node::FIGURE => {
            writer
                .write_event(Event::Start(BytesStart::new("fig")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("fig")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::IMAGE => {
            let mut graphic = BytesStart::new("graphic");
            if let Some(url) = node.props.get_str(prop::URL) {
                graphic.push_attribute(("xlink:href", url));
            }
            writer
                .write_event(Event::Empty(graphic))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::HORIZONTAL_RULE => {
            // JATS doesn't have HR, skip
        }

        node::FOOTNOTE_DEF => {
            writer
                .write_event(Event::Start(BytesStart::new("fn")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_node(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("fn")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        "math_display" => {
            writer
                .write_event(Event::Start(BytesStart::new("disp-formula")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            if let Some(source) = node.props.get_str("math:source") {
                writer
                    .write_event(Event::Start(BytesStart::new("tex-math")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::Text(BytesText::new(source)))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::End(BytesEnd::new("tex-math")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("disp-formula")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        // Inline nodes that appear at block level
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
                .write_event(Event::Start(BytesStart::new("italic")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("italic")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::STRONG => {
            writer
                .write_event(Event::Start(BytesStart::new("bold")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("bold")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::UNDERLINE => {
            writer
                .write_event(Event::Start(BytesStart::new("underline")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("underline")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::STRIKEOUT => {
            writer
                .write_event(Event::Start(BytesStart::new("strike")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("strike")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::CODE => {
            writer
                .write_event(Event::Start(BytesStart::new("monospace")))
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
                .write_event(Event::End(BytesEnd::new("monospace")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LINK => {
            let mut link = BytesStart::new("ext-link");
            if let Some(url) = node.props.get_str(prop::URL) {
                link.push_attribute(("xlink:href", url));
                link.push_attribute(("ext-link-type", "uri"));
            }
            writer
                .write_event(Event::Start(link))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("ext-link")))
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

        node::SMALL_CAPS => {
            writer
                .write_event(Event::Start(BytesStart::new("sc")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            for child in &node.children {
                write_inline(writer, child)?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("sc")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::LINE_BREAK => {
            writer
                .write_event(Event::Empty(BytesStart::new("break")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::SOFT_BREAK => {
            writer
                .write_event(Event::Text(BytesText::new(" ")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        node::IMAGE => {
            let mut graphic = BytesStart::new("inline-graphic");
            if let Some(url) = node.props.get_str(prop::URL) {
                graphic.push_attribute(("xlink:href", url));
            }
            writer
                .write_event(Event::Empty(graphic))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        }

        "math_inline" => {
            writer
                .write_event(Event::Start(BytesStart::new("inline-formula")))
                .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
            if let Some(source) = node.props.get_str("math:source") {
                writer
                    .write_event(Event::Start(BytesStart::new("tex-math")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::Text(BytesText::new(source)))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
                writer
                    .write_event(Event::End(BytesEnd::new("tex-math")))
                    .map_err(|e| {
                        EmitError::Io(std::io::Error::other(format!("XML error: {}", e)))
                    })?;
            }
            writer
                .write_event(Event::End(BytesEnd::new("inline-formula")))
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
        assert!(xml.contains("<p>Hello, world!</p>"));
    }

    #[test]
    fn test_emit_with_title() {
        let mut metadata = Properties::new();
        metadata.set("title", "Test Article".to_string());

        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata,
            source: None,
        };

        let result = emit(&doc).unwrap();
        let xml = String::from_utf8(result.value).unwrap();
        assert!(xml.contains("<article-title>Test Article</article-title>"));
    }

    #[test]
    fn test_emit_formatting() {
        let doc = Document {
            content: Node::new(node::DOCUMENT).child(
                Node::new(node::PARAGRAPH)
                    .child(
                        Node::new(node::EMPHASIS)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, "italic")),
                    )
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, " and "))
                    .child(
                        Node::new(node::STRONG)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, "bold")),
                    ),
            ),
            resources: Default::default(),
            metadata: Properties::new(),
            source: None,
        };

        let result = emit(&doc).unwrap();
        let xml = String::from_utf8(result.value).unwrap();
        assert!(xml.contains("<italic>italic</italic>"));
        assert!(xml.contains("<bold>bold</bold>"));
    }
}
