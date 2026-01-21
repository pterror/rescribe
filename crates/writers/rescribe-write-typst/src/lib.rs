//! Typst writer for rescribe.
//!
//! Emits documents as Typst markup.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as Typst.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Typst with custom options.
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
            ctx.write("#line(length: 100%)\n\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK | node::RAW_INLINE => {
            let format = node.props.get_str(prop::FORMAT).unwrap_or("");
            if format == "typst"
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
            ctx.write("_");
            emit_nodes(&node.children, ctx);
            ctx.write("_");
        }

        node::STRONG => {
            ctx.write("*");
            emit_nodes(&node.children, ctx);
            ctx.write("*");
        }

        node::STRIKEOUT => {
            ctx.write("#strike[");
            emit_nodes(&node.children, ctx);
            ctx.write("]");
        }

        node::UNDERLINE => {
            ctx.write("#underline[");
            emit_nodes(&node.children, ctx);
            ctx.write("]");
        }

        node::SUBSCRIPT => {
            ctx.write("#sub[");
            emit_nodes(&node.children, ctx);
            ctx.write("]");
        }

        node::SUPERSCRIPT => {
            ctx.write("#super[");
            emit_nodes(&node.children, ctx);
            ctx.write("]");
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write("`");
                ctx.write(content);
                ctx.write("`");
            }
        }

        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("\\ \n"),
        node::SOFT_BREAK => ctx.write("\n"),

        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),

        node::SMALL_CAPS => {
            ctx.write("#smallcaps[");
            emit_nodes(&node.children, ctx);
            ctx.write("]");
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
                ctx.write("$ ");
                ctx.write(source);
                ctx.write(" $\n\n");
            }
        }

        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type for Typst: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a heading.
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1);

    // Typst uses = for headings
    for _ in 0..level {
        ctx.write("=");
    }
    ctx.write(" ");

    emit_nodes(&node.children, ctx);
    ctx.write("\n\n");
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE);

    ctx.write("```");
    if let Some(lang) = lang {
        ctx.write(lang);
    }
    ctx.write("\n");

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
        if !content.ends_with('\n') {
            ctx.write("\n");
        }
    }
    ctx.write("```\n\n");
}

/// Emit a blockquote.
fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    ctx.write("#quote[\n");
    emit_nodes(&node.children, ctx);
    ctx.write("]\n\n");
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

/// Emit a list item.
fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

    // Indentation
    for _ in 1..ctx.list_depth {
        ctx.write("  ");
    }

    if ordered {
        ctx.write("+ ");
    } else {
        ctx.write("- ");
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
    // Count columns
    let num_cols = count_table_columns(node);

    ctx.write("#table(\n  columns: ");
    ctx.write(&num_cols.to_string());
    ctx.write(",\n");

    emit_table_rows(&node.children, ctx);

    ctx.write(")\n\n");
}

fn count_table_columns(table: &Node) -> usize {
    for child in &table.children {
        match child.kind.as_str() {
            node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
                if let Some(row) = child.children.first() {
                    return row.children.len();
                }
            }
            node::TABLE_ROW => {
                return child.children.len();
            }
            _ => {}
        }
    }
    1
}

fn emit_table_rows(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        match node.kind.as_str() {
            node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
                emit_table_rows(&node.children, ctx);
            }
            node::TABLE_ROW => {
                for cell in &node.children {
                    ctx.write("  [");
                    emit_nodes(&cell.children, ctx);
                    ctx.write("],\n");
                }
            }
            _ => {}
        }
    }
}

/// Emit a figure.
fn emit_figure(node: &Node, ctx: &mut EmitContext) {
    ctx.write("#figure(\n");

    // Find image and caption
    let mut has_image = false;
    for child in &node.children {
        if child.kind.as_str() == node::IMAGE
            && let Some(url) = child.props.get_str(prop::URL)
        {
            ctx.write("  image(\"");
            ctx.write(url);
            ctx.write("\"),\n");
            has_image = true;
        }
    }

    if !has_image {
        // Emit other content
        ctx.write("  [\n");
        for child in &node.children {
            if child.kind.as_str() != node::CAPTION {
                emit_node(child, ctx);
            }
        }
        ctx.write("  ],\n");
    }

    for child in &node.children {
        if child.kind.as_str() == node::CAPTION {
            ctx.write("  caption: [");
            emit_nodes(&child.children, ctx);
            ctx.write("],\n");
        }
    }

    ctx.write(")\n\n");
}

/// Emit a definition list.
fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    ctx.write("#terms(\n");
    emit_nodes(&node.children, ctx);
    ctx.write(")\n\n");
}

/// Emit a definition term.
fn emit_definition_term(node: &Node, ctx: &mut EmitContext) {
    ctx.write("  / ");
    emit_nodes(&node.children, ctx);
    ctx.write(": ");
}

/// Emit a definition description.
fn emit_definition_desc(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.write(",\n");
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("#link(\"");
        ctx.write(url);
        ctx.write("\")[");
        emit_nodes(&node.children, ctx);
        ctx.write("]");
    } else {
        emit_nodes(&node.children, ctx);
    }
}

/// Emit an image.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("#image(\"");
        ctx.write(url);
        ctx.write("\")\n");
    }
}

/// Emit a footnote reference.
fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write("#footnote[");
        ctx.write(label);
        ctx.write("]");
    }
}

/// Emit a footnote definition.
fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write("// Footnote ");
        ctx.write(label);
        ctx.write(": ");
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
        assert!(output.contains("= Title"));
    }

    #[test]
    fn test_emit_heading_level2() {
        let doc = doc(|d| d.heading(2, |h| h.text("Subtitle")));
        let output = emit_str(&doc);
        assert!(output.contains("== Subtitle"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("_italic_"));
    }

    #[test]
    fn test_emit_strong() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("*bold*"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("code")));
        let output = emit_str(&doc);
        assert!(output.contains("`code`"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("https://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("#link(\"https://example.com\")[click]"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block_lang("print('hi')", "python"));
        let output = emit_str(&doc);
        assert!(output.contains("```python"));
        assert!(output.contains("print('hi')"));
        assert!(output.contains("```"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("- one"));
        assert!(output.contains("- two"));
    }

    #[test]
    fn test_emit_image() {
        let doc = doc(|d| d.para(|p| p.image("photo.png", "A photo")));
        let output = emit_str(&doc);
        assert!(output.contains("#image(\"photo.png\")"));
    }
}
