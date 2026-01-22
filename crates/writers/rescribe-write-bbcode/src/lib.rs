//! BBCode writer for rescribe.
//!
//! Serializes rescribe's document IR to BBCode forum markup.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to BBCode markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to BBCode markup with options.
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
            // BBCode doesn't have native headings - use size and bold
            let size = match level {
                1 => "6",
                2 => "5",
                3 => "4",
                _ => "3",
            };
            output.push_str(&format!("[size={}][b]", size));
            emit_inline_nodes(&node.children, output);
            output.push_str("[/b][/size]\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, output);
            output.push_str("\n\n");
        }

        node::CODE_BLOCK => {
            output.push_str("[code]\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.push_str("[/code]\n\n");
        }

        node::BLOCKQUOTE => {
            output.push_str("[quote]\n");
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, output);
                    output.push('\n');
                } else {
                    emit_node(child, output);
                }
            }
            output.push_str("[/quote]\n\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            if ordered {
                output.push_str("[list=1]\n");
            } else {
                output.push_str("[list]\n");
            }

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str("[*]");
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

            output.push_str("[/list]\n\n");
        }

        node::TABLE => {
            output.push_str("[table]\n");
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    output.push_str("[tr]");
                    for cell in &row.children {
                        let tag = if cell.kind.as_str() == node::TABLE_HEADER {
                            "th"
                        } else {
                            "td"
                        };
                        output.push_str(&format!("[{}]", tag));
                        emit_inline_nodes(&cell.children, output);
                        output.push_str(&format!("[/{}]", tag));
                    }
                    output.push_str("[/tr]\n");
                }
            }
            output.push_str("[/table]\n\n");
        }

        node::HORIZONTAL_RULE => {
            output.push_str("[hr]\n\n");
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
            output.push_str("[b]");
            emit_inline_nodes(&node.children, output);
            output.push_str("[/b]");
        }

        node::EMPHASIS => {
            output.push_str("[i]");
            emit_inline_nodes(&node.children, output);
            output.push_str("[/i]");
        }

        node::UNDERLINE => {
            output.push_str("[u]");
            emit_inline_nodes(&node.children, output);
            output.push_str("[/u]");
        }

        node::STRIKEOUT => {
            output.push_str("[s]");
            emit_inline_nodes(&node.children, output);
            output.push_str("[/s]");
        }

        node::CODE => {
            output.push_str("[code]");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
            emit_inline_nodes(&node.children, output);
            output.push_str("[/code]");
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!("[url={}]", url));
                emit_inline_nodes(&node.children, output);
                output.push_str("[/url]");
            } else {
                emit_inline_nodes(&node.children, output);
            }
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str("[img]");
                output.push_str(url);
                output.push_str("[/img]");
            }
        }

        node::SUBSCRIPT => {
            output.push_str("[sub]");
            emit_inline_nodes(&node.children, output);
            output.push_str("[/sub]");
        }

        node::SUPERSCRIPT => {
            output.push_str("[sup]");
            emit_inline_nodes(&node.children, output);
            output.push_str("[/sup]");
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
    fn test_emit_bold() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        assert!(emit_str(&doc).contains("[b]bold[/b]"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        assert!(emit_str(&doc).contains("[i]italic[/i]"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("http://example.com", |l| l.text("Example"))));
        assert!(emit_str(&doc).contains("[url=http://example.com]Example[/url]"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("[list]"));
        assert!(output.contains("[*]one"));
        assert!(output.contains("[/list]"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block("print('hello')"));
        let output = emit_str(&doc);
        assert!(output.contains("[code]"));
        assert!(output.contains("print('hello')"));
        assert!(output.contains("[/code]"));
    }
}
