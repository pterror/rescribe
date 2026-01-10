//! HTML writer for rescribe.
//!
//! Emits rescribe's document IR as HTML5.

pub mod builder;

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, ResourceId,
    ResourceMap, Severity, WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as HTML with custom options.
pub fn emit_with_options(
    doc: &Document,
    options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new(&doc.resources, options.pretty);

    // Emit children of the root document node
    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit a document as a complete HTML document with doctype.
pub fn emit_full_document(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_full_document_with_options(doc, &EmitOptions::default())
}

/// Emit a document as a complete HTML document with doctype and options.
pub fn emit_full_document_with_options(
    doc: &Document,
    options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new(&doc.resources, options.pretty);

    if ctx.pretty {
        ctx.write("<!DOCTYPE html>\n<html>\n<head>\n  <meta charset=\"utf-8\">\n</head>\n<body>\n");
    } else {
        ctx.write("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n</head>\n<body>\n");
    }
    emit_nodes(&doc.content.children, &mut ctx);
    ctx.write("\n</body>\n</html>\n");

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit context for tracking state during emission.
struct EmitContext<'a> {
    output: String,
    warnings: Vec<FidelityWarning>,
    resources: &'a ResourceMap,
    pretty: bool,
    indent: usize,
}

impl<'a> EmitContext<'a> {
    fn new(resources: &'a ResourceMap, pretty: bool) -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            resources,
            pretty,
            indent: 0,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    /// Write a newline and indentation (only in pretty mode).
    fn newline(&mut self) {
        if self.pretty {
            self.output.push('\n');
            for _ in 0..self.indent {
                self.output.push_str("  ");
            }
        }
    }

    /// Increase indentation level.
    fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decrease indentation level.
    fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }
}

/// Check if a node kind is a block element.
fn is_block_node(kind: &str) -> bool {
    matches!(
        kind,
        node::PARAGRAPH
            | node::HEADING
            | node::CODE_BLOCK
            | node::BLOCKQUOTE
            | node::LIST
            | node::LIST_ITEM
            | node::TABLE
            | node::TABLE_HEAD
            | node::TABLE_BODY
            | node::TABLE_FOOT
            | node::TABLE_ROW
            | node::FIGURE
            | node::HORIZONTAL_RULE
            | node::DIV
            | node::RAW_BLOCK
            | node::DEFINITION_LIST
            | node::DEFINITION_TERM
            | node::DEFINITION_DESC
            | node::FOOTNOTE_DEF
    )
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
        node::PARAGRAPH => emit_block_tag("p", node, ctx),
        node::HEADING => emit_heading(node, ctx),
        node::CODE_BLOCK => emit_code_block(node, ctx),
        node::BLOCKQUOTE => emit_block_tag("blockquote", node, ctx),
        node::LIST => emit_list(node, ctx),
        node::LIST_ITEM => emit_block_tag("li", node, ctx),
        node::TABLE => emit_block_tag("table", node, ctx),
        node::TABLE_HEAD => emit_block_tag("thead", node, ctx),
        node::TABLE_BODY => emit_block_tag("tbody", node, ctx),
        node::TABLE_FOOT => emit_block_tag("tfoot", node, ctx),
        node::TABLE_ROW => emit_block_tag("tr", node, ctx),
        node::TABLE_CELL => emit_table_cell(node, "td", ctx),
        node::TABLE_HEADER => emit_table_cell(node, "th", ctx),
        node::FIGURE => emit_block_tag("figure", node, ctx),
        node::CAPTION => emit_block_tag("figcaption", node, ctx),
        node::HORIZONTAL_RULE => {
            ctx.newline();
            ctx.write("<hr>");
        }
        node::DIV => emit_div(node, ctx),
        node::RAW_BLOCK => emit_raw(node, ctx),
        node::DEFINITION_LIST => emit_block_tag("dl", node, ctx),
        node::DEFINITION_TERM => emit_block_tag("dt", node, ctx),
        node::DEFINITION_DESC => emit_block_tag("dd", node, ctx),
        node::TEXT => emit_text(node, ctx),
        node::EMPHASIS => emit_inline_tag("em", node, ctx),
        node::STRONG => emit_inline_tag("strong", node, ctx),
        node::STRIKEOUT => emit_inline_tag("del", node, ctx),
        node::UNDERLINE => emit_inline_tag("u", node, ctx),
        node::SUBSCRIPT => emit_inline_tag("sub", node, ctx),
        node::SUPERSCRIPT => emit_inline_tag("sup", node, ctx),
        node::CODE => emit_inline_code(node, ctx),
        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("<br>"),
        node::SOFT_BREAK => ctx.write("\n"),
        node::SPAN => emit_span(node, ctx),
        node::RAW_INLINE => emit_raw(node, ctx),
        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),
        node::SMALL_CAPS => emit_inline_tag("small", node, ctx),
        node::QUOTED => emit_quoted(node, ctx),
        "math_inline" => emit_math_inline(node, ctx),
        "math_display" => emit_math_display(node, ctx),
        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type: {}", node.kind.as_str()),
            ));
            // Try to emit children
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a block-level tag with children (adds newlines in pretty mode).
fn emit_block_tag(tag: &str, node: &Node, ctx: &mut EmitContext) {
    ctx.newline();
    ctx.write("<");
    ctx.write(tag);
    emit_common_attrs(node, ctx);
    ctx.write(">");

    // Check if children are all inline
    let has_block_children = node.children.iter().any(|c| is_block_node(c.kind.as_str()));

    if has_block_children {
        ctx.indent();
        emit_nodes(&node.children, ctx);
        ctx.dedent();
        ctx.newline();
    } else {
        emit_nodes(&node.children, ctx);
    }

    ctx.write("</");
    ctx.write(tag);
    ctx.write(">");
}

