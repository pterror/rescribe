//! Djot reader for rescribe.
//!
//! Parses Djot markup into rescribe's document IR using the jotdown crate.
//!
//! # Example
//!
//! ```
//! use rescribe_read_djot::parse;
//!
//! let result = parse("# Hello\n\nWorld!").unwrap();
//! let doc = result.value;
//! ```

use jotdown::{Container, Event, ListKind, Parser};
use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse Djot text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let mut converter = Converter::new();
    let parser = Parser::new(input);

    converter.convert(parser)?;

    let document = Document {
        content: Node::new(node::DOCUMENT).children(converter.result),
        resources: Default::default(),
        metadata: Properties::new(),
        source: None,
    };

    Ok(ConversionResult::with_warnings(
        document,
        converter.warnings,
    ))
}

struct Converter {
    result: Vec<Node>,
    stack: Vec<StackFrame>,
    warnings: Vec<FidelityWarning>,
}

struct StackFrame {
    container: FrameContainer,
    children: Vec<Node>,
    props: Properties,
}

#[derive(Clone)]
#[allow(dead_code)]
enum FrameContainer {
    Document,
    Paragraph,
    Heading(i64),
    Blockquote,
    List { ordered: bool, start: i64 },
    ListItem,
    TaskListItem { checked: bool },
    CodeBlock { language: Option<String> },
    Table,
    TableRow { is_header: bool },
    TableCell,
    Section,
    Div,
    Link { url: String, title: Option<String> },
    Image { url: String, alt: Option<String> },
    Emphasis,
    Strong,
    Strikeout,
    Subscript,
    Superscript,
    Mark,
    Insert,
    Span,
    Verbatim,
    Math { display: bool },
    Footnote { label: String },
    DescriptionList,
    DescriptionTerm,
    DescriptionDetails,
    RawBlock { format: String },
    RawInline { format: String },
}

