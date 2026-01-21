//! ANSI terminal writer for rescribe.
//!
//! Emits documents with ANSI escape codes for terminal display.
//! Supports bold, italic, underline, colors, and basic structure.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

// ANSI escape codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const UNDERLINE: &str = "\x1b[4m";
const STRIKETHROUGH: &str = "\x1b[9m";

// Colors
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";

// Background colors
const BG_BLACK: &str = "\x1b[40m";

/// Emit a document as ANSI-formatted text.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as ANSI-formatted text with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    emit_nodes(&doc.content.children, &mut ctx);

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
    in_code_block: bool,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            list_depth: 0,
            in_code_block: false,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_indent(&mut self) {
        for _ in 0..self.list_depth {
            self.write("  ");
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
            ctx.write("\n\n");
        }

        node::HEADING => emit_heading(node, ctx),
        node::CODE_BLOCK => emit_code_block(node, ctx),
        node::BLOCKQUOTE => emit_blockquote(node, ctx),
        node::LIST => emit_list(node, ctx),
        node::LIST_ITEM => emit_list_item(node, ctx),
        node::TABLE => emit_table(node, ctx),
        node::FIGURE => emit_nodes(&node.children, ctx),
        node::HORIZONTAL_RULE => {
            ctx.write(DIM);
            ctx.write("────────────────────────────────────────");
            ctx.write(RESET);
            ctx.write("\n\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK | node::RAW_INLINE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
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
            ctx.write(ITALIC);
            emit_nodes(&node.children, ctx);
            ctx.write(RESET);
        }

        node::STRONG => {
            ctx.write(BOLD);
            emit_nodes(&node.children, ctx);
            ctx.write(RESET);
        }

        node::STRIKEOUT => {
            ctx.write(STRIKETHROUGH);
            emit_nodes(&node.children, ctx);
            ctx.write(RESET);
        }

        node::UNDERLINE => {
            ctx.write(UNDERLINE);
            emit_nodes(&node.children, ctx);
            ctx.write(RESET);
        }

        node::SUBSCRIPT => {
            // Terminals don't support subscript, use dimmed
            ctx.write(DIM);
            emit_nodes(&node.children, ctx);
            ctx.write(RESET);
        }

        node::SUPERSCRIPT => {
            // Terminals don't support superscript, use dimmed
            ctx.write(DIM);
            emit_nodes(&node.children, ctx);
            ctx.write(RESET);
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                if !ctx.in_code_block {
                    ctx.write(BG_BLACK);
                    ctx.write(CYAN);
                }
                ctx.write(content);
                if !ctx.in_code_block {
                    ctx.write(RESET);
                }
            }
        }

        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("\n"),
        node::SOFT_BREAK => ctx.write(" "),

        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),

        node::SMALL_CAPS => {
            // Convert to uppercase as approximation
            let mut upper = String::new();
            for child in &node.children {
                if let Some(content) = child.props.get_str(prop::CONTENT) {
                    upper.push_str(&content.to_uppercase());
                }
            }
            ctx.write(&upper);
        }

        node::QUOTED => {
            let quote_type = node.props.get_str(prop::QUOTE_TYPE).unwrap_or("double");
            if quote_type == "single" {
                ctx.write("'");
                emit_nodes(&node.children, ctx);
                ctx.write("'");
            } else {
                ctx.write("\u{201C}"); // "
                emit_nodes(&node.children, ctx);
                ctx.write("\u{201D}"); // "
            }
        }

        "math_inline" | "math_display" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write(MAGENTA);
                ctx.write(source);
                ctx.write(RESET);
            }
        }

        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type for ANSI: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a heading with color and style.
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1);

    // Different colors for different levels
    let color = match level {
        1 => BLUE,
        2 => GREEN,
        3 => YELLOW,
        4 => CYAN,
        _ => MAGENTA,
    };

    ctx.write(BOLD);
    ctx.write(color);

    // Add level indicator
    for _ in 0..level {
        ctx.write("#");
    }
    ctx.write(" ");

    emit_nodes(&node.children, ctx);
    ctx.write(RESET);
    ctx.write("\n\n");
}

/// Emit a code block with syntax highlighting approximation.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE);

    // Header with language
    ctx.write(DIM);
    ctx.write("┌─");
    if let Some(lang) = lang {
        ctx.write(" ");
        ctx.write(lang);
        ctx.write(" ");
    }
    ctx.write("─\n");
    ctx.write(RESET);

    ctx.write(BG_BLACK);
    ctx.write(CYAN);
    ctx.in_code_block = true;

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        for line in content.lines() {
            ctx.write(DIM);
            ctx.write("│ ");
            ctx.write(RESET);
            ctx.write(BG_BLACK);
            ctx.write(CYAN);
            ctx.write(line);
            ctx.write(RESET);
            ctx.write("\n");
        }
    }

    ctx.in_code_block = false;
    ctx.write(RESET);
    ctx.write(DIM);
    ctx.write("└─\n");
    ctx.write(RESET);
    ctx.write("\n");
}

/// Emit a blockquote with left border.
fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    let mut inner = EmitContext::new();
    emit_nodes(&node.children, &mut inner);

    for line in inner.output.lines() {
        ctx.write(DIM);
        ctx.write("│ ");
        ctx.write(RESET);
        ctx.write(ITALIC);
        ctx.write(line);
        ctx.write(RESET);
        ctx.write("\n");
    }
    ctx.write("\n");
    ctx.warnings.extend(inner.warnings);
}

/// Emit a list.
fn emit_list(node: &Node, ctx: &mut EmitContext) {
    ctx.list_depth += 1;
    emit_nodes(&node.children, ctx);
    ctx.list_depth -= 1;
    if ctx.list_depth == 0 {
        ctx.write("\n");
    }
}

