//! DocBook reader for rescribe.
//!
//! Parses DocBook XML into rescribe's document IR.
//! Supports DocBook 5 and DocBook 4 elements.
//!
//! # Example
//!
//! ```
//! use rescribe_read_docbook::parse;
//!
//! let docbook = r#"<?xml version="1.0"?>
//! <article xmlns="http://docbook.org/ns/docbook">
//!   <title>Example Article</title>
//!   <para>Hello, world!</para>
//! </article>"#;
//!
//! let result = parse(docbook).unwrap();
//! let doc = result.value;
//! ```

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse DocBook XML into a document.
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
    role: Option<String>,
    url: Option<String>,
    language: Option<String>,
    level: Option<u32>,
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
                "role" => attrs.role = Some(value),
                "url" | "xlink:href" => attrs.url = Some(value),
                "language" => attrs.language = Some(value),
                _ => {}
            }
        }

        // Extract level from section numbers (sect1, sect2, etc.)
        if let Some(level) = name.strip_prefix("sect")
            && let Ok(n) = level.parse::<u32>()
        {
            attrs.level = Some(n);
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
            "imagedata" | "graphic" => {
                let mut url = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    if key == "fileref" {
                        url = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                url.map(|url| Node::new(node::IMAGE).prop(prop::URL, url))
            }
            "xref" | "link" => {
                let mut linkend = None;
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    if key == "linkend" || key == "xlink:href" || key == "url" {
                        linkend = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                linkend.map(|url| {
                    Node::new(node::LINK)
                        .prop(prop::URL, format!("#{}", url.clone()))
                        .child(Node::new(node::TEXT).prop(prop::CONTENT, url))
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
            // Document level
            "article" | "book" | "chapter" | "part" | "appendix" => {
                Some(Node::new(node::DIV).children(frame.children.clone()))
            }

            // Sections
            "section" | "sect1" | "sect2" | "sect3" | "sect4" | "sect5" | "simplesect" => {
                Some(Node::new(node::DIV).children(frame.children.clone()))
            }

            // Titles - convert to heading
            "title" => {
                // Determine heading level from parent
                let level = self
                    .stack
                    .last()
                    .and_then(|p| match p.element.as_str() {
                        "article" | "book" => Some(1),
                        "chapter" | "part" => Some(1),
                        "sect1" | "section" => p.attrs.level.or(Some(2)),
                        "sect2" => Some(3),
                        "sect3" => Some(4),
                        "sect4" => Some(5),
                        "sect5" => Some(6),
                        _ => None,
                    })
                    .unwrap_or(2);

                Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, level as i64)
                        .children(frame.children.clone()),
                )
            }

            // Paragraphs
            "para" | "simpara" => Some(Node::new(node::PARAGRAPH).children(frame.children.clone())),

            // Block quote
            "blockquote" => Some(Node::new(node::BLOCKQUOTE).children(frame.children.clone())),

            // Lists
            "itemizedlist" => Some(
                Node::new(node::LIST)
                    .prop(prop::ORDERED, false)
                    .children(frame.children.clone()),
            ),
            "orderedlist" => Some(
                Node::new(node::LIST)
                    .prop(prop::ORDERED, true)
                    .children(frame.children.clone()),
            ),
            "listitem" => Some(Node::new(node::LIST_ITEM).children(frame.children.clone())),

            // Definition lists
            "variablelist" => {
                Some(Node::new(node::DEFINITION_LIST).children(frame.children.clone()))
            }
            "varlistentry" => {
                Some(Node::new("docbook:varlistentry").children(frame.children.clone()))
            }
            "term" => Some(Node::new(node::DEFINITION_TERM).children(frame.children.clone())),

            // Code
            "programlisting" | "screen" | "literallayout" => {
                let text = extract_text(&frame.children);
                let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, text);
                if let Some(lang) = &frame.attrs.language {
                    node = node.prop(prop::LANGUAGE, lang.clone());
                }
                Some(node)
            }
            "code" | "literal" | "command" | "filename" | "option" | "computeroutput"
            | "userinput" => {
                let text = extract_text(&frame.children);
                Some(Node::new(node::CODE).prop(prop::CONTENT, text))
            }

            // Inline formatting
            "emphasis" => {
                if frame.attrs.role.as_deref() == Some("strong")
                    || frame.attrs.role.as_deref() == Some("bold")
                {
                    Some(Node::new(node::STRONG).children(frame.children.clone()))
                } else {
                    Some(Node::new(node::EMPHASIS).children(frame.children.clone()))
                }
            }
            "subscript" => Some(Node::new(node::SUBSCRIPT).children(frame.children.clone())),
            "superscript" => Some(Node::new(node::SUPERSCRIPT).children(frame.children.clone())),

            // Links
            "link" | "ulink" | "xref" => {
                let mut node = Node::new(node::LINK).children(frame.children.clone());
                if let Some(url) = &frame.attrs.url {
                    node = node.prop(prop::URL, url.clone());
                }
                Some(node)
            }

            // Figures and media
            "figure" | "informalfigure" => {
                Some(Node::new(node::FIGURE).children(frame.children.clone()))
            }
            "mediaobject" | "inlinemediaobject" => {
                // Just pass through children (imageobject, etc.)
                None
            }
            "imageobject" | "textobject" => None, // Pass through
            "imagedata" => {
                // Should be handled in empty, but just in case
                None
            }
            "caption" => Some(
                Node::new("figcaption")
                    .prop("html:tag", "figcaption")
                    .children(frame.children.clone()),
            ),

            // Tables
            "table" | "informaltable" => {
                Some(Node::new(node::TABLE).children(frame.children.clone()))
            }
            "tgroup" | "thead" | "tbody" | "tfoot" => None, // Pass through
            "row" | "tr" => Some(Node::new(node::TABLE_ROW).children(frame.children.clone())),
            "entry" | "td" => Some(Node::new(node::TABLE_CELL).children(frame.children.clone())),
            "th" => Some(Node::new(node::TABLE_HEADER).children(frame.children.clone())),

            // Footnotes
            "footnote" => {
                // Create a footnote reference and definition
                Some(Node::new(node::FOOTNOTE_DEF).children(frame.children.clone()))
            }

            // Admonitions
            "note" | "tip" | "warning" | "caution" | "important" => Some(
                Node::new(node::BLOCKQUOTE)
                    .prop("docbook:type", frame.element.clone())
                    .children(frame.children.clone()),
            ),

            // Abstract and other metadata
            "abstract" => Some(
                Node::new(node::DIV)
                    .prop("html:class", "abstract")
                    .children(frame.children.clone()),
            ),
            "info" | "articleinfo" | "bookinfo" => {
                // Extract metadata from info block
                self.extract_metadata(&frame.children);
                None
            }
            "author" | "authorgroup" | "date" | "copyright" | "legalnotice" | "pubdate"
            | "releaseinfo" | "revhistory" | "revision" => {
                // Metadata elements - handled in extract_metadata
                None
            }
            "personname" | "firstname" | "surname" | "othername" => {
                // Author name parts - just extract text
                None
            }

            // Line break
            "sbr" => Some(Node::new(node::LINE_BREAK)),

            // Horizontal rule
            "bridgehead" => Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 4i64)
                    .children(frame.children.clone()),
            ),

            // Default: pass through children
            _ => None,
        }
    }

    fn extract_metadata(&mut self, nodes: &[Node]) {
        // Simple extraction - just look for title text
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
        let docbook = r#"<?xml version="1.0"?>
<article xmlns="http://docbook.org/ns/docbook">
  <title>Test Article</title>
  <para>Hello, world!</para>
</article>"#;

        let result = parse(docbook).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_sections() {
        let docbook = r#"<?xml version="1.0"?>
<article>
  <section>
    <title>Section 1</title>
    <para>Content</para>
  </section>
</article>"#;

        let result = parse(docbook).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_lists() {
        let docbook = r#"<?xml version="1.0"?>
<article>
  <itemizedlist>
    <listitem><para>Item 1</para></listitem>
    <listitem><para>Item 2</para></listitem>
  </itemizedlist>
</article>"#;

        let result = parse(docbook).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_code() {
        let docbook = r#"<?xml version="1.0"?>
<article>
  <programlisting language="rust">fn main() {}</programlisting>
</article>"#;

        let result = parse(docbook).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_emphasis() {
        let docbook = r#"<?xml version="1.0"?>
<article>
  <para><emphasis>italic</emphasis> and <emphasis role="strong">bold</emphasis></para>
</article>"#;

        let result = parse(docbook).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
