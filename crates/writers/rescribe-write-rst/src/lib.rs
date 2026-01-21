//! reStructuredText writer for rescribe.
//!
//! Emits documents as RST source.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as RST.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as RST with custom options.
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
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            list_depth: 0,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_indent(&mut self) {
        for _ in 0..self.list_depth {
            self.write("   ");
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
        node::FIGURE => emit_figure(node, ctx),
        node::HORIZONTAL_RULE => {
            ctx.write("----\n\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK | node::RAW_INLINE => {
            let format = node.props.get_str(prop::FORMAT).unwrap_or("");
            if format == "rst"
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
            ctx.write("*");
            emit_nodes(&node.children, ctx);
            ctx.write("*");
        }

        node::STRONG => {
            ctx.write("**");
            emit_nodes(&node.children, ctx);
            ctx.write("**");
        }

        node::STRIKEOUT => {
            // RST doesn't have native strikethrough, use role
            ctx.write(":strike:`");
            emit_nodes(&node.children, ctx);
            ctx.write("`");
        }

        node::UNDERLINE => {
            // RST doesn't have native underline, use role
            ctx.write(":underline:`");
            emit_nodes(&node.children, ctx);
            ctx.write("`");
        }

        node::SUBSCRIPT => {
            ctx.write(":sub:`");
            emit_nodes(&node.children, ctx);
            ctx.write("`");
        }

        node::SUPERSCRIPT => {
            ctx.write(":sup:`");
            emit_nodes(&node.children, ctx);
            ctx.write("`");
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write("``");
                ctx.write(content);
                ctx.write("``");
            }
        }

        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("\n"),
        node::SOFT_BREAK => ctx.write("\n"),

        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),

        node::SMALL_CAPS => {
            ctx.write(":sc:`");
            emit_nodes(&node.children, ctx);
            ctx.write("`");
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
                ctx.write(":math:`");
                ctx.write(source);
                ctx.write("`");
            }
        }

        "math_display" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write(".. math::\n\n   ");
                ctx.write(&source.replace('\n', "\n   "));
                ctx.write("\n\n");
            }
        }

        "admonition" => emit_admonition(node, ctx),

        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type for RST: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a heading with appropriate underline character.
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1);

    // Collect heading text
    let mut text = String::new();
    collect_text(&node.children, &mut text);

    let underline_char = match level {
        1 => '=',
        2 => '-',
        3 => '~',
        4 => '^',
        5 => '"',
        _ => '\'',
    };

    // For level 1, add overline
    if level == 1 {
        let line: String = std::iter::repeat_n(underline_char, text.len()).collect();
        ctx.write(&line);
        ctx.write("\n");
    }

    emit_nodes(&node.children, ctx);
    ctx.write("\n");

    let line: String = std::iter::repeat_n(underline_char, text.len()).collect();
    ctx.write(&line);
    ctx.write("\n\n");
}

/// Collect plain text from nodes for length calculation.
fn collect_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            out.push_str(content);
        }
        collect_text(&node.children, out);
    }
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE);

    if let Some(lang) = lang {
        ctx.write(".. code-block:: ");
        ctx.write(lang);
        ctx.write("\n\n");
    } else {
        ctx.write("::\n\n");
    }

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        for line in content.lines() {
            ctx.write("   ");
            ctx.write(line);
            ctx.write("\n");
        }
    }
    ctx.write("\n");
}

/// Emit a blockquote.
fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    // Blockquotes in RST are indented
    let mut inner = EmitContext::new();
    emit_nodes(&node.children, &mut inner);

    for line in inner.output.lines() {
        ctx.write("   ");
        ctx.write(line);
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
    ctx.write("\n");
}

/// Emit a list item.
fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    // Check if parent list is ordered
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

    if ordered {
        ctx.write("#. ");
    } else {
        ctx.write("- ");
    }

    // Emit children
    let mut first = true;
    for child in &node.children {
        if child.kind.as_str() == node::PARAGRAPH {
            if !first {
                ctx.write_indent();
                ctx.write("   ");
            }
            emit_nodes(&child.children, ctx);
            ctx.write("\n");
        } else if child.kind.as_str() == node::LIST {
            ctx.write("\n");
            ctx.write_indent();
            emit_node(child, ctx);
        } else {
            emit_node(child, ctx);
        }
        first = false;
    }
}

