//! OPML reader for rescribe.
//!
//! Parses OPML (Outline Processor Markup Language) into rescribe's document IR.
//! Outlines are converted to nested lists.
//!
//! # Example
//!
//! ```
//! use rescribe_read_opml::parse;
//!
//! let opml = r#"<?xml version="1.0"?>
//! <opml version="2.0">
//!   <head><title>Example</title></head>
//!   <body>
//!     <outline text="Item 1"/>
//!     <outline text="Item 2"/>
//!   </body>
//! </opml>"#;
//!
//! let result = parse(opml).unwrap();
//! let doc = result.value;
//! ```

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse OPML text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut converter = Converter::new();
    converter.parse(&mut reader)?;

    let document = Document {
        content: Node::new(node::DOCUMENT).children(converter.result),
        resources: Default::default(),
        metadata: converter.metadata,
        source: None,
    };

    Ok(ConversionResult::with_warnings(
        document,
        converter.warnings,
    ))
}

struct Converter {
    result: Vec<Node>,
    metadata: Properties,
    warnings: Vec<FidelityWarning>,
    in_head: bool,
    outline_stack: Vec<Vec<Node>>,
}

impl Converter {
    fn new() -> Self {
        Self {
            result: Vec::new(),
            metadata: Properties::new(),
            warnings: Vec::new(),
            in_head: false,
            outline_stack: Vec::new(),
        }
    }

    fn parse(&mut self, reader: &mut Reader<&[u8]>) -> Result<(), ParseError> {
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    self.handle_start(&e, reader)?;
                }
                Ok(Event::Empty(e)) => {
                    self.handle_empty(&e)?;
                }
                Ok(Event::End(e)) => {
                    self.handle_end(&e)?;
                }
                Ok(Event::Eof) => break,
                Ok(_) => {} // Ignore other events
                Err(e) => {
                    return Err(ParseError::Invalid(format!("XML parse error: {}", e)));
                }
            }
            buf.clear();
        }

        // Build final list from remaining outlines
        self.finalize_outlines();

        Ok(())
    }

    fn handle_start(
        &mut self,
        e: &BytesStart<'_>,
        reader: &mut Reader<&[u8]>,
    ) -> Result<(), ParseError> {
        match e.local_name().as_ref() {
            b"head" => {
                self.in_head = true;
            }
            b"title" if self.in_head => {
                if let Ok(text) = reader.read_text(e.to_end().name()) {
                    self.metadata.set("title", text.to_string());
                }
            }
            b"dateCreated" if self.in_head => {
                if let Ok(text) = reader.read_text(e.to_end().name()) {
                    self.metadata.set("date", text.to_string());
                }
            }
            b"ownerName" if self.in_head => {
                if let Ok(text) = reader.read_text(e.to_end().name()) {
                    self.metadata.set("author", text.to_string());
                }
            }
            b"outline" => {
                let outline_node = self.create_outline_node(e);
                // Push a new level for nested outlines
                self.outline_stack.push(vec![outline_node]);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_empty(&mut self, e: &BytesStart<'_>) -> Result<(), ParseError> {
        if e.local_name().as_ref() == b"outline" {
            let outline_node = self.create_outline_node(e);
            // Add to current level or result
            if let Some(current) = self.outline_stack.last_mut() {
                // Add as sibling to the last item in current level
                current.push(outline_node);
            } else {
                self.result.push(outline_node);
            }
        }
        Ok(())
    }

    fn handle_end(&mut self, e: &quick_xml::events::BytesEnd<'_>) -> Result<(), ParseError> {
        match e.local_name().as_ref() {
            b"head" => {
                self.in_head = false;
            }
            b"outline" => {
                // Pop the current level and merge with parent
                if let Some(children) = self.outline_stack.pop() {
                    if children.is_empty() {
                        return Ok(());
                    }

                    // The first item in children is the outline node itself
                    // The rest are its nested children
                    let mut items: Vec<Node> = children;

                    // Convert to list items
                    let list_items: Vec<Node> = items
                        .drain(..)
                        .map(|n| {
                            if n.kind.as_str() == node::LIST_ITEM {
                                n
                            } else {
                                // Convert outline node to list item
                                Node::new(node::LIST_ITEM).children(vec![n])
                            }
                        })
                        .collect();

                    // Create a list from the items
                    let list = Node::new(node::LIST)
                        .prop(prop::ORDERED, false)
                        .children(list_items);

                    if let Some(parent) = self.outline_stack.last_mut() {
                        // Add as child to parent's last item
                        if let Some(last) = parent.last_mut() {
                            last.children.push(list);
                        } else {
                            parent.push(list);
                        }
                    } else {
                        self.result.push(list);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn create_outline_node(&self, e: &BytesStart<'_>) -> Node {
        let mut text = String::new();
        let mut url: Option<String> = None;

        for attr in e.attributes().flatten() {
            match attr.key.local_name().as_ref() {
                b"text" => {
                    text = String::from_utf8_lossy(&attr.value).to_string();
                }
                b"title" if text.is_empty() => {
                    text = String::from_utf8_lossy(&attr.value).to_string();
                }
                b"xmlUrl" | b"htmlUrl" | b"url" => {
                    if url.is_none() {
                        url = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                _ => {}
            }
        }

        // Create the content: either a link or plain text
        let content = if let Some(url) = url {
            Node::new(node::LINK)
                .prop(prop::URL, url)
                .child(Node::new(node::TEXT).prop(prop::CONTENT, text))
        } else {
            Node::new(node::TEXT).prop(prop::CONTENT, text)
        };

        Node::new(node::PARAGRAPH).child(content)
    }

    fn finalize_outlines(&mut self) {
        // Any remaining items in the stack should be added to result
        while let Some(items) = self.outline_stack.pop() {
            if items.is_empty() {
                continue;
            }

            let list_items: Vec<Node> = items
                .into_iter()
                .map(|n| Node::new(node::LIST_ITEM).children(vec![n]))
                .collect();

            let list = Node::new(node::LIST)
                .prop(prop::ORDERED, false)
                .children(list_items);

            if let Some(parent) = self.outline_stack.last_mut() {
                parent.push(list);
            } else {
                self.result.push(list);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_opml() {
        let opml = r#"<?xml version="1.0"?>
<opml version="2.0">
  <head><title>Test</title></head>
  <body>
    <outline text="Item 1"/>
    <outline text="Item 2"/>
  </body>
</opml>"#;

        let result = parse(opml).unwrap();
        let doc = result.value;
        assert_eq!(doc.metadata.get_str("title"), Some("Test"));
    }

    #[test]
    fn test_parse_nested_opml() {
        let opml = r#"<?xml version="1.0"?>
<opml version="2.0">
  <head><title>Nested</title></head>
  <body>
    <outline text="Parent">
      <outline text="Child 1"/>
      <outline text="Child 2"/>
    </outline>
  </body>
</opml>"#;

        let result = parse(opml).unwrap();
        let doc = result.value;
        // Should have a nested list structure
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_opml_with_links() {
        let opml = r#"<?xml version="1.0"?>
<opml version="2.0">
  <body>
    <outline text="Example" xmlUrl="https://example.com/feed.xml"/>
  </body>
</opml>"#;

        let result = parse(opml).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
