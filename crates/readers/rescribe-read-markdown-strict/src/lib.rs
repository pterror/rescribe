//! Markdown strict reader for rescribe.
//!
//! Parses original Markdown.pl compatible syntax (no extensions).
//! This is more restrictive than CommonMark - no fenced code blocks,
//! no tables, no strikethrough, etc.
//!
//! # Example
//!
//! ```
//! use rescribe_read_markdown_strict::parse;
//!
//! let md = "# Hello\n\nThis is **bold** text.";
//! let result = parse(md).unwrap();
//! ```

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use rescribe_core::{ConversionResult, Document, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse strict Markdown into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &Default::default())
}

/// Parse strict Markdown with options.
pub fn parse_with_options(
    input: &str,
    _options: &rescribe_core::ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    // Empty options = strict original Markdown (no extensions)
    let parser = Parser::new_ext(input, Options::empty());

    let mut converter = Converter::new();
    converter.convert(parser);

    let document = Document {
        content: Node::new(node::DOCUMENT).children(converter.root),
        resources: Default::default(),
        metadata: Properties::new(),
        source: None,
    };

    Ok(ConversionResult::ok(document))
}

struct Converter {
    root: Vec<Node>,
    stack: Vec<(String, Vec<Node>)>,
}

impl Converter {
    fn new() -> Self {
        Self {
            root: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn convert(&mut self, parser: Parser) {
        for event in parser {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.add_text(&text),
            Event::Code(code) => self.add_inline_code(&code),
            Event::SoftBreak => self.add_soft_break(),
            Event::HardBreak => self.add_hard_break(),
            Event::Rule => self.add_horizontal_rule(),
            Event::Html(html) => self.add_raw_html(&html),
            Event::InlineHtml(html) => self.add_inline_html(&html),
            Event::FootnoteReference(_) => {} // Not in strict markdown
            Event::TaskListMarker(_) => {}    // Not in strict markdown
            Event::InlineMath(_) | Event::DisplayMath(_) => {} // Not in strict markdown
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        let kind = match &tag {
            Tag::Paragraph => node::PARAGRAPH,
            Tag::Heading { level, .. } => {
                self.stack
                    .push((format!("heading:{}", *level as u8), Vec::new()));
                return;
            }
            Tag::BlockQuote(_) => node::BLOCKQUOTE,
            Tag::CodeBlock(_) => {
                // In strict markdown, only indented code blocks
                self.stack.push((node::CODE_BLOCK.to_string(), Vec::new()));
                return;
            }
            Tag::List(ordered) => {
                let kind = node::LIST.to_string();
                self.stack
                    .push((format!("{}:{}", kind, ordered.is_some()), Vec::new()));
                return;
            }
            Tag::Item => node::LIST_ITEM,
            Tag::Emphasis => node::EMPHASIS,
            Tag::Strong => node::STRONG,
            Tag::Link {
                dest_url, title, ..
            } => {
                self.stack
                    .push((format!("link:{}:{}", dest_url, title), Vec::new()));
                return;
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.stack
                    .push((format!("image:{}:{}", dest_url, title), Vec::new()));
                return;
            }
            Tag::HtmlBlock => {
                self.stack.push((node::RAW_BLOCK.to_string(), Vec::new()));
                return;
            }
            // These don't exist in strict markdown
            Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => return,
            Tag::Strikethrough => return,
            Tag::FootnoteDefinition(_) => return,
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                return;
            }
            Tag::MetadataBlock(_) => return,
        };

        self.stack.push((kind.to_string(), Vec::new()));
    }

    fn end_tag(&mut self, tag: TagEnd) {
        if let Some((kind, children)) = self.stack.pop() {
            let node = if let Some(level_str) = kind.strip_prefix("heading:") {
                let level: i64 = level_str.parse().unwrap_or(1);
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level)
                    .children(children)
            } else if let Some(ordered_str) = kind.strip_prefix("list:") {
                let ordered = ordered_str != "false";
                Node::new(node::LIST)
                    .prop(prop::ORDERED, ordered)
                    .children(children)
            } else if let Some(rest) = kind.strip_prefix("link:") {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                let url = parts.first().unwrap_or(&"");
                let title = parts.get(1).unwrap_or(&"");
                let mut link = Node::new(node::LINK)
                    .prop(prop::URL, *url)
                    .children(children);
                if !title.is_empty() {
                    link = link.prop(prop::TITLE, *title);
                }
                link
            } else if let Some(rest) = kind.strip_prefix("image:") {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                let url = parts.first().unwrap_or(&"");
                let title = parts.get(1).unwrap_or(&"");
                let alt = children
                    .iter()
                    .filter_map(|n| n.props.get_str(prop::CONTENT))
                    .collect::<Vec<_>>()
                    .join("");
                let mut img = Node::new(node::IMAGE)
                    .prop(prop::URL, *url)
                    .prop(prop::ALT, alt);
                if !title.is_empty() {
                    img = img.prop(prop::TITLE, *title);
                }
                img
            } else if kind == node::CODE_BLOCK {
                let content = children
                    .iter()
                    .filter_map(|n| n.props.get_str(prop::CONTENT))
                    .collect::<Vec<_>>()
                    .join("");
                Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
            } else if kind == node::RAW_BLOCK {
                let content = children
                    .iter()
                    .filter_map(|n| n.props.get_str(prop::CONTENT))
                    .collect::<Vec<_>>()
                    .join("");
                Node::new(node::RAW_BLOCK)
                    .prop(prop::FORMAT, "html")
                    .prop(prop::CONTENT, content)
            } else {
                Node::new(&*kind).children(children)
            };

            // Handle specific end tags
            match tag {
                TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow | TagEnd::TableCell => return,
                TagEnd::Strikethrough => return,
                TagEnd::FootnoteDefinition => return,
                TagEnd::DefinitionList
                | TagEnd::DefinitionListTitle
                | TagEnd::DefinitionListDefinition => return,
                TagEnd::MetadataBlock(_) => return,
                _ => {}
            }

            if let Some((_, parent_children)) = self.stack.last_mut() {
                parent_children.push(node);
            } else {
                self.root.push(node);
            }
        }
    }

    fn add_text(&mut self, text: &str) {
        let text_node = Node::new(node::TEXT).prop(prop::CONTENT, text.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(text_node);
        } else {
            self.root.push(text_node);
        }
    }

    fn add_inline_code(&mut self, code: &str) {
        let code_node = Node::new(node::CODE).prop(prop::CONTENT, code.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(code_node);
        } else {
            self.root.push(code_node);
        }
    }

    fn add_soft_break(&mut self) {
        let br = Node::new(node::SOFT_BREAK);
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(br);
        } else {
            self.root.push(br);
        }
    }

    fn add_hard_break(&mut self) {
        let br = Node::new(node::LINE_BREAK);
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(br);
        } else {
            self.root.push(br);
        }
    }

    fn add_horizontal_rule(&mut self) {
        let hr = Node::new(node::HORIZONTAL_RULE);
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(hr);
        } else {
            self.root.push(hr);
        }
    }

    fn add_raw_html(&mut self, html: &str) {
        let raw = Node::new(node::RAW_BLOCK)
            .prop(prop::FORMAT, "html")
            .prop(prop::CONTENT, html.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(raw);
        } else {
            self.root.push(raw);
        }
    }

    fn add_inline_html(&mut self, html: &str) {
        let raw = Node::new(node::RAW_INLINE)
            .prop(prop::FORMAT, "html")
            .prop(prop::CONTENT, html.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(raw);
        } else {
            self.root.push(raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let md = "# Hello\n\nThis is a paragraph.";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 2);
    }

    #[test]
    fn test_parse_emphasis() {
        let md = "This is *italic* and **bold** text.";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_code() {
        let md = "Use `code` inline.";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_link() {
        let md = "[link](https://example.com)";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_list() {
        let md = "- item 1\n- item 2\n- item 3";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
