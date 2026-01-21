//! JATS XML reader for rescribe.
//!
//! Parses JATS (Journal Article Tag Suite) XML into rescribe's document IR.
//! Supports JATS 1.0/1.1/1.2 elements commonly used in scholarly publishing.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse JATS XML into a document.
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
    id: Option<String>,
    rid: Option<String>,
    href: Option<String>,
    specific_use: Option<String>,
    content_type: Option<String>,
    list_type: Option<String>,
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
                "id" => attrs.id = Some(value),
                "rid" => attrs.rid = Some(value),
                "href" | "xlink:href" => attrs.href = Some(value),
                "specific-use" => attrs.specific_use = Some(value),
                "content-type" => attrs.content_type = Some(value),
                "list-type" => attrs.list_type = Some(value),
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
            "graphic" | "inline-graphic" => {
                let mut url = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    if key == "href" || key == "xlink:href" {
                        url = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                url.map(|url| Node::new(node::IMAGE).prop(prop::URL, url))
            }
            "xref" => {
                let mut rid = None;
                let mut ref_type = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    let value = String::from_utf8_lossy(&attr.value).to_string();
                    match key.as_str() {
                        "rid" => rid = Some(value),
                        "ref-type" => ref_type = Some(value),
                        _ => {}
                    }
                }
                rid.map(|r| {
                    let mut n = Node::new(node::LINK)
                        .prop(prop::URL, format!("#{}", r.clone()))
                        .child(Node::new(node::TEXT).prop(prop::CONTENT, r));
                    if let Some(rt) = ref_type {
                        n = n.prop("jats:ref-type", rt);
                    }
                    n
                })
            }
            "break" => Some(Node::new(node::LINE_BREAK)),
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
            "article" => Some(Node::new(node::DIV).children(frame.children.clone())),
            "front" | "body" | "back" => None, // Pass through
            "article-meta" | "journal-meta" => {
                self.extract_metadata(&frame.children);
                None
            }

            // Sections
            "sec" => Some(Node::new(node::DIV).children(frame.children.clone())),

            // Titles
            "title" | "article-title" => {
                // Determine heading level from parent
                let level = self
                    .stack
                    .last()
                    .map(|p| match p.element.as_str() {
                        "article" | "front" | "article-meta" => 1,
                        "sec" => 2,
                        "fig" | "table-wrap" => 3,
                        _ => 2,
                    })
                    .unwrap_or(2);

                Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, level as i64)
                        .children(frame.children.clone()),
                )
            }
            "subtitle" => Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 2i64)
                    .children(frame.children.clone()),
            ),

            // Paragraphs
            "p" => Some(Node::new(node::PARAGRAPH).children(frame.children.clone())),

            // Abstract
            "abstract" => Some(
                Node::new(node::DIV)
                    .prop("html:class", "abstract")
                    .children(frame.children.clone()),
            ),

            // Lists
            "list" => {
                let ordered = frame.attrs.list_type.as_deref() == Some("order");
                Some(
                    Node::new(node::LIST)
                        .prop(prop::ORDERED, ordered)
                        .children(frame.children.clone()),
                )
            }
            "list-item" => Some(Node::new(node::LIST_ITEM).children(frame.children.clone())),

            // Definition lists
            "def-list" => Some(Node::new(node::DEFINITION_LIST).children(frame.children.clone())),
            "def-item" => None, // Pass through
            "term" => Some(Node::new(node::DEFINITION_TERM).children(frame.children.clone())),
            "def" => Some(Node::new(node::DEFINITION_DESC).children(frame.children.clone())),

            // Code
            "code" | "preformat" => {
                let text = extract_text(&frame.children);
                let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, text);
                if let Some(lang) = &frame.attrs.content_type {
                    node = node.prop(prop::LANGUAGE, lang.clone());
                }
                Some(node)
            }
            "monospace" => {
                let text = extract_text(&frame.children);
                Some(Node::new(node::CODE).prop(prop::CONTENT, text))
            }

            // Block quote
            "disp-quote" | "boxed-text" => {
                Some(Node::new(node::BLOCKQUOTE).children(frame.children.clone()))
            }

            // Inline formatting
            "italic" => Some(Node::new(node::EMPHASIS).children(frame.children.clone())),
            "bold" => Some(Node::new(node::STRONG).children(frame.children.clone())),
            "underline" => Some(Node::new(node::UNDERLINE).children(frame.children.clone())),
            "strike" => Some(Node::new(node::STRIKEOUT).children(frame.children.clone())),
            "sub" => Some(Node::new(node::SUBSCRIPT).children(frame.children.clone())),
            "sup" => Some(Node::new(node::SUPERSCRIPT).children(frame.children.clone())),
            "sc" => Some(Node::new(node::SMALL_CAPS).children(frame.children.clone())),

            // Links
            "ext-link" => {
                let mut node = Node::new(node::LINK).children(frame.children.clone());
                if let Some(url) = &frame.attrs.href {
                    node = node.prop(prop::URL, url.clone());
                }
                Some(node)
            }
            "xref" => {
                let mut node = Node::new(node::LINK).children(frame.children.clone());
                if let Some(rid) = &frame.attrs.rid {
                    node = node.prop(prop::URL, format!("#{}", rid));
                }
                Some(node)
            }
            "uri" => {
                let url = extract_text(&frame.children);
                Some(
                    Node::new(node::LINK)
                        .prop(prop::URL, url.clone())
                        .child(Node::new(node::TEXT).prop(prop::CONTENT, url)),
                )
            }

            // Figures
            "fig" | "fig-group" => Some(Node::new(node::FIGURE).children(frame.children.clone())),
            "caption" => Some(
                Node::new("figcaption")
                    .prop("html:tag", "figcaption")
                    .children(frame.children.clone()),
            ),
            "graphic" | "inline-graphic" => {
                let mut node = Node::new(node::IMAGE);
                if let Some(href) = &frame.attrs.href {
                    node = node.prop(prop::URL, href.clone());
                }
                Some(node)
            }

            // Tables
            "table-wrap" => Some(Node::new(node::FIGURE).children(frame.children.clone())),
            "table" => Some(Node::new(node::TABLE).children(frame.children.clone())),
            "thead" => Some(Node::new(node::TABLE_HEAD).children(frame.children.clone())),
            "tbody" => Some(Node::new(node::TABLE_BODY).children(frame.children.clone())),
            "tr" => Some(Node::new(node::TABLE_ROW).children(frame.children.clone())),
            "th" => Some(Node::new(node::TABLE_HEADER).children(frame.children.clone())),
            "td" => Some(Node::new(node::TABLE_CELL).children(frame.children.clone())),

            // Math
            "disp-formula" => {
                let text = extract_text(&frame.children);
                Some(Node::new("math_display").prop("math:source", text))
            }
            "inline-formula" => {
                let text = extract_text(&frame.children);
                Some(Node::new("math_inline").prop("math:source", text))
            }
            "tex-math" | "mml:math" => {
                // Already captured by parent formula element
                None
            }

            // Footnotes
            "fn" => Some(Node::new(node::FOOTNOTE_DEF).children(frame.children.clone())),
            "fn-group" => Some(Node::new(node::DIV).children(frame.children.clone())),

            // References
            "ref-list" => Some(
                Node::new(node::DIV)
                    .prop("html:class", "references")
                    .children(frame.children.clone()),
            ),
            "ref" => Some(
                Node::new(node::PARAGRAPH)
                    .prop("jats:ref", true)
                    .children(frame.children.clone()),
            ),
            "mixed-citation" | "element-citation" => {
                Some(Node::new(node::SPAN).children(frame.children.clone()))
            }

            // Metadata elements (usually skip, but may contain useful info)
            "contrib-group" | "contrib" | "name" | "surname" | "given-names" | "aff"
            | "pub-date" | "volume" | "issue" | "fpage" | "lpage" | "kwd-group" | "kwd" => None,

            // Default: pass through children
            _ => None,
        }
    }

    fn extract_metadata(&mut self, nodes: &[Node]) {
        // Simple extraction - look for title text
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
    fn test_parse_simple_article() {
        let jats = r#"<?xml version="1.0"?>
<article>
  <front>
    <article-meta>
      <title-group>
        <article-title>Test Article</article-title>
      </title-group>
    </article-meta>
  </front>
  <body>
    <p>Hello, world!</p>
  </body>
</article>"#;

        let result = parse(jats).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_sections() {
        let jats = r#"<?xml version="1.0"?>
<article>
  <body>
    <sec>
      <title>Introduction</title>
      <p>Content here.</p>
    </sec>
  </body>
</article>"#;

        let result = parse(jats).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_lists() {
        let jats = r#"<?xml version="1.0"?>
<article>
  <body>
    <list list-type="bullet">
      <list-item><p>Item 1</p></list-item>
      <list-item><p>Item 2</p></list-item>
    </list>
  </body>
</article>"#;

        let result = parse(jats).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_formatting() {
        let jats = r#"<?xml version="1.0"?>
<article>
  <body>
    <p><italic>italic</italic> and <bold>bold</bold> text</p>
  </body>
</article>"#;

        let result = parse(jats).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_table() {
        let jats = r#"<?xml version="1.0"?>
<article>
  <body>
    <table-wrap>
      <table>
        <thead>
          <tr><th>Header</th></tr>
        </thead>
        <tbody>
          <tr><td>Cell</td></tr>
        </tbody>
      </table>
    </table-wrap>
  </body>
</article>"#;

        let result = parse(jats).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
