//! XWiki writer for rescribe.
//!
//! Serializes rescribe's document IR to XWiki 2.0 markup.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to XWiki markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to XWiki markup with options.
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
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;
            for _ in 0..level.min(6) {
                output.push('=');
            }
            output.push(' ');
            emit_inline_nodes(&node.children, output);
            output.push(' ');
            for _ in 0..level.min(6) {
                output.push('=');
            }
            output.push('\n');
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, output);
            output.push_str("\n\n");
        }

        node::CODE_BLOCK => {
            if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
                output.push_str(&format!("{{{{code language=\"{}\"}}}}\n", lang));
            } else {
                output.push_str("{{code}}\n");
            }
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.push_str("{{/code}}\n\n");
        }

        node::BLOCKQUOTE => {
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    output.push_str("> ");
                    emit_inline_nodes(&child.children, output);
                    output.push('\n');
                } else {
                    emit_node(child, output);
                }
            }
            output.push('\n');
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    if ordered {
                        output.push_str("1. ");
                    } else {
                        output.push_str("* ");
                    }
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                        } else {
                            emit_inline_node(item_child, output);
                        }
                    }
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        node::TABLE => {
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    output.push('|');
                    for cell in &row.children {
                        if cell.kind.as_str() == node::TABLE_HEADER {
                            output.push('=');
                        }
                        emit_inline_nodes(&cell.children, output);
                        output.push('|');
                    }
                    output.push('\n');
                }
            }
            output.push('\n');
        }

        node::HORIZONTAL_RULE => {
            output.push_str("----\n\n");
        }

        node::DIV | node::SPAN | node::FIGURE => {
            emit_nodes(&node.children, output);
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
            output.push_str("//");
            emit_inline_nodes(&node.children, output);
            output.push_str("//");
        }

        node::UNDERLINE => {
            output.push_str("__");
            emit_inline_nodes(&node.children, output);
            output.push_str("__");
        }

        node::STRIKEOUT => {
            output.push_str("--");
            emit_inline_nodes(&node.children, output);
            output.push_str("--");
        }

        node::CODE => {
            output.push_str("##");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
            emit_inline_nodes(&node.children, output);
            output.push_str("##");
        }

        node::LINK => {
            output.push_str("[[");
            emit_inline_nodes(&node.children, output);
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(">>");
                output.push_str(url);
            }
            output.push_str("]]");
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str("[[image:");
                output.push_str(url);
                output.push_str("]]");
            }
        }

        node::LINE_BREAK => output.push('\n'),
        node::SOFT_BREAK => output.push(' '),

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
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(1, |h| h.text("Title")));
        assert!(emit_str(&doc).contains("= Title ="));
    }

    #[test]
    fn test_emit_bold() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        assert!(emit_str(&doc).contains("**bold**"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        assert!(emit_str(&doc).contains("//italic//"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("http://example.com", |l| l.text("Example"))));
        assert!(emit_str(&doc).contains("[[Example>>http://example.com]]"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("* one"));
        assert!(output.contains("* two"));
    }
}
