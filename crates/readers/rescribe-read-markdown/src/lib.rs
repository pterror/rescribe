//! Markdown reader for rescribe.
//!
//! Parses CommonMark (with extensions) into rescribe's document IR.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Severity, WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse markdown text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse markdown with custom options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut warnings = Vec::new();

    // Enable common extensions
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(input, opts);
    let events: Vec<_> = parser.collect();

    let children = parse_events(&events, &mut warnings);

    // Wrap children in a document root node
    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root);
    Ok(ConversionResult::with_warnings(doc, warnings))
}

/// Parse a slice of events into nodes.
fn parse_events(events: &[Event<'_>], warnings: &mut Vec<FidelityWarning>) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut idx = 0;

    while idx < events.len() {
        let (node, consumed) = parse_event(&events[idx..], warnings);
        if let Some(n) = node {
            nodes.push(n);
        }
        idx += consumed.max(1);
    }

    nodes
}

/// Parse a single event or matched tag pair, returning the node and events consumed.
fn parse_event(events: &[Event<'_>], warnings: &mut Vec<FidelityWarning>) -> (Option<Node>, usize) {
    match &events[0] {
        Event::Start(tag) => parse_tag(tag.clone(), events, warnings),
        Event::Text(text) => (
            Some(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string())),
            1,
        ),
        Event::Code(code) => (
            Some(Node::new(node::CODE).prop(prop::CONTENT, code.to_string())),
            1,
        ),
        Event::SoftBreak => (Some(Node::new(node::SOFT_BREAK)), 1),
        Event::HardBreak => (Some(Node::new(node::LINE_BREAK)), 1),
        Event::Rule => (Some(Node::new(node::HORIZONTAL_RULE)), 1),
        Event::End(_) => (None, 1), // Handled by parent
        Event::Html(html) => {
            // Raw HTML block
            let node = Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "html")
                .prop(prop::CONTENT, html.to_string());
            (Some(node), 1)
        }
        Event::InlineHtml(html) => {
            // Raw HTML inline
            let node = Node::new(node::RAW_INLINE)
                .prop(prop::FORMAT, "html")
                .prop(prop::CONTENT, html.to_string());
            (Some(node), 1)
        }
        Event::FootnoteReference(label) => {
            let node = Node::new(node::FOOTNOTE_REF).prop(prop::LABEL, label.to_string());
            (Some(node), 1)
        }
        Event::TaskListMarker(_checked) => {
            // This modifies the list item; we'll handle it in list item parsing
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::FeatureLost("task_list".to_string()),
                "Task list markers are partially supported",
            ));
            (None, 1)
        }
        Event::InlineMath(math) => {
            let node = Node::new("math_inline")
                .prop("math:format", "latex")
                .prop("math:source", math.to_string());
            (Some(node), 1)
        }
        Event::DisplayMath(math) => {
            let node = Node::new("math_display")
                .prop("math:format", "latex")
                .prop("math:source", math.to_string());
            (Some(node), 1)
        }
    }
}

