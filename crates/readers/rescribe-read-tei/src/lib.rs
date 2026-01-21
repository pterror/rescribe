//! TEI XML reader for rescribe.
//!
//! Parses TEI (Text Encoding Initiative) XML into rescribe's document IR.
//! Supports common TEI P5 elements used in digital humanities.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse TEI XML into a document.
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
    stack: Vec<StackFrame>,
    current_text: String,
}

#[derive(Debug)]
struct StackFrame {
    element: String,
    children: Vec<Node>,
    attrs: FrameAttrs,
}

#[derive(Debug, Default)]
struct FrameAttrs {
    rend: Option<String>,
    target: Option<String>,
    url: Option<String>,
    n: Option<String>,
    xml_id: Option<String>,
}

impl Converter {
    fn new() -> Self {
        Self {
            result: Vec::new(),
            metadata: Properties::new(),
            warnings: Vec::new(),
            stack: Vec::new(),
            current_text: String::new(),
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
                Ok(_) => {} // Ignore other events
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
                "rend" => attrs.rend = Some(value),
                "target" => attrs.target = Some(value),
                "url" => attrs.url = Some(value),
                "n" => attrs.n = Some(value),
                "id" => attrs.xml_id = Some(value),
                _ => {}
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
            "graphic" => {
                let mut url = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    if key == "url" {
                        url = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                url.map(|url| Node::new(node::IMAGE).prop(prop::URL, url))
            }
            "lb" => Some(Node::new(node::LINE_BREAK)),
            "pb" => Some(Node::new(node::HORIZONTAL_RULE)),
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

        if let Some(frame) = self.stack.pop() {
            if frame.element != name {
                // Mismatched tags, just continue
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
            "TEI" | "text" | "body" | "front" | "back" => None, // Pass through

            // Header - extract metadata
            "teiHeader" | "fileDesc" | "titleStmt" | "publicationStmt" | "sourceDesc" => {
                self.extract_metadata(&frame.children);
                None
            }

            // Divisions
            "div" | "div1" | "div2" | "div3" | "div4" => {
                Some(Node::new(node::DIV).children(frame.children.clone()))
            }

            // Headings
            "head" => {
                let level = self
                    .stack
                    .last()
                    .map(|p| match p.element.as_str() {
                        "div1" | "div" => 1,
                        "div2" => 2,
                        "div3" => 3,
                        "div4" => 4,
                        _ => 2,
                    })
                    .unwrap_or(2);

                Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, level as i64)
                        .children(frame.children.clone()),
                )
            }

            // Paragraphs
            "p" => Some(Node::new(node::PARAGRAPH).children(frame.children.clone())),

            // Lists
            "list" => {
                let ordered = frame.attrs.rend.as_deref() == Some("numbered");
                Some(
                    Node::new(node::LIST)
                        .prop(prop::ORDERED, ordered)
                        .children(frame.children.clone()),
                )
            }
            "item" => Some(Node::new(node::LIST_ITEM).children(frame.children.clone())),

            // Glossary/definition lists
            "gloss" => Some(Node::new(node::DEFINITION_LIST).children(frame.children.clone())),
            "term" => Some(Node::new(node::DEFINITION_TERM).children(frame.children.clone())),
            "def" | "desc" => {
                Some(Node::new(node::DEFINITION_DESC).children(frame.children.clone()))
            }

            // Block quote
            "quote" | "cit" => Some(Node::new(node::BLOCKQUOTE).children(frame.children.clone())),

            // Poetry/verse
            "lg" => Some(
                Node::new(node::DIV)
                    .prop("tei:type", "verse")
                    .children(frame.children.clone()),
            ),
            "l" => {
                // Line of verse - treat as paragraph
                Some(
                    Node::new(node::PARAGRAPH)
                        .prop("tei:type", "line")
                        .children(frame.children.clone()),
                )
            }

            // Code
            "code" | "eg" => {
                let text = extract_text(&frame.children);
                Some(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, text))
            }

            // Highlighting (inline formatting)
            "hi" => match frame.attrs.rend.as_deref() {
                Some("bold") | Some("b") => {
                    Some(Node::new(node::STRONG).children(frame.children.clone()))
                }
                Some("italic") | Some("i") | Some("it") => {
                    Some(Node::new(node::EMPHASIS).children(frame.children.clone()))
                }
                Some("underline") | Some("u") => {
                    Some(Node::new(node::UNDERLINE).children(frame.children.clone()))
                }
                Some("strike") | Some("strikethrough") => {
                    Some(Node::new(node::STRIKEOUT).children(frame.children.clone()))
                }
                Some("sup") | Some("superscript") => {
                    Some(Node::new(node::SUPERSCRIPT).children(frame.children.clone()))
                }
                Some("sub") | Some("subscript") => {
                    Some(Node::new(node::SUBSCRIPT).children(frame.children.clone()))
                }
                Some("sc") | Some("smallcaps") => {
                    Some(Node::new(node::SMALL_CAPS).children(frame.children.clone()))
                }
                _ => Some(Node::new(node::EMPHASIS).children(frame.children.clone())),
            },

            // Semantic highlighting
            "emph" => Some(Node::new(node::EMPHASIS).children(frame.children.clone())),
            "foreign" => Some(Node::new(node::EMPHASIS).children(frame.children.clone())),
            "title" => {
                // Could be in metadata or inline
                if self
                    .stack
                    .iter()
                    .any(|f| f.element == "titleStmt" || f.element == "teiHeader")
                {
                    let title = extract_text(&frame.children);
                    if !title.is_empty() {
                        self.metadata.set("title", title);
                    }
                    None
                } else {
                    Some(Node::new(node::EMPHASIS).children(frame.children.clone()))
                }
            }

            // Links
            "ref" | "ptr" => {
                let mut node = Node::new(node::LINK).children(frame.children.clone());
                if let Some(target) = &frame.attrs.target {
                    node = node.prop(prop::URL, target.clone());
                }
                Some(node)
            }

            // Figures
            "figure" => Some(Node::new(node::FIGURE).children(frame.children.clone())),
            "figDesc" => Some(
                Node::new("figcaption")
                    .prop("html:tag", "figcaption")
                    .children(frame.children.clone()),
            ),
            "graphic" => {
                let mut node = Node::new(node::IMAGE);
                if let Some(url) = &frame.attrs.url {
                    node = node.prop(prop::URL, url.clone());
                }
                Some(node)
            }

            // Tables
            "table" => Some(Node::new(node::TABLE).children(frame.children.clone())),
            "row" => Some(Node::new(node::TABLE_ROW).children(frame.children.clone())),
            "cell" => {
                let role = frame.attrs.rend.as_deref();
                if role == Some("header") || role == Some("label") {
                    Some(Node::new(node::TABLE_HEADER).children(frame.children.clone()))
                } else {
                    Some(Node::new(node::TABLE_CELL).children(frame.children.clone()))
                }
            }

            // Notes/footnotes
            "note" => Some(Node::new(node::FOOTNOTE_DEF).children(frame.children.clone())),

            // Formula
            "formula" => {
                let text = extract_text(&frame.children);
                Some(Node::new("math_display").prop("math:source", text))
            }

            // Default: pass through children
            _ => None,
        }
    }

