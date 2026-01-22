//! GFM (GitHub Flavored Markdown) reader for rescribe.
//!
//! Parses GitHub Flavored Markdown into rescribe's document IR.
//! Supports tables, strikethrough, autolinks, and task lists.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};
use rescribe_std::{Node, node, prop};

/// Parse GFM input into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse GFM input into a document with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    // GFM extensions: tables, strikethrough, autolinks, task lists
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(input, options);
    let content = parse_events(parser)?;

    Ok(ConversionResult::ok(Document {
        content,
        resources: Default::default(),
        metadata: Default::default(),
        source: None,
    }))
}

fn parse_events<'a>(parser: impl Iterator<Item = Event<'a>>) -> Result<Node, ParseError> {
    let mut doc = Node::new(node::DOCUMENT);
    let mut stack: Vec<Node> = vec![];
    let mut in_table_head = false;

    for event in parser {
        match event {
            Event::Start(tag) => {
                let node = match tag {
                    Tag::Paragraph => Node::new(node::PARAGRAPH),
                    Tag::Heading { level, .. } => {
                        Node::new(node::HEADING).prop(prop::LEVEL, level as i64)
                    }
                    Tag::BlockQuote(_) => Node::new(node::BLOCKQUOTE),
                    Tag::CodeBlock(kind) => {
                        let mut node = Node::new(node::CODE_BLOCK);
                        if let pulldown_cmark::CodeBlockKind::Fenced(lang) = kind
                            && !lang.is_empty()
                        {
                            node = node.prop(prop::LANGUAGE, lang.to_string());
                        }
                        node
                    }
                    Tag::List(first) => {
                        let mut node = Node::new(node::LIST);
                        if first.is_some() {
                            node = node.prop(prop::ORDERED, true);
                        }
                        node
                    }
                    Tag::Item => Node::new(node::LIST_ITEM),
                    Tag::Emphasis => Node::new(node::EMPHASIS),
                    Tag::Strong => Node::new(node::STRONG),
                    Tag::Strikethrough => Node::new(node::STRIKEOUT),
                    Tag::Link { dest_url, .. } => {
                        Node::new(node::LINK).prop(prop::URL, dest_url.to_string())
                    }
                    Tag::Image { dest_url, .. } => {
                        Node::new(node::IMAGE).prop(prop::URL, dest_url.to_string())
                    }
                    Tag::Table(_) => Node::new(node::TABLE),
                    Tag::TableHead => {
                        in_table_head = true;
                        Node::new(node::TABLE_ROW)
                    }
                    Tag::TableRow => Node::new(node::TABLE_ROW),
                    Tag::TableCell => {
                        if in_table_head {
                            Node::new(node::TABLE_HEADER)
                        } else {
                            Node::new(node::TABLE_CELL)
                        }
                    }
                    _ => Node::new(node::SPAN),
                };
                stack.push(node);
            }
            Event::End(tag) => {
                if matches!(tag, TagEnd::TableHead) {
                    in_table_head = false;
                }
                if let Some(node) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        *parent = parent.clone().child(node);
                    } else {
                        doc = doc.child(node);
                    }
                }
            }
            Event::Text(text) => {
                let text_node = Node::new(node::TEXT).prop(prop::CONTENT, text.to_string());
                if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(text_node);
                }
            }
            Event::Code(code) => {
                let code_node = Node::new(node::CODE).prop(prop::CONTENT, code.to_string());
                if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(code_node);
                }
            }
            Event::SoftBreak => {
                let br = Node::new(node::SOFT_BREAK);
                if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(br);
                }
            }
            Event::HardBreak => {
                let br = Node::new(node::LINE_BREAK);
                if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(br);
                }
            }
            Event::Rule => {
                let hr = Node::new(node::HORIZONTAL_RULE);
                if stack.is_empty() {
                    doc = doc.child(hr);
                } else if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(hr);
                }
            }
            Event::TaskListMarker(checked) => {
                // Add task list marker as a property on the list item
                if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().prop("checked", checked);
                }
            }
            Event::Html(html) => {
                let raw = Node::new(node::RAW_BLOCK)
                    .prop(prop::CONTENT, html.to_string())
                    .prop("format", "html");
                if stack.is_empty() {
                    doc = doc.child(raw);
                } else if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(raw);
                }
            }
            Event::InlineHtml(html) => {
                let raw = Node::new(node::RAW_INLINE)
                    .prop(prop::CONTENT, html.to_string())
                    .prop("format", "html");
                if let Some(parent) = stack.last_mut() {
                    *parent = parent.clone().child(raw);
                }
            }
            _ => {}
        }
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let input = "# Hello\n\nWorld!";
        let result = parse(input).unwrap();
        assert_eq!(result.value.content.children.len(), 2);
    }

    #[test]
    fn test_parse_strikethrough() {
        let input = "~~deleted~~";
        let result = parse(input).unwrap();
        let para = &result.value.content.children[0];
        let strike = &para.children[0];
        assert_eq!(strike.kind.as_str(), node::STRIKEOUT);
    }

    #[test]
    fn test_parse_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let result = parse(input).unwrap();
        let table = &result.value.content.children[0];
        assert_eq!(table.kind.as_str(), node::TABLE);
        assert_eq!(table.children.len(), 2); // header row + data row
    }

    #[test]
    fn test_parse_task_list() {
        let input = "- [x] Done\n- [ ] Todo";
        let result = parse(input).unwrap();
        let list = &result.value.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
    }
}
