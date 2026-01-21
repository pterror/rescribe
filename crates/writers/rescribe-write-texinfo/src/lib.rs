//! Texinfo writer for rescribe.
//!
//! Serializes rescribe's document IR to GNU Texinfo format.
//!
//! # Example
//!
//! ```
//! use rescribe_write_texinfo::emit;
//! use rescribe_core::{Document, Node, Properties};
//!
//! let doc = Document {
//!     content: Node::new("document"),
//!     resources: Default::default(),
//!     metadata: Properties::new(),
//!     source: None,
//! };
//!
//! let result = emit(&doc).unwrap();
//! let texinfo = String::from_utf8(result.value).unwrap();
//! ```

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to Texinfo format.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to Texinfo format with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Write header
    ctx.write("\\input texinfo\n");
    ctx.write("@setfilename output.info\n");

    // Write title if present
    if let Some(title) = doc.metadata.get_str("title") {
        ctx.write("@settitle ");
        ctx.write(title);
        ctx.write("\n");
    }

    ctx.write("\n@node Top\n");

    if let Some(title) = doc.metadata.get_str("title") {
        ctx.write("@top ");
        ctx.write(title);
        ctx.write("\n\n");
    }

    // Write content
    emit_nodes(&doc.content.children, &mut ctx);

    // Write footer
    ctx.write("\n@bye\n");

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
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            let command = match level {
                1 => "@chapter",
                2 => "@section",
                3 => "@subsection",
                4 => "@subsubsection",
                _ => "@subsubsection",
            };

            ctx.write(command);
            ctx.write(" ");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::CODE_BLOCK => {
            ctx.write("@example\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
                if !content.ends_with('\n') {
                    ctx.write("\n");
                }
            }
            ctx.write("@end example\n\n");
        }

        node::BLOCKQUOTE => {
            ctx.write("@quotation\n");
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, ctx);
                    ctx.write("\n");
                } else {
                    emit_node(child, ctx);
                }
            }
            ctx.write("@end quotation\n\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            if ordered {
                ctx.write("@enumerate\n");
            } else {
                ctx.write("@itemize @bullet\n");
            }

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    ctx.write("@item ");
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, ctx);
                        } else {
                            emit_inline_node(item_child, ctx);
                        }
                    }
                    ctx.write("\n");
                }
            }

            if ordered {
                ctx.write("@end enumerate\n\n");
            } else {
                ctx.write("@end itemize\n\n");
            }
        }

        node::LIST_ITEM => {
            emit_nodes(&node.children, ctx);
        }

        node::DEFINITION_LIST => {
            ctx.write("@table @asis\n");

            let mut i = 0;
            while i < node.children.len() {
                let child = &node.children[i];

                if child.kind.as_str() == node::DEFINITION_TERM {
                    ctx.write("@item ");
                    emit_inline_nodes(&child.children, ctx);
                    ctx.write("\n");
                } else if child.kind.as_str() == node::DEFINITION_DESC {
                    for desc_child in &child.children {
                        if desc_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&desc_child.children, ctx);
                            ctx.write("\n");
                        } else {
                            emit_node(desc_child, ctx);
                        }
                    }
                }

                i += 1;
            }

            ctx.write("@end table\n\n");
        }

        node::TABLE => {
            ctx.write("@multitable @columnfractions");
            // Estimate column count from first row
            if let Some(first_row) = node.children.first() {
                let col_count = first_row.children.len();
                if col_count > 0 {
                    let frac = 1.0 / col_count as f64;
                    for _ in 0..col_count {
                        ctx.write(&format!(" {:.2}", frac));
                    }
                }
            }
            ctx.write("\n");

            for (row_idx, row) in node.children.iter().enumerate() {
                if row.kind.as_str() == node::TABLE_ROW {
                    if row_idx == 0 {
                        // Header row
                        ctx.write("@headitem ");
                    } else {
                        ctx.write("@item ");
                    }

                    for (cell_idx, cell) in row.children.iter().enumerate() {
                        if cell_idx > 0 {
                            ctx.write(" @tab ");
                        }
                        emit_inline_nodes(&cell.children, ctx);
                    }
                    ctx.write("\n");
                }
            }

            ctx.write("@end multitable\n\n");
        }

        node::HORIZONTAL_RULE => {
            ctx.write("\n@sp 1\n@noindent\n@center * * *\n@sp 1\n\n");
        }

        node::DIV | node::SPAN | node::FIGURE => {
            emit_nodes(&node.children, ctx);
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                // Remove extension for texinfo
                let base = url.rsplit_once('.').map(|(b, _)| b).unwrap_or(url);
                ctx.write("@image{");
                ctx.write(base);
                ctx.write("}\n\n");
            }
        }

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
                // Escape special characters
                for c in content.chars() {
                    match c {
                        '@' => ctx.write("@@"),
                        '{' => ctx.write("@{"),
                        '}' => ctx.write("@}"),
                        _ => ctx.output.push(c),
                    }
                }
            }
        }

        node::STRONG => {
            ctx.write("@strong{");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::EMPHASIS => {
            ctx.write("@emph{");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::CODE => {
            ctx.write("@code{");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                // Escape special characters in code
                for c in content.chars() {
                    match c {
                        '@' => ctx.write("@@"),
                        '{' => ctx.write("@{"),
                        '}' => ctx.write("@}"),
                        _ => ctx.output.push(c),
                    }
                }
            }
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                if url.starts_with("mailto:") {
                    let email = url.strip_prefix("mailto:").unwrap_or(url);
                    ctx.write("@email{");
                    ctx.write(email);
                    if !node.children.is_empty() {
                        ctx.write(", ");
                        emit_inline_nodes(&node.children, ctx);
                    }
                    ctx.write("}");
                } else if url.starts_with('#') {
                    // Internal reference
                    let node_name = url.strip_prefix('#').unwrap_or(url);
                    ctx.write("@ref{");
                    ctx.write(node_name);
                    ctx.write("}");
                } else {
                    ctx.write("@uref{");
                    ctx.write(url);
                    if !node.children.is_empty() {
                        ctx.write(", ");
                        emit_inline_nodes(&node.children, ctx);
                    }
                    ctx.write("}");
                }
            } else {
                emit_inline_nodes(&node.children, ctx);
            }
        }

        node::SUBSCRIPT => {
            ctx.write("@sub{");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::SUPERSCRIPT => {
            ctx.write("@sup{");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::STRIKEOUT => {
            // Texinfo doesn't have strikeout, use emphasis as fallback
            ctx.write("@emph{");
            emit_inline_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::LINE_BREAK => {
            ctx.write("@*\n");
        }

        node::SOFT_BREAK => {
            ctx.write(" ");
        }

        node::FOOTNOTE_DEF => {
            ctx.write("@footnote{");
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, ctx);
                } else {
                    emit_inline_node(child, ctx);
                }
            }
            ctx.write("}");
        }

        _ => emit_inline_nodes(&node.children, ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::Properties;
    use rescribe_std::builder::*;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_empty() {
        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata: Properties::new(),
            source: None,
        };

        let output = emit_str(&doc);
        assert!(output.contains("\\input texinfo"));
        assert!(output.contains("@bye"));
    }

    #[test]
    fn test_emit_with_title() {
        let mut metadata = Properties::new();
        metadata.set("title", "Test Document".to_string());

        let doc = Document {
            content: Node::new(node::DOCUMENT),
            resources: Default::default(),
            metadata,
            source: None,
        };

        let output = emit_str(&doc);
        assert!(output.contains("@settitle Test Document"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = doc(|d| d.heading(1, |h| h.text("Chapter Title")));
        let output = emit_str(&doc);
        assert!(output.contains("@chapter Chapter Title"));
    }

    #[test]
    fn test_emit_section() {
        let doc = doc(|d| d.heading(2, |h| h.text("Section Title")));
        let output = emit_str(&doc);
        assert!(output.contains("@section Section Title"));
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = doc(|d| d.para(|p| p.text("Hello, world!")));
        let output = emit_str(&doc);
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = doc(|d| d.para(|p| p.em(|e| e.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("@emph{italic}"));
    }

    #[test]
    fn test_emit_strong() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("@strong{bold}"));
    }

    #[test]
    fn test_emit_code() {
        let doc = doc(|d| d.para(|p| p.code("printf")));
        let output = emit_str(&doc);
        assert!(output.contains("@code{printf}"));
    }

    #[test]
    fn test_emit_link() {
        let doc = doc(|d| d.para(|p| p.link("https://example.com", |l| l.text("Example"))));
        let output = emit_str(&doc);
        assert!(output.contains("@uref{https://example.com, Example}"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("one")).item(|i| i.text("two"))));
        let output = emit_str(&doc);
        assert!(output.contains("@itemize @bullet"));
        assert!(output.contains("@item one"));
        assert!(output.contains("@item two"));
        assert!(output.contains("@end itemize"));
    }

    #[test]
    fn test_emit_enumerate() {
        let doc =
            doc(|d| d.ordered_list(|l| l.item(|i| i.text("first")).item(|i| i.text("second"))));
        let output = emit_str(&doc);
        assert!(output.contains("@enumerate"));
        assert!(output.contains("@item first"));
        assert!(output.contains("@end enumerate"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = doc(|d| d.code_block("int main() {}"));
        let output = emit_str(&doc);
        assert!(output.contains("@example"));
        assert!(output.contains("int main() {}"));
        assert!(output.contains("@end example"));
    }

    #[test]
    fn test_emit_blockquote() {
        let doc = doc(|d| d.blockquote(|b| b.para(|p| p.text("Quoted text"))));
        let output = emit_str(&doc);
        assert!(output.contains("@quotation"));
        assert!(output.contains("Quoted text"));
        assert!(output.contains("@end quotation"));
    }

    #[test]
    fn test_escape_special_chars() {
        let doc = doc(|d| d.para(|p| p.text("Use @{braces}")));
        let output = emit_str(&doc);
        // @ -> @@, { -> @{, } -> @}
        assert!(output.contains("Use @@@{braces@}"));
    }
}
