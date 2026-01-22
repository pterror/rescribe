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
    options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new(options.use_source_info);

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
    /// Whether to use source formatting hints.
    use_source_info: bool,
}

impl EmitContext {
    fn new(use_source_info: bool) -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            list_depth: 0,
            in_tight_list: false,
            use_source_info,
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
        node::HORIZONTAL_RULE => emit_horizontal_rule(node, ctx),
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

    // Check for setext style preference (only works for level 1 and 2)
    let use_setext = ctx.use_source_info
        && level <= 2
        && node.props.get_str(prop::MD_HEADING_STYLE) == Some("setext");

    if use_setext {
        // Emit content first
        let mut content_ctx = EmitContext::new(ctx.use_source_info);
        emit_nodes(&node.children, &mut content_ctx);
        let content = content_ctx.output.trim_end();
        ctx.write(content);
        ctx.newline();

        // Emit underline
        let underline_char = if level == 1 { "=" } else { "-" };
        let underline_len = content.len().max(3);
        ctx.write(&underline_char.repeat(underline_len));
        ctx.newline();
    } else {
        // ATX style
        let hashes = "#".repeat(level.min(6));
        ctx.write(&hashes);
        ctx.write(" ");
        emit_nodes(&node.children, ctx);
        ctx.newline();
    }
}

fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE).unwrap_or("");
    let content = node.props.get_str(prop::CONTENT).unwrap_or("");

    // Get fence character and length from source info
    let fence_char = if ctx.use_source_info {
        node.props
            .get_str(prop::MD_FENCE_CHAR)
            .and_then(|s| s.chars().next())
            .unwrap_or('`')
    } else {
        '`'
    };

    let fence_length = if ctx.use_source_info {
        node.props
            .get_int(prop::MD_FENCE_LENGTH)
            .map(|n| n as usize)
            .unwrap_or(3)
            .max(3)
    } else {
        3
    };

    let fence: String = std::iter::repeat_n(fence_char, fence_length).collect();

    ctx.write(&fence);
    ctx.write(lang);
    ctx.newline();
    ctx.write(content);
    if !content.ends_with('\n') {
        ctx.newline();
    }
    ctx.write(&fence);
    ctx.newline();
}

fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    // Emit children line by line, prefixing with >
    let mut inner_ctx = EmitContext::new(ctx.use_source_info);
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

    // Get list marker from source info for unordered lists
    let list_marker = if ctx.use_source_info && !ordered {
        node.props
            .get_str(prop::MD_LIST_MARKER)
            .and_then(|s| s.chars().next())
            .unwrap_or('-')
    } else {
        '-'
    };

    ctx.list_depth += 1;
    let old_tight = ctx.in_tight_list;
    ctx.in_tight_list = tight;

    for (i, child) in node.children.iter().enumerate() {
        let indent = ctx.list_indent();
        ctx.write(&indent);

        if ordered {
            ctx.write(&format!("{}. ", start + i));
        } else {
            ctx.write(&format!("{} ", list_marker));
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
    // Emit task list marker if present
    if let Some(checked) = node.props.get_bool(prop::CHECKED) {
        if checked {
            ctx.write("[x] ");
        } else {
            ctx.write("[ ] ");
        }
    }

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

    // Get column alignments if present
    let alignments: Vec<&str> = node
        .props
        .get_str("column_alignments")
        .map(|s| s.split(',').collect())
        .unwrap_or_default();

    // First pass: collect all cell contents and calculate widths
    for child in &node.children {
        match child.kind.as_str() {
            node::TABLE_HEAD => {
                for row in &child.children {
                    let cells = collect_row_cells(row, ctx.use_source_info);
                    update_col_widths(&cells, &mut col_widths);
                    rows.push(cells);
                }
            }
            node::TABLE_ROW => {
                let cells = collect_row_cells(child, ctx.use_source_info);
                update_col_widths(&cells, &mut col_widths);
                rows.push(cells);
            }
            node::TABLE_BODY => {
                for row in &child.children {
                    let cells = collect_row_cells(row, ctx.use_source_info);
                    update_col_widths(&cells, &mut col_widths);
                    rows.push(cells);
                }
            }
            _ => {}
        }
    }

    // Emit header row (first row)
    if let Some(header) = rows.first() {
        emit_table_row(header, &col_widths, &alignments, ctx);

        // Emit separator with alignment markers
        ctx.write("|");
        for (i, width) in col_widths.iter().enumerate() {
            let align = alignments.get(i).copied().unwrap_or("none");
            let dashes = width.saturating_sub(match align {
                "left" | "right" => 1,
                "center" => 2,
                _ => 0,
            });
            match align {
                "left" => {
                    ctx.write(&format!(" :{} |", "-".repeat(dashes.max(1) + 1)));
                }
                "right" => {
                    ctx.write(&format!(" {}: |", "-".repeat(dashes.max(1) + 1)));
                }
                "center" => {
                    ctx.write(&format!(" :{}: |", "-".repeat(dashes.max(1))));
                }
                _ => {
                    ctx.write(&format!(" {} |", "-".repeat(*width)));
                }
            }
        }
        ctx.newline();

        // Emit remaining rows
        for row in rows.iter().skip(1) {
            emit_table_row(row, &col_widths, &alignments, ctx);
        }
    }
}

fn collect_row_cells(row: &Node, use_source_info: bool) -> Vec<String> {
    row.children
        .iter()
        .map(|cell| {
            let mut cell_ctx = EmitContext::new(use_source_info);
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

fn emit_table_row(cells: &[String], widths: &[usize], alignments: &[&str], ctx: &mut EmitContext) {
    ctx.write("|");
    for (i, cell) in cells.iter().enumerate() {
        let width = widths.get(i).copied().unwrap_or(3);
        let align = alignments.get(i).copied().unwrap_or("none");
        match align {
            "right" => ctx.write(&format!(" {:>width$} |", cell, width = width)),
            "center" => ctx.write(&format!(" {:^width$} |", cell, width = width)),
            _ => ctx.write(&format!(" {:width$} |", cell, width = width)),
        }
    }
    ctx.newline();
}

fn emit_horizontal_rule(node: &Node, ctx: &mut EmitContext) {
    // Get break character from source info
    let break_char = if ctx.use_source_info {
        node.props
            .get_str(prop::MD_BREAK_CHAR)
            .and_then(|s| s.chars().next())
            .unwrap_or('-')
    } else {
        '-'
    };

    let rule: String = std::iter::repeat_n(break_char, 3).collect();
    ctx.write(&rule);
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
    let at_line_start = |i: usize| i == 0 || chars.get(i.wrapping_sub(1)) == Some(&'\n');

    for (i, &c) in chars.iter().enumerate() {
        match c {
            // Always escape these markdown special characters
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '|' => {
                result.push('\\');
                result.push(c);
            }
            // Escape ~ only if followed by another ~ (strikethrough)
            '~' if chars.get(i + 1) == Some(&'~') => {
                result.push('\\');
                result.push(c);
            }
            // Escape ! if followed by [ (image syntax)
            '!' if chars.get(i + 1) == Some(&'[') => {
                result.push('\\');
                result.push(c);
            }
            // Escape # at start of line (heading)
            '#' if at_line_start(i) => {
                result.push('\\');
                result.push(c);
            }
            // Escape - + at start of line (unordered list)
            '-' | '+' if at_line_start(i) && chars.get(i + 1) == Some(&' ') => {
                result.push('\\');
                result.push(c);
            }
            // Escape digits followed by . or ) at start of line (ordered list)
            '0'..='9' if at_line_start(i) => {
                // Check if this is the start of an ordered list: number followed by . or )
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j < chars.len() && (chars[j] == '.' || chars[j] == ')') {
                    // This could be interpreted as an ordered list, escape first digit
                    result.push('\\');
                }
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

fn emit_emphasis(node: &Node, ctx: &mut EmitContext) {
    // Get emphasis marker from source info
    let marker = if ctx.use_source_info {
        node.props
            .get_str(prop::MD_EMPHASIS_MARKER)
            .and_then(|s| s.chars().next())
            .unwrap_or('*')
    } else {
        '*'
    };

    ctx.write(&marker.to_string());
    emit_nodes(&node.children, ctx);
    ctx.write(&marker.to_string());
}

fn emit_strong(node: &Node, ctx: &mut EmitContext) {
    // Get strong marker from source info
    let marker = if ctx.use_source_info {
        node.props.get_str(prop::MD_STRONG_MARKER).unwrap_or("**")
    } else {
        "**"
    };

    ctx.write(marker);
    emit_nodes(&node.children, ctx);
    ctx.write(marker);
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
    ctx.blank_line();
    for child in &node.children {
        match child.kind.as_str() {
            node::DEFINITION_TERM => {
                emit_nodes(&child.children, ctx);
                ctx.newline();
            }
            node::DEFINITION_DESC => {
                ctx.write(":   ");
                // Emit content, indenting continuation lines
                let mut inner_ctx = EmitContext::new(ctx.use_source_info);
                emit_nodes(&child.children, &mut inner_ctx);
                let content = inner_ctx.output.trim_end();
                let mut first_line = true;
                for line in content.lines() {
                    if first_line {
                        ctx.write(line);
                        first_line = false;
                    } else {
                        ctx.newline();
                        ctx.write("    ");
                        ctx.write(line);
                    }
                }
                ctx.newline();
            }
            _ => emit_node(child, ctx),
        }
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
    use crate::builder::markdown;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = markdown(|d| d.para(|i| i.text("Hello, world!")));
        let output = emit_str(&doc);
        assert_eq!(output, "Hello, world!\n");
    }

    #[test]
    fn test_emit_heading() {
        let doc = markdown(|d| d.h2(|i| i.text("Title")));
        let output = emit_str(&doc);
        assert_eq!(output, "## Title\n");
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = markdown(|d| d.para(|i| i.em(|i| i.text("italic"))));
        let output = emit_str(&doc);
        assert_eq!(output, "*italic*\n");
    }

    #[test]
    fn test_emit_link() {
        let doc = markdown(|d| d.para(|i| i.link("https://example.com", |i| i.text("link"))));
        let output = emit_str(&doc);
        assert_eq!(output, "[link](https://example.com)\n");
    }

    #[test]
    fn test_emit_code_block() {
        let doc = markdown(|d| d.code_block_lang("rust", "fn main() {}"));
        let output = emit_str(&doc);
        assert_eq!(output, "```rust\nfn main() {}\n```\n");
    }

    #[test]
    fn test_emit_list() {
        let doc = markdown(|d| {
            d.bullet_list(|l| l.item(|i| i.text("item 1")).item(|i| i.text("item 2")))
        });
        let output = emit_str(&doc);
        assert!(output.contains("- item 1"));
        assert!(output.contains("- item 2"));
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use rescribe_core::{EmitOptions, ParseOptions};
    use rescribe_read_markdown::parse_with_options as md_parse;

    use super::*;

    fn roundtrip(input: &str) -> String {
        let parse_opts = ParseOptions {
            preserve_source_info: true,
            ..Default::default()
        };
        let emit_opts = EmitOptions {
            use_source_info: true,
            ..Default::default()
        };

        let doc = md_parse(input, &parse_opts).unwrap().value;
        let result = emit_with_options(&doc, &emit_opts).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_roundtrip_atx_heading() {
        let input = "# Heading 1\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_setext_heading() {
        let input = "Heading 1\n=========\n";
        let output = roundtrip(input);
        assert!(output.contains("Heading 1\n"));
        assert!(output.contains("==="));
    }

    #[test]
    fn test_roundtrip_emphasis_asterisk() {
        let input = "*italic*\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_emphasis_underscore() {
        let input = "_italic_\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_strong_asterisk() {
        let input = "**bold**\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_strong_underscore() {
        let input = "__bold__\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_list_dash() {
        let input = "- item 1\n- item 2\n";
        let output = roundtrip(input);
        assert!(output.contains("- item 1"));
        assert!(output.contains("- item 2"));
    }

    #[test]
    fn test_roundtrip_list_asterisk() {
        let input = "* item 1\n* item 2\n";
        let output = roundtrip(input);
        assert!(output.contains("* item 1"));
        assert!(output.contains("* item 2"));
    }

    #[test]
    fn test_roundtrip_list_plus() {
        let input = "+ item 1\n+ item 2\n";
        let output = roundtrip(input);
        assert!(output.contains("+ item 1"));
        assert!(output.contains("+ item 2"));
    }

    #[test]
    fn test_roundtrip_code_fence_backtick() {
        let input = "```rust\nfn main() {}\n```\n";
        let output = roundtrip(input);
        assert!(output.starts_with("```"));
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn test_roundtrip_code_fence_tilde() {
        let input = "~~~rust\nfn main() {}\n~~~\n";
        let output = roundtrip(input);
        assert!(output.starts_with("~~~"));
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn test_roundtrip_hr_dash() {
        let input = "---\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_hr_asterisk() {
        let input = "***\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_hr_underscore() {
        let input = "___\n";
        let output = roundtrip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_roundtrip_task_list() {
        let input = "- [ ] unchecked\n- [x] checked\n";
        let output = roundtrip(input);
        assert!(output.contains("[ ] unchecked"));
        assert!(output.contains("[x] checked"));
    }

    #[test]
    fn test_roundtrip_table_alignment() {
        let input =
            "| Left | Center | Right |\n|:-----|:------:|------:|\n| a    | b      | c     |\n";
        let output = roundtrip(input);
        // Just check table structure is preserved
        assert!(output.contains("| Left"));
        assert!(output.contains("| a"));
        // Note: alignment markers are preserved via column_alignments property
        // but exact formatting may vary
    }
}

#[cfg(test)]
mod escape_tests {
    use super::escape_markdown;

    #[test]
    fn test_escape_basic_chars() {
        assert_eq!(escape_markdown("*bold*"), "\\*bold\\*");
        assert_eq!(escape_markdown("`code`"), "\\`code\\`");
        assert_eq!(escape_markdown("[link]"), "\\[link\\]");
        assert_eq!(escape_markdown("<html>"), "\\<html\\>");
    }

    #[test]
    fn test_escape_pipe() {
        assert_eq!(escape_markdown("a | b"), "a \\| b");
    }

    #[test]
    fn test_escape_strikethrough() {
        // Only the first ~ of a pair is escaped, which breaks the syntax
        assert_eq!(escape_markdown("~~strike~~"), "\\~~strike\\~~");
        assert_eq!(escape_markdown("~not strike~"), "~not strike~");
    }

    #[test]
    fn test_escape_image_syntax() {
        assert_eq!(escape_markdown("![alt]"), "\\!\\[alt\\]");
        assert_eq!(escape_markdown("! not image"), "! not image");
    }

    #[test]
    fn test_escape_heading_at_line_start() {
        assert_eq!(escape_markdown("# heading"), "\\# heading");
        // # in middle of line is not escaped
        assert_eq!(escape_markdown("not # heading"), "not # heading");
    }

    #[test]
    fn test_escape_list_at_line_start() {
        assert_eq!(escape_markdown("- item"), "\\- item");
        assert_eq!(escape_markdown("+ item"), "\\+ item");
        // Without space, not a list
        assert_eq!(escape_markdown("-item"), "-item");
    }

    #[test]
    fn test_escape_ordered_list() {
        assert_eq!(escape_markdown("1. item"), "\\1. item");
        assert_eq!(escape_markdown("10. item"), "\\10. item");
        // Not at line start
        assert_eq!(escape_markdown("see 1. item"), "see 1. item");
    }
}
