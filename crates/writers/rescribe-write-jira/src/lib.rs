//! Jira markup writer for rescribe.
//!
//! Emits documents as Jira/Confluence wiki markup.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as Jira markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Jira markup with custom options.
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
    list_depth: usize,
    in_list: bool,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            list_depth: 0,
            in_list: false,
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
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1).min(6);
            ctx.write(&format!("h{}. ", level));
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::CODE_BLOCK => {
            let lang = node.props.get_str(prop::LANGUAGE);

            if let Some(lang) = lang {
                ctx.write(&format!("{{code:{}}}\n", lang));
            } else {
                ctx.write("{code}\n");
            }

            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
                if !content.ends_with('\n') {
                    ctx.write("\n");
                }
            }
            ctx.write("{code}\n\n");
        }

        node::BLOCKQUOTE => {
            ctx.write("{quote}\n");
            emit_nodes(&node.children, ctx);
            ctx.write("{quote}\n\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            ctx.list_depth += 1;
            ctx.in_list = true;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    let marker = if ordered { "#" } else { "*" };
                    for _ in 0..ctx.list_depth {
                        ctx.write(marker);
                    }
                    ctx.write(" ");

                    // Emit list item content
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, ctx);
                        } else if item_child.kind.as_str() == node::LIST {
                            ctx.write("\n");
                            emit_node(item_child, ctx);
                            continue;
                        } else {
                            emit_node(item_child, ctx);
                        }
                    }
                    ctx.write("\n");
                }
            }

            ctx.list_depth -= 1;
            if ctx.list_depth == 0 {
                ctx.in_list = false;
                ctx.write("\n");
            }
        }

        node::LIST_ITEM => {
            // Handled by LIST
            emit_nodes(&node.children, ctx);
        }

        node::TABLE => {
            for child in &node.children {
                emit_table_element(child, ctx);
            }
            ctx.write("\n");
        }

        node::HORIZONTAL_RULE => {
            ctx.write("----\n\n");
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

fn emit_table_element(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::TABLE_HEAD => {
            for child in &node.children {
                emit_table_element(child, ctx);
            }
        }
        node::TABLE_BODY | node::TABLE_FOOT => {
            for child in &node.children {
                emit_table_element(child, ctx);
            }
        }
        node::TABLE_ROW => {
            // Check if this is a header row
            let is_header = node
                .children
                .first()
                .map(|c| c.kind.as_str() == node::TABLE_HEADER)
                .unwrap_or(false);

            if is_header {
                ctx.write("||");
                for cell in &node.children {
                    emit_inline_nodes(&cell.children, ctx);
                    ctx.write("||");
                }
            } else {
                ctx.write("|");
                for cell in &node.children {
                    emit_inline_nodes(&cell.children, ctx);
                    ctx.write("|");
                }
            }
            ctx.write("\n");
        }
        _ => {}
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
            ctx.write("*");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("*");
        }

        node::EMPHASIS => {
            ctx.write("_");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("_");
        }

        node::STRIKEOUT => {
            ctx.write("-");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("-");
        }

        node::UNDERLINE => {
            ctx.write("+");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("+");
        }

        node::SUPERSCRIPT => {
            ctx.write("^");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("^");
        }

        node::SUBSCRIPT => {
            ctx.write("~");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("~");
        }

        node::CODE => {
            ctx.write("{{");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}}");
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("[");
                emit_inline_nodes(&node.children, ctx);
                ctx.write("|");
                ctx.write(url);
                ctx.write("]");
            } else {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::IMAGE => {
            ctx.write("!");
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write(url);
                if let Some(alt) = node.props.get_str(prop::ALT) {
                    ctx.write("|");
                    ctx.write(alt);
                }
            }
            ctx.write("!");
        }

        node::LINE_BREAK => {
            ctx.write("\\\\\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
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
        assert!(output.contains("h1. Title"));
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
        assert!(output.contains("*bold*"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("_italic_"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("code")));
        let output = emit_str(&doc);
        assert!(output.contains("{{code}}"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("https://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("[click|https://example.com]"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block_lang("print('hi')", "python"));
        let output = emit_str(&doc);
        assert!(output.contains("{code:python}"));
        assert!(output.contains("print('hi')"));
        assert!(output.contains("{code}"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("* one"));
        assert!(output.contains("* two"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc =
            doc(|d| d.ordered_list(|l| l.item(|i| i.text("first")).item(|i| i.text("second"))));
        let output = emit_str(&doc);
        assert!(output.contains("# first"));
        assert!(output.contains("# second"));
    }
}
