//! CommonMark reader for rescribe.
//!
//! Parses strict CommonMark (no extensions) into rescribe's document IR.

use pulldown_cmark::{Event, Options, Parser, Tag};
use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};
use rescribe_std::{Node, node, prop};

/// Parse CommonMark input into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse CommonMark input into a document with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    // CommonMark uses no extensions - strict spec only
    let parser = Parser::new_ext(input, Options::empty());
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
                    Tag::Link { dest_url, .. } => {
                        Node::new(node::LINK).prop(prop::URL, dest_url.to_string())
                    }
                    Tag::Image { dest_url, .. } => {
                        Node::new(node::IMAGE).prop(prop::URL, dest_url.to_string())
                    }
                    _ => Node::new(node::SPAN),
                };
                stack.push(node);
            }
            Event::End(_) => {
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
            Event::Html(html) => {
                // CommonMark includes raw HTML
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
    fn test_parse_emphasis() {
        let input = "*italic* and **bold**";
        let result = parse(input).unwrap();
        let para = &result.value.content.children[0];
        assert_eq!(para.kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_code() {
        let input = "```\ncode\n```";
        let result = parse(input).unwrap();
        let code = &result.value.content.children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
    }

    #[test]
    fn test_no_extensions() {
        // CommonMark should NOT parse tables (that's a GFM extension)
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let result = parse(input).unwrap();
        // Should be parsed as paragraphs, not a table
        let first = &result.value.content.children[0];
        assert_eq!(first.kind.as_str(), node::PARAGRAPH);
    }
}
