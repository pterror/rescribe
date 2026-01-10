//! Org-mode writer for rescribe.
//!
//! Emits documents in Emacs Org-mode format.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as Org-mode.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Org-mode with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    emit_nodes(&doc.content.children, &mut ctx);

    // Ensure trailing newline
    if !ctx.output.ends_with('\n') {
        ctx.output.push('\n');
    }

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit context for tracking state during emission.
struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
    list_depth: usize,
    in_table: bool,
    table_rows: Vec<Vec<String>>,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            list_depth: 0,
            in_table: false,
            table_rows: Vec::new(),
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn ensure_blank_line(&mut self) {
        let trimmed = self.output.trim_end();
        let len = trimmed.len();
        self.output.truncate(len);
        self.output.push_str("\n\n");
    }

    fn ensure_newline(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
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
            emit_nodes(&node.children, ctx);
            ctx.ensure_blank_line();
        }

        node::HEADING => emit_heading(node, ctx),
        node::CODE_BLOCK => emit_code_block(node, ctx),
        node::BLOCKQUOTE => emit_blockquote(node, ctx),
        node::LIST => emit_list(node, ctx),
        node::LIST_ITEM => emit_list_item(node, ctx),
        node::TABLE => emit_table(node, ctx),
        node::FIGURE => emit_nodes(&node.children, ctx),
        node::CAPTION => {
            ctx.write("#+CAPTION: ");
            emit_nodes(&node.children, ctx);
            ctx.ensure_newline();
        }
        node::HORIZONTAL_RULE => {
            ctx.ensure_newline();
            ctx.write("-----\n\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK => {
            let format = node.props.get_str(prop::FORMAT).unwrap_or("");
            if format == "org"
                && let Some(content) = node.props.get_str(prop::CONTENT)
            {
                ctx.write(content);
            }
        }

        node::RAW_INLINE => {
            let format = node.props.get_str(prop::FORMAT).unwrap_or("");
            if format == "org"
                && let Some(content) = node.props.get_str(prop::CONTENT)
            {
                ctx.write(content);
            }
        }

        node::DEFINITION_LIST => emit_definition_list(node, ctx),
        node::DEFINITION_TERM => emit_definition_term(node, ctx),
        node::DEFINITION_DESC => emit_definition_desc(node, ctx),

        // Inline elements
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }

        node::EMPHASIS => {
            ctx.write("/");
            emit_nodes(&node.children, ctx);
            ctx.write("/");
        }

        node::STRONG => {
            ctx.write("*");
            emit_nodes(&node.children, ctx);
            ctx.write("*");
        }

        node::STRIKEOUT => {
            ctx.write("+");
            emit_nodes(&node.children, ctx);
            ctx.write("+");
        }

        node::UNDERLINE => {
            ctx.write("_");
            emit_nodes(&node.children, ctx);
            ctx.write("_");
        }

        node::SUBSCRIPT => {
            ctx.write("_{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::SUPERSCRIPT => {
            ctx.write("^{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write("=");
                ctx.write(content);
                ctx.write("=");
            }
        }

        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("\\\\\n"),
        node::SOFT_BREAK => ctx.write("\n"),

        node::FOOTNOTE_REF => {
            if let Some(label) = node.props.get_str(prop::LABEL) {
                ctx.write("[fn:");
                ctx.write(label);
                ctx.write("]");
            }
        }

        node::FOOTNOTE_DEF => {
            if let Some(label) = node.props.get_str(prop::LABEL) {
                ctx.write("[fn:");
                ctx.write(label);
                ctx.write("] ");
                emit_nodes(&node.children, ctx);
                ctx.ensure_newline();
            }
        }

        node::SMALL_CAPS => {
            // Org doesn't have native small caps, just emit text
            emit_nodes(&node.children, ctx);
        }

        node::QUOTED => {
            let quote_type = node.props.get_str(prop::QUOTE_TYPE).unwrap_or("double");
            if quote_type == "single" {
                ctx.write("'");
                emit_nodes(&node.children, ctx);
                ctx.write("'");
            } else {
                ctx.write("\"");
                emit_nodes(&node.children, ctx);
                ctx.write("\"");
            }
        }

        "math_inline" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write("$");
                ctx.write(source);
                ctx.write("$");
            }
        }

        "math_display" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write("\\[\n");
                ctx.write(source);
                ctx.write("\n\\]\n");
            }
        }

        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type for Org: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a heading.
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;

    ctx.ensure_newline();

    // Org uses * for heading levels
    for _ in 0..level {
        ctx.write("*");
    }
    ctx.write(" ");

    emit_nodes(&node.children, ctx);
    ctx.ensure_blank_line();
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_newline();

    if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
        ctx.write("#+BEGIN_SRC ");
        ctx.write(lang);
        ctx.write("\n");
    } else {
        ctx.write("#+BEGIN_SRC\n");
    }

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
        if !content.ends_with('\n') {
            ctx.write("\n");
        }
    }

    ctx.write("#+END_SRC\n\n");
}

