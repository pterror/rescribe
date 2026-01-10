//! Plain text writer for rescribe.
//!
//! Emits documents as plain text, stripping all formatting but preserving
//! structure through whitespace.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as plain text.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as plain text with custom options.
pub fn emit_with_options(
    doc: &Document,
    options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new(options);

    emit_nodes(&doc.content.children, &mut ctx);

    // Trim trailing whitespace and ensure single trailing newline
    let output = ctx.output.trim_end().to_string() + "\n";

    Ok(ConversionResult::ok(output.into_bytes()))
}

/// Configuration options for plain text emission.
struct EmitContext<'a> {
    output: String,
    #[allow(dead_code)]
    options: &'a EmitOptions,
    list_depth: usize,
    ordered_list_counters: Vec<usize>,
    in_table: bool,
    table_row: Vec<String>,
    table_rows: Vec<Vec<String>>,
}

impl<'a> EmitContext<'a> {
    fn new(options: &'a EmitOptions) -> Self {
        Self {
            output: String::new(),
            options,
            list_depth: 0,
            ordered_list_counters: Vec::new(),
            in_table: false,
            table_row: Vec::new(),
            table_rows: Vec::new(),
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_line(&mut self, s: &str) {
        self.write(s);
        self.write("\n");
    }

    fn ensure_blank_line(&mut self) {
        let trimmed = self.output.trim_end();
        let output_len = trimmed.len();
        self.output.truncate(output_len);
        self.output.push_str("\n\n");
    }

    fn ensure_newline(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
    }

    fn indent(&self) -> String {
        "  ".repeat(self.list_depth)
    }
}

/// Emit a sequence of nodes.
fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_node(node, ctx);
    }
}

