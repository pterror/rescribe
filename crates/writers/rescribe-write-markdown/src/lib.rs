//! Markdown writer for rescribe.
//!
//! Emits rescribe's document IR as CommonMark-compatible Markdown.

pub mod builder;

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as Markdown.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Markdown with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Emit children of the root document node
    emit_nodes(&doc.content.children, &mut ctx);

    let output = ctx.output.trim_end().to_string() + "\n";
    Ok(ConversionResult::with_warnings(
        output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit context for tracking state during emission.
struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
    list_depth: usize,
    in_tight_list: bool,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            list_depth: 0,
            in_tight_list: false,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
    }

    fn blank_line(&mut self) {
        self.newline();
        if !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn list_indent(&self) -> String {
        "  ".repeat(self.list_depth.saturating_sub(1))
    }
}

/// Emit a sequence of nodes.
fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for (i, node) in nodes.iter().enumerate() {
        emit_node(node, ctx);

        // Add blank lines between block elements
        if i + 1 < nodes.len() && is_block_node(node) && is_block_node(&nodes[i + 1]) {
            ctx.blank_line();
        }
    }
}

/// Check if a node is a block-level element.
fn is_block_node(node: &Node) -> bool {
    matches!(
        node.kind.as_str(),
        node::PARAGRAPH
            | node::HEADING
            | node::CODE_BLOCK
            | node::BLOCKQUOTE
            | node::LIST
            | node::TABLE
            | node::HORIZONTAL_RULE
            | node::DIV
            | node::RAW_BLOCK
            | node::DEFINITION_LIST
    )
}

/// Emit a single node.
fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::PARAGRAPH => emit_paragraph(node, ctx),
        node::HEADING => emit_heading(node, ctx),
        node::CODE_BLOCK => emit_code_block(node, ctx),
        node::BLOCKQUOTE => emit_blockquote(node, ctx),
        node::LIST => emit_list(node, ctx),
        node::LIST_ITEM => emit_list_item(node, ctx),
        node::TABLE => emit_table(node, ctx),
        node::HORIZONTAL_RULE => emit_horizontal_rule(ctx),
        node::TEXT => emit_text(node, ctx),
        node::EMPHASIS => emit_emphasis(node, ctx),
        node::STRONG => emit_strong(node, ctx),
        node::STRIKEOUT => emit_strikeout(node, ctx),
        node::CODE => emit_inline_code(node, ctx),
        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => emit_line_break(ctx),
        node::SOFT_BREAK => emit_soft_break(ctx),
        node::RAW_BLOCK => emit_raw_block(node, ctx),
        node::RAW_INLINE => emit_raw_inline(node, ctx),
        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),
        node::DEFINITION_LIST => emit_definition_list(node, ctx),
        "math_inline" => emit_math_inline(node, ctx),
        "math_display" => emit_math_display(node, ctx),
        _ => {
            // Unknown node type - try to emit children
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

fn emit_paragraph(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.newline();
}

fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;
    let hashes = "#".repeat(level.min(6));
    ctx.write(&hashes);
    ctx.write(" ");
    emit_nodes(&node.children, ctx);
    ctx.newline();
}

fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE).unwrap_or("");
    let content = node.props.get_str(prop::CONTENT).unwrap_or("");

    ctx.write("```");
    ctx.write(lang);
    ctx.newline();
    ctx.write(content);
    if !content.ends_with('\n') {
        ctx.newline();
    }
    ctx.write("```");
    ctx.newline();
}

fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    // Emit children line by line, prefixing with >
    let mut inner_ctx = EmitContext::new();
    emit_nodes(&node.children, &mut inner_ctx);

    for line in inner_ctx.output.lines() {
        ctx.write("> ");
        ctx.write(line);
        ctx.newline();
    }
}

fn emit_list(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
    let start = node.props.get_int(prop::START).unwrap_or(1) as usize;
    let tight = node.props.get_bool(prop::TIGHT).unwrap_or(true);

    ctx.list_depth += 1;
    let old_tight = ctx.in_tight_list;
    ctx.in_tight_list = tight;

    for (i, child) in node.children.iter().enumerate() {
        let indent = ctx.list_indent();
        ctx.write(&indent);

        if ordered {
            ctx.write(&format!("{}. ", start + i));
        } else {
            ctx.write("- ");
        }

        // Emit list item content
        emit_list_item_content(child, ctx);

        if !tight && i + 1 < node.children.len() {
            ctx.newline();
        }
    }

    ctx.in_tight_list = old_tight;
    ctx.list_depth -= 1;
}

fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    emit_list_item_content(node, ctx);
}

fn emit_list_item_content(node: &Node, ctx: &mut EmitContext) {
    // For tight lists, emit inline content; for loose lists, emit blocks
    if ctx.in_tight_list && node.children.len() == 1 {
        // Tight list item - emit paragraph content inline
        let child = &node.children[0];
        if child.kind.as_str() == node::PARAGRAPH {
            emit_nodes(&child.children, ctx);
            ctx.newline();
            return;
        }
    }

    // Loose list or complex content
    let mut first = true;
    for child in &node.children {
        if !first {
            let indent = ctx.list_indent();
            ctx.write(&indent);
            ctx.write("  "); // Extra indent for continuation
        }
        emit_node(child, ctx);
        first = false;
    }
}

fn emit_table(node: &Node, ctx: &mut EmitContext) {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut col_widths: Vec<usize> = Vec::new();

    // First pass: collect all cell contents and calculate widths
    for child in &node.children {
        match child.kind.as_str() {
            node::TABLE_HEAD => {
                for row in &child.children {
                    let cells = collect_row_cells(row);
                    update_col_widths(&cells, &mut col_widths);
                    rows.push(cells);
                }
            }
            node::TABLE_ROW => {
                let cells = collect_row_cells(child);
                update_col_widths(&cells, &mut col_widths);
                rows.push(cells);
            }
            node::TABLE_BODY => {
                for row in &child.children {
                    let cells = collect_row_cells(row);
                    update_col_widths(&cells, &mut col_widths);
                    rows.push(cells);
                }
            }
            _ => {}
        }
    }

    // Emit header row (first row)
    if let Some(header) = rows.first() {
        emit_table_row(header, &col_widths, ctx);

        // Emit separator
        ctx.write("|");
        for width in &col_widths {
            ctx.write(&"-".repeat(*width + 2));
            ctx.write("|");
        }
        ctx.newline();

        // Emit remaining rows
        for row in rows.iter().skip(1) {
            emit_table_row(row, &col_widths, ctx);
        }
    }
}

fn collect_row_cells(row: &Node) -> Vec<String> {
    row.children
        .iter()
        .map(|cell| {
            let mut cell_ctx = EmitContext::new();
            emit_nodes(&cell.children, &mut cell_ctx);
            cell_ctx.output.trim().to_string()
        })
        .collect()
}

fn update_col_widths(cells: &[String], widths: &mut Vec<usize>) {
    for (i, cell) in cells.iter().enumerate() {
        let len = cell.len().max(3); // Minimum width of 3
        if i >= widths.len() {
            widths.push(len);
        } else {
            widths[i] = widths[i].max(len);
        }
    }
}

fn emit_table_row(cells: &[String], widths: &[usize], ctx: &mut EmitContext) {
    ctx.write("|");
    for (i, cell) in cells.iter().enumerate() {
        let width = widths.get(i).copied().unwrap_or(3);
        ctx.write(&format!(" {:width$} |", cell, width = width));
    }
    ctx.newline();
}

fn emit_horizontal_rule(ctx: &mut EmitContext) {
    ctx.write("---");
    ctx.newline();
}

fn emit_text(node: &Node, ctx: &mut EmitContext) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        // Escape special markdown characters in text
        let escaped = escape_markdown(content);
        ctx.write(&escaped);
    }
}

fn escape_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' => {
                result.push('\\');
                result.push(c);
            }
            // Only escape ! if followed by [ (image syntax)
            '!' if chars.get(i + 1) == Some(&'[') => {
                result.push('\\');
                result.push(c);
            }
            // Only escape # at start of line
            '#' if i == 0 || chars.get(i.wrapping_sub(1)) == Some(&'\n') => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

fn emit_emphasis(node: &Node, ctx: &mut EmitContext) {
    ctx.write("*");
    emit_nodes(&node.children, ctx);
    ctx.write("*");
}

fn emit_strong(node: &Node, ctx: &mut EmitContext) {
    ctx.write("**");
    emit_nodes(&node.children, ctx);
    ctx.write("**");
}

fn emit_strikeout(node: &Node, ctx: &mut EmitContext) {
    ctx.write("~~");
    emit_nodes(&node.children, ctx);
    ctx.write("~~");
}