/// Emit a blockquote.
fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_newline();
    ctx.write("#+BEGIN_QUOTE\n");
    emit_nodes(&node.children, ctx);
    ctx.write("#+END_QUOTE\n\n");
}

/// Emit a list.
fn emit_list(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

    ctx.list_depth += 1;

    let mut counter = 1;
    for child in &node.children {
        if child.kind.as_str() == node::LIST_ITEM {
            emit_list_item_with_marker(child, ordered, &mut counter, ctx);
        } else {
            emit_node(child, ctx);
        }
    }

    ctx.list_depth -= 1;

    if ctx.list_depth == 0 {
        ctx.ensure_newline();
    }
}

/// Emit a list item with the appropriate marker.
fn emit_list_item_with_marker(
    node: &Node,
    ordered: bool,
    counter: &mut i32,
    ctx: &mut EmitContext,
) {
    let indent = "  ".repeat(ctx.list_depth - 1);
    ctx.write(&indent);

    if ordered {
        ctx.write(&format!("{}. ", counter));
        *counter += 1;
    } else {
        ctx.write("- ");
    }

    // Emit children
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
                let content_indent = "  ".repeat(ctx.list_depth);
                ctx.write(&content_indent);
                emit_nodes(&child.children, ctx);
                ctx.ensure_newline();
            }
        } else {
            emit_node(child, ctx);
        }
        first = false;
    }
}

/// Emit a list item (fallback).
fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    ctx.write("- ");
    for child in &node.children {
        if child.kind.as_str() == node::PARAGRAPH {
            emit_nodes(&child.children, ctx);
            ctx.ensure_newline();
        } else {
            emit_node(child, ctx);
        }
    }
}

/// Emit a table.
fn emit_table(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_newline();
    ctx.in_table = true;
    ctx.table_rows.clear();

    collect_table_rows(&node.children, ctx);

    // Emit table
    if !ctx.table_rows.is_empty() {
        let rows = std::mem::take(&mut ctx.table_rows);

        // Calculate column widths
        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![0; num_cols];
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }

        // Emit rows
        let mut first_row = true;
        for row in &rows {
            ctx.write("|");
            for (i, cell) in row.iter().enumerate() {
                ctx.write(" ");
                ctx.write(cell);
                let padding = col_widths[i].saturating_sub(cell.len());
                ctx.write(&" ".repeat(padding));
                ctx.write(" |");
            }
            ctx.write("\n");

            // Add separator after first row (header)
            if first_row && rows.len() > 1 {
                ctx.write("|");
                for width in &col_widths {
                    ctx.write("-");
                    ctx.write(&"-".repeat(*width));
                    ctx.write("-+");
                }
                // Fix the last + to |
                if !col_widths.is_empty() {
                    ctx.output.pop();
                    ctx.write("|");
                }
                ctx.write("\n");
                first_row = false;
            }
        }
    }

    ctx.in_table = false;
    ctx.ensure_blank_line();
}

/// Collect table rows recursively.
fn collect_table_rows(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        match node.kind.as_str() {
            node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
                collect_table_rows(&node.children, ctx);
            }
            node::TABLE_ROW => {
                let mut cells = Vec::new();
                for cell in &node.children {
                    let mut cell_ctx = EmitContext::new();
                    emit_nodes(&cell.children, &mut cell_ctx);
                    cells.push(cell_ctx.output.trim().to_string());
                }
                ctx.table_rows.push(cells);
            }
            _ => {}
        }
    }
}