impl Converter {
    fn new() -> Self {
        Self {
            result: Vec::new(),
            stack: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn convert<'a>(&mut self, parser: impl Iterator<Item = Event<'a>>) -> Result<(), ParseError> {
        for event in parser {
            self.process_event(event)?;
        }
        Ok(())
    }

    fn process_event(&mut self, event: Event<'_>) -> Result<(), ParseError> {
        match event {
            Event::Start(container, attrs) => {
                self.start_container(container, attrs)?;
            }
            Event::End(container) => {
                self.end_container(container)?;
            }
            Event::Str(text) => {
                self.push_text(&text);
            }
            Event::Softbreak => {
                self.push_node(Node::new(node::SOFT_BREAK));
            }
            Event::Hardbreak => {
                self.push_node(Node::new(node::LINE_BREAK));
            }
            Event::NonBreakingSpace => {
                self.push_text("\u{00A0}");
            }
            Event::ThematicBreak(_attrs) => {
                self.push_node(Node::new(node::HORIZONTAL_RULE));
            }
            Event::FootnoteReference(label) => {
                let node = Node::new(node::FOOTNOTE_REF).prop(prop::LABEL, label.to_string());
                self.push_node(node);
            }
            Event::LeftSingleQuote => self.push_text("\u{2018}"),
            Event::RightSingleQuote => self.push_text("\u{2019}"),
            Event::LeftDoubleQuote => self.push_text("\u{201C}"),
            Event::RightDoubleQuote => self.push_text("\u{201D}"),
            Event::Ellipsis => self.push_text("\u{2026}"),
            Event::EnDash => self.push_text("\u{2013}"),
            Event::EmDash => self.push_text("\u{2014}"),
            Event::Escape => {}    // Escape character is consumed
            Event::Blankline => {} // Blank lines are structural, not content
            Event::Symbol(sym) => {
                // Symbols like :emoji_name: - just output as text for now
                self.push_text(&format!(":{sym}:"));
            }
            Event::Attributes(_) => {
                // Dangling attributes - we ignore these for now
            }
        }
        Ok(())
    }

    fn start_container(
        &mut self,
        container: Container<'_>,
        _attrs: jotdown::Attributes<'_>,
    ) -> Result<(), ParseError> {
        let frame_container = match container {
            Container::Paragraph => FrameContainer::Paragraph,
            Container::Heading { level, .. } => FrameContainer::Heading(level as i64),
            Container::Blockquote => FrameContainer::Blockquote,
            Container::List { kind, .. } => {
                let (ordered, start) = match kind {
                    ListKind::Unordered(..) => (false, 1),
                    ListKind::Ordered { start, .. } => (true, start as i64),
                    ListKind::Task(..) => (false, 1),
                };
                FrameContainer::List { ordered, start }
            }
            Container::ListItem => FrameContainer::ListItem,
            Container::TaskListItem { checked } => FrameContainer::TaskListItem { checked },
            Container::CodeBlock { language, .. } => FrameContainer::CodeBlock {
                language: if language.is_empty() {
                    None
                } else {
                    Some(language.to_string())
                },
            },
            Container::Table => FrameContainer::Table,
            Container::TableRow { head } => FrameContainer::TableRow { is_header: head },
            Container::TableCell { .. } => FrameContainer::TableCell,
            Container::Section { .. } => FrameContainer::Section,
            Container::Div { class } => {
                let frame = FrameContainer::Div;
                if !class.is_empty() {
                    // Store class in properties
                    let mut props = Properties::new();
                    props.set("html:class", class.to_string());
                    self.stack.push(StackFrame {
                        container: frame,
                        children: Vec::new(),
                        props,
                    });
                    return Ok(());
                }
                frame
            }
            Container::Link(url, _link_type) => FrameContainer::Link {
                url: url.to_string(),
                title: None,
            },
            Container::Image(url, _link_type) => FrameContainer::Image {
                url: url.to_string(),
                alt: None,
            },
            Container::Emphasis => FrameContainer::Emphasis,
            Container::Strong => FrameContainer::Strong,
            Container::Delete => FrameContainer::Strikeout,
            Container::Subscript => FrameContainer::Subscript,
            Container::Superscript => FrameContainer::Superscript,
            Container::Mark => FrameContainer::Mark,
            Container::Insert => FrameContainer::Insert,
            Container::Span => FrameContainer::Span,
            Container::Verbatim => FrameContainer::Verbatim,
            Container::Math { display } => FrameContainer::Math { display },
            Container::Footnote { label } => FrameContainer::Footnote {
                label: label.to_string(),
            },
            Container::DescriptionList => FrameContainer::DescriptionList,
            Container::DescriptionTerm => FrameContainer::DescriptionTerm,
            Container::DescriptionDetails => FrameContainer::DescriptionDetails,
            Container::RawBlock { format } => FrameContainer::RawBlock {
                format: format.to_string(),
            },
            Container::RawInline { format } => FrameContainer::RawInline {
                format: format.to_string(),
            },
            Container::Caption | Container::LinkDefinition { .. } => {
                // Caption and link definitions - skip for now
                return Ok(());
            }
        };

        self.stack.push(StackFrame {
            container: frame_container,
            children: Vec::new(),
            props: Properties::new(),
        });
        Ok(())
    }

    fn end_container(&mut self, _container: Container<'_>) -> Result<(), ParseError> {
        let frame = match self.stack.pop() {
            Some(f) => f,
            None => return Ok(()), // Handle mismatched end events gracefully
        };

        let node = match frame.container {
            FrameContainer::Document => {
                // Should not happen
                return Ok(());
            }
            FrameContainer::Paragraph => Node::new(node::PARAGRAPH).children(frame.children),
            FrameContainer::Heading(level) => Node::new(node::HEADING)
                .prop(prop::LEVEL, level)
                .children(frame.children),
            FrameContainer::Blockquote => Node::new(node::BLOCKQUOTE).children(frame.children),
            FrameContainer::List { ordered, start } => {
                let mut list = Node::new(node::LIST)
                    .prop(prop::ORDERED, ordered)
                    .children(frame.children);
                if ordered && start != 1 {
                    list = list.prop(prop::START, start);
                }
                list
            }
            FrameContainer::ListItem => Node::new(node::LIST_ITEM).children(frame.children),
            FrameContainer::TaskListItem { checked } => Node::new(node::LIST_ITEM)
                .prop(prop::CHECKED, checked)
                .children(frame.children),
            FrameContainer::CodeBlock { language } => {
                // Collect text content
                let content = collect_text(&frame.children);
                let mut cb = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
                if let Some(lang) = language {
                    cb = cb.prop(prop::LANGUAGE, lang);
                }
                cb
            }
            FrameContainer::Table => Node::new(node::TABLE).children(frame.children),
            FrameContainer::TableRow { is_header } => {
                if is_header {
                    // Convert children to table_header nodes
                    let headers: Vec<_> = frame
                        .children
                        .into_iter()
                        .map(|cell| {
                            let mut header = Node::new(node::TABLE_HEADER);
                            header.children = cell.children;
                            header.props = cell.props;
                            header
                        })
                        .collect();
                    Node::new(node::TABLE_ROW).children(headers)
                } else {
                    Node::new(node::TABLE_ROW).children(frame.children)
                }
            }
            FrameContainer::TableCell => Node::new(node::TABLE_CELL).children(frame.children),
            FrameContainer::Section => {
                // Section is structural in Djot, we flatten it
                // Just push children to parent
                for child in frame.children {
                    self.push_node(child);
                }
                return Ok(());
            }
            FrameContainer::Div => {
                let mut div = Node::new(node::DIV).children(frame.children);
                div.props = frame.props;
                div
            }
            FrameContainer::Link { url, title } => {
                let mut link = Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .children(frame.children);
                if let Some(t) = title {
                    link = link.prop(prop::TITLE, t);
                }
                link
            }
            FrameContainer::Image { url, alt } => {
                let mut img = Node::new(node::IMAGE).prop(prop::URL, url);
                if let Some(a) = alt {
                    img = img.prop(prop::ALT, a);
                } else {
                    // Extract alt from children
                    let alt_text = collect_text(&frame.children);
                    if !alt_text.is_empty() {
                        img = img.prop(prop::ALT, alt_text);
                    }
                }
                img
            }
            FrameContainer::Emphasis => Node::new(node::EMPHASIS).children(frame.children),
            FrameContainer::Strong => Node::new(node::STRONG).children(frame.children),
            FrameContainer::Strikeout => Node::new(node::STRIKEOUT).children(frame.children),
            FrameContainer::Subscript => Node::new(node::SUBSCRIPT).children(frame.children),
            FrameContainer::Superscript => Node::new(node::SUPERSCRIPT).children(frame.children),
            FrameContainer::Mark => {
                // Mark is highlight - use span with class
                Node::new(node::SPAN)
                    .prop("html:class", "mark")
                    .children(frame.children)
            }
            FrameContainer::Insert => {
                // Inserted text - use underline as a reasonable approximation
                Node::new(node::UNDERLINE).children(frame.children)
            }
            FrameContainer::Span => Node::new(node::SPAN).children(frame.children),
            FrameContainer::Verbatim => {
                // Inline code
                let content = collect_text(&frame.children);
                Node::new(node::CODE).prop(prop::CONTENT, content)
            }
            FrameContainer::Math { display } => {
                let content = collect_text(&frame.children);
                if display {
                    Node::new("math:display").prop(prop::CONTENT, content)
                } else {
                    Node::new("math:inline").prop(prop::CONTENT, content)
                }
            }
            FrameContainer::Footnote { label } => Node::new(node::FOOTNOTE_DEF)
                .prop(prop::LABEL, label)
                .children(frame.children),
            FrameContainer::DescriptionList => {
                Node::new(node::DEFINITION_LIST).children(frame.children)
            }
            FrameContainer::DescriptionTerm => {
                Node::new(node::DEFINITION_TERM).children(frame.children)
            }
            FrameContainer::DescriptionDetails => {
                Node::new(node::DEFINITION_DESC).children(frame.children)
            }
            FrameContainer::RawBlock { format } => {
                let content = collect_text(&frame.children);
                Node::new(node::RAW_BLOCK)
                    .prop(prop::FORMAT, format)
                    .prop(prop::CONTENT, content)
            }
            FrameContainer::RawInline { format } => {
                let content = collect_text(&frame.children);
                Node::new(node::RAW_INLINE)
                    .prop(prop::FORMAT, format)
                    .prop(prop::CONTENT, content)
            }
        };

        self.push_node(node);
        Ok(())
    }

    fn push_node(&mut self, node: Node) {
        if let Some(frame) = self.stack.last_mut() {
            frame.children.push(node);
        } else {
            self.result.push(node);
        }
    }

    fn push_text(&mut self, text: &str) {
        let node = Node::new(node::TEXT).prop(prop::CONTENT, text.to_string());
        self.push_node(node);
    }
}

fn collect_text(nodes: &[Node]) -> String {
    let mut result = String::new();
    for node in nodes {
        if node.kind.as_str() == node::TEXT
            && let Some(content) = node.props.get_str(prop::CONTENT)
        {
            result.push_str(content);
        }
        result.push_str(&collect_text(&node.children));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paragraph() {
        let result = parse("Hello, world!").unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_heading() {
        let result = parse("# Heading 1\n\n## Heading 2").unwrap();
        let doc = result.value;
        // Djot creates sections, so we might have nested content
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_emphasis() {
        let result = parse("_emphasis_ and *strong*").unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 1);
        let para = &doc.content.children[0];
        // Should have emphasis and strong nodes
        let has_emphasis = para
            .children
            .iter()
            .any(|n| n.kind.as_str() == node::EMPHASIS);
        let has_strong = para
            .children
            .iter()
            .any(|n| n.kind.as_str() == node::STRONG);
        assert!(has_emphasis);
        assert!(has_strong);
    }

    #[test]
    fn test_parse_link() {
        let result = parse("[link](https://example.com)").unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 1);
        let para = &doc.content.children[0];
        let link = para.children.iter().find(|n| n.kind.as_str() == node::LINK);
        assert!(link.is_some());
        let link = link.unwrap();
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_code_block() {
        let result = parse("```rust\nfn main() {}\n```").unwrap();
        let doc = result.value;
        let cb = doc
            .content
            .children
            .iter()
            .find(|n| n.kind.as_str() == node::CODE_BLOCK);
        assert!(cb.is_some());
        let cb = cb.unwrap();
        assert_eq!(cb.props.get_str(prop::LANGUAGE), Some("rust"));
    }

    #[test]
    fn test_parse_list() {
        let result = parse("- item 1\n- item 2").unwrap();
        let doc = result.value;
        let list = doc
            .content
            .children
            .iter()
            .find(|n| n.kind.as_str() == node::LIST);
        assert!(list.is_some());
        let list = list.unwrap();
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }
}