/// Emit a single node.
fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::DOCUMENT => emit_nodes(&node.children, ctx),

        node::PARAGRAPH => {
            ctx.ensure_newline();
            let indent = ctx.indent();
            ctx.write(&indent);
            emit_nodes(&node.children, ctx);
            ctx.ensure_blank_line();
        }

        node::HEADING => {
            ctx.ensure_blank_line();
            emit_nodes(&node.children, ctx);
            ctx.ensure_blank_line();
        }

        node::CODE_BLOCK => {
            ctx.ensure_blank_line();
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                for line in content.lines() {
                    ctx.write("    ");
                    ctx.write_line(line);
                }
            }
            ctx.ensure_blank_line();
        }

        node::BLOCKQUOTE => {
            ctx.ensure_blank_line();
            // Capture blockquote content and prefix with >
            let mut inner_ctx = EmitContext::new(ctx.options);
            emit_nodes(&node.children, &mut inner_ctx);
            for line in inner_ctx.output.trim().lines() {
                ctx.write("> ");
                ctx.write_line(line);
            }
            ctx.ensure_blank_line();
        }

        node::LIST => {
            ctx.ensure_newline();
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let start = node.props.get_int(prop::START).unwrap_or(1) as usize;

            if ordered {
                ctx.ordered_list_counters.push(start);
            }

            ctx.list_depth += 1;
            emit_nodes(&node.children, ctx);
            ctx.list_depth -= 1;

            if ordered {
                ctx.ordered_list_counters.pop();
            }

            ctx.ensure_newline();
        }

        node::LIST_ITEM => {
            let indent = "  ".repeat(ctx.list_depth.saturating_sub(1));

            // Determine bullet/number
            let marker = if let Some(counter) = ctx.ordered_list_counters.last_mut() {
                let n = *counter;
                *counter += 1;
                format!("{}. ", n)
            } else {
                "- ".to_string()
            };

            ctx.write(&indent);
            ctx.write(&marker);

            // Emit children inline, handling nested lists specially
            let mut first = true;
            for child in &node.children {
                if child.kind.as_str() == node::LIST {
                    ctx.ensure_newline();
                    emit_node(child, ctx);
                } else if child.kind.as_str() == node::PARAGRAPH {
                    if first {
                        emit_nodes(&child.children, ctx);
                        ctx.ensure_newline();
                    } else {
                        let inner_indent = "  ".repeat(ctx.list_depth);
                        ctx.write(&inner_indent);
                        emit_nodes(&child.children, ctx);
                        ctx.ensure_newline();
                    }
                } else {
                    emit_node(child, ctx);
                }
                first = false;
            }
        }

        node::TABLE => {
            ctx.ensure_blank_line();
            ctx.in_table = true;
            ctx.table_rows.clear();
            emit_nodes(&node.children, ctx);

            // Render table
            let rows = std::mem::take(&mut ctx.table_rows);
            if !rows.is_empty() {
                // Find max width for each column
                let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                let mut col_widths = vec![0; num_cols];

                for row in &rows {
                    for (i, cell) in row.iter().enumerate() {
                        col_widths[i] = col_widths[i].max(cell.len());
                    }
                }

                // Render rows
                for row in &rows {
                    let mut line = String::new();
                    for (i, cell) in row.iter().enumerate() {
                        if i > 0 {
                            line.push_str(" | ");
                        }
                        line.push_str(cell);
                        let padding = col_widths[i].saturating_sub(cell.len());
                        line.push_str(&" ".repeat(padding));
                    }
                    ctx.write_line(&line);
                }
            }
            ctx.in_table = false;
            ctx.ensure_blank_line();
        }

        node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
            emit_nodes(&node.children, ctx);
        }

        node::TABLE_ROW => {
            ctx.table_row.clear();
            emit_nodes(&node.children, ctx);
            ctx.table_rows.push(ctx.table_row.clone());
        }

        node::TABLE_CELL | node::TABLE_HEADER => {
            let mut cell_ctx = EmitContext::new(ctx.options);
            emit_nodes(&node.children, &mut cell_ctx);
            ctx.table_row.push(cell_ctx.output.trim().to_string());
        }

        node::FIGURE => emit_nodes(&node.children, ctx),
        node::CAPTION => {
            ctx.write("Caption: ");
            emit_nodes(&node.children, ctx);
            ctx.ensure_newline();
        }

        node::HORIZONTAL_RULE => {
            ctx.ensure_blank_line();
            ctx.write_line("---");
            ctx.ensure_blank_line();
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK | node::RAW_INLINE => {
            // Skip raw content in plain text
        }

        node::DEFINITION_LIST => {
            ctx.ensure_blank_line();
            emit_nodes(&node.children, ctx);
            ctx.ensure_blank_line();
        }

        node::DEFINITION_TERM => {
            emit_nodes(&node.children, ctx);
            ctx.ensure_newline();
        }

        node::DEFINITION_DESC => {
            ctx.write("  ");
            emit_nodes(&node.children, ctx);
            ctx.ensure_newline();
        }

        // Inline elements
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }

        node::EMPHASIS
        | node::STRONG
        | node::STRIKEOUT
        | node::UNDERLINE
        | node::SUBSCRIPT
        | node::SUPERSCRIPT
        | node::SMALL_CAPS
        | node::QUOTED => {
            emit_nodes(&node.children, ctx);
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }

        node::LINK => {
            emit_nodes(&node.children, ctx);
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write(" (");
                ctx.write(url);
                ctx.write(")");
            }
        }

        node::IMAGE => {
            ctx.write("[Image");
            if let Some(alt) = node.props.get_str(prop::ALT) {
                ctx.write(": ");
                ctx.write(alt);
            }
            ctx.write("]");
        }

        node::LINE_BREAK => {
            ctx.write("\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        node::FOOTNOTE_REF => {
            if let Some(label) = node.props.get_str(prop::LABEL) {
                ctx.write("[");
                ctx.write(label);
                ctx.write("]");
            }
        }

        node::FOOTNOTE_DEF => {
            if let Some(label) = node.props.get_str(prop::LABEL) {
                ctx.ensure_blank_line();
                ctx.write("[");
                ctx.write(label);
                ctx.write("] ");
                emit_nodes(&node.children, ctx);
                ctx.ensure_newline();
            }
        }

        // Math - render source as-is
        "math_inline" | "math_display" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write(source);
            }
        }

        _ => {
            // Unknown node - try to emit children
            emit_nodes(&node.children, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::helpers;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::text(
                "Hello, world!",
            )])]));

        let text = emit_str(&doc);
        assert!(text.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = Document::new().with_content(helpers::document([helpers::heading(
            1,
            [helpers::text("Main Title")],
        )]));

        let text = emit_str(&doc);
        assert!(text.contains("Main Title"));
    }

    #[test]
    fn test_emit_strips_formatting() {
        let doc = Document::new().with_content(helpers::document([helpers::paragraph([
            helpers::text("Normal "),
            helpers::strong([helpers::text("bold")]),
            helpers::text(" and "),
            helpers::emphasis([helpers::text("italic")]),
        ])]));

        let text = emit_str(&doc);
        assert!(text.contains("Normal bold and italic"));
    }

    #[test]
    fn test_emit_link() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::link(
                "https://example.com",
                [helpers::text("click here")],
            )])]));

        let text = emit_str(&doc);
        assert!(text.contains("click here"));
        assert!(text.contains("(https://example.com)"));
    }

    #[test]
    fn test_emit_list() {
        let doc = Document::new().with_content(helpers::document([helpers::bullet_list([
            helpers::list_item([helpers::paragraph([helpers::text("item 1")])]),
            helpers::list_item([helpers::paragraph([helpers::text("item 2")])]),
        ])]));

        let text = emit_str(&doc);
        assert!(text.contains("- item 1"));
        assert!(text.contains("- item 2"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc = Document::new().with_content(helpers::document([helpers::ordered_list([
            helpers::list_item([helpers::paragraph([helpers::text("first")])]),
            helpers::list_item([helpers::paragraph([helpers::text("second")])]),
        ])]));

        let text = emit_str(&doc);
        assert!(text.contains("1. first"));
        assert!(text.contains("2. second"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = Document::new().with_content(helpers::document([helpers::code_block(
            "fn main() {}",
            None,
        )]));

        let text = emit_str(&doc);
        assert!(text.contains("    fn main() {}"));
    }

    #[test]
    fn test_emit_blockquote() {
        let doc = Document::new().with_content(helpers::document([helpers::blockquote([
            helpers::paragraph([helpers::text("A quote")]),
        ])]));

        let text = emit_str(&doc);
        assert!(text.contains("> A quote"));
    }

    #[test]
    fn test_emit_image() {
        let doc = Document::new().with_content(helpers::document([helpers::image(
            "test.png",
            "Test image",
        )]));

        let text = emit_str(&doc);
        assert!(text.contains("[Image: Test image]"));
    }
}