/// Emit a definition list.
fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.ensure_newline();
}

/// Emit a definition term.
fn emit_definition_term(node: &Node, ctx: &mut EmitContext) {
    ctx.write("- ");
    emit_nodes(&node.children, ctx);
    ctx.write(" :: ");
}

/// Emit a definition description.
fn emit_definition_desc(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.ensure_newline();
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("[[");
        ctx.write(url);
        ctx.write("][");
        emit_nodes(&node.children, ctx);
        ctx.write("]]");
    } else {
        emit_nodes(&node.children, ctx);
    }
}

/// Emit an image.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("[[");
        if !url.starts_with("file:") && !url.starts_with("http") {
            ctx.write("file:");
        }
        ctx.write(url);
        ctx.write("]]");
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

        let org = emit_str(&doc);
        assert!(org.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = Document::new().with_content(helpers::document([helpers::heading(
            1,
            [helpers::text("Main Title")],
        )]));

        let org = emit_str(&doc);
        assert!(org.contains("* Main Title"));
    }

    #[test]
    fn test_emit_heading_levels() {
        let doc = Document::new().with_content(helpers::document([
            helpers::heading(1, [helpers::text("Level 1")]),
            helpers::heading(2, [helpers::text("Level 2")]),
            helpers::heading(3, [helpers::text("Level 3")]),
        ]));

        let org = emit_str(&doc);
        assert!(org.contains("* Level 1"));
        assert!(org.contains("** Level 2"));
        assert!(org.contains("*** Level 3"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = Document::new().with_content(helpers::document([helpers::paragraph([
            helpers::emphasis([helpers::text("italic")]),
        ])]));

        let org = emit_str(&doc);
        assert!(org.contains("/italic/"));
    }

    #[test]
    fn test_emit_strong() {
        let doc = Document::new().with_content(helpers::document([helpers::paragraph([
            helpers::strong([helpers::text("bold")]),
        ])]));

        let org = emit_str(&doc);
        assert!(org.contains("*bold*"));
    }

    #[test]
    fn test_emit_link() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::link(
                "https://example.com",
                [helpers::text("click")],
            )])]));

        let org = emit_str(&doc);
        assert!(org.contains("[[https://example.com][click]]"));
    }

    #[test]
    fn test_emit_list() {
        let doc = Document::new().with_content(helpers::document([helpers::bullet_list([
            helpers::list_item([helpers::paragraph([helpers::text("item 1")])]),
            helpers::list_item([helpers::paragraph([helpers::text("item 2")])]),
        ])]));

        let org = emit_str(&doc);
        assert!(org.contains("- item 1"));
        assert!(org.contains("- item 2"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc = Document::new().with_content(helpers::document([helpers::ordered_list([
            helpers::list_item([helpers::paragraph([helpers::text("first")])]),
            helpers::list_item([helpers::paragraph([helpers::text("second")])]),
        ])]));

        let org = emit_str(&doc);
        assert!(org.contains("1. first"));
        assert!(org.contains("2. second"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = Document::new().with_content(helpers::document([helpers::code_block(
            "fn main() {}",
            Some("rust"),
        )]));

        let org = emit_str(&doc);
        assert!(org.contains("#+BEGIN_SRC rust"));
        assert!(org.contains("fn main() {}"));
        assert!(org.contains("#+END_SRC"));
    }

    #[test]
    fn test_emit_blockquote() {
        let doc = Document::new().with_content(helpers::document([helpers::blockquote([
            helpers::paragraph([helpers::text("A quote")]),
        ])]));

        let org = emit_str(&doc);
        assert!(org.contains("#+BEGIN_QUOTE"));
        assert!(org.contains("A quote"));
        assert!(org.contains("#+END_QUOTE"));
    }

    #[test]
    fn test_emit_image() {
        let doc = Document::new().with_content(helpers::document([helpers::image(
            "test.png",
            "Test image",
        )]));

        let org = emit_str(&doc);
        assert!(org.contains("[[file:test.png]]"));
    }

    #[test]
    fn test_emit_inline_code() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::code(
                "inline code",
            )])]));

        let org = emit_str(&doc);
        assert!(org.contains("=inline code="));
    }
}
