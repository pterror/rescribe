//! RTF (Rich Text Format) writer for rescribe.
//!
//! Emits documents as RTF format.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as RTF.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as RTF with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Write RTF header
    ctx.write(r"{\rtf1\ansi\deff0");
    ctx.write(r"{\fonttbl{\f0 Times New Roman;}}");
    ctx.write("\n");

    emit_nodes(&doc.content.children, &mut ctx);

    ctx.write("}");

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

    fn write_escaped(&mut self, s: &str) {
        for ch in s.chars() {
            match ch {
                '\\' => self.write("\\\\"),
                '{' => self.write("\\{"),
                '}' => self.write("\\}"),
                '\t' => self.write("\\tab "),
                '\n' => self.write("\\line "),
                '\u{00A0}' => self.write("\\~"), // non-breaking space
                '\u{2014}' => self.write("\\emdash "),
                '\u{2013}' => self.write("\\endash "),
                '\u{2018}' => self.write("\\lquote "),
                '\u{2019}' => self.write("\\rquote "),
                '\u{201C}' => self.write("\\ldblquote "),
                '\u{201D}' => self.write("\\rdblquote "),
                '\u{2022}' => self.write("\\bullet "),
                c if c.is_ascii() => self.output.push(c),
                c => {
                    // Non-ASCII: use Unicode escape
                    let code = c as u32;
                    if code <= 0x7FFF {
                        self.write(&format!("\\u{}?", code as i16));
                    } else {
                        // For characters > 0x7FFF, use negative value
                        self.write(&format!("\\u{}?", code as i16));
                    }
                }
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
        node::DOCUMENT => emit_nodes(&node.children, ctx),

        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            // Use font size based on heading level
            let size = match level {
                1 => 48, // 24pt
                2 => 40, // 20pt
                3 => 32, // 16pt
                4 => 28, // 14pt
                _ => 24, // 12pt
            };
            ctx.write(&format!("\\pard\\fs{} \\b ", size));
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\\b0\\par\n");
        }

        node::PARAGRAPH => {
            ctx.write("\\pard\\fs24 ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\\par\n");
        }

        node::CODE_BLOCK => {
            ctx.write("\\pard\\f1\\fs20 ");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                for line in content.lines() {
                    ctx.write_escaped(line);
                    ctx.write("\\line ");
                }
            }
            ctx.write("\\f0\\par\n");
        }

        node::BLOCKQUOTE => {
            ctx.write("\\pard\\li720 "); // indent 720 twips (0.5 inch)
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, ctx);
                } else {
                    emit_node(child, ctx);
                }
            }
            ctx.write("\\par\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let mut num = 1;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    ctx.write("\\pard\\li720\\fi-360 ");
                    if ordered {
                        ctx.write(&format!("{}. ", num));
                        num += 1;
                    } else {
                        ctx.write("\\bullet  ");
                    }

                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, ctx);
                        } else {
                            emit_node(item_child, ctx);
                        }
                    }
                    ctx.write("\\par\n");
                }
            }
        }

        node::LIST_ITEM => {
            emit_nodes(&node.children, ctx);
        }

        node::TABLE => {
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    ctx.write("\\trowd ");

                    // Define cell positions
                    let _cell_count = row.children.len();
                    for (i, _) in row.children.iter().enumerate() {
                        let right = (i + 1) * 2000; // 2000 twips per cell
                        ctx.write(&format!("\\cellx{}", right));
                    }

                    for cell in &row.children {
                        ctx.write("\\pard\\intbl ");
                        emit_inline_nodes(&cell.children, ctx);
                        ctx.write("\\cell ");
                    }
                    ctx.write("\\row\n");
                }
            }
        }

        node::HORIZONTAL_RULE => {
            ctx.write("\\pard\\brdrb\\brdrs\\brdrw10\\brsp20 \\par\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::FIGURE => emit_nodes(&node.children, ctx),

        // Inline nodes at block level
        node::TEXT | node::STRONG | node::EMPHASIS | node::CODE | node::LINK => {
            ctx.write("\\pard ");
            emit_inline_node(node, ctx);
            ctx.write("\\par\n");
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
                ctx.write_escaped(content);
            }
        }

        node::STRONG => {
            ctx.write("{\\b ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::EMPHASIS => {
            ctx.write("{\\i ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::UNDERLINE => {
            ctx.write("{\\ul ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::STRIKEOUT => {
            ctx.write("{\\strike ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::CODE => {
            ctx.write("{\\f1 ");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write_escaped(content);
            }
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::LINK => {
            // RTF hyperlinks are complex; just emit the text
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("{\\field{\\*\\fldinst HYPERLINK \"");
                ctx.write(url);
                ctx.write("\"}{\\fldrslt ");
                if node.children.is_empty() {
                    ctx.write_escaped(url);
                } else {
                    emit_inline_nodes(&node.children, ctx);
                }
                ctx.write("}}");
            } else {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::IMAGE => {
            // RTF images are binary and complex; skip for now
            if let Some(alt) = node.props.get_str(prop::ALT) {
                ctx.write("[Image: ");
                ctx.write_escaped(alt);
                ctx.write("]");
            } else if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("[Image: ");
                ctx.write_escaped(url);
                ctx.write("]");
            }
        }

        node::LINE_BREAK => {
            ctx.write("\\line ");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        node::SUPERSCRIPT => {
            ctx.write("{\\super ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::SUBSCRIPT => {
            ctx.write("{\\sub ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
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
    fn test_emit_rtf_header() {
        let doc = doc(|d| d.para(|p| p.text("Hello")));
        let output = emit_str(&doc);
        assert!(output.starts_with("{\\rtf1"));
        assert!(output.ends_with("}"));
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = doc(|d| d.para(|p| p.text("Hello, world!")));
        let output = emit_str(&doc);
        assert!(output.contains("Hello, world!"));
        assert!(output.contains("\\par"));
    }

    #[test]
    fn test_emit_bold() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("{\\b bold}"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("{\\i italic}"));
    }

    #[test]
    fn test_emit_underline() {
        let doc = doc(|d| d.para(|p| p.underline(|u| u.text("underlined"))));
        let output = emit_str(&doc);
        assert!(output.contains("{\\ul underlined}"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(1, |h| h.text("Title")));
        let output = emit_str(&doc);
        assert!(output.contains("\\b "));
        assert!(output.contains("Title"));
    }

    #[test]
    fn test_emit_escaped_chars() {
        let doc = doc(|d| d.para(|p| p.text("Open { and close }")));
        let output = emit_str(&doc);
        assert!(output.contains("\\{"));
        assert!(output.contains("\\}"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("http://example.com", |l| l.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("HYPERLINK"));
        assert!(output.contains("http://example.com"));
        assert!(output.contains("click"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\bullet"));
        assert!(output.contains("one"));
        assert!(output.contains("two"));
    }
}
