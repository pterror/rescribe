//! FictionBook 2 (FB2) reader for rescribe.
//!
//! Parses FB2 XML (a Russian ebook format) into rescribe's document IR.
//!
//! # Example
//!
//! ```
//! use rescribe_read_fb2::parse;
//!
//! let fb2 = r#"<?xml version="1.0" encoding="UTF-8"?>
//! <FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
//!   <description>
//!     <title-info><book-title>Example</book-title></title-info>
//!   </description>
//!   <body>
//!     <section><p>Hello, world!</p></section>
//!   </body>
//! </FictionBook>"#;
//!
//! let result = parse(fb2).unwrap();
//! let doc = result.value;
//! ```

use base64::Engine;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, Node, ParseError, Properties, Resource,
    ResourceId, ResourceMap,
};
use rescribe_std::{node, prop};

/// Parse FB2 XML into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse options for FB2.
#[derive(Default)]
pub struct ParseOptions {
    /// Whether to extract binary resources (images).
    pub extract_binaries: bool,
}

/// Parse FB2 XML with options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut converter = Converter::new(options.extract_binaries);
    converter.parse(&mut reader)?;

    let document = Document {
        content: Node::new(node::DOCUMENT).children(converter.result),
        resources: converter.resources,
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
    resources: ResourceMap,
    stack: Vec<StackFrame>,
    current_text: String,
    extract_binaries: bool,
    in_description: bool,
    current_binary_id: Option<String>,
    current_binary_type: Option<String>,
}

#[derive(Debug)]
struct StackFrame {
    element: String,
    children: Vec<Node>,
    attrs: FrameAttrs,
}

#[derive(Debug, Default)]
struct FrameAttrs {
    href: Option<String>,
    id: Option<String>,
}

impl Converter {
    fn new(extract_binaries: bool) -> Self {
        Self {
            result: Vec::new(),
            metadata: Properties::new(),
            warnings: Vec::new(),
            resources: ResourceMap::default(),
            stack: Vec::new(),
            current_text: String::new(),
            extract_binaries,
            in_description: false,
            current_binary_id: None,
            current_binary_type: None,
        }
    }

