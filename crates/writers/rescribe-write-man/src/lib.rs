//! Man page (roff/troff) writer for rescribe.
//!
//! Emits documents as Unix man page format using common macros.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document as man page format.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as man page format with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Write title header if available
    let title = doc.metadata.get_str("title").unwrap_or("UNTITLED");
    let section = doc.metadata.get_str("man:section").unwrap_or("1");
    ctx.write(&format!(".TH {} {} ", title.to_uppercase(), section));
    ctx.write("\"\" \"\" \"\"\n");

    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::ok(ctx.output.into_bytes()))
}

struct EmitContext {
    output: String,
    in_paragraph: bool,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            in_paragraph: false,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        if !self.output.ends_with('\n') {
            self.write("\n");
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
            ctx.newline();

            // Level 1 is document title (already handled), 2 is .SH, 3+ is .SS
            let macro_name = if level <= 2 { ".SH" } else { ".SS" };
            ctx.write(macro_name);
            ctx.write(" ");

            // Emit text in uppercase for sections
            let text = extract_text(&node.children);
            ctx.write(&text.to_uppercase());
            ctx.write("\n");
        }

        node::PARAGRAPH => {
            ctx.newline();
            ctx.write(".PP\n");
            ctx.in_paragraph = true;
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n");
            ctx.in_paragraph = false;
        }

        node::CODE_BLOCK => {
            ctx.newline();
            ctx.write(".nf\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                // Escape special characters in code
                for line in content.lines() {
                    // Lines starting with . need escaping
                    if line.starts_with('.') {
                        ctx.write("\\&");
                    }
                    ctx.write(line);
                    ctx.write("\n");
                }
            }
            ctx.write(".fi\n");
        }

        node::BLOCKQUOTE => {
            ctx.newline();
            ctx.write(".RS\n");
            emit_nodes(&node.children, ctx);
            ctx.write(".RE\n");
        }

        node::LIST => {
            ctx.newline();
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            for (i, child) in node.children.iter().enumerate() {
                if ordered {
                    ctx.write(&format!(".IP {}.\n", i + 1));
                } else {
                    ctx.write(".IP \\(bu\n");
                }
                emit_list_item_content(child, ctx);
            }
        }

        node::LIST_ITEM => {
            // Handled by LIST
            emit_nodes(&node.children, ctx);
        }

        node::DEFINITION_LIST => {
            for child in &node.children {
                emit_node(child, ctx);
            }
        }

        node::DEFINITION_TERM => {
            ctx.newline();
            ctx.write(".TP\n");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n");
        }

        node::DEFINITION_DESC => {
            // Content follows the .TP term
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, ctx);
                    ctx.write("\n");
                } else {
                    emit_node(child, ctx);
                }
            }
        }

        node::HORIZONTAL_RULE => {
            ctx.newline();
            ctx.write(".sp\n");
        }

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::FIGURE => emit_nodes(&node.children, ctx),

        // Block-level inline elements
        node::TEXT | node::STRONG | node::EMPHASIS | node::CODE | node::LINK => {
            ctx.newline();
            ctx.write(".PP\n");
            emit_inline_node(node, ctx);
            ctx.write("\n");
        }

        _ => emit_nodes(&node.children, ctx),
    }
}

fn emit_list_item_content(node: &Node, ctx: &mut EmitContext) {
    if node.kind.as_str() == node::LIST_ITEM {
        for child in &node.children {
            if child.kind.as_str() == node::PARAGRAPH {
                emit_inline_nodes(&child.children, ctx);
                ctx.write("\n");
            } else {
                emit_node(child, ctx);
            }
        }
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
                // Escape special characters
                let escaped = escape_man(content);
                ctx.write(&escaped);
            }
        }

        node::STRONG => {
            ctx.write("\\fB");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\\fR");
        }

        node::EMPHASIS => {
            ctx.write("\\fI");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\\fR");
        }

        node::CODE => {
            // Bold for code
            ctx.write("\\fB");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(&escape_man(content));
            }
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\\fR");
        }

        node::LINK => {
            // Show URL in parentheses after text
            emit_inline_nodes(&node.children, ctx);
            if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write(" (");
                ctx.write(&escape_man(url));
                ctx.write(")");
            }
        }

        node::SUBSCRIPT | node::SUPERSCRIPT => {
            // No native support, just emit text
            emit_inline_nodes(&node.children, ctx);
        }

        node::LINE_BREAK => {
            ctx.write("\n.br\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        node::IMAGE => {
            // No native image support, show alt text or URL
            if let Some(alt) = node.props.get_str(prop::ALT) {
                ctx.write("[Image: ");
                ctx.write(&escape_man(alt));
                ctx.write("]");
            } else if let Some(url) = node.props.get_str(prop::URL) {
                ctx.write("[Image: ");
                ctx.write(&escape_man(url));
                ctx.write("]");
            }
        }

        _ => emit_inline_nodes(&node.children, ctx),
    }
}

fn escape_man(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '-' => result.push_str("\\-"),
            _ => result.push(c),
        }
    }
    result
}

fn extract_text(nodes: &[Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        if node.kind.as_str() == node::TEXT
            && let Some(content) = node.props.get_str(prop::CONTENT)
        {
            text.push_str(content);
        }
        text.push_str(&extract_text(&node.children));
    }
    text
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
    fn test_emit_basic() {
        let doc = doc(|d| d.para(|p| p.text("Hello, world!")));
        let output = emit_str(&doc);
        assert!(output.contains(".TH"));
        assert!(output.contains(".PP"));
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(2, |h| h.text("Section Title")));
        let output = emit_str(&doc);
        assert!(output.contains(".SH SECTION TITLE"));
    }

    #[test]
    fn test_emit_subsection() {
        let doc = doc(|d| d.heading(3, |h| h.text("Subsection")));
        let output = emit_str(&doc);
        assert!(output.contains(".SS SUBSECTION"));
    }

    #[test]
    fn test_emit_bold() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\fBbold\\fR"));
    }

    #[test]
    fn test_emit_italic() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\fIitalic\\fR"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block("fn main() {}"));
        let output = emit_str(&doc);
        assert!(output.contains(".nf"));
        assert!(output.contains("fn main() {}"));
        assert!(output.contains(".fi"));
    }

    #[test]
    fn test_emit_list() {
        let doc =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));
        let output = emit_str(&doc);
        assert!(output.contains(".IP \\(bu"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
    }
}