    fn extract_metadata(&mut self, nodes: &[Node]) {
        for node in nodes {
            if node.kind.as_str() == node::HEADING {
                let title = extract_text(&node.children);
                if !title.is_empty() {
                    self.metadata.set("title", title);
                }
            }
            self.extract_metadata(&node.children);
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
    fn test_parse_simple_document() {
        let tei = r#"<?xml version="1.0"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt>
        <title>Test Document</title>
      </titleStmt>
    </fileDesc>
  </teiHeader>
  <text>
    <body>
      <p>Hello, world!</p>
    </body>
  </text>
</TEI>"#;

        let result = parse(tei).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_divisions() {
        let tei = r#"<?xml version="1.0"?>
<TEI>
  <text>
    <body>
      <div>
        <head>Introduction</head>
        <p>Content here.</p>
      </div>
    </body>
  </text>
</TEI>"#;

        let result = parse(tei).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_lists() {
        let tei = r#"<?xml version="1.0"?>
<TEI>
  <text>
    <body>
      <list>
        <item>Item 1</item>
        <item>Item 2</item>
      </list>
    </body>
  </text>
</TEI>"#;

        let result = parse(tei).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_formatting() {
        let tei = r#"<?xml version="1.0"?>
<TEI>
  <text>
    <body>
      <p><hi rend="italic">italic</hi> and <hi rend="bold">bold</hi> text</p>
    </body>
  </text>
</TEI>"#;

        let result = parse(tei).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_table() {
        let tei = r#"<?xml version="1.0"?>
<TEI>
  <text>
    <body>
      <table>
        <row>
          <cell rend="header">Header</cell>
        </row>
        <row>
          <cell>Cell</cell>
        </row>
      </table>
    </body>
  </text>
</TEI>"#;

        let result = parse(tei).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