fn emit_inline_code(node: &Node, ctx: &mut EmitContext) {
    let content = node.props.get_str(prop::CONTENT).unwrap_or("");

    // Handle backticks in code
    if content.contains('`') {
        ctx.write("`` ");
        ctx.write(content);
        ctx.write(" ``");
    } else {
        ctx.write("`");
        ctx.write(content);
        ctx.write("`");
    }
}

fn emit_link(node: &Node, ctx: &mut EmitContext) {
    let url = node.props.get_str(prop::URL).unwrap_or("");
    let title = node.props.get_str(prop::TITLE);

    ctx.write("[");
    emit_nodes(&node.children, ctx);
    ctx.write("](");
    ctx.write(url);
    if let Some(t) = title {
        ctx.write(" \"");
        ctx.write(t);
        ctx.write("\"");
    }
    ctx.write(")");
}

fn emit_image(node: &Node, ctx: &mut EmitContext) {
    let url = node.props.get_str(prop::URL).unwrap_or("");
    let alt = node.props.get_str(prop::ALT).unwrap_or("");
    let title = node.props.get_str(prop::TITLE);

    ctx.write("![");
    ctx.write(alt);
    ctx.write("](");
    ctx.write(url);
    if let Some(t) = title {
        ctx.write(" \"");
        ctx.write(t);
        ctx.write("\"");
    }
    ctx.write(")");
}

fn emit_line_break(ctx: &mut EmitContext) {
    ctx.write("  \n");
}

fn emit_soft_break(ctx: &mut EmitContext) {
    ctx.newline();
}

fn emit_raw_block(node: &Node, ctx: &mut EmitContext) {
    let content = node.props.get_str(prop::CONTENT).unwrap_or("");
    ctx.write(content);
    ctx.newline();
}

fn emit_raw_inline(node: &Node, ctx: &mut EmitContext) {
    let content = node.props.get_str(prop::CONTENT).unwrap_or("");
    ctx.write(content);
}

fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    let label = node.props.get_str(prop::LABEL).unwrap_or("?");
    ctx.write("[^");
    ctx.write(label);
    ctx.write("]");
}

fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    let label = node.props.get_str(prop::LABEL).unwrap_or("?");
    ctx.write("[^");
    ctx.write(label);
    ctx.write("]: ");
    emit_nodes(&node.children, ctx);
}

fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    for child in &node.children {
        emit_node(child, ctx);
    }
}

fn emit_math_inline(node: &Node, ctx: &mut EmitContext) {
    let source = node.props.get_str("math:source").unwrap_or("");
    ctx.write("$");
    ctx.write(source);
    ctx.write("$");
}

fn emit_math_display(node: &Node, ctx: &mut EmitContext) {
    let source = node.props.get_str("math:source").unwrap_or("");
    ctx.write("$$");
    ctx.newline();
    ctx.write(source);
    ctx.newline();
    ctx.write("$$");
    ctx.newline();
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

        let md = emit_str(&doc);
        assert_eq!(md, "Hello, world!\n");
    }

    #[test]
    fn test_emit_heading() {
        let doc = Document::new().with_content(helpers::document([helpers::heading(
            2,
            [helpers::text("Title")],
        )]));

        let md = emit_str(&doc);
        assert_eq!(md, "## Title\n");
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = Document::new().with_content(helpers::document([helpers::paragraph([
            helpers::emphasis([helpers::text("italic")]),
        ])]));

        let md = emit_str(&doc);
        assert_eq!(md, "*italic*\n");
    }

    #[test]
    fn test_emit_link() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::link(
                "https://example.com",
                [helpers::text("link")],
            )])]));

        let md = emit_str(&doc);
        assert_eq!(md, "[link](https://example.com)\n");
    }

    #[test]
    fn test_emit_code_block() {
        let doc = Document::new().with_content(helpers::document([helpers::code_block(
            "fn main() {}",
            Some("rust"),
        )]));

        let md = emit_str(&doc);
        assert_eq!(md, "```rust\nfn main() {}\n```\n");
    }

    #[test]
    fn test_emit_list() {
        let doc = Document::new().with_content(helpers::document([helpers::bullet_list([
            helpers::list_item([helpers::paragraph([helpers::text("item 1")])]),
            helpers::list_item([helpers::paragraph([helpers::text("item 2")])]),
        ])]));

        let md = emit_str(&doc);
        assert!(md.contains("- item 1"));
        assert!(md.contains("- item 2"));
    }
}