/// Emit a table using simple RST table format.
fn emit_table(node: &Node, ctx: &mut EmitContext) {
    // Collect all rows first to calculate column widths
    let rows = collect_table_rows(node);
    if rows.is_empty() {
        return;
    }

    let col_widths = calculate_column_widths(&rows);

    // Top border
    emit_table_border(&col_widths, ctx);

    // Emit rows
    let mut is_header = true;
    for row in &rows {
        ctx.write("|");
        for (i, cell) in row.iter().enumerate() {
            let width = col_widths.get(i).copied().unwrap_or(1);
            ctx.write(" ");
            ctx.write(cell);
            for _ in cell.len()..width {
                ctx.write(" ");
            }
            ctx.write(" |");
        }
        ctx.write("\n");

        // Header separator
        if is_header && rows.len() > 1 {
            emit_table_border(&col_widths, ctx);
            is_header = false;
        }
    }

    // Bottom border
    emit_table_border(&col_widths, ctx);
    ctx.write("\n");
}

fn emit_table_border(widths: &[usize], ctx: &mut EmitContext) {
    ctx.write("+");
    for w in widths {
        for _ in 0..(*w + 2) {
            ctx.write("-");
        }
        ctx.write("+");
    }
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

/// Emit a figure.
fn emit_figure(node: &Node, ctx: &mut EmitContext) {
    // Look for image child
    for child in &node.children {
        if child.kind.as_str() == node::IMAGE {
            if let Some(url) = child.props.get_str(prop::URL) {
                ctx.write(".. figure:: ");
                ctx.write(url);
                ctx.write("\n");

                if let Some(alt) = child.props.get_str(prop::ALT) {
                    ctx.write("   :alt: ");
                    ctx.write(alt);
                    ctx.write("\n");
                }
            }
        } else if child.kind.as_str() == node::CAPTION {
            ctx.write("\n   ");
            emit_nodes(&child.children, ctx);
            ctx.write("\n");
        }
    }
    ctx.write("\n");
}

/// Emit a definition list.
fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
}

/// Emit a definition term.
fn emit_definition_term(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.write("\n");
}

/// Emit a definition description.
fn emit_definition_desc(node: &Node, ctx: &mut EmitContext) {
    ctx.write("   ");
    emit_nodes(&node.children, ctx);
    ctx.write("\n\n");
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("`");
        emit_nodes(&node.children, ctx);
        ctx.write(" <");
        ctx.write(url);
        ctx.write(">`_");
    } else {
        emit_nodes(&node.children, ctx);
    }
}

/// Emit an image.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write(".. image:: ");
        ctx.write(url);
        ctx.write("\n");

        if let Some(alt) = node.props.get_str(prop::ALT) {
            ctx.write("   :alt: ");
            ctx.write(alt);
            ctx.write("\n");
        }
        ctx.write("\n");
    }
}

/// Emit a footnote reference.
fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write("[");
        ctx.write(label);
        ctx.write("]_");
    }
}

/// Emit a footnote definition.
fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write(".. [");
        ctx.write(label);
        ctx.write("] ");
        emit_nodes(&node.children, ctx);
        ctx.write("\n");
    }
}

/// Emit an admonition.
fn emit_admonition(node: &Node, ctx: &mut EmitContext) {
    let adm_type = node
        .props
        .get_str("admonition_type")
        .unwrap_or("note")
        .to_lowercase();

    ctx.write(".. ");
    ctx.write(&adm_type);
    ctx.write("::\n\n");

    let mut inner = EmitContext::new();
    emit_nodes(&node.children, &mut inner);

    for line in inner.output.lines() {
        ctx.write("   ");
        ctx.write(line);
        ctx.write("\n");
    }
    ctx.write("\n");
    ctx.warnings.extend(inner.warnings);
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
        assert!(output.contains("====="));
        assert!(output.contains("Title"));
    }

    #[test]
    fn test_emit_heading_level2() {
        let doc = doc(|d| d.heading(2, |h| h.text("Subtitle")));
        let output = emit_str(&doc);
        assert!(output.contains("--------"));
        assert!(output.contains("Subtitle"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("*italic*"));
    }

    #[test]
    fn test_emit_strong() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("**bold**"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("code")));
        let output = emit_str(&doc);
        assert!(output.contains("``code``"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("https://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("`click <https://example.com>`_"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block_lang("print('hi')", "python"));
        let output = emit_str(&doc);
        assert!(output.contains(".. code-block:: python"));
        assert!(output.contains("   print('hi')"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("- one"));
        assert!(output.contains("- two"));
    }
}
