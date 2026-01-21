//! POD (Plain Old Documentation) writer for rescribe.
//!
//! Emits documents as Perl POD markup.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as POD markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as POD markup with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    ctx.write("=pod\n\n");
    emit_nodes(&doc.content.children, &mut ctx);
    ctx.write("=cut\n");

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
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1).clamp(1, 6);
            ctx.write(&format!("=head{} ", level));
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::CODE_BLOCK => {
            // Verbatim paragraphs need 4-space indentation
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                for line in content.lines() {
                    ctx.write("    ");
                    ctx.write(line);
                    ctx.write("\n");
                }
            }
            ctx.write("\n");
        }

        node::BLOCKQUOTE => {
            // POD doesn't have native blockquote, use indentation
            for child in &node.children {
                emit_node(child, ctx);
            }
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            ctx.write("=over 4\n\n");

            let mut num = 1;
            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    if ordered {
                        ctx.write(&format!("=item {}. ", num));
                        num += 1;
                    } else {
                        ctx.write("=item * ");
                    }

                    // Emit first paragraph inline with =item
                    let mut first = true;
                    for item_child in &child.children {
                        if first && item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, ctx);
                            ctx.write("\n\n");
                            first = false;
                        } else {
                            emit_node(item_child, ctx);
                        }
                    }
                }
            }

            ctx.write("=back\n\n");
        }

        node::LIST_ITEM => {
            emit_nodes(&node.children, ctx);
        }

        node::HORIZONTAL_RULE => {
            // POD doesn't have native horizontal rule
            ctx.write("\n");
        }

        node::TABLE => {
            // POD doesn't have native tables, render as verbatim
            ctx.write("    ");
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    for (i, cell) in row.children.iter().enumerate() {
                        if i > 0 {
                            ctx.write(" | ");
                        }
                        let mut cell_text = String::new();
                        collect_text(&cell.children, &mut cell_text);
                        ctx.write(&cell_text);
                    }
                    ctx.write("\n    ");
                }
            }
            ctx.write("\n");
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
                // Escape < and > in plain text
                let escaped = content.replace('<', "E<lt>").replace('>', "E<gt>");
                ctx.write(&escaped);
            }
        }

        node::STRONG => {
            ctx.write("B<");
            emit_inline_nodes(&node.children, ctx);
            ctx.write(">");
        }

        node::EMPHASIS => {
            ctx.write("I<");
            emit_inline_nodes(&node.children, ctx);
            ctx.write(">");
        }

        node::UNDERLINE => {
            ctx.write("U<");
            emit_inline_nodes(&node.children, ctx);
            ctx.write(">");
        }

        node::STRIKEOUT => {
            // POD doesn't have strikethrough
            emit_inline_nodes(&node.children, ctx);
        }

        node::CODE => {
            let content = node.props.get_str(prop::CONTENT).unwrap_or("");
            // Use double brackets if content contains > or <
            if content.contains('>') || content.contains('<') {
                ctx.write("C<< ");
                ctx.write(content);
                ctx.write(" >>");
            } else {
                ctx.write("C<");
                ctx.write(content);
                ctx.write(">");
            }
            // Handle any children
            if !node.children.is_empty() {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                // Check if we have label text
                let mut label_text = String::new();
                collect_text(&node.children, &mut label_text);

                if label_text.is_empty() || label_text == url {
                    ctx.write("L<");
                    ctx.write(url);
                    ctx.write(">");
                } else {
                    ctx.write("L<");
                    ctx.write(&label_text);
                    ctx.write("|");
                    ctx.write(url);
                    ctx.write(">");
                }
            } else {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::IMAGE => {
            // POD doesn't support images
            if let Some(alt) = node.props.get_str(prop::ALT) {
                ctx.write("[Image: ");
                ctx.write(alt);
                ctx.write("]");
            }
        }

        node::SUBSCRIPT | node::SUPERSCRIPT => {
            // POD doesn't support sub/superscript
            emit_inline_nodes(&node.children, ctx);
        }

        node::LINE_BREAK => {
            ctx.write("\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        _ => emit_inline_nodes(&node.children, ctx),
    }
}

fn collect_text(nodes: &[Node], output: &mut String) {
    for node in nodes {
        if node.kind.as_str() == node::TEXT
            && let Some(content) = node.props.get_str(prop::CONTENT)
        {
            output.push_str(content);
        }
        collect_text(&node.children, output);
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
        let doc = doc(|d| d.heading(1, |h| h.text("NAME")));
        let output = emit_str(&doc);
        assert!(output.contains("=head1 NAME"));
    }

    #[test]
    fn test_emit_heading_level2() {
        let doc = doc(|d| d.heading(2, |h| h.text("DESCRIPTION")));
        let output = emit_str(&doc);
        assert!(output.contains("=head2 DESCRIPTION"));
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
        assert!(output.contains("B<bold>"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("I<italic>"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("$var")));
        let output = emit_str(&doc);
        assert!(output.contains("C<$var>"));
    }

    #[test]
    fn test_emit_code_with_angle_brackets() {
        let doc = doc(|d| d.para(|p| p.code("$a <=> $b")));
        let output = emit_str(&doc);
        assert!(output.contains("C<< $a <=> $b >>"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("perlpod", |l| l.text("perlpod"))));
        let output = emit_str(&doc);
        assert!(output.contains("L<perlpod>"));
    }

    #[test]
    fn test_emit_link_with_label() {
        let doc = doc(|d| d.para(|p| p.link("perlpod", |l| l.text("documentation"))));
        let output = emit_str(&doc);
        assert!(output.contains("L<documentation|perlpod>"));
    }

    #[test]
    fn test_emit_unordered_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("=over"));
        assert!(output.contains("=item * one"));
        assert!(output.contains("=item * two"));
        assert!(output.contains("=back"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc =
            doc(|d| d.ordered_list(|l| l.item(|i| i.text("first")).item(|i| i.text("second"))));
        let output = emit_str(&doc);
        assert!(output.contains("=item 1."));
        assert!(output.contains("=item 2."));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block("print 'Hello';"));
        let output = emit_str(&doc);
        assert!(output.contains("    print 'Hello';"));
    }

    #[test]
    fn test_emit_pod_cut() {
        let doc = doc(|d| d.para(|p| p.text("Content")));
        let output = emit_str(&doc);
        assert!(output.starts_with("=pod"));
        assert!(output.ends_with("=cut\n"));
    }
}
