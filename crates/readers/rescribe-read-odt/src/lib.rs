//! ODT (OpenDocument Text) reader for rescribe.
//!
//! Parses ODF/ODT documents into rescribe's document IR.

use quick_xml::Reader;
use quick_xml::events::Event;
use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions, Properties};
use rescribe_std::{Node, node, prop};
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Parse ODT input into a document.
pub fn parse(input: &[u8]) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse ODT input into a document with options.
pub fn parse_with_options(
    input: &[u8],
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let cursor = Cursor::new(input);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| ParseError::Invalid(format!("Invalid ODT: {}", e)))?;

    let mut metadata = Properties::new();

    // Read meta.xml for metadata
    if let Ok(mut meta_file) = archive.by_name("meta.xml") {
        let mut meta_content = String::new();
        meta_file
            .read_to_string(&mut meta_content)
            .map_err(ParseError::Io)?;
        parse_metadata(&meta_content, &mut metadata);
    }

    // Read content.xml
    let mut content_file = archive
        .by_name("content.xml")
        .map_err(|e| ParseError::Invalid(format!("Missing content.xml: {}", e)))?;

    let mut content_xml = String::new();
    content_file
        .read_to_string(&mut content_xml)
        .map_err(ParseError::Io)?;

    let content = parse_content(&content_xml)?;

    Ok(ConversionResult::ok(Document {
        content,
        resources: Default::default(),
        metadata,
        source: None,
    }))
}

fn parse_metadata(xml: &str, metadata: &mut Properties) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_element = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_element = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                match current_element.as_str() {
                    "dc:title" => {
                        metadata.set("title", text);
                    }
                    "dc:creator" => {
                        metadata.set("author", text);
                    }
                    "dc:date" => {
                        metadata.set("date", text);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_content(xml: &str) -> Result<Node, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut doc = Node::new(node::DOCUMENT);
    let mut buf = Vec::new();
    let mut text_content = String::new();
    let mut in_paragraph = false;
    let mut in_heading = false;
    let mut _in_list = false;
    let mut in_list_item = false;
    let mut current_style = String::new();
    let mut pending_children: Vec<Node> = Vec::new();
    let mut list_stack: Vec<Node> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match name.as_str() {
                    "text:p" => {
                        in_paragraph = true;
                        text_content.clear();
                        pending_children.clear();

                        // Check for heading style
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"text:style-name" {
                                current_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }

                        if current_style.starts_with("Heading") {
                            in_heading = true;
                        }
                    }
                    "text:h" => {
                        in_heading = true;
                        in_paragraph = true;
                        text_content.clear();
                        pending_children.clear();

                        // Get outline level
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"text:outline-level" {
                                current_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "text:list" => {
                        _in_list = true;
                        list_stack.push(Node::new(node::LIST));
                    }
                    "text:list-item" => {
                        in_list_item = true;
                    }
                    "text:span" => {
                        // Check for bold/italic styles - flush pending text
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"text:style-name" {
                                let style = String::from_utf8_lossy(&attr.value).to_string();
                                let is_styled = style.contains("Bold")
                                    || style.contains("bold")
                                    || style.contains("Italic")
                                    || style.contains("italic");
                                if is_styled && !text_content.is_empty() {
                                    pending_children.push(
                                        Node::new(node::TEXT)
                                            .prop(prop::CONTENT, text_content.clone()),
                                    );
                                    text_content.clear();
                                }
                            }
                        }
                    }
                    "text:a" => {
                        // Link
                        if !text_content.is_empty() {
                            pending_children.push(
                                Node::new(node::TEXT).prop(prop::CONTENT, text_content.clone()),
                            );
                            text_content.clear();
                        }
                    }
                    "text:line-break" => {
                        pending_children.push(Node::new(node::LINE_BREAK));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match name.as_str() {
                    "text:p" | "text:h" => {
                        if in_paragraph {
                            if !text_content.is_empty() {
                                pending_children.push(
                                    Node::new(node::TEXT).prop(prop::CONTENT, text_content.clone()),
                                );
                            }

                            let node = if in_heading {
                                let level: i64 = current_style.parse().unwrap_or(1);
                                let mut heading = Node::new(node::HEADING).prop(prop::LEVEL, level);
                                for child in pending_children.drain(..) {
                                    heading = heading.child(child);
                                }
                                heading
                            } else {
                                let mut para = Node::new(node::PARAGRAPH);
                                for child in pending_children.drain(..) {
                                    para = para.child(child);
                                }
                                para
                            };

                            if in_list_item {
                                if let Some(list) = list_stack.last_mut() {
                                    let item = Node::new(node::LIST_ITEM).child(node);
                                    *list = list.clone().child(item);
                                }
                            } else {
                                doc = doc.child(node);
                            }

                            in_paragraph = false;
                            in_heading = false;
                            text_content.clear();
                            current_style.clear();
                        }
                    }
                    "text:list" => {
                        if let Some(list) = list_stack.pop() {
                            if list_stack.is_empty() {
                                doc = doc.child(list);
                            } else if let Some(parent) = list_stack.last_mut() {
                                *parent = parent.clone().child(list);
                            }
                        }
                        _in_list = list_stack.is_empty();
                    }
                    "text:list-item" => {
                        in_list_item = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_paragraph {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    text_content.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParseError::Invalid(format!("XML error: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn create_test_odt(content_xml: &str) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default();

            zip.start_file("mimetype", options).unwrap();
            zip.write_all(b"application/vnd.oasis.opendocument.text")
                .unwrap();

            zip.start_file("content.xml", options).unwrap();
            zip.write_all(content_xml.as_bytes()).unwrap();

            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn test_parse_basic() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                         xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:body>
    <office:text>
      <text:p>Hello world</text:p>
    </office:text>
  </office:body>
</office:document-content>"#;

        let odt = create_test_odt(content);
        let result = parse(&odt).unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_heading() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                         xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:body>
    <office:text>
      <text:h text:outline-level="1">Title</text:h>
    </office:text>
  </office:body>
</office:document-content>"#;

        let odt = create_test_odt(content);
        let result = parse(&odt).unwrap();
        let heading = &result.value.content.children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
    }
}