/// Emit a list item with bullet or number.
fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

    ctx.write_indent();

    if ordered {
        ctx.write(YELLOW);
        ctx.write("• ");
        ctx.write(RESET);
    } else {
        ctx.write(GREEN);
        ctx.write("• ");
        ctx.write(RESET);
    }

    // Emit children
    for child in &node.children {
        if child.kind.as_str() == node::PARAGRAPH {
            emit_nodes(&child.children, ctx);
            ctx.write("\n");
        } else {
            emit_node(child, ctx);
        }
    }
}

/// Emit a table.
fn emit_table(node: &Node, ctx: &mut EmitContext) {
    let rows = collect_table_rows(node);
    if rows.is_empty() {
        return;
    }

    let col_widths = calculate_column_widths(&rows);

    // Top border
    emit_table_border(&col_widths, ctx, '┌', '┬', '┐');

    // Emit rows
    let mut is_header = true;
    for row in &rows {
        ctx.write("│");
        for (i, cell) in row.iter().enumerate() {
            let width = col_widths.get(i).copied().unwrap_or(1);
            if is_header {
                ctx.write(BOLD);
            }
            ctx.write(" ");
            ctx.write(cell);
            for _ in cell.len()..width {
                ctx.write(" ");
            }
            if is_header {
                ctx.write(RESET);
            }
            ctx.write(" │");
        }
        ctx.write("\n");

        // Header separator
        if is_header && rows.len() > 1 {
            emit_table_border(&col_widths, ctx, '├', '┼', '┤');
            is_header = false;
        }
    }

    // Bottom border
    emit_table_border(&col_widths, ctx, '└', '┴', '┘');
    ctx.write("\n");
}

fn emit_table_border(widths: &[usize], ctx: &mut EmitContext, left: char, mid: char, right: char) {
    ctx.write(DIM);
    ctx.write(&left.to_string());
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..(*w + 2) {
            ctx.write("─");
        }
        if i < widths.len() - 1 {
            ctx.write(&mid.to_string());
        }
    }
    ctx.write(&right.to_string());
    ctx.write(RESET);
    ctx.write("\n");
}

fn collect_table_rows(node: &Node) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    collect_table_rows_inner(&node.children, &mut rows);
    rows
}

fn collect_table_rows_inner(nodes: &[Node], rows: &mut Vec<Vec<String>>) {
    for node in nodes {
        match node.kind.as_str() {
            node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
                collect_table_rows_inner(&node.children, rows);
            }
            node::TABLE_ROW => {
                let mut row = Vec::new();
                for cell in &node.children {
                    let mut text = String::new();
                    collect_text(&cell.children, &mut text);
                    row.push(text);
                }
                rows.push(row);
            }
            _ => {}
        }
    }
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            out.push_str(content);
        }
        collect_text(&node.children, out);
    }
}

fn calculate_column_widths(rows: &[Vec<String>]) -> Vec<usize> {
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut widths = vec![1; num_cols];

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }
    widths
}

/// Emit a definition list.
fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.write("\n");
}

/// Emit a definition term.
fn emit_definition_term(node: &Node, ctx: &mut EmitContext) {
    ctx.write(BOLD);
    emit_nodes(&node.children, ctx);
    ctx.write(RESET);
    ctx.write("\n");
}

/// Emit a definition description.
fn emit_definition_desc(node: &Node, ctx: &mut EmitContext) {
    ctx.write("  ");
    emit_nodes(&node.children, ctx);
    ctx.write("\n");
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write(UNDERLINE);
        ctx.write(BLUE);
        emit_nodes(&node.children, ctx);
        ctx.write(RESET);
        ctx.write(DIM);
        ctx.write(" (");
        ctx.write(url);
        ctx.write(")");
        ctx.write(RESET);
    } else {
        emit_nodes(&node.children, ctx);
    }
}

/// Emit an image placeholder.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    ctx.write(DIM);
    ctx.write("[Image");
    if let Some(alt) = node.props.get_str(prop::ALT) {
        ctx.write(": ");
        ctx.write(alt);
    }
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write(" (");
        ctx.write(url);
        ctx.write(")");
    }
    ctx.write("]");
    ctx.write(RESET);
}

/// Emit a footnote reference.
fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write(CYAN);
        ctx.write("[");
        ctx.write(label);
        ctx.write("]");
        ctx.write(RESET);
    }
}

/// Emit a footnote definition.
fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write(CYAN);
        ctx.write("[");
        ctx.write(label);
        ctx.write("] ");
        ctx.write(RESET);
        emit_nodes(&node.children, ctx);
        ctx.write("\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::*;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = doc(|d| d.para(|p| p.text("Hello, world!")));
        let output = emit_str(&doc);
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(1, |h| h.text("Title")));
        let output = emit_str(&doc);
        assert!(output.contains("# Title"));
        assert!(output.contains(BOLD));
    }

    #[test]
    fn test_emit_bold() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("bold"));
        assert!(output.contains(BOLD));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("italic"));
        assert!(output.contains(ITALIC));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("code")));
        let output = emit_str(&doc);
        assert!(output.contains("code"));
        assert!(output.contains(CYAN));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block_lang("fn main() {}", "rust"));
        let output = emit_str(&doc);
        assert!(output.contains("rust"));
        assert!(output.contains("fn main() {}"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("https://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("click"));
        assert!(output.contains("https://example.com"));
        assert!(output.contains(UNDERLINE));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("one"));
        assert!(output.contains("two"));
        assert!(output.contains("•"));
    }

    #[test]
    fn test_emit_horizontal_rule() {
        let doc = doc(|d| d.hr());
        let output = emit_str(&doc);
        assert!(output.contains("───"));
    }
}
