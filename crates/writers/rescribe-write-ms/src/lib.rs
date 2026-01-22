//! Groff ms macro writer for rescribe.
//!
//! Generates groff ms macro output from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to groff ms format.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to groff ms format with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();

    // Title if present
    if let Some(title) = doc.metadata.get_str("title") {
        output.push_str(".TL\n");
        output.push_str(&escape_ms(title));
        output.push('\n');
    }

    if let Some(author) = doc.metadata.get_str("author") {
        output.push_str(".AU\n");
        output.push_str(&escape_ms(author));
        output.push('\n');
    }

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
            match level {
                1 => output.push_str(".SH\n"),
                2 => output.push_str(".SS\n"),
                _ => output.push_str(".SS\n"),
            }
            emit_inline_nodes(&node.children, output);
            output.push('\n');
        }

        node::PARAGRAPH => {
            output.push_str(".PP\n");
            emit_inline_nodes(&node.children, output);
            output.push('\n');
        }

        node::CODE_BLOCK => {
            output.push_str(".DS\n");
            output.push_str(".ft CW\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                // Escape and output code
                for line in content.lines() {
                    output.push_str(&escape_ms(line));
                    output.push('\n');
                }
            }
            output.push_str(".ft\n");
            output.push_str(".DE\n");
        }

        node::BLOCKQUOTE => {
            output.push_str(".QS\n");
            emit_nodes(&node.children, output);
            output.push_str(".QE\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let mut item_num = 1;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    if ordered {
                        output.push_str(&format!(".IP {}.\n", item_num));
                        item_num += 1;
                    } else {
                        output.push_str(".IP \\(bu\n");
                    }

                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                            output.push('\n');
                        } else {
                            emit_node(item_child, output);
                        }
                    }
                }
            }
        }

        node::TABLE => {
            output.push_str(".TS\n");
            output.push_str("allbox;\n");

            // Determine column count from first row
            let col_count = node
                .children
                .first()
                .map(|row| row.children.len())
                .unwrap_or(0);
            if col_count > 0 {
                output.push_str("l ".repeat(col_count).trim_end());
                output.push_str(".\n");
            }

            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    let cells: Vec<String> = row
                        .children
                        .iter()
                        .map(|cell| {
                            let mut cell_content = String::new();
                            emit_inline_nodes(&cell.children, &mut cell_content);
                            cell_content.trim().to_string()
                        })
                        .collect();
                    output.push_str(&cells.join("\t"));
                    output.push('\n');
                }
            }

            output.push_str(".TE\n");
        }

        node::HORIZONTAL_RULE => {
            output.push_str(".LP\n");
            output.push_str("\\l'\\n(.lu'\n");
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
                output.push_str(&escape_ms(content));
            }
        }

        node::STRONG => {
            output.push_str("\\fB");
            emit_inline_nodes(&node.children, output);
            output.push_str("\\fP");
        }

        node::EMPHASIS => {
            output.push_str("\\fI");
            emit_inline_nodes(&node.children, output);
            output.push_str("\\fP");
        }

        node::UNDERLINE => {
            // ms doesn't have underline; use italics as fallback
            output.push_str("\\fI");
            emit_inline_nodes(&node.children, output);
            output.push_str("\\fP");
        }

        node::CODE => {
            output.push_str("\\f(CW");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_ms(content));
            }
            emit_inline_nodes(&node.children, output);
            output.push_str("\\fP");
        }

        node::LINK => {
            emit_inline_nodes(&node.children, output);
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!(" <{}>", escape_ms(url)));
            }
        }

        node::SUBSCRIPT => {
            output.push_str("\\d");
            emit_inline_nodes(&node.children, output);
            output.push_str("\\u");
        }

        node::SUPERSCRIPT => {
            output.push_str("\\u");
            emit_inline_nodes(&node.children, output);
            output.push_str("\\d");
        }

        node::LINE_BREAK => output.push_str("\n.br\n"),
        node::SOFT_BREAK => output.push(' '),

        _ => emit_inline_nodes(&node.children, output),
    }
}

fn escape_ms(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\e"),
            '.' if result.is_empty() || result.ends_with('\n') => result.push_str("\\&."),
            '\'' if result.is_empty() || result.ends_with('\n') => result.push_str("\\&'"),
            _ => result.push(c),
        }
    }
    result
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
        assert!(output.contains(".SH"));
        assert!(output.contains("Title"));
        assert!(output.contains(".PP"));
    }

    #[test]
    fn test_emit_formatting() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\fBbold\\fP"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item"))));
        let output = emit_str(&doc);
        assert!(output.contains(".IP \\(bu"));
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape_ms("\\test"), "\\etest");
        assert_eq!(escape_ms(".test"), "\\&.test");
    }
}
