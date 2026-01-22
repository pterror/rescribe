//! MultiMarkdown reader for rescribe.
//!
//! Parses MultiMarkdown format with its extensions:
//! - Metadata blocks
//! - Footnotes
//! - Tables
//! - Definition lists
//! - Math (LaTeX-style)
//! - Smart punctuation

use pulldown_cmark::{Event, MetadataBlockKind, Options, Parser, Tag, TagEnd};
use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions, Properties};
use rescribe_std::{Node, node, prop};

/// Parse MultiMarkdown input into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse MultiMarkdown input into a document with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    // MultiMarkdown extensions
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_MATH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;

    let parser = Parser::new_ext(input, options);
    let mut converter = Converter::new();
    converter.convert(parser);

    let document = Document {
        content: Node::new(node::DOCUMENT).children(converter.root),
        resources: Default::default(),
        metadata: converter.metadata,
        source: None,
    };

    Ok(ConversionResult::ok(document))
}

struct Converter {
    root: Vec<Node>,
    stack: Vec<(String, Vec<Node>)>,
    metadata: Properties,
    in_table_head: bool,
}

impl Converter {
    fn new() -> Self {
        Self {
            root: Vec::new(),
            stack: Vec::new(),
            metadata: Properties::new(),
            in_table_head: false,
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
            Event::FootnoteReference(name) => self.add_footnote_ref(&name),
            Event::TaskListMarker(checked) => self.add_task_marker(checked),
            Event::InlineMath(math) => self.add_inline_math(&math),
            Event::DisplayMath(math) => self.add_display_math(&math),
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                self.stack.push((node::PARAGRAPH.to_string(), Vec::new()));
            }
            Tag::Heading { level, id, .. } => {
                let kind = format!("heading:{}:{}", level as u8, id.as_deref().unwrap_or(""));
                self.stack.push((kind, Vec::new()));
            }
            Tag::BlockQuote(_) => {
                self.stack.push((node::BLOCKQUOTE.to_string(), Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                self.stack
                    .push((format!("code_block:{}", lang), Vec::new()));
            }
            Tag::List(start) => {
                let kind = format!("list:{}", start.is_some());
                self.stack.push((kind, Vec::new()));
            }
            Tag::Item => {
                self.stack.push((node::LIST_ITEM.to_string(), Vec::new()));
            }
            Tag::FootnoteDefinition(name) => {
                self.stack
                    .push((format!("footnote_def:{}", name), Vec::new()));
            }
            Tag::Table(_alignments) => {
                self.stack.push((node::TABLE.to_string(), Vec::new()));
            }
            Tag::TableHead => {
                self.in_table_head = true;
                self.stack.push((node::TABLE_ROW.to_string(), Vec::new()));
            }
            Tag::TableRow => {
                self.stack.push((node::TABLE_ROW.to_string(), Vec::new()));
            }
            Tag::TableCell => {
                if self.in_table_head {
                    self.stack
                        .push((node::TABLE_HEADER.to_string(), Vec::new()));
                } else {
                    self.stack.push((node::TABLE_CELL.to_string(), Vec::new()));
                }
            }
            Tag::Emphasis => {
                self.stack.push((node::EMPHASIS.to_string(), Vec::new()));
            }
            Tag::Strong => {
                self.stack.push((node::STRONG.to_string(), Vec::new()));
            }
            Tag::Strikethrough => {
                self.stack.push((node::STRIKEOUT.to_string(), Vec::new()));
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                self.stack
                    .push((format!("link:{}:{}", dest_url, title), Vec::new()));
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.stack
                    .push((format!("image:{}:{}", dest_url, title), Vec::new()));
            }
            Tag::DefinitionList => {
                self.stack
                    .push((node::DEFINITION_LIST.to_string(), Vec::new()));
            }
            Tag::DefinitionListTitle => {
                self.stack
                    .push((node::DEFINITION_TERM.to_string(), Vec::new()));
            }
            Tag::DefinitionListDefinition => {
                self.stack
                    .push((node::DEFINITION_DESC.to_string(), Vec::new()));
            }
            Tag::HtmlBlock => {
                self.stack.push((node::RAW_BLOCK.to_string(), Vec::new()));
            }
            Tag::MetadataBlock(kind) => {
                let format = match kind {
                    MetadataBlockKind::YamlStyle => "yaml",
                    MetadataBlockKind::PlusesStyle => "toml",
                };
                self.stack
                    .push((format!("metadata_block:{}", format), Vec::new()));
            }
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        if matches!(tag, TagEnd::TableHead) {
            self.in_table_head = false;
        }

        if let Some((kind, children)) = self.stack.pop() {
            let node = if let Some(rest) = kind.strip_prefix("heading:") {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                let level: i64 = parts.first().unwrap_or(&"1").parse().unwrap_or(1);
                let id = parts.get(1).unwrap_or(&"");
                let mut heading = Node::new(node::HEADING)
                    .prop(prop::LEVEL, level)
                    .children(children);
                if !id.is_empty() {
                    heading = heading.prop(prop::ID, *id);
                }
                heading
            } else if let Some(rest) = kind.strip_prefix("list:") {
                let ordered = rest == "true";
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
            } else if let Some(rest) = kind.strip_prefix("code_block:") {
                let content = children
                    .iter()
                    .filter_map(|n| n.props.get_str(prop::CONTENT))
                    .collect::<Vec<_>>()
                    .join("");
                let mut code = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
                if !rest.is_empty() {
                    code = code.prop(prop::LANGUAGE, rest);
                }
                code
            } else if let Some(name) = kind.strip_prefix("footnote_def:") {
                Node::new(node::FOOTNOTE_DEF)
                    .prop(prop::ID, name)
                    .children(children)
            } else if let Some(format) = kind.strip_prefix("metadata_block:") {
                // Parse metadata from children
                let content = children
                    .iter()
                    .filter_map(|n| n.props.get_str(prop::CONTENT))
                    .collect::<Vec<_>>()
                    .join("");
                self.parse_metadata(&content, format);
                return; // Don't add metadata block to document
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

            // Skip adding to tree for metadata blocks (handled above)
            if let TagEnd::MetadataBlock(_) = tag {
                return;
            }

            if let Some((_, parent_children)) = self.stack.last_mut() {
                parent_children.push(node);
            } else {
                self.root.push(node);
            }
        }
    }

    fn parse_metadata(&mut self, content: &str, _format: &str) {
        // Simple key: value parsing for YAML-style metadata
        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !value.is_empty() {
                    self.metadata.set(
                        key.to_string(),
                        rescribe_core::PropValue::String(value.to_string()),
                    );
                }
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

    fn add_footnote_ref(&mut self, name: &str) {
        let footnote = Node::new(node::FOOTNOTE_REF).prop(prop::ID, name.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(footnote);
        } else {
            self.root.push(footnote);
        }
    }

    fn add_task_marker(&mut self, checked: bool) {
        // Add checked property to current list item
        if let Some((kind, _)) = self.stack.last_mut()
            && kind == node::LIST_ITEM
        {
            *kind = format!("list_item:{}", checked);
        }
    }

    fn add_inline_math(&mut self, math: &str) {
        let math_node = Node::new("math_inline").prop(prop::CONTENT, math.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(math_node);
        } else {
            self.root.push(math_node);
        }
    }

    fn add_display_math(&mut self, math: &str) {
        let math_node = Node::new("math_block").prop(prop::CONTENT, math.to_string());
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(math_node);
        } else {
            self.root.push(math_node);
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
    fn test_parse_footnote() {
        let md = "Here is a footnote[^1].\n\n[^1]: This is the footnote.";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_definition_list() {
        let md = "Term\n: Definition";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let result = parse(md).unwrap();
        let doc = result.value;
        let table = &doc.content.children[0];
        assert_eq!(table.kind.as_str(), node::TABLE);
    }

    #[test]
    fn test_parse_math() {
        let md = "Inline $x^2$ math and display $$y = mx + b$$ math.";
        let result = parse(md).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
