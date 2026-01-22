//! CommonMark writer for rescribe.
//!
//! Generates strict CommonMark output (no extensions) from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to CommonMark.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to CommonMark with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();
    emit_nodes(&doc.content.children, &mut output, 0);
    Ok(ConversionResult::ok(output.into_bytes()))
}

fn emit_nodes(nodes: &[Node], output: &mut String, indent: usize) {
    for node in nodes {
        emit_node(node, output, indent);
    }
}

fn emit_node(node: &Node, output: &mut String, indent: usize) {
    match node.kind.as_str() {
        node::DOCUMENT => emit_nodes(&node.children, output, indent),

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
                    emit_node(child, &mut inner, indent);
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
                    if ordered {
                        output.push_str(&format!("{}. ", item_num));
                        item_num += 1;
                    } else {
                        output.push_str("- ");
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
                            emit_node(item_child, &mut inner, indent + 1);
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

        node::HORIZONTAL_RULE => {
            output.push_str("---\n\n");
        }

        // CommonMark doesn't support tables - output as text
        node::TABLE => {
            output.push_str("<!-- Table not supported in CommonMark -->\n\n");
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    for cell in &row.children {
                        emit_inline_nodes(&cell.children, output);
                        output.push_str(" | ");
                    }
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        node::DIV | node::FIGURE => {
            emit_nodes(&node.children, output, indent);
        }

        node::RAW_BLOCK => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
        }

        _ => emit_nodes(&node.children, output, indent),
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

        // CommonMark doesn't have strikethrough, underline, etc.
        node::STRIKEOUT | node::UNDERLINE => {
            emit_inline_nodes(&node.children, output);
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
    fn test_emit_formatting() {
        let doc = doc(|d| {
            d.para(|p| {
                p.strong(|s| s.text("bold"))
                    .text(" and ")
                    .em(|e| e.text("italic"))
            })
        });
        let output = emit_str(&doc);
        assert!(output.contains("**bold**"));
        assert!(output.contains("*italic*"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.code_block("fn main() {}"));
        let output = emit_str(&doc);
        assert!(output.contains("```"));
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn test_emit_list() {
        let doc =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));
        let output = emit_str(&doc);
        assert!(output.contains("- Item 1"));
        assert!(output.contains("- Item 2"));
    }
}
