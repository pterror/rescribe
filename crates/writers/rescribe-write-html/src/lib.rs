//! HTML writer for rescribe.
//!
//! Emits rescribe's document IR as HTML5.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as HTML with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Emit children of the root document node
    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit a document as a complete HTML document with doctype.
pub fn emit_full_document(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    ctx.write("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n</head>\n<body>\n");
    emit_nodes(&doc.content.children, &mut ctx);
    ctx.write("\n</body>\n</html>\n");

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit context for tracking state during emission.
struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
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
        node::PARAGRAPH => emit_tag("p", node, ctx),
        node::HEADING => emit_heading(node, ctx),
        node::CODE_BLOCK => emit_code_block(node, ctx),
        node::BLOCKQUOTE => emit_tag("blockquote", node, ctx),
        node::LIST => emit_list(node, ctx),
        node::LIST_ITEM => emit_tag("li", node, ctx),
        node::TABLE => emit_tag("table", node, ctx),
        node::TABLE_HEAD => emit_tag("thead", node, ctx),
        node::TABLE_BODY => emit_tag("tbody", node, ctx),
        node::TABLE_FOOT => emit_tag("tfoot", node, ctx),
        node::TABLE_ROW => emit_tag("tr", node, ctx),
        node::TABLE_CELL => emit_table_cell(node, "td", ctx),
        node::TABLE_HEADER => emit_table_cell(node, "th", ctx),
        node::FIGURE => emit_tag("figure", node, ctx),
        node::CAPTION => emit_tag("figcaption", node, ctx),
        node::HORIZONTAL_RULE => ctx.write("<hr>"),
        node::DIV => emit_div(node, ctx),
        node::RAW_BLOCK => emit_raw(node, ctx),
        node::DEFINITION_LIST => emit_tag("dl", node, ctx),
        node::DEFINITION_TERM => emit_tag("dt", node, ctx),
        node::DEFINITION_DESC => emit_tag("dd", node, ctx),
        node::TEXT => emit_text(node, ctx),
        node::EMPHASIS => emit_tag("em", node, ctx),
        node::STRONG => emit_tag("strong", node, ctx),
        node::STRIKEOUT => emit_tag("del", node, ctx),
        node::UNDERLINE => emit_tag("u", node, ctx),
        node::SUBSCRIPT => emit_tag("sub", node, ctx),
        node::SUPERSCRIPT => emit_tag("sup", node, ctx),
        node::CODE => emit_inline_code(node, ctx),
        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("<br>"),
        node::SOFT_BREAK => ctx.write("\n"),
        node::SPAN => emit_span(node, ctx),
        node::RAW_INLINE => emit_raw(node, ctx),
        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),
        node::SMALL_CAPS => emit_tag("small", node, ctx),
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

/// Emit a simple tag with children.
fn emit_tag(tag: &str, node: &Node, ctx: &mut EmitContext) {
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
    emit_tag(tag, node, ctx);
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    ctx.write("<pre>");
    ctx.write("<code");

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
    emit_nodes(&node.children, ctx);
    ctx.write("</");
    ctx.write(tag);
    ctx.write(">");
}

/// Emit a table cell.
fn emit_table_cell(node: &Node, tag: &str, ctx: &mut EmitContext) {
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
    ctx.write("<div");
    emit_common_attrs(node, ctx);
    ctx.write(">");
    emit_nodes(&node.children, ctx);
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

    if let Some(url) = node.props.get_str(prop::URL) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::helpers;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::text(
                "Hello, world!",
            )])]));

        let html = emit_str(&doc);
        assert_eq!(html, "<p>Hello, world!</p>");
    }

    #[test]
    fn test_emit_heading() {
        let doc = Document::new().with_content(helpers::document([helpers::heading(
            2,
            [helpers::text("Title")],
        )]));

        let html = emit_str(&doc);
        assert_eq!(html, "<h2>Title</h2>");
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = Document::new().with_content(helpers::document([helpers::paragraph([
            helpers::emphasis([helpers::text("italic")]),
        ])]));

        let html = emit_str(&doc);
        assert_eq!(html, "<p><em>italic</em></p>");
    }

    #[test]
    fn test_emit_link() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::link(
                "https://example.com",
                [helpers::text("link")],
            )])]));

        let html = emit_str(&doc);
        assert_eq!(html, "<p><a href=\"https://example.com\">link</a></p>");
    }

    #[test]
    fn test_emit_code_block() {
        let doc = Document::new().with_content(helpers::document([helpers::code_block(
            "fn main() {}",
            Some("rust"),
        )]));

        let html = emit_str(&doc);
        assert_eq!(
            html,
            "<pre><code class=\"language-rust\">fn main() {}</code></pre>"
        );
    }

    #[test]
    fn test_emit_list() {
        let doc = Document::new().with_content(helpers::document([helpers::bullet_list([
            helpers::list_item([helpers::paragraph([helpers::text("item 1")])]),
            helpers::list_item([helpers::paragraph([helpers::text("item 2")])]),
        ])]));

        let html = emit_str(&doc);
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("item 1"));
        assert!(html.contains("item 2"));
    }

    #[test]
    fn test_emit_image() {
        let doc = Document::new().with_content(helpers::document([helpers::image(
            "test.png",
            "Test image",
        )]));

        let html = emit_str(&doc);
        assert_eq!(html, "<img src=\"test.png\" alt=\"Test image\">");
    }

    #[test]
    fn test_escape_html() {
        let doc =
            Document::new().with_content(helpers::document([helpers::paragraph([helpers::text(
                "<script>alert('xss')</script>",
            )])]));

        let html = emit_str(&doc);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }
}
