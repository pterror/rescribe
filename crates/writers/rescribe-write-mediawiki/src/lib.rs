//! MediaWiki writer for rescribe.
//!
//! Emits rescribe's document IR as MediaWiki markup.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_write_mediawiki::emit;
//!
//! let doc = Document::new();
//! let result = emit(&doc)?;
//! let wiki = String::from_utf8(result.value).unwrap();
//! ```

use rescribe_core::{ConversionResult, Document, EmitError, FidelityWarning, Node};
use rescribe_std::{node, prop};

/// Emit a document as MediaWiki markup.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();
    emit_node(&doc.content, &mut ctx);

    let output = ctx.output.trim_end().to_string() + "\n";
    Ok(ConversionResult::with_warnings(
        output.into_bytes(),
        ctx.warnings,
    ))
}

struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
    list_depth: usize,
    list_markers: Vec<char>,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            list_depth: 0,
            list_markers: Vec::new(),
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }
}

fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::DOCUMENT => {
            for child in &node.children {
                emit_node(child, ctx);
            }
        }
        node::PARAGRAPH => {
            emit_inline_children(node, ctx);
            ctx.newline();
            ctx.newline();
        }
        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;
            let markers = "=".repeat(level);
            ctx.write(&markers);
            ctx.write(" ");
            emit_inline_children(node, ctx);
            ctx.write(" ");
            ctx.writeln(&markers);
            ctx.newline();
        }
        node::BLOCKQUOTE => {
            // MediaWiki doesn't have native blockquote, use template or HTML
            ctx.writeln("<blockquote>");
            for child in &node.children {
                emit_node(child, ctx);
            }
            ctx.writeln("</blockquote>");
            ctx.newline();
        }
        node::CODE_BLOCK => {
            let content = node.props.get_str(prop::CONTENT).unwrap_or("");
            let language = node.props.get_str(prop::LANGUAGE);

            if let Some(lang) = language {
                ctx.writeln(&format!("<syntaxhighlight lang=\"{}\">", lang));
            } else {
                ctx.writeln("<pre>");
            }
            ctx.write(content);
            if !content.ends_with('\n') {
                ctx.newline();
            }
            if language.is_some() {
                ctx.writeln("</syntaxhighlight>");
            } else {
                ctx.writeln("</pre>");
            }
            ctx.newline();
        }
        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let marker = if ordered { '#' } else { '*' };
            ctx.list_markers.push(marker);
            ctx.list_depth += 1;

            for child in &node.children {
                emit_node(child, ctx);
            }

            ctx.list_depth -= 1;
            ctx.list_markers.pop();

            if ctx.list_depth == 0 {
                ctx.newline();
            }
        }
        node::LIST_ITEM => {
            // Write markers for nesting
            let markers: String = ctx.list_markers.iter().collect();
            ctx.write(&markers);
            ctx.write(" ");

            // Emit inline content
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_children(child, ctx);
                } else if child.kind.as_str() == node::LIST {
                    ctx.newline();
                    emit_node(child, ctx);
                } else {
                    emit_inline(child, ctx);
                }
            }
            ctx.newline();
        }
        node::HORIZONTAL_RULE => {
            ctx.writeln("----");
            ctx.newline();
        }
        node::TABLE => {
            ctx.writeln("{| class=\"wikitable\"");
            for child in &node.children {
                emit_node(child, ctx);
            }
            ctx.writeln("|}");
            ctx.newline();
        }
        node::TABLE_ROW => {
            ctx.writeln("|-");
            for child in &node.children {
                emit_node(child, ctx);
            }
        }
        node::TABLE_HEADER => {
            ctx.write("! ");
            emit_inline_children(node, ctx);
            ctx.newline();
        }
        node::TABLE_CELL => {
            ctx.write("| ");
            emit_inline_children(node, ctx);
            ctx.newline();
        }
        _ => {
            // Try to emit as inline or recurse into children
            emit_inline(node, ctx);
        }
    }
}

fn emit_inline_children(node: &Node, ctx: &mut EmitContext) {
    for child in &node.children {
        emit_inline(child, ctx);
    }
}

fn emit_inline(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }
        node::EMPHASIS => {
            ctx.write("''");
            emit_inline_children(node, ctx);
            ctx.write("''");
        }
        node::STRONG => {
            ctx.write("'''");
            emit_inline_children(node, ctx);
            ctx.write("'''");
        }
        node::STRIKEOUT => {
            ctx.write("<s>");
            emit_inline_children(node, ctx);
            ctx.write("</s>");
        }
        node::UNDERLINE => {
            ctx.write("<u>");
            emit_inline_children(node, ctx);
            ctx.write("</u>");
        }
        node::SUBSCRIPT => {
            ctx.write("<sub>");
            emit_inline_children(node, ctx);
            ctx.write("</sub>");
        }
        node::SUPERSCRIPT => {
            ctx.write("<sup>");
            emit_inline_children(node, ctx);
            ctx.write("</sup>");
        }
        node::CODE => {
            let content = node.props.get_str(prop::CONTENT).unwrap_or("");
            ctx.write("<code>");
            ctx.write(content);
            ctx.write("</code>");
        }
        node::LINK => {
            let url = node.props.get_str(prop::URL).unwrap_or("");
            let text = extract_text(node);

            // Determine if internal or external link
            if url.starts_with("http://") || url.starts_with("https://") {
                // External link
                if text.is_empty() || text == url {
                    ctx.write(&format!("[{}]", url));
                } else {
                    ctx.write(&format!("[{} {}]", url, text));
                }
            } else {
                // Internal link
                if text.is_empty() || text == url {
                    ctx.write(&format!("[[{}]]", url));
                } else {
                    ctx.write(&format!("[[{}|{}]]", url, text));
                }
            }
        }
        node::IMAGE => {
            let url = node.props.get_str(prop::URL).unwrap_or("");
            let alt = node.props.get_str(prop::ALT).unwrap_or("");
            if alt.is_empty() {
                ctx.write(&format!("[[File:{}]]", url));
            } else {
                ctx.write(&format!("[[File:{}|{}]]", url, alt));
            }
        }
        node::LINE_BREAK => {
            ctx.write("<br/>");
        }
        node::SOFT_BREAK => {
            ctx.write(" ");
        }
        _ => {
            // Try to emit children
            emit_inline_children(node, ctx);
        }
    }
}

fn extract_text(node: &Node) -> String {
    let mut result = String::new();
    extract_text_recursive(node, &mut result);
    result
}

fn extract_text_recursive(node: &Node, output: &mut String) {
    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        output.push_str(content);
    }
    for child in &node.children {
        extract_text_recursive(child, output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::doc;

    #[test]
    fn test_emit_heading() {
        let document = doc(|d| d.heading(2, |i| i.text("Title")));
        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("== Title =="));
    }

    #[test]
    fn test_emit_bold() {
        let document = doc(|d| d.para(|i| i.strong(|i| i.text("bold"))));
        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("'''bold'''"));
    }

    #[test]
    fn test_emit_italic() {
        let document = doc(|d| d.para(|i| i.em(|i| i.text("italic"))));
        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("''italic''"));
    }

    #[test]
    fn test_emit_list() {
        let document =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));
        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("* Item 1"));
        assert!(output.contains("* Item 2"));
    }

    #[test]
    fn test_emit_link() {
        let document = doc(|d| d.para(|i| i.link("https://example.com", |i| i.text("Example"))));
        let result = emit(&document).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("[https://example.com Example]"));
    }
}
