//! Markua (Leanpub) writer for rescribe.
//!
//! Emits documents as Markua markup (Markdown for books).

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as Markua markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Markua markup with custom options.
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
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1).clamp(1, 6) as usize;
            let marker: String = "#".repeat(level);
            ctx.write(&marker);
            ctx.write(" ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::CODE_BLOCK => {
            ctx.write("```");
            if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
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

        node::BLOCKQUOTE => {
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    ctx.write("> ");
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
            let mut num = 1;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    if ordered {
                        ctx.write(&format!("{}. ", num));
                        num += 1;
                    } else {
                        ctx.write("- ");
                    }

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

        node::HORIZONTAL_RULE => {
            ctx.write("* * *\n\n");
        }

        node::DIV => {
            // Check for Markua special blocks
            if let Some(class) = node.props.get_str("class") {
                let prefix = match class {
                    "aside" => "A> ",
                    "blurb" => "B> ",
                    "warning" => "W> ",
                    "tip" => "T> ",
                    "error" => "E> ",
                    "discussion" => "D> ",
                    "question" => "Q> ",
                    "information" => "I> ",
                    _ => "",
                };

                if !prefix.is_empty() {
                    for child in &node.children {
                        if child.kind.as_str() == node::PARAGRAPH {
                            ctx.write(prefix);
                            emit_inline_nodes(&child.children, ctx);
                            ctx.write("\n");
                        } else {
                            emit_node(child, ctx);
                        }
                    }
                    ctx.write("\n");
                    return;
                }
            }
            emit_nodes(&node.children, ctx);
        }

        node::TABLE => {
            // Markua supports GFM tables
            for (row_idx, row) in node.children.iter().enumerate() {
                if row.kind.as_str() == node::TABLE_ROW {
                    ctx.write("|");
                    for cell in &row.children {
                        ctx.write(" ");
                        emit_inline_nodes(&cell.children, ctx);
                        ctx.write(" |");
                    }
                    ctx.write("\n");

                    // Add separator after header row
                    if row_idx == 0 {
                        ctx.write("|");
                        for _ in &row.children {
                            ctx.write(" --- |");
                        }
                        ctx.write("\n");
                    }
                }
            }
            ctx.write("\n");
        }

        node::SPAN | node::FIGURE => emit_nodes(&node.children, ctx),

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
            ctx.write("*");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("*");
        }

        node::STRIKEOUT => {
            ctx.write("~~");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("~~");
        }

        node::UNDERLINE => {
            // Markua doesn't have underline, render as emphasis
            ctx.write("*");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("*");
        }

        node::CODE => {
            let content = node.props.get_str(prop::CONTENT).unwrap_or("");
            // Use double backticks if content contains single backtick
            if content.contains('`') {
                ctx.write("`` ");
                ctx.write(content);
                ctx.write(" ``");
            } else {
                ctx.write("`");
                ctx.write(content);
                ctx.write("`");
            }
            emit_inline_nodes(&node.children, ctx);
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("[");
                if node.children.is_empty() {
                    ctx.write(url);
                } else {
                    emit_inline_nodes(&node.children, ctx);
                }
                ctx.write("](");
                ctx.write(url);
                ctx.write(")");
            } else {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("![");
                if let Some(alt) = node.props.get_str(prop::ALT) {
                    ctx.write(alt);
                }
                ctx.write("](");
                ctx.write(url);
                ctx.write(")");
            }
        }

        node::LINE_BREAK => {
            ctx.write("\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        node::SUBSCRIPT | node::SUPERSCRIPT => {
            // Markua doesn't support these directly
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
        assert!(output.contains("# Title"));
    }

    #[test]
    fn test_emit_heading_level2() {
        let doc = doc(|d| d.heading(2, |h| h.text("Subtitle")));
        let output = emit_str(&doc);
        assert!(output.contains("## Subtitle"));
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
        assert!(output.contains("*italic*"));
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
        assert!(output.contains("[click](https://example.com)"));
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
        assert!(output.contains("1. first"));
        assert!(output.contains("2. second"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block("print hi"));
        let output = emit_str(&doc);
        assert!(output.contains("```"));
        assert!(output.contains("print hi"));
    }

    #[test]
    fn test_emit_code_block_with_language() {
        let doc = doc(|d| d.code_block_lang("puts 'hello'", "ruby"));
        let output = emit_str(&doc);
        assert!(output.contains("```ruby"));
    }

    #[test]
    fn test_emit_blockquote() {
        let doc = doc(|d| d.blockquote(|b| b.para(|p| p.text("quoted"))));
        let output = emit_str(&doc);
        assert!(output.contains("> quoted"));
    }

    #[test]
    fn test_emit_aside() {
        let div = Node::new(node::DIV).prop("class", "aside").children(vec![
            Node::new(node::PARAGRAPH).children(vec![
                Node::new(node::TEXT).prop(prop::CONTENT, "This is an aside."),
            ]),
        ]);
        let root = Node::new(node::DOCUMENT).children(vec![div]);
        let doc = Document::new().with_content(root);
        let output = emit_str(&doc);
        assert!(output.contains("A> This is an aside."));
    }

    #[test]
    fn test_emit_warning() {
        let div = Node::new(node::DIV).prop("class", "warning").children(vec![
            Node::new(node::PARAGRAPH).children(vec![
                Node::new(node::TEXT).prop(prop::CONTENT, "Be careful!"),
            ]),
        ]);
        let root = Node::new(node::DOCUMENT).children(vec![div]);
        let doc = Document::new().with_content(root);
        let output = emit_str(&doc);
        assert!(output.contains("W> Be careful!"));
    }

    #[test]
    fn test_emit_scene_break() {
        let mut root = Node::new(node::DOCUMENT);
        root.children.push(
            Node::new(node::PARAGRAPH)
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, "before")]),
        );
        root.children.push(Node::new(node::HORIZONTAL_RULE));
        root.children.push(
            Node::new(node::PARAGRAPH)
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, "after")]),
        );
        let doc = Document::new().with_content(root);
        let output = emit_str(&doc);
        assert!(output.contains("* * *"));
    }

    #[test]
    fn test_emit_image() {
        let mut root = Node::new(node::DOCUMENT);
        root.children.push(Node::new(node::PARAGRAPH).children(vec![
            Node::new(node::IMAGE)
                .prop(prop::URL, "image.png")
                .prop(prop::ALT, "Alt text"),
        ]));
        let doc = Document::new().with_content(root);
        let output = emit_str(&doc);
        assert!(output.contains("![Alt text](image.png)"));
    }
}
