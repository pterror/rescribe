//! MultiMarkdown writer for rescribe.
//!
//! Generates MultiMarkdown output with its extensions:
//! - Metadata blocks
//! - Footnotes
//! - Tables
//! - Definition lists
//! - Math (LaTeX-style)

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node};
use rescribe_std::{node, prop};

/// Emit a document as MultiMarkdown.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as MultiMarkdown with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Emit metadata first
    emit_metadata(doc, &mut ctx);

    // Emit content
    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
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

fn emit_metadata(doc: &Document, ctx: &mut EmitContext) {
    if doc.metadata.is_empty() {
        return;
    }

    for (key, value) in doc.metadata.iter() {
        if let rescribe_core::PropValue::String(s) = value {
            ctx.write(key);
            ctx.write(": ");
            ctx.write(s);
            ctx.write("\n");
        }
    }
    ctx.write("\n");
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
        "table" => emit_table(node, ctx),
        "definition_list" => emit_definition_list(node, ctx),
        "footnote_def" => emit_footnote_def(node, ctx),

        "text" => emit_text(node, ctx),
        "emphasis" => emit_emphasis(node, ctx),
        "strong" => emit_strong(node, ctx),
        "strikeout" => emit_strikeout(node, ctx),
        "code" => emit_inline_code(node, ctx),
        "link" => emit_link(node, ctx),
        "image" => emit_image(node, ctx),
        "line_break" => emit_line_break(ctx),
        "soft_break" => emit_soft_break(ctx),
        "raw_inline" => emit_raw_inline(node, ctx),
        "footnote_ref" => emit_footnote_ref(node, ctx),
        "math_inline" => emit_math_inline(node, ctx),
        "math_block" => emit_math_block(node, ctx),

        _ => emit_nodes(&node.children, ctx),
    }
}

fn emit_paragraph(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    emit_nodes(&node.children, ctx);
    ctx.write("\n");
}

fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;
    let prefix = "#".repeat(level.min(6));
    ctx.write(&prefix);
    ctx.write(" ");
    emit_nodes(&node.children, ctx);

    // Add heading ID if present (MultiMarkdown extension)
    if let Some(id) = node.props.get_str(prop::ID)
        && !id.is_empty()
    {
        ctx.write(" {#");
        ctx.write(id);
        ctx.write("}");
    }
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

    let lang = node.props.get_str(prop::LANGUAGE).unwrap_or("");
    ctx.write("```");
    ctx.write(lang);
    ctx.write("\n");

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
        if !content.ends_with('\n') {
            ctx.write("\n");
        }
    } else {
        for child in &node.children {
            if let Some(text) = child.props.get_str(prop::CONTENT) {
                ctx.write(text);
            }
        }
        ctx.write("\n");
    }

    ctx.write("```\n");
}

fn emit_list(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
    let mut num = 1;

    for child in &node.children {
        if child.kind.as_str() == node::LIST_ITEM {
            let is_task = child.props.contains("checked");
            let checked = child.props.get_bool("checked").unwrap_or(false);

            if ordered {
                ctx.write(&format!("{}. ", num));
                num += 1;
            } else {
                ctx.write("- ");
            }

            if is_task {
                if checked {
                    ctx.write("[x] ");
                } else {
                    ctx.write("[ ] ");
                }
            }

            // Emit list item content
            for (i, item_child) in child.children.iter().enumerate() {
                if item_child.kind.as_str() == node::PARAGRAPH {
                    emit_nodes(&item_child.children, ctx);
                    if i < child.children.len() - 1 {
                        ctx.write("\n");
                    }
                } else if item_child.kind.as_str() == node::LIST {
                    ctx.write("\n");
                    let mut inner = EmitContext::new();
                    emit_node(item_child, &mut inner);
                    for line in inner.output.lines() {
                        ctx.write("  ");
                        ctx.write(line);
                        ctx.write("\n");
                    }
                } else {
                    emit_node(item_child, ctx);
                }
            }
            ctx.write("\n");
        }
    }
}

fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
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

fn emit_table(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    let mut is_first_row = true;

    for row in &node.children {
        if row.kind.as_str() == node::TABLE_ROW {
            ctx.write("|");
            for cell in &row.children {
                ctx.write(" ");
                emit_nodes(&cell.children, ctx);
                ctx.write(" |");
            }
            ctx.write("\n");

            // Add separator after header
            if is_first_row {
                ctx.write("|");
                for _ in &row.children {
                    ctx.write(" --- |");
                }
                ctx.write("\n");
                is_first_row = false;
            }
        }
    }
    ctx.write("\n");
}

fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();

    let mut i = 0;
    while i < node.children.len() {
        let child = &node.children[i];
        if child.kind.as_str() == node::DEFINITION_TERM {
            emit_nodes(&child.children, ctx);
            ctx.write("\n");
            i += 1;

            // Collect all definitions for this term
            while i < node.children.len() && node.children[i].kind.as_str() == node::DEFINITION_DESC
            {
                ctx.write(": ");
                emit_nodes(&node.children[i].children, ctx);
                ctx.write("\n");
                i += 1;
            }
            ctx.write("\n");
        } else {
            i += 1;
        }
    }
}

fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    let id = node.props.get_str(prop::ID).unwrap_or("");
    ctx.ensure_blank_line();
    ctx.write("[^");
    ctx.write(id);
    ctx.write("]: ");

    // Emit footnote content inline
    for (i, child) in node.children.iter().enumerate() {
        if child.kind.as_str() == node::PARAGRAPH {
            emit_nodes(&child.children, ctx);
        } else {
            emit_node(child, ctx);
        }
        if i < node.children.len() - 1 {
            ctx.write("\n    ");
        }
    }
    ctx.write("\n");
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

fn emit_strikeout(node: &Node, ctx: &mut EmitContext) {
    ctx.write("~~");
    emit_nodes(&node.children, ctx);
    ctx.write("~~");
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

fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    let id = node.props.get_str(prop::ID).unwrap_or("");
    ctx.write("[^");
    ctx.write(id);
    ctx.write("]");
}

fn emit_math_inline(node: &Node, ctx: &mut EmitContext) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write("$");
        ctx.write(content);
        ctx.write("$");
    }
}

fn emit_math_block(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write("$$\n");
        ctx.write(content);
        if !content.ends_with('\n') {
            ctx.write("\n");
        }
        ctx.write("$$\n");
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
    fn test_emit_basic() {
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
    fn test_emit_footnote() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document"))
                .child(
                    Node::new(NodeKind::from("paragraph"))
                        .child(Node::new(NodeKind::from("text")).prop("content", "Text"))
                        .child(Node::new(NodeKind::from("footnote_ref")).prop("id", "1")),
                )
                .child(
                    Node::new(NodeKind::from("footnote_def"))
                        .prop("id", "1")
                        .child(Node::new(NodeKind::from("paragraph")).child(
                            Node::new(NodeKind::from("text")).prop("content", "Footnote text"),
                        )),
                ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("[^1]"));
        assert!(output.contains("[^1]: Footnote text"));
    }

    #[test]
    fn test_emit_definition_list() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("definition_list"))
                    .child(
                        Node::new(NodeKind::from("definition_term"))
                            .child(Node::new(NodeKind::from("text")).prop("content", "Term")),
                    )
                    .child(
                        Node::new(NodeKind::from("definition_desc"))
                            .child(Node::new(NodeKind::from("text")).prop("content", "Definition")),
                    ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("Term"));
        assert!(output.contains(": Definition"));
    }

    #[test]
    fn test_emit_math() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("paragraph"))
                    .child(Node::new(NodeKind::from("math_inline")).prop("content", "x^2")),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("$x^2$"));
    }

    #[test]
    fn test_emit_table() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("table"))
                    .child(
                        Node::new(NodeKind::from("table_row"))
                            .child(
                                Node::new(NodeKind::from("table_header"))
                                    .child(Node::new(NodeKind::from("text")).prop("content", "A")),
                            )
                            .child(
                                Node::new(NodeKind::from("table_header"))
                                    .child(Node::new(NodeKind::from("text")).prop("content", "B")),
                            ),
                    )
                    .child(
                        Node::new(NodeKind::from("table_row"))
                            .child(
                                Node::new(NodeKind::from("table_cell"))
                                    .child(Node::new(NodeKind::from("text")).prop("content", "1")),
                            )
                            .child(
                                Node::new(NodeKind::from("table_cell"))
                                    .child(Node::new(NodeKind::from("text")).prop("content", "2")),
                            ),
                    ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("| A | B |"));
        assert!(output.contains("| --- |"));
        assert!(output.contains("| 1 | 2 |"));
    }
}
