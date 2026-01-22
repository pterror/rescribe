//! TSV (Tab-Separated Values) reader for rescribe.
//!
//! Parses TSV data into rescribe's document IR as a table.

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};
use rescribe_std::{Node, node, prop};

/// Parse TSV input into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse TSV input into a document with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut table = Node::new(node::TABLE);
    let mut is_first_row = true;

    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut row = Node::new(node::TABLE_ROW);
        let fields = parse_tsv_line(line);

        for field in fields {
            let cell_kind = if is_first_row {
                node::TABLE_HEADER
            } else {
                node::TABLE_CELL
            };
            let cell =
                Node::new(cell_kind).child(Node::new(node::TEXT).prop(prop::CONTENT, field.trim()));
            row = row.child(cell);
        }

        table = table.child(row);
        is_first_row = false;
    }

    let doc = Document {
        content: Node::new(node::DOCUMENT).child(table),
        resources: Default::default(),
        metadata: Default::default(),
        source: None,
    };

    Ok(ConversionResult::ok(doc))
}

fn parse_tsv_line(line: &str) -> Vec<String> {
    // TSV is simpler than CSV - just split on tabs
    // Quoted fields can contain tabs and newlines
    let mut fields = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if !in_quotes => {
                in_quotes = true;
            }
            '"' if in_quotes => {
                // Check for escaped quote
                if chars.peek() == Some(&'"') {
                    current_field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '\t' if !in_quotes => {
                fields.push(current_field);
                current_field = String::new();
            }
            _ => {
                current_field.push(c);
            }
        }
    }

    fields.push(current_field);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_simple_tsv() {
        let input = "Name\tAge\tCity\nAlice\t30\tNew York\nBob\t25\tLondon";
        let doc = parse_str(input);
        let table = &doc.content.children[0];
        assert_eq!(table.kind.as_str(), node::TABLE);
        assert_eq!(table.children.len(), 3); // 3 rows
    }

    #[test]
    fn test_parse_quoted_fields() {
        let input = "Name\tDescription\n\"Item\"\t\"Has\ttabs\"";
        let doc = parse_str(input);
        let table = &doc.content.children[0];
        let data_row = &table.children[1];
        assert_eq!(data_row.children.len(), 2);
    }

    #[test]
    fn test_parse_tsv_line() {
        assert_eq!(parse_tsv_line("a\tb\tc"), vec!["a", "b", "c"]);
        assert_eq!(parse_tsv_line("\"a\tb\"\tc"), vec!["a\tb", "c"]);
    }
}