    fn parse(&mut self, reader: &mut Reader<&[u8]>) -> Result<(), ParseError> {
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    self.flush_text();
                    self.handle_start(&e)?;
                }
                Ok(Event::Empty(e)) => {
                    self.flush_text();
                    self.handle_empty(&e)?;
                }
                Ok(Event::End(e)) => {
                    self.flush_text();
                    self.handle_end(&e)?;
                }
                Ok(Event::Text(e)) => {
                    self.current_text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
                Ok(Event::CData(e)) => {
                    self.current_text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    return Err(ParseError::Invalid(format!("XML parse error: {}", e)));
                }
            }
            buf.clear();
        }

        Ok(())
    }

    fn flush_text(&mut self) {
        if self.current_text.is_empty() {
            return;
        }

        let text = std::mem::take(&mut self.current_text);

        // Handle binary content
        if self.current_binary_id.is_some() {
            return; // Text handled in handle_end for binary
        }

        if text.trim().is_empty() {
            return;
        }

        let text_node = Node::new(node::TEXT).prop(prop::CONTENT, text);
        if let Some(frame) = self.stack.last_mut() {
            frame.children.push(text_node);
        }
    }

    fn handle_start(&mut self, e: &BytesStart<'_>) -> Result<(), ParseError> {
        let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
        let mut attrs = FrameAttrs::default();

        for attr in e.attributes().flatten() {
            let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
            let value = String::from_utf8_lossy(&attr.value).to_string();
            match key.as_str() {
                "href" => attrs.href = Some(value),
                "id" => attrs.id = Some(value),
                _ => {}
            }
        }

        // Track description section
        if name == "description" {
            self.in_description = true;
        }

        // Track binary elements
        if name == "binary" {
            for attr in e.attributes().flatten() {
                let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                let value = String::from_utf8_lossy(&attr.value).to_string();
                match key.as_str() {
                    "id" => self.current_binary_id = Some(value),
                    "content-type" => self.current_binary_type = Some(value),
                    _ => {}
                }
            }
        }

        self.stack.push(StackFrame {
            element: name,
            children: Vec::new(),
            attrs,
        });

        Ok(())
    }

    fn handle_empty(&mut self, e: &BytesStart<'_>) -> Result<(), ParseError> {
        let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();

        let node = match name.as_str() {
            "empty-line" => Some(Node::new(node::PARAGRAPH)),
            "image" => {
                let mut href = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    if key == "href" {
                        href = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                href.map(|h| {
                    // Remove # prefix if present
                    let url = h.strip_prefix('#').unwrap_or(&h);
                    Node::new(node::IMAGE).prop(prop::URL, url.to_string())
                })
            }
            _ => None,
        };

        if let Some(n) = node {
            if let Some(frame) = self.stack.last_mut() {
                frame.children.push(n);
            } else {
                self.result.push(n);
            }
        }

        Ok(())
    }

    fn handle_end(&mut self, e: &quick_xml::events::BytesEnd<'_>) -> Result<(), ParseError> {
        let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();

        // Track description section
        if name == "description" {
            self.in_description = false;
        }

        // Handle binary content
        if name == "binary" {
            if self.extract_binaries {
                if let Some(id) = self.current_binary_id.take() {
                    let content_type = self.current_binary_type.take();
                    let text = std::mem::take(&mut self.current_text);
                    let text = text.trim();
                    if !text.is_empty()
                        && let Ok(data) = base64::engine::general_purpose::STANDARD.decode(text)
                    {
                        self.resources.insert(
                            ResourceId::from_string(id.clone()),
                            Resource {
                                name: Some(id),
                                mime_type: content_type
                                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                                data,
                                metadata: Properties::new(),
                            },
                        );
                    }
                }
            } else {
                self.current_binary_id = None;
                self.current_binary_type = None;
                self.current_text.clear();
            }
            self.stack.pop();
            return Ok(());
        }

        if let Some(frame) = self.stack.pop() {
            if frame.element != name {
                self.stack.push(frame);
                return Ok(());
            }

            let node = self.convert_element(&frame);

            if let Some(parent) = self.stack.last_mut() {
                if let Some(n) = node {
                    parent.children.push(n);
                } else {
                    parent.children.extend(frame.children);
                }
            } else if let Some(n) = node {
                self.result.push(n);
            } else {
                self.result.extend(frame.children);
            }
        }

        Ok(())
    }

    fn convert_element(&mut self, frame: &StackFrame) -> Option<Node> {
        match frame.element.as_str() {
            // Document structure
            "FictionBook" => None, // Pass through

            // Description/metadata elements
            "description" | "title-info" | "document-info" | "publish-info" | "custom-info" => {
                // Extract metadata
                self.extract_metadata_from(&frame.children);
                None
            }

            "book-title" => {
                let title = extract_text(&frame.children);
                if !title.is_empty() {
                    self.metadata.set("title", title);
                }
                None
            }

            "author" => {
                // Combine name parts
                let mut parts = Vec::new();
                for child in &frame.children {
                    if matches!(
                        child.kind.as_str(),
                        "fb2:first-name" | "fb2:middle-name" | "fb2:last-name"
                    ) {
                        let text = extract_text(&child.children);
                        if !text.is_empty() {
                            parts.push(text);
                        }
                    }
                }
                if !parts.is_empty() {
                    self.metadata.set("author", parts.join(" "));
                }
                None
            }

            "first-name" | "middle-name" | "last-name" | "nickname" => {
                // Return as fb2: prefixed node for parent to handle
                let text = extract_text(&frame.children);
                if !text.is_empty() {
                    Some(
                        Node::new(format!("fb2:{}", frame.element))
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, text)),
                    )
                } else {
                    None
                }
            }

            "annotation" => {
                if !frame.children.is_empty() {
                    Some(
                        Node::new(node::DIV)
                            .prop("html:class", "annotation")
                            .children(frame.children.clone()),
                    )
                } else {
                    None
                }
            }

            "lang" | "src-lang" | "genre" | "keywords" | "date" | "version" | "program-used" => {
                // Metadata elements - handled in extract_metadata
                None
            }

            // Body content
            "body" => Some(Node::new(node::DIV).children(frame.children.clone())),

            "section" => Some(Node::new(node::DIV).children(frame.children.clone())),

            "title" => {
                // Title in body is a heading (level based on nesting)
                if !self.in_description && !frame.children.is_empty() {
                    // Count section nesting for heading level
                    let level = self
                        .stack
                        .iter()
                        .filter(|f| f.element == "section")
                        .count()
                        .clamp(1, 6) as i64;

                    // Title contains <p> elements - extract their content
                    let mut inline_children = Vec::new();
                    for child in &frame.children {
                        if child.kind.as_str() == node::PARAGRAPH {
                            inline_children.extend(child.children.clone());
                        } else {
                            inline_children.push(child.clone());
                        }
                    }

                    Some(
                        Node::new(node::HEADING)
                            .prop(prop::LEVEL, level)
                            .children(inline_children),
                    )
                } else {
                    None
                }
            }

            "subtitle" => {
                // Subtitle as h4
                let mut inline_children = Vec::new();
                for child in &frame.children {
                    if child.kind.as_str() == node::PARAGRAPH {
                        inline_children.extend(child.children.clone());
                    } else {
                        inline_children.push(child.clone());
                    }
                }
                Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, 4i64)
                        .children(inline_children),
                )
            }

            "p" => Some(Node::new(node::PARAGRAPH).children(frame.children.clone())),

            "empty-line" => Some(Node::new(node::PARAGRAPH)),

            // Quote/cite
            "cite" => Some(Node::new(node::BLOCKQUOTE).children(frame.children.clone())),

            "text-author" => Some(
                Node::new(node::PARAGRAPH)
                    .prop("html:class", "text-author")
                    .children(frame.children.clone()),
            ),

            "epigraph" => Some(
                Node::new(node::BLOCKQUOTE)
                    .prop("fb2:type", "epigraph")
                    .children(frame.children.clone()),
            ),

            // Poetry
            "poem" => Some(
                Node::new(node::DIV)
                    .prop("html:class", "poem")
                    .children(frame.children.clone()),
            ),

            "stanza" => Some(
                Node::new(node::DIV)
                    .prop("html:class", "stanza")
                    .children(frame.children.clone()),
            ),

            "v" => {
                // Verse line
                let mut children = frame.children.clone();
                children.push(Node::new(node::LINE_BREAK));
                Some(Node::new(node::SPAN).children(children))
            }

            // Inline formatting
            "emphasis" => Some(Node::new(node::EMPHASIS).children(frame.children.clone())),

            "strong" => Some(Node::new(node::STRONG).children(frame.children.clone())),

            "strikethrough" => Some(Node::new(node::STRIKEOUT).children(frame.children.clone())),

            "code" => {
                let text = extract_text(&frame.children);
                Some(Node::new(node::CODE).prop(prop::CONTENT, text))
            }

            "sub" => Some(Node::new(node::SUBSCRIPT).children(frame.children.clone())),

            "sup" => Some(Node::new(node::SUPERSCRIPT).children(frame.children.clone())),

            // Links
            "a" => {
                let mut node = Node::new(node::LINK).children(frame.children.clone());
                if let Some(href) = &frame.attrs.href {
                    // Remove # prefix for internal links
                    let url = if href.starts_with('#') {
                        href.to_string()
                    } else {
                        href.clone()
                    };
                    node = node.prop(prop::URL, url);
                }
                Some(node)
            }

            // Images (handled in empty, but in case of non-empty)
            "image" => {
                if let Some(href) = &frame.attrs.href {
                    let url = href.strip_prefix('#').unwrap_or(href);
                    Some(Node::new(node::IMAGE).prop(prop::URL, url.to_string()))
                } else {
                    None
                }
            }

            // Tables (FB2 has limited table support)
            "table" => Some(Node::new(node::TABLE).children(frame.children.clone())),

            "tr" => Some(Node::new(node::TABLE_ROW).children(frame.children.clone())),

            "td" => Some(Node::new(node::TABLE_CELL).children(frame.children.clone())),

            "th" => Some(Node::new(node::TABLE_HEADER).children(frame.children.clone())),

            // Default: pass through
            _ => None,
        }
    }

    fn extract_metadata_from(&mut self, nodes: &[Node]) {
        for node in nodes {
            // Extract any additional metadata from nodes if needed
            // Currently, title and author are handled directly in convert_element
            if let Some(title) = node.props.get_str("fb2:title") {
                self.metadata.set("title", title.to_string());
            }
            self.extract_metadata_from(&node.children);
        }
    }
}