/// Parse a tag and its contents.
fn parse_tag(
    tag: Tag<'_>,
    events: &[Event<'_>],
    warnings: &mut Vec<FidelityWarning>,
) -> (Option<Node>, usize) {
    // Find the matching end tag
    let end_idx = find_matching_end(&events[1..], &tag);
    let inner_events = &events[1..=end_idx];
    let children = parse_events(inner_events, warnings);
    let consumed = end_idx + 2; // +1 for start, +1 for end

    let node = match tag {
        Tag::Paragraph => Some(Node::new(node::PARAGRAPH).children(children)),

        Tag::Heading { level, id, .. } => {
            let level_num = match level {
                HeadingLevel::H1 => 1,
                HeadingLevel::H2 => 2,
                HeadingLevel::H3 => 3,
                HeadingLevel::H4 => 4,
                HeadingLevel::H5 => 5,
                HeadingLevel::H6 => 6,
            };
            let mut h = Node::new(node::HEADING)
                .prop(prop::LEVEL, level_num as i64)
                .children(children);
            if let Some(id) = id {
                h = h.prop(prop::ID, id.to_string());
            }
            Some(h)
        }

        Tag::BlockQuote(_) => Some(Node::new(node::BLOCKQUOTE).children(children)),

        Tag::CodeBlock(kind) => {
            // For code blocks, children should be text content
            let content = children
                .into_iter()
                .filter_map(|n| {
                    if n.kind.as_str() == node::TEXT {
                        n.props.get_str(prop::CONTENT).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
            if let CodeBlockKind::Fenced(lang) = kind {
                let lang_str = lang.to_string();
                if !lang_str.is_empty() {
                    node = node.prop(prop::LANGUAGE, lang_str);
                }
            }
            Some(node)
        }

        Tag::List(start) => {
            let ordered = start.is_some();
            let mut list = Node::new(node::LIST)
                .prop(prop::ORDERED, ordered)
                .children(children);
            if let Some(start_num) = start {
                list = list.prop(prop::START, start_num as i64);
            }
            Some(list)
        }

        Tag::Item => Some(Node::new(node::LIST_ITEM).children(children)),

        Tag::FootnoteDefinition(label) => Some(
            Node::new(node::FOOTNOTE_DEF)
                .prop(prop::LABEL, label.to_string())
                .children(children),
        ),

        Tag::Table(alignments) => {
            // Store alignment info
            let align_strs: Vec<_> = alignments
                .iter()
                .map(|a| match a {
                    pulldown_cmark::Alignment::None => "none",
                    pulldown_cmark::Alignment::Left => "left",
                    pulldown_cmark::Alignment::Center => "center",
                    pulldown_cmark::Alignment::Right => "right",
                })
                .collect();
            Some(
                Node::new(node::TABLE)
                    .prop("column_alignments", align_strs.join(","))
                    .children(children),
            )
        }

        Tag::TableHead => Some(Node::new(node::TABLE_HEAD).children(children)),

        Tag::TableRow => Some(Node::new(node::TABLE_ROW).children(children)),

        Tag::TableCell => Some(Node::new(node::TABLE_CELL).children(children)),

        Tag::Emphasis => Some(Node::new(node::EMPHASIS).children(children)),

        Tag::Strong => Some(Node::new(node::STRONG).children(children)),

        Tag::Strikethrough => Some(Node::new(node::STRIKEOUT).children(children)),

        Tag::Link {
            dest_url, title, ..
        } => {
            let mut link = Node::new(node::LINK)
                .prop(prop::URL, dest_url.to_string())
                .children(children);
            if !title.is_empty() {
                link = link.prop(prop::TITLE, title.to_string());
            }
            Some(link)
        }

        Tag::Image {
            dest_url, title, ..
        } => {
            // For images, children are alt text
            let alt = children
                .into_iter()
                .filter_map(|n| {
                    if n.kind.as_str() == node::TEXT {
                        n.props.get_str(prop::CONTENT).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            let mut img = Node::new(node::IMAGE)
                .prop(prop::URL, dest_url.to_string())
                .prop(prop::ALT, alt);
            if !title.is_empty() {
                img = img.prop(prop::TITLE, title.to_string());
            }
            Some(img)
        }

        Tag::HtmlBlock => {
            // Raw HTML block - content is in children
            let content = children
                .into_iter()
                .filter_map(|n| n.props.get_str(prop::CONTENT).map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("");
            Some(
                Node::new(node::RAW_BLOCK)
                    .prop(prop::FORMAT, "html")
                    .prop(prop::CONTENT, content),
            )
        }

        Tag::MetadataBlock(_) => {
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode("metadata_block".to_string()),
                "Metadata blocks are not yet supported",
            ));
            None
        }

        Tag::DefinitionList => Some(Node::new(node::DEFINITION_LIST).children(children)),

        Tag::DefinitionListTitle => Some(Node::new(node::DEFINITION_TERM).children(children)),

        Tag::DefinitionListDefinition => Some(Node::new(node::DEFINITION_DESC).children(children)),
    };

    (node, consumed)
}

/// Find the index of the matching end tag.
fn find_matching_end(events: &[Event<'_>], start_tag: &Tag<'_>) -> usize {
    let mut depth = 1;
    for (i, event) in events.iter().enumerate() {
        match event {
            Event::Start(t) if tags_match(t, start_tag) => depth += 1,
            Event::End(t) if tag_end_matches(t, start_tag) => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    events.len().saturating_sub(1)
}

/// Check if two start tags are the same type.
fn tags_match(a: &Tag<'_>, b: &Tag<'_>) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Check if an end tag matches a start tag.
fn tag_end_matches(end: &TagEnd, start: &Tag<'_>) -> bool {
    matches!(
        (end, start),
        (TagEnd::Paragraph, Tag::Paragraph)
            | (TagEnd::Heading(_), Tag::Heading { .. })
            | (TagEnd::BlockQuote(_), Tag::BlockQuote(_))
            | (TagEnd::CodeBlock, Tag::CodeBlock(_))
            | (TagEnd::List(_), Tag::List(_))
            | (TagEnd::Item, Tag::Item)
            | (TagEnd::FootnoteDefinition, Tag::FootnoteDefinition(_))
            | (TagEnd::Table, Tag::Table(_))
            | (TagEnd::TableHead, Tag::TableHead)
            | (TagEnd::TableRow, Tag::TableRow)
            | (TagEnd::TableCell, Tag::TableCell)
            | (TagEnd::Emphasis, Tag::Emphasis)
            | (TagEnd::Strong, Tag::Strong)
            | (TagEnd::Strikethrough, Tag::Strikethrough)
            | (TagEnd::Link, Tag::Link { .. })
            | (TagEnd::Image, Tag::Image { .. })
            | (TagEnd::HtmlBlock, Tag::HtmlBlock)
            | (TagEnd::MetadataBlock(_), Tag::MetadataBlock(_))
            | (TagEnd::DefinitionList, Tag::DefinitionList)
            | (TagEnd::DefinitionListTitle, Tag::DefinitionListTitle)
            | (
                TagEnd::DefinitionListDefinition,
                Tag::DefinitionListDefinition
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_children(doc: &Document) -> &[Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_paragraph() {
        let result = parse("Hello, world!").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_heading() {
        let result = parse("# Heading 1\n\n## Heading 2").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(1));
        assert_eq!(children[1].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_emphasis() {
        let result = parse("*italic* and **bold**").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        let para = &children[0];
        // Should have: emphasis, text(" and "), strong
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::STRONG)
        );
    }

    #[test]
    fn test_parse_link() {
        let result = parse("[example](https://example.com)").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        let para = &children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_code_block() {
        let result = parse("```rust\nfn main() {}\n```").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(children[0].props.get_str(prop::LANGUAGE), Some("rust"));
    }

    #[test]
    fn test_parse_list() {
        let result = parse("- item 1\n- item 2").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(children[0].children.len(), 2);
    }

    #[test]
    fn test_parse_ordered_list() {
        let result = parse("1. first\n2. second").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(true));
    }
}
