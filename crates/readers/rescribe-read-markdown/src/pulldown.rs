//! Markdown parser using pulldown-cmark.

use std::ops::Range;

use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Properties, Severity,
    Span, WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse markdown text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse markdown with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut warnings = Vec::new();
    let mut metadata = Properties::new();

    // Enable common extensions including full GFM support
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    opts.insert(Options::ENABLE_GFM); // GitHub-style blockquotes like [!NOTE]

    let parser = Parser::new_ext(input, opts);
    // Collect events with source ranges for span tracking
    let events: Vec<_> = parser.into_offset_iter().collect();

    let children = parse_events(
        &events,
        &mut warnings,
        &mut metadata,
        options.preserve_source_info,
    );

    // Wrap children in a document root node
    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root).with_metadata(metadata);
    Ok(ConversionResult::with_warnings(doc, warnings))
}

/// Parse a slice of events into nodes.
fn parse_events(
    events: &[(Event<'_>, Range<usize>)],
    warnings: &mut Vec<FidelityWarning>,
    metadata: &mut Properties,
    preserve_spans: bool,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut idx = 0;

    while idx < events.len() {
        let (node, consumed) = parse_event(&events[idx..], warnings, metadata, preserve_spans);
        if let Some(n) = node {
            nodes.push(n);
        }
        idx += consumed.max(1);
    }

    nodes
}

/// Helper to optionally add span to a node.
fn with_span(mut node: Node, range: &Range<usize>, preserve_spans: bool) -> Node {
    if preserve_spans {
        node.span = Some(Span {
            start: range.start,
            end: range.end,
        });
    }
    node
}

/// Parse a single event or matched tag pair, returning the node and events consumed.
fn parse_event(
    events: &[(Event<'_>, Range<usize>)],
    warnings: &mut Vec<FidelityWarning>,
    metadata: &mut Properties,
    preserve_spans: bool,
) -> (Option<Node>, usize) {
    let (event, range) = &events[0];
    match event {
        Event::Start(tag) => parse_tag(tag.clone(), events, warnings, metadata, preserve_spans),
        Event::Text(text) => (
            Some(with_span(
                Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()),
                range,
                preserve_spans,
            )),
            1,
        ),
        Event::Code(code) => (
            Some(with_span(
                Node::new(node::CODE).prop(prop::CONTENT, code.to_string()),
                range,
                preserve_spans,
            )),
            1,
        ),
        Event::SoftBreak => (
            Some(with_span(
                Node::new(node::SOFT_BREAK),
                range,
                preserve_spans,
            )),
            1,
        ),
        Event::HardBreak => (
            Some(with_span(
                Node::new(node::LINE_BREAK),
                range,
                preserve_spans,
            )),
            1,
        ),
        Event::Rule => (
            Some(with_span(
                Node::new(node::HORIZONTAL_RULE),
                range,
                preserve_spans,
            )),
            1,
        ),
        Event::End(_) => (None, 1), // Handled by parent
        Event::Html(html) => {
            // Raw HTML block
            let node = Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "html")
                .prop(prop::CONTENT, html.to_string());
            (Some(with_span(node, range, preserve_spans)), 1)
        }
        Event::InlineHtml(html) => {
            // Raw HTML inline
            let node = Node::new(node::RAW_INLINE)
                .prop(prop::FORMAT, "html")
                .prop(prop::CONTENT, html.to_string());
            (Some(with_span(node, range, preserve_spans)), 1)
        }
        Event::FootnoteReference(label) => {
            let node = Node::new(node::FOOTNOTE_REF).prop(prop::LABEL, label.to_string());
            (Some(with_span(node, range, preserve_spans)), 1)
        }
        Event::TaskListMarker(_checked) => {
            // Task list markers are handled in list item parsing (parse_tag for Tag::Item)
            // This branch should rarely be reached since we process them there
            (None, 1)
        }
        Event::InlineMath(math) => {
            let node = Node::new("math_inline")
                .prop("math:format", "latex")
                .prop("math:source", math.to_string());
            (Some(with_span(node, range, preserve_spans)), 1)
        }
        Event::DisplayMath(math) => {
            let node = Node::new("math_display")
                .prop("math:format", "latex")
                .prop("math:source", math.to_string());
            (Some(with_span(node, range, preserve_spans)), 1)
        }
    }
}

/// Parse a tag and its contents.
fn parse_tag(
    tag: Tag<'_>,
    events: &[(Event<'_>, Range<usize>)],
    warnings: &mut Vec<FidelityWarning>,
    metadata: &mut Properties,
    preserve_spans: bool,
) -> (Option<Node>, usize) {
    // Find the matching end tag
    let end_idx = find_matching_end(&events[1..], &tag);
    let inner_events = &events[1..=end_idx];
    let children = parse_events(inner_events, warnings, metadata, preserve_spans);
    let consumed = end_idx + 2; // +1 for start, +1 for end

    // Calculate span from start of first event to end of last event
    let tag_range = {
        let start = events[0].1.start;
        let end = events[end_idx + 1].1.end;
        start..end
    };

    let node = match tag {
        Tag::Paragraph => Some(with_span(
            Node::new(node::PARAGRAPH).children(children),
            &tag_range,
            preserve_spans,
        )),

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
            Some(with_span(h, &tag_range, preserve_spans))
        }

        Tag::BlockQuote(_) => Some(with_span(
            Node::new(node::BLOCKQUOTE).children(children),
            &tag_range,
            preserve_spans,
        )),

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
            Some(with_span(node, &tag_range, preserve_spans))
        }

        Tag::List(start) => {
            let ordered = start.is_some();
            let mut list = Node::new(node::LIST)
                .prop(prop::ORDERED, ordered)
                .children(children);
            if let Some(start_num) = start {
                list = list.prop(prop::START, start_num as i64);
            }
            Some(with_span(list, &tag_range, preserve_spans))
        }

        Tag::Item => {
            // Check for task list marker in inner events
            let task_checked = inner_events.iter().find_map(|(event, _)| {
                if let Event::TaskListMarker(checked) = event {
                    Some(*checked)
                } else {
                    None
                }
            });

            let mut item = Node::new(node::LIST_ITEM).children(children);
            if let Some(checked) = task_checked {
                item = item.prop(prop::CHECKED, checked);
            }
            Some(with_span(item, &tag_range, preserve_spans))
        }

        Tag::FootnoteDefinition(label) => Some(with_span(
            Node::new(node::FOOTNOTE_DEF)
                .prop(prop::LABEL, label.to_string())
                .children(children),
            &tag_range,
            preserve_spans,
        )),

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
            Some(with_span(
                Node::new(node::TABLE)
                    .prop("column_alignments", align_strs.join(","))
                    .children(children),
                &tag_range,
                preserve_spans,
            ))
        }

        Tag::TableHead => Some(with_span(
            Node::new(node::TABLE_HEAD).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::TableRow => Some(with_span(
            Node::new(node::TABLE_ROW).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::TableCell => Some(with_span(
            Node::new(node::TABLE_CELL).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::Emphasis => Some(with_span(
            Node::new(node::EMPHASIS).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::Strong => Some(with_span(
            Node::new(node::STRONG).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::Strikethrough => Some(with_span(
            Node::new(node::STRIKEOUT).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::Link {
            dest_url, title, ..
        } => {
            let mut link = Node::new(node::LINK)
                .prop(prop::URL, dest_url.to_string())
                .children(children);
            if !title.is_empty() {
                link = link.prop(prop::TITLE, title.to_string());
            }
            Some(with_span(link, &tag_range, preserve_spans))
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
            Some(with_span(img, &tag_range, preserve_spans))
        }

        Tag::HtmlBlock => {
            // Raw HTML block - content is in children
            let content = children
                .into_iter()
                .filter_map(|n| n.props.get_str(prop::CONTENT).map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("");
            Some(with_span(
                Node::new(node::RAW_BLOCK)
                    .prop(prop::FORMAT, "html")
                    .prop(prop::CONTENT, content),
                &tag_range,
                preserve_spans,
            ))
        }

        Tag::MetadataBlock(kind) => {
            // Extract YAML content from children (text nodes)
            let content = children
                .iter()
                .filter_map(|n| {
                    if n.kind.as_str() == node::TEXT {
                        n.props.get_str(prop::CONTENT).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            match kind {
                MetadataBlockKind::YamlStyle => {
                    parse_yaml_metadata(&content, metadata, warnings);
                }
                MetadataBlockKind::PlusesStyle => {
                    // TOML-style frontmatter, not yet supported
                    warnings.push(FidelityWarning::new(
                        Severity::Minor,
                        WarningKind::UnsupportedNode("toml_frontmatter".to_string()),
                        "TOML frontmatter is not yet supported",
                    ));
                }
            }
            None
        }

        Tag::DefinitionList => Some(with_span(
            Node::new(node::DEFINITION_LIST).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::DefinitionListTitle => Some(with_span(
            Node::new(node::DEFINITION_TERM).children(children),
            &tag_range,
            preserve_spans,
        )),

        Tag::DefinitionListDefinition => Some(with_span(
            Node::new(node::DEFINITION_DESC).children(children),
            &tag_range,
            preserve_spans,
        )),
    };

    (node, consumed)
}

/// Find the index of the matching end tag.
fn find_matching_end(events: &[(Event<'_>, Range<usize>)], start_tag: &Tag<'_>) -> usize {
    let mut depth = 1;
    for (i, (event, _)) in events.iter().enumerate() {
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

/// Parse YAML frontmatter and populate document metadata.
fn parse_yaml_metadata(
    content: &str,
    metadata: &mut Properties,
    warnings: &mut Vec<FidelityWarning>,
) {
    // Parse YAML as a mapping
    let yaml: Result<serde_yaml::Value, _> = serde_yaml::from_str(content);

    match yaml {
        Ok(serde_yaml::Value::Mapping(map)) => {
            for (key, value) in map {
                if let serde_yaml::Value::String(key_str) = key {
                    match value {
                        serde_yaml::Value::String(s) => {
                            metadata.set(&key_str, s);
                        }
                        serde_yaml::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                metadata.set(&key_str, i);
                            } else if let Some(f) = n.as_f64() {
                                // Store floats as strings for now
                                metadata.set(&key_str, f.to_string());
                            }
                        }
                        serde_yaml::Value::Bool(b) => {
                            metadata.set(&key_str, b);
                        }
                        serde_yaml::Value::Sequence(seq) => {
                            // Store arrays as comma-separated strings
                            let items: Vec<String> = seq
                                .into_iter()
                                .filter_map(|v| match v {
                                    serde_yaml::Value::String(s) => Some(s),
                                    _ => None,
                                })
                                .collect();
                            if !items.is_empty() {
                                metadata.set(&key_str, items.join(", "));
                            }
                        }
                        _ => {
                            // Nested objects not supported yet
                        }
                    }
                }
            }
        }
        Ok(_) => {
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::FeatureLost("yaml_frontmatter".to_string()),
                "YAML frontmatter must be a mapping/object",
            ));
        }
        Err(e) => {
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::FeatureLost("yaml_frontmatter".to_string()),
                format!("Failed to parse YAML frontmatter: {}", e),
            ));
        }
    }
}
