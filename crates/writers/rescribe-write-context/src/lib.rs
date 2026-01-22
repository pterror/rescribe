//! ConTeXt writer for rescribe.
//!
//! Generates ConTeXt markup from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to ConTeXt.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to ConTeXt with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();

    // Preamble
    output.push_str("\\starttext\n\n");

    // Title if present
    if let Some(title) = doc.metadata.get_str("title") {
        output.push_str("\\startalignment[center]\n");
        output.push_str(&format!("{{\\tfd {}}}\n", escape_context(title)));
        output.push_str("\\stopalignment\n\\blank[big]\n\n");
    }

    emit_nodes(&doc.content.children, &mut output);

    output.push_str("\n\\stoptext\n");

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
            let cmd = match level {
                1 => "chapter",
                2 => "section",
                3 => "subsection",
                4 => "subsubsection",
                _ => "subsubsubsection",
            };
            output.push_str(&format!("\\{}", cmd));
            output.push('{');
            emit_inline_nodes(&node.children, output);
            output.push_str("}\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, output);
            output.push_str("\n\n");
        }

        node::CODE_BLOCK => {
            output.push_str("\\starttyping\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.push_str("\\stoptyping\n\n");
        }

        node::BLOCKQUOTE => {
            output.push_str("\\startblockquote\n");
            emit_nodes(&node.children, output);
            output.push_str("\\stopblockquote\n\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            if ordered {
                output.push_str("\\startitemize[n]\n");
            } else {
                output.push_str("\\startitemize\n");
            }

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str("\\item ");
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                        } else {
                            emit_node(item_child, output);
                        }
                    }
                    output.push('\n');
                }
            }

            output.push_str("\\stopitemize\n\n");
        }

        node::TABLE => {
            output.push_str("\\starttabulate\n");
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    output.push_str("\\NC ");
                    let cells: Vec<String> = row
                        .children
                        .iter()
                        .map(|cell| {
                            let mut cell_content = String::new();
                            emit_inline_nodes(&cell.children, &mut cell_content);
                            cell_content
                        })
                        .collect();
                    output.push_str(&cells.join(" \\NC "));
                    output.push_str(" \\NC\\NR\n");
                }
            }
            output.push_str("\\stoptabulate\n\n");
        }

        node::HORIZONTAL_RULE => {
            output.push_str("\\hairline\n\\blank\n\n");
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
                output.push_str(&escape_context(content));
            }
        }

        node::STRONG => {
            output.push_str("{\\bf ");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::EMPHASIS => {
            output.push_str("{\\em ");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::UNDERLINE => {
            output.push_str("\\underbar{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::STRIKEOUT => {
            output.push_str("\\overstrike{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::CODE => {
            output.push_str("\\type{");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str("\\goto{");
                emit_inline_nodes(&node.children, output);
                output.push_str(&format!("}}[url({})]", url));
            } else {
                emit_inline_nodes(&node.children, output);
            }
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!("\\externalfigure[{}]", url));
            }
        }

        node::SUBSCRIPT => {
            output.push_str("\\low{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::SUPERSCRIPT => {
            output.push_str("\\high{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }

        node::LINE_BREAK => output.push_str("\\crlf\n"),
        node::SOFT_BREAK => output.push(' '),

        _ => emit_inline_nodes(&node.children, output),
    }
}

fn escape_context(s: &str) -> String {
    s.replace('\\', "\\letterbackslash{}")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('$', "\\$")
        .replace('&', "\\&")
        .replace('#', "\\#")
        .replace('%', "\\%")
        .replace('~', "\\lettertilde{}")
        .replace('^', "\\letterhat{}")
        .replace('_', "\\_")
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
        assert!(output.contains("\\starttext"));
        assert!(output.contains("\\chapter{Title}"));
        assert!(output.contains("Hello world"));
        assert!(output.contains("\\stoptext"));
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
        assert!(output.contains("{\\bf bold}"));
        assert!(output.contains("{\\em italic}"));
    }

    #[test]
    fn test_emit_list() {
        let doc =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\startitemize"));
        assert!(output.contains("\\item"));
        assert!(output.contains("\\stopitemize"));
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape_context("$100"), "\\$100");
        assert_eq!(escape_context("50%"), "50\\%");
    }
}