/// Emit an inline tag with children (no newlines).
fn emit_inline_tag(tag: &str, node: &Node, ctx: &mut EmitContext) {
    ctx.write("<");
    ctx.write(tag);
    emit_common_attrs(node, ctx);
    ctx.write(">");
    emit_nodes(&node.children, ctx);
    ctx.write("</");
    ctx.write(tag);
    ctx.write(">");
}

/// Emit common attributes (id, class).
fn emit_common_attrs(node: &Node, ctx: &mut EmitContext) {
    if let Some(id) = node.props.get_str(prop::ID) {
        ctx.write(" id=\"");
        ctx.write(&escape_attr(id));
        ctx.write("\"");
    }
    if let Some(classes) = node.props.get_str(prop::CLASSES) {
        ctx.write(" class=\"");
        ctx.write(&escape_attr(classes));
        ctx.write("\"");
    }
}

/// Emit a heading element.
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
    let tag = match level {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        _ => "h6",
    };
    emit_block_tag(tag, node, ctx);
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    ctx.newline();
    ctx.write("<pre><code");

    if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
        ctx.write(" class=\"language-");
        ctx.write(&escape_attr(lang));
        ctx.write("\"");
    }

    ctx.write(">");

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(&escape_html(content));
    }

    ctx.write("</code></pre>");
}

/// Emit a list.
fn emit_list(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
    let tag = if ordered { "ol" } else { "ul" };

    ctx.newline();
    ctx.write("<");
    ctx.write(tag);

    if ordered
        && let Some(start) = node.props.get_int(prop::START)
        && start != 1
    {
        ctx.write(" start=\"");
        ctx.write(&start.to_string());
        ctx.write("\"");
    }

    ctx.write(">");
    ctx.indent();
    emit_nodes(&node.children, ctx);
    ctx.dedent();
    ctx.newline();
    ctx.write("</");
    ctx.write(tag);
    ctx.write(">");
}

/// Emit a table cell.
fn emit_table_cell(node: &Node, tag: &str, ctx: &mut EmitContext) {
    ctx.newline();
    ctx.write("<");
    ctx.write(tag);

    if let Some(colspan) = node.props.get_int(prop::COLSPAN)
        && colspan > 1
    {
        ctx.write(" colspan=\"");
        ctx.write(&colspan.to_string());
        ctx.write("\"");
    }

    if let Some(rowspan) = node.props.get_int(prop::ROWSPAN)
        && rowspan > 1
    {
        ctx.write(" rowspan=\"");
        ctx.write(&rowspan.to_string());
        ctx.write("\"");
    }

    ctx.write(">");
    emit_nodes(&node.children, ctx);
    ctx.write("</");
    ctx.write(tag);
    ctx.write(">");
}

