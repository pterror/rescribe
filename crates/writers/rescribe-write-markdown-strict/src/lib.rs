//! Markdown strict writer for rescribe.
//!
//! Emits original Markdown.pl compatible syntax (no extensions).
//! Uses indented code blocks, no tables, no strikethrough, etc.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node};
use rescribe_std::prop;

/// Emit a document as strict Markdown.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as strict Markdown with options.
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

    fn ensure_blank_line(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            if self.output.ends_with('\n') {
                self.output.push('\n');
            } else {
                self.output.push_str("\n\n");
            }
        }
    }
}

fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_node(node, ctx);
    }
}

fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        "document" => emit_nodes(&node.children, ctx),

        "paragraph" => emit_paragraph(node, ctx),
        "heading" => emit_heading(node, ctx),
        "blockquote" => emit_blockquote(node, ctx),
        "code_block" => emit_code_block(node, ctx),
        "list" => emit_list(node, ctx),
        "list_item" => emit_list_item(node, ctx),
        "horizontal_rule" => emit_horizontal_rule(ctx),
        "raw_block" => emit_raw_block(node, ctx),

        "text" => emit_text(node, ctx),
        "emphasis" => emit_emphasis(node, ctx),
        "strong" => emit_strong(node, ctx),
        "code" => emit_inline_code(node, ctx),
        "link" => emit_link(node, ctx),
        "image" => emit_image(node, ctx),
        "line_break" => emit_line_break(ctx),
        "soft_break" => emit_soft_break(ctx),
        "raw_inline" => emit_raw_inline(node, ctx),

        // Unsupported in strict markdown - emit as raw HTML or skip
        "table" | "table_row" | "table_cell" | "table_header" => {
            // Tables not supported in strict markdown
            emit_nodes(&node.children, ctx);
        }
        "strikeout" => {
            // Strikethrough not supported
            emit_nodes(&node.children, ctx);
        }
        "definition_list" | "definition_term" | "definition_desc" => {
            emit_nodes(&node.children, ctx);
        }

        _ => emit_nodes(&node.children, ctx),
    }
}

fn emit_paragraph(node: &Node, ctx: &mut EmitContext) {
    if !ctx.in_tight_list {
        ctx.ensure_blank_line();
    }
    emit_nodes(&node.children, ctx);
    if !ctx.in_tight_list {
        ctx.write("\n");
    }
}

fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;
    let prefix = "#".repeat(level.min(6));
    ctx.write(&prefix);
    ctx.write(" ");
    emit_nodes(&node.children, ctx);
    ctx.write("\n");
}

fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();

    let mut inner = EmitContext::new();
    emit_nodes(&node.children, &mut inner);

    for line in inner.output.lines() {
        ctx.write("> ");
        ctx.write(line);
        ctx.write("\n");
    }
}

fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();

    // Strict markdown uses indented code blocks (4 spaces)
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        for line in content.lines() {
            ctx.write("    ");
            ctx.write(line);
            ctx.write("\n");
        }
    } else {
        for child in &node.children {
            if let Some(text) = child.props.get_str(prop::CONTENT) {
                for line in text.lines() {
                    ctx.write("    ");
                    ctx.write(line);
                    ctx.write("\n");
                }
            }
        }
    }

    ctx.write("\n");
}

fn emit_list(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    ctx.list_depth += 1;
    emit_nodes(&node.children, ctx);
    ctx.list_depth -= 1;
    if ctx.list_depth == 0 {
        ctx.write("\n");
    }
}

fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    let indent = "    ".repeat(ctx.list_depth.saturating_sub(1));

    // Check if parent is ordered
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

    let marker = if ordered { "1. " } else { "- " };

    ctx.write(&indent);
    ctx.write(marker);

    // Check if this is a tight list item (single paragraph)
    let is_tight = node.children.len() == 1
        && node.children.first().map(|n| n.kind.as_str()) == Some("paragraph");

    if is_tight {
        ctx.in_tight_list = true;
        emit_nodes(&node.children, ctx);
        ctx.in_tight_list = false;
    } else {
        let mut first = true;
        for child in &node.children {
            if !first {
                ctx.write(&indent);
                ctx.write("    ");
            }
            first = false;
            emit_node(child, ctx);
        }
    }

    ctx.write("\n");
}

fn emit_horizontal_rule(ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    ctx.write("---\n");
}

fn emit_raw_block(node: &Node, ctx: &mut EmitContext) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.ensure_blank_line();
        ctx.write(content);
        ctx.write("\n");
    }
}

fn emit_text(node: &Node, ctx: &mut EmitContext) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
    }
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

fn emit_inline_code(node: &Node, ctx: &mut EmitContext) {
    let content = node.props.get_str(prop::CONTENT).unwrap_or("");
    let backticks = if content.contains('`') { "``" } else { "`" };
    ctx.write(backticks);
    if content.starts_with('`') || content.ends_with('`') {
        ctx.write(" ");
    }
    ctx.write(content);
    if content.starts_with('`') || content.ends_with('`') {
        ctx.write(" ");
    }
    ctx.write(backticks);
}

fn emit_link(node: &Node, ctx: &mut EmitContext) {
    let url = node.props.get_str(prop::URL).unwrap_or("");
    let title = node.props.get_str(prop::TITLE).unwrap_or("");

    ctx.write("[");
    emit_nodes(&node.children, ctx);
    ctx.write("](");
    ctx.write(url);
    if !title.is_empty() {
        ctx.write(" \"");
        ctx.write(title);
        ctx.write("\"");
    }
    ctx.write(")");
}

fn emit_image(node: &Node, ctx: &mut EmitContext) {
    let url = node.props.get_str(prop::URL).unwrap_or("");
    let alt = node.props.get_str(prop::ALT).unwrap_or("");
    let title = node.props.get_str(prop::TITLE).unwrap_or("");

    ctx.write("![");
    ctx.write(alt);
    ctx.write("](");
    ctx.write(url);
    if !title.is_empty() {
        ctx.write(" \"");
        ctx.write(title);
        ctx.write("\"");
    }
    ctx.write(")");
}

fn emit_line_break(ctx: &mut EmitContext) {
    ctx.write("  \n");
}

fn emit_soft_break(ctx: &mut EmitContext) {
    ctx.write("\n");
}

fn emit_raw_inline(node: &Node, ctx: &mut EmitContext) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::NodeKind;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("paragraph"))
                    .child(Node::new(NodeKind::from("text")).prop("content", "Hello world")),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("heading"))
                    .prop("level", 2i64)
                    .child(Node::new(NodeKind::from("text")).prop("content", "Title")),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("## Title"));
    }

    #[test]
    fn test_emit_code_block_indented() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document"))
                .child(Node::new(NodeKind::from("code_block")).prop("content", "let x = 1;")),
        );

        let output = emit_str(&doc);
        // Strict markdown uses indented code blocks
        assert!(output.contains("    let x = 1;"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("paragraph")).child(
                    Node::new(NodeKind::from("emphasis"))
                        .child(Node::new(NodeKind::from("text")).prop("content", "italic")),
                ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("*italic*"));
    }

    #[test]
    fn test_emit_link() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("paragraph")).child(
                    Node::new(NodeKind::from("link"))
                        .prop("url", "https://example.com")
                        .child(Node::new(NodeKind::from("text")).prop("content", "link")),
                ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("[link](https://example.com)"));
    }
}
