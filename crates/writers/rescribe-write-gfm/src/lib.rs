//! GFM (GitHub Flavored Markdown) writer for rescribe.
//!
//! Generates GitHub Flavored Markdown output from rescribe's document IR.
//! Supports tables, strikethrough, autolinks, and task lists.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to GFM.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to GFM with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();
    emit_nodes(&doc.content.children, &mut output);
    Ok(ConversionResult::ok(output.into_bytes()))
}

fn emit_nodes(nodes: &[Node], output: &mut String) {
    for node in nodes {
        emit_node(node, output);
    }
}

fn emit_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::DOCUMENT => emit_nodes(&node.children, output),

        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            output.push_str(&"#".repeat(level as usize));
            output.push(' ');
            emit_inline_nodes(&node.children, output);
            output.push_str("\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, output);
            output.push_str("\n\n");
        }

        node::CODE_BLOCK => {
            let lang = node.props.get_str(prop::LANGUAGE).unwrap_or("");
            output.push_str("```");
            output.push_str(lang);
            output.push('\n');
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.push_str("```\n\n");
        }

        node::BLOCKQUOTE => {
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    output.push_str("> ");
                    emit_inline_nodes(&child.children, output);
                    output.push_str("\n\n");
                } else {
                    let mut inner = String::new();
                    emit_node(child, &mut inner);
                    for line in inner.lines() {
                        output.push_str("> ");
                        output.push_str(line);
                        output.push('\n');
                    }
                }
            }
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let mut item_num = 1;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    // Check for task list
                    let is_task = child.props.contains("checked");
                    let checked = child.props.get_bool("checked").unwrap_or(false);

                    if ordered {
                        output.push_str(&format!("{}. ", item_num));
                        item_num += 1;
                    } else {
                        output.push_str("- ");
                    }

                    if is_task {
                        if checked {
                            output.push_str("[x] ");
                        } else {
                            output.push_str("[ ] ");
                        }
                    }

                    for (i, item_child) in child.children.iter().enumerate() {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                            if i < child.children.len() - 1 {
                                output.push('\n');
                            }
                        } else if item_child.kind.as_str() == node::LIST {
                            output.push('\n');
                            let mut inner = String::new();
                            emit_node(item_child, &mut inner);
                            for line in inner.lines() {
                                output.push_str("  ");
                                output.push_str(line);
                                output.push('\n');
                            }
                        }
                    }
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        node::TABLE => {
            let mut is_first_row = true;

            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    output.push('|');
                    for cell in &row.children {
                        output.push(' ');
                        emit_inline_nodes(&cell.children, output);
                        output.push_str(" |");
                    }
                    output.push('\n');

                    // Add separator after header
                    if is_first_row {
                        output.push('|');
                        for _ in &row.children {
                            output.push_str(" --- |");
                        }
                        output.push('\n');
                        is_first_row = false;
                    }
                }
            }
            output.push('\n');
        }

        node::HORIZONTAL_RULE => {
            output.push_str("---\n\n");
        }

        node::DIV | node::FIGURE => {
            emit_nodes(&node.children, output);
        }

        node::RAW_BLOCK => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
        }

        _ => emit_nodes(&node.children, output),
    }
}

fn emit_inline_nodes(nodes: &[Node], output: &mut String) {
    for node in nodes {
        emit_inline_node(node, output);
    }
}

fn emit_inline_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
        }

        node::STRONG => {
            output.push_str("**");
            emit_inline_nodes(&node.children, output);
            output.push_str("**");
        }

        node::EMPHASIS => {
            output.push('*');
            emit_inline_nodes(&node.children, output);
            output.push('*');
        }

        node::STRIKEOUT => {
            output.push_str("~~");
            emit_inline_nodes(&node.children, output);
            output.push_str("~~");
        }

        node::CODE => {
            output.push('`');
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
            emit_inline_nodes(&node.children, output);
            output.push('`');
        }

        node::LINK => {
            output.push('[');
            emit_inline_nodes(&node.children, output);
            output.push(']');
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push('(');
                output.push_str(url);
                output.push(')');
            }
        }

        node::IMAGE => {
            output.push_str("![");
            let alt = node.props.get_str(prop::ALT).unwrap_or("");
            output.push_str(alt);
            output.push(']');
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push('(');
                output.push_str(url);
                output.push(')');
            }
        }

        node::UNDERLINE => {
            // GFM doesn't have underline - use HTML
            output.push_str("<u>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</u>");
        }

        node::LINE_BREAK => output.push_str("  \n"),
        node::SOFT_BREAK => output.push('\n'),

        node::RAW_INLINE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
        }

        _ => emit_inline_nodes(&node.children, output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::*;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_basic() {
        let doc = doc(|d| {
            d.heading(1, |h| h.text("Title"))
                .para(|p| p.text("Hello world"))
        });
        let output = emit_str(&doc);
        assert!(output.contains("# Title"));
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn test_emit_strikethrough() {
        let doc = doc(|d| d.para(|p| p.strike(|s| s.text("deleted"))));
        let output = emit_str(&doc);
        assert!(output.contains("~~deleted~~"));
    }

    #[test]
    fn test_emit_table() {
        let doc = Document {
            content: Node::new(node::DOCUMENT).child(
                Node::new(node::TABLE)
                    .child(
                        Node::new(node::TABLE_ROW)
                            .child(
                                Node::new(node::TABLE_HEADER)
                                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "A")),
                            )
                            .child(
                                Node::new(node::TABLE_HEADER)
                                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "B")),
                            ),
                    )
                    .child(
                        Node::new(node::TABLE_ROW)
                            .child(
                                Node::new(node::TABLE_CELL)
                                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "1")),
                            )
                            .child(
                                Node::new(node::TABLE_CELL)
                                    .child(Node::new(node::TEXT).prop(prop::CONTENT, "2")),
                            ),
                    ),
            ),
            resources: Default::default(),
            metadata: Default::default(),
            source: None,
        };
        let output = emit_str(&doc);
        assert!(output.contains("| A | B |"));
        assert!(output.contains("| --- |"));
        assert!(output.contains("| 1 | 2 |"));
    }

    #[test]
    fn test_emit_task_list() {
        let doc = Document {
            content: Node::new(node::DOCUMENT).child(
                Node::new(node::LIST)
                    .child(
                        Node::new(node::LIST_ITEM).prop("checked", true).child(
                            Node::new(node::PARAGRAPH)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, "Done")),
                        ),
                    )
                    .child(
                        Node::new(node::LIST_ITEM).prop("checked", false).child(
                            Node::new(node::PARAGRAPH)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, "Todo")),
                        ),
                    ),
            ),
            resources: Default::default(),
            metadata: Default::default(),
            source: None,
        };
        let output = emit_str(&doc);
        assert!(output.contains("[x] Done"));
        assert!(output.contains("[ ] Todo"));
    }
}