/// Emit a div element.
fn emit_div(node: &Node, ctx: &mut EmitContext) {
    ctx.newline();
    ctx.write("<div");
    emit_common_attrs(node, ctx);
    ctx.write(">");

    let has_block_children = node.children.iter().any(|c| is_block_node(c.kind.as_str()));
    if has_block_children {
        ctx.indent();
        emit_nodes(&node.children, ctx);
        ctx.dedent();
        ctx.newline();
    } else {
        emit_nodes(&node.children, ctx);
    }

    ctx.write("</div>");
}

/// Emit raw content (pass-through).
fn emit_raw(node: &Node, ctx: &mut EmitContext) {
    let format = node.props.get_str(prop::FORMAT).unwrap_or("html");
    if format == "html"
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        ctx.write(content);
    }
}

/// Emit text content.
fn emit_text(node: &Node, ctx: &mut EmitContext) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(&escape_html(content));
    }
}

/// Emit inline code.
fn emit_inline_code(node: &Node, ctx: &mut EmitContext) {
    ctx.write("<code>");
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(&escape_html(content));
    }
    ctx.write("</code>");
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    ctx.write("<a");

    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write(" href=\"");
        ctx.write(&escape_attr(url));
        ctx.write("\"");
    }

    if let Some(title) = node.props.get_str(prop::TITLE) {
        ctx.write(" title=\"");
        ctx.write(&escape_attr(title));
        ctx.write("\"");
    }

    ctx.write(">");
    emit_nodes(&node.children, ctx);
    ctx.write("</a>");
}

/// Emit an image.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    ctx.write("<img");

    // Check for embedded resource first
    if let Some(resource_id_str) = node.props.get_str(prop::RESOURCE_ID) {
        let resource_id = ResourceId::from_string(resource_id_str);
        if let Some(resource) = ctx.resources.get(&resource_id) {
            // Emit as data URI
            ctx.write(" src=\"data:");
            ctx.write(&resource.mime_type);
            ctx.write(";base64,");
            ctx.write(&base64_encode(&resource.data));
            ctx.write("\"");
        }
    } else if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write(" src=\"");
        ctx.write(&escape_attr(url));
        ctx.write("\"");
    }

    if let Some(alt) = node.props.get_str(prop::ALT) {
        ctx.write(" alt=\"");
        ctx.write(&escape_attr(alt));
        ctx.write("\"");
    }

    if let Some(title) = node.props.get_str(prop::TITLE) {
        ctx.write(" title=\"");
        ctx.write(&escape_attr(title));
        ctx.write("\"");
    }

    ctx.write(">");
}

/// Emit a span element.
fn emit_span(node: &Node, ctx: &mut EmitContext) {
    ctx.write("<span");
    emit_common_attrs(node, ctx);
    ctx.write(">");
    emit_nodes(&node.children, ctx);
    ctx.write("</span>");
}

/// Emit a footnote reference.
fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    let label = node.props.get_str(prop::LABEL).unwrap_or("?");
    ctx.write("<sup><a href=\"#fn-");
    ctx.write(&escape_attr(label));
    ctx.write("\">");
    ctx.write(&escape_html(label));
    ctx.write("</a></sup>");
}

/// Emit a footnote definition.
fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    let label = node.props.get_str(prop::LABEL).unwrap_or("?");
    ctx.newline();
    ctx.write("<div id=\"fn-");
    ctx.write(&escape_attr(label));
    ctx.write("\" class=\"footnote\"><sup>");
    ctx.write(&escape_html(label));
    ctx.write("</sup> ");
    emit_nodes(&node.children, ctx);
    ctx.write("</div>");
}

/// Emit quoted text.
fn emit_quoted(node: &Node, ctx: &mut EmitContext) {
    ctx.write("<q>");
    emit_nodes(&node.children, ctx);
    ctx.write("</q>");
}

