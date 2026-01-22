//! TSV writer for rescribe.
//!
//! Serializes rescribe's document IR tables to TSV format.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to TSV.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to TSV with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();

    // Find first table in document
    if let Some(table) = find_table(&doc.content) {
        emit_table(table, &mut output);
    }

    Ok(ConversionResult::ok(output.into_bytes()))
}

fn find_table(node: &Node) -> Option<&Node> {
    if node.kind.as_str() == node::TABLE {
        return Some(node);
    }
    for child in &node.children {
        if let Some(table) = find_table(child) {
            return Some(table);
        }
    }
    None
}

fn emit_table(table: &Node, output: &mut String) {
    for row in &table.children {
        if row.kind.as_str() == node::TABLE_ROW {
            let cells: Vec<String> = row
                .children
                .iter()
                .map(|cell| {
                    let text = get_text_content(cell);
                    escape_tsv_field(&text)
                })
                .collect();
            output.push_str(&cells.join("\t"));
            output.push('\n');
        }
    }
}

fn get_text_content(node: &Node) -> String {
    let mut text = String::new();
    collect_text(node, &mut text);
    text
}

fn collect_text(node: &Node, output: &mut String) {
    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        output.push_str(content);
    }
    for child in &node.children {
        collect_text(child, output);
    }
}

fn escape_tsv_field(field: &str) -> String {
    // TSV escaping: if field contains tab, newline, or quote, wrap in quotes
    if field.contains('\t') || field.contains('"') || field.contains('\n') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_simple_table() {
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
        assert!(output.contains("A\tB"));
        assert!(output.contains("1\t2"));
    }

    #[test]
    fn test_escape_tsv() {
        assert_eq!(escape_tsv_field("hello"), "hello");
        assert_eq!(escape_tsv_field("hello\tworld"), "\"hello\tworld\"");
        assert_eq!(escape_tsv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
