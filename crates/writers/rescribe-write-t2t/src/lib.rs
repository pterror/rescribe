//! txt2tags (t2t) writer for rescribe.
//!
//! Emits documents as txt2tags markup.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as txt2tags markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as txt2tags markup with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::ok(ctx.output.into_bytes()))
}

struct EmitContext {
    output: String,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }
}

fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_node(node, ctx);
    }
}

fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::DOCUMENT => emit_nodes(&node.children, ctx),

        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1).min(5) as usize;
            let numbered = node.props.get_bool("numbered").unwrap_or(false);
            let marker = if numbered { '+' } else { '=' };

            for _ in 0..level {
                ctx.write(&marker.to_string());
            }
            ctx.write(" ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write(" ");
            for _ in 0..level {
                ctx.write(&marker.to_string());
            }
            ctx.write("\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::CODE_BLOCK => {
            ctx.write("```\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
                if !content.ends_with('\n') {
                    ctx.write("\n");
                }
            }
            ctx.write("```\n\n");
        }

        node::RAW_BLOCK => {
            ctx.write("\"\"\"\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
                if !content.ends_with('\n') {
                    ctx.write("\n");
                }
            }
            ctx.write("\"\"\"\n\n");
        }

        node::BLOCKQUOTE => {
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    ctx.write("\t");
                    emit_inline_nodes(&child.children, ctx);
                    ctx.write("\n");
                } else {
                    emit_node(child, ctx);
                }
            }
            ctx.write("\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let marker = if ordered { "+ " } else { "- " };

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    ctx.write(marker);
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, ctx);
                        } else {
                            emit_node(item_child, ctx);
                        }
                    }
                    ctx.write("\n");
                }
            }
            ctx.write("\n");
        }

        node::LIST_ITEM => {
            emit_nodes(&node.children, ctx);
        }

        node::TABLE => {
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    let is_header = row
                        .children
                        .first()
                        .map(|c| c.kind.as_str() == node::TABLE_HEADER)
                        .unwrap_or(false);

                    if is_header {
                        ctx.write("||");
                    } else {
                        ctx.write("|");
                    }

                    for cell in &row.children {
                        ctx.write(" ");
                        emit_inline_nodes(&cell.children, ctx);
                        ctx.write(" |");
                    }
                    ctx.write("\n");
                }
            }
            ctx.write("\n");
        }

        node::HORIZONTAL_RULE => {
            ctx.write("--------------------\n\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::FIGURE => emit_nodes(&node.children, ctx),

        // Inline nodes at block level
        node::TEXT | node::STRONG | node::EMPHASIS | node::CODE | node::LINK => {
            emit_inline_node(node, ctx);
            ctx.write("\n\n");
        }

        _ => emit_nodes(&node.children, ctx),
    }
}

fn emit_inline_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_inline_node(node, ctx);
    }
}

fn emit_inline_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }

        node::STRONG => {
            ctx.write("**");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("**");
        }

        node::EMPHASIS => {
            ctx.write("//");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("//");
        }

        node::UNDERLINE => {
            ctx.write("__");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("__");
        }

        node::STRIKEOUT => {
            ctx.write("--");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("--");
        }

        node::CODE => {
            ctx.write("``");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
            emit_inline_nodes(&node.children, ctx);
            ctx.write("``");
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("[");
                if !node.children.is_empty() {
                    emit_inline_nodes(&node.children, ctx);
                    ctx.write(" ");
                }
                ctx.write(url);
                ctx.write("]");
            } else {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("[");
                ctx.write(url);
                ctx.write("]");
            }
        }

        node::LINE_BREAK => {
            ctx.write("\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        node::SUPERSCRIPT | node::SUBSCRIPT => {
            // t2t doesn't support these, emit as-is
            emit_inline_nodes(&node.children, ctx);
        }

        _ => emit_inline_nodes(&node.children, ctx),
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
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(1, |h| h.text("Title")));
        let output = emit_str(&doc);
        assert!(output.contains("= Title ="));
    }

    #[test]
    fn test_emit_heading_level2() {
        let doc = doc(|d| d.heading(2, |h| h.text("Subtitle")));
        let output = emit_str(&doc);
        assert!(output.contains("== Subtitle =="));
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = doc(|d| d.para(|p| p.text("Hello, world!")));
        let output = emit_str(&doc);
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_bold() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("**bold**"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("//italic//"));
    }

    #[test]
    fn test_emit_underline() {
        let doc = doc(|d| d.para(|p| p.underline(|u| u.text("underlined"))));
        let output = emit_str(&doc);
        assert!(output.contains("__underlined__"));
    }

    #[test]
    fn test_emit_strikeout() {
        let doc = doc(|d| d.para(|p| p.strike(|s| s.text("strikeout"))));
        let output = emit_str(&doc);
        assert!(output.contains("--strikeout--"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("code")));
        let output = emit_str(&doc);
        assert!(output.contains("``code``"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("http://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("[click http://example.com]"));
    }

    #[test]
    fn test_emit_unordered_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("- one"));
        assert!(output.contains("- two"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc =
            doc(|d| d.ordered_list(|l| l.item(|i| i.text("first")).item(|i| i.text("second"))));
        let output = emit_str(&doc);
        assert!(output.contains("+ first"));
        assert!(output.contains("+ second"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block("print hi"));
        let output = emit_str(&doc);
        assert!(output.contains("```"));
        assert!(output.contains("print hi"));
    }

    #[test]
    fn test_emit_horizontal_rule() {
        use rescribe_core::Node;
        let root = Node::new(node::DOCUMENT).children(vec![Node::new(node::HORIZONTAL_RULE)]);
        let document = Document::new().with_content(root);
        let output = emit_str(&document);
        assert!(output.contains("--------------------"));
    }
}