/// Emit inline math.
fn emit_math_inline(node: &Node, ctx: &mut EmitContext) {
    if let Some(source) = node.props.get_str("math:source") {
        ctx.write("<span class=\"math math-inline\">\\(");
        ctx.write(&escape_html(source));
        ctx.write("\\)</span>");
    }
}

/// Emit display math.
fn emit_math_display(node: &Node, ctx: &mut EmitContext) {
    if let Some(source) = node.props.get_str("math:source") {
        ctx.write("<div class=\"math math-display\">\\[");
        ctx.write(&escape_html(source));
        ctx.write("\\]</div>");
    }
}

/// Escape HTML special characters.
fn escape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// Escape attribute values.
fn escape_attr(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            _ => result.push(c),
        }
    }
    result
}

/// Base64 encode bytes.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        result.push(ALPHABET[(b0 >> 2) as usize] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4 | b1 >> 4) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2 | b2 >> 6) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::html;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = html(|d| d.p(|i| i.text("Hello, world!")));
        let output = emit_str(&doc);
        assert_eq!(output, "<p>Hello, world!</p>");
    }

    #[test]
    fn test_emit_heading() {
        let doc = html(|d| d.h2(|i| i.text("Title")));
        let output = emit_str(&doc);
        assert_eq!(output, "<h2>Title</h2>");
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = html(|d| d.p(|i| i.em(|i| i.text("italic"))));
        let output = emit_str(&doc);
        assert_eq!(output, "<p><em>italic</em></p>");
    }

    #[test]
    fn test_emit_link() {
        let doc = html(|d| d.p(|i| i.a("https://example.com", |i| i.text("link"))));
        let output = emit_str(&doc);
        assert_eq!(output, "<p><a href=\"https://example.com\">link</a></p>");
    }

    #[test]
    fn test_emit_code_block() {
        let doc = html(|d| d.pre_lang("rust", "fn main() {}"));
        let output = emit_str(&doc);
        assert_eq!(
            output,
            "<pre><code class=\"language-rust\">fn main() {}</code></pre>"
        );
    }

    #[test]
    fn test_emit_list() {
        let doc = html(|d| d.ul(|l| l.li(|i| i.text("item 1")).li(|i| i.text("item 2"))));
        let output = emit_str(&doc);
        assert!(output.contains("<ul>"));
        assert!(output.contains("<li>"));
        assert!(output.contains("item 1"));
        assert!(output.contains("item 2"));
    }

    #[test]
    fn test_emit_image() {
        let doc = html(|d| d.p(|i| i.img("test.png", "Test image")));
        let output = emit_str(&doc);
        assert!(output.contains("<img src=\"test.png\" alt=\"Test image\">"));
    }

    #[test]
    fn test_escape_html() {
        let doc = html(|d| d.p(|i| i.text("<script>alert('xss')</script>")));
        let output = emit_str(&doc);
        assert!(output.contains("&lt;script&gt;"));
        assert!(!output.contains("<script>"));
    }

    #[test]
    fn test_pretty_print_list() {
        let doc = html(|d| d.ul(|l| l.li(|i| i.text("item 1")).li(|i| i.text("item 2"))));

        // Default (not pretty)
        let output = emit_str(&doc);
        assert!(!output.contains('\n'));

        // Pretty mode
        let options = EmitOptions {
            pretty: true,
            ..Default::default()
        };
        let result = emit_with_options(&doc, &options).unwrap();
        let pretty_output = String::from_utf8(result.value).unwrap();

        // Should have newlines and indentation
        assert!(pretty_output.contains('\n'));
        assert!(pretty_output.contains("  <li>"));
    }

    #[test]
    fn test_pretty_print_nested() {
        // Build a nested structure: blockquote with paragraphs
        let doc = html(|d| d.blockquote(|bq| bq.p(|p| p.text("first")).p(|p| p.text("second"))));

        let options = EmitOptions {
            pretty: true,
            ..Default::default()
        };
        let result = emit_with_options(&doc, &options).unwrap();
        let output = String::from_utf8(result.value).unwrap();

        // Should have proper nesting with increased indentation
        assert!(output.contains('\n'));
        assert!(output.contains("  <p>"));
    }
}