fn extract_text(nodes: &[Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        if node.kind.as_str() == node::TEXT
            && let Some(content) = node.props.get_str(prop::CONTENT)
        {
            text.push_str(content);
        }
        text.push_str(&extract_text(&node.children));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let fb2 = r#"<?xml version="1.0" encoding="UTF-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
  <description>
    <title-info><book-title>Test Book</book-title></title-info>
  </description>
  <body>
    <section><p>Hello, world!</p></section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(doc.metadata.get_str("title"), Some("Test Book"));
    }

    #[test]
    fn test_parse_with_sections() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook>
  <body>
    <title><p>Book Title</p></title>
    <section>
      <title><p>Chapter 1</p></title>
      <p>Content here.</p>
    </section>
    <section>
      <title><p>Chapter 2</p></title>
      <p>More content.</p>
    </section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_inline_formatting() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook>
  <body>
    <section>
      <p>This is <emphasis>italic</emphasis> and <strong>bold</strong> text.</p>
    </section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_links() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook xmlns:l="http://www.w3.org/1999/xlink">
  <body>
    <section>
      <p>Visit <a l:href="http://example.com">example</a>.</p>
    </section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_cite() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook>
  <body>
    <section>
      <cite>
        <p>A famous quote.</p>
        <text-author>Someone Famous</text-author>
      </cite>
    </section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_poem() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook>
  <body>
    <section>
      <poem>
        <stanza>
          <v>Line one</v>
          <v>Line two</v>
        </stanza>
      </poem>
    </section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_empty_line() {
        let fb2 = r#"<?xml version="1.0"?>
<FictionBook>
  <body>
    <section>
      <p>Before</p>
      <empty-line/>
      <p>After</p>
    </section>
  </body>
</FictionBook>"#;

        let result = parse(fb2).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
