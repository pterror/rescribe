//! DokuWiki writer for rescribe.
//!
//! Emits documents as DokuWiki markup.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as DokuWiki.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as DokuWiki with custom options.
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
        node::FIGURE => emit_nodes(&node.children, ctx),
        node::HORIZONTAL_RULE => {
            ctx.write("----\n\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK | node::RAW_INLINE => {
            let format = node.props.get_str(prop::FORMAT).unwrap_or("");
            if format == "dokuwiki"
                && let Some(content) = node.props.get_str(prop::CONTENT)
            {
                ctx.write(content);
            }
        }

        node::DEFINITION_LIST => emit_nodes(&node.children, ctx),
        node::DEFINITION_TERM => {
            ctx.write("  ");
            emit_nodes(&node.children, ctx);
        }
        node::DEFINITION_DESC => {
            ctx.write(" : ");
            emit_nodes(&node.children, ctx);
            ctx.write("\n");
        }

        // Inline elements
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }

        node::EMPHASIS => {
            ctx.write("//");
            emit_nodes(&node.children, ctx);
            ctx.write("//");
        }

        node::STRONG => {
            ctx.write("**");
            emit_nodes(&node.children, ctx);
            ctx.write("**");
        }

        node::STRIKEOUT => {
            ctx.write("<del>");
            emit_nodes(&node.children, ctx);
            ctx.write("</del>");
        }

        node::UNDERLINE => {
            ctx.write("__");
            emit_nodes(&node.children, ctx);
            ctx.write("__");
        }

        node::SUBSCRIPT => {
            ctx.write("<sub>");
            emit_nodes(&node.children, ctx);
            ctx.write("</sub>");
        }

        node::SUPERSCRIPT => {
            ctx.write("<sup>");
            emit_nodes(&node.children, ctx);
            ctx.write("</sup>");
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write("''");
                ctx.write(content);
                ctx.write("''");
            }
        }

        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("\\\\\n"),
        node::SOFT_BREAK => ctx.write(" "),

        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),

        node::SMALL_CAPS => {
            // No native support, just emit text
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
                ctx.write("<math>");
                ctx.write(source);
                ctx.write("</math>");
            }
        }

        "math_display" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write("<math>\n");
                ctx.write(source);
                ctx.write("\n</math>\n\n");
            }
        }

        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type for DokuWiki: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a heading (DokuWiki uses more = for lower level).
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;

    // DokuWiki: H1 = 6 equals, H2 = 5 equals, etc.
    let equals_count = 7 - level.min(6);

    for _ in 0..equals_count {
        ctx.write("=");
    }
    ctx.write(" ");

    emit_nodes(&node.children, ctx);

    ctx.write(" ");
    for _ in 0..equals_count {
        ctx.write("=");
    }
    ctx.write("\n\n");
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE);

    ctx.write("<code");
    if let Some(lang) = lang {
        ctx.write(" ");
        ctx.write(lang);
    }
    ctx.write(">\n");

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
        if !content.ends_with('\n') {
            ctx.write("\n");
        }
    }
    ctx.write("</code>\n\n");
}

/// Emit a blockquote.
fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    let mut inner = EmitContext::new();
    emit_nodes(&node.children, &mut inner);

    for line in inner.output.lines() {
        ctx.write("> ");
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
    if ctx.list_depth == 0 {
        ctx.write("\n");
    }
}

/// Emit a list item.
fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);

    // Indentation: 2 spaces per level
    for _ in 0..ctx.list_depth {
        ctx.write("  ");
    }

    if ordered {
        ctx.write("- ");
    } else {
        ctx.write("* ");
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
    emit_table_rows(&node.children, ctx, true);
    ctx.write("\n");
}

fn emit_table_rows(nodes: &[Node], ctx: &mut EmitContext, is_header: bool) {
    let mut first_row = is_header;
    for node in nodes {
        match node.kind.as_str() {
            node::TABLE_HEAD => {
                emit_table_rows(&node.children, ctx, true);
            }
            node::TABLE_BODY | node::TABLE_FOOT => {
                emit_table_rows(&node.children, ctx, false);
            }
            node::TABLE_ROW => {
                if first_row {
                    ctx.write("^");
                    for cell in &node.children {
                        ctx.write(" ");
                        emit_nodes(&cell.children, ctx);
                        ctx.write(" ^");
                    }
                    first_row = false;
                } else {
                    ctx.write("|");
                    for cell in &node.children {
                        ctx.write(" ");
                        emit_nodes(&cell.children, ctx);
                        ctx.write(" |");
                    }
                }
                ctx.write("\n");
            }
            _ => {}
        }
    }
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("[[");
        ctx.write(url);
        ctx.write("|");
        emit_nodes(&node.children, ctx);
        ctx.write("]]");
    } else {
        emit_nodes(&node.children, ctx);
    }
}

/// Emit an image.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("{{");
        ctx.write(url);
        if let Some(alt) = node.props.get_str(prop::ALT) {
            ctx.write("|");
            ctx.write(alt);
        }
        ctx.write("}}");
    }
}

/// Emit a footnote reference.
fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write("((");
        ctx.write(label);
        ctx.write("))");
    }
}

/// Emit a footnote definition.
fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    // DokuWiki uses inline footnotes with ((content))
    ctx.write("((");
    emit_nodes(&node.children, ctx);
    ctx.write("))");
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
        assert!(output.contains("====== Title ======"));
    }

    #[test]
    fn test_emit_heading_level2() {
        let doc = doc(|d| d.heading(2, |h| h.text("Subtitle")));
        let output = emit_str(&doc);
        assert!(output.contains("===== Subtitle ====="));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("//italic//"));
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
        assert!(output.contains("''code''"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("https://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("[[https://example.com|click]]"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block_lang("print('hi')", "python"));
        let output = emit_str(&doc);
        assert!(output.contains("<code python>"));
        assert!(output.contains("print('hi')"));
        assert!(output.contains("</code>"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("  * one"));
        assert!(output.contains("  * two"));
    }
}
