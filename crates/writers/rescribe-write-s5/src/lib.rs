//! S5 presentation writer for rescribe.
//!
//! Generates S5 HTML presentations from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to S5 HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to S5 HTML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();
    let title = doc.metadata.get_str("title").unwrap_or("Presentation");

    output.push_str(&format!(
        r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"
 "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
<meta http-equiv="Content-Type" content="text/html; charset=utf-8" />
<title>{}</title>
<!-- S5 links - adjust paths as needed -->
<link rel="stylesheet" href="ui/default/slides.css" type="text/css" media="projection" />
<link rel="stylesheet" href="ui/default/outline.css" type="text/css" media="screen" />
<link rel="stylesheet" href="ui/default/print.css" type="text/css" media="print" />
<script src="ui/default/slides.js" type="text/javascript"></script>
</head>
<body>
<div class="layout">
<div id="controls"></div>
<div id="currentSlide"></div>
<div id="header"></div>
<div id="footer"><h1>{}</h1></div>
</div>
<div class="presentation">
"#,
        escape_html(title),
        escape_html(title)
    ));

    let mut current_slide = Vec::new();
    let mut in_slide = false;

    for child in &doc.content.children {
        if child.kind.as_str() == node::HEADING {
            let level = child.props.get_int(prop::LEVEL).unwrap_or(1);
            if level <= 2 {
                if in_slide {
                    emit_slide(&current_slide, &mut output);
                    current_slide.clear();
                }
                in_slide = true;
            }
        }
        if in_slide {
            current_slide.push(child.clone());
        }
    }

    if !current_slide.is_empty() {
        emit_slide(&current_slide, &mut output);
    }

    output.push_str("</div>\n</body>\n</html>\n");

    Ok(ConversionResult::ok(output.into_bytes()))
}

fn emit_slide(nodes: &[Node], output: &mut String) {
    output.push_str("<div class=\"slide\">\n");
    for node in nodes {
        emit_node(node, output);
    }
    output.push_str("</div>\n");
}

fn emit_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1).min(6);
            output.push_str(&format!("<h{}>", level));
            emit_inline_nodes(&node.children, output);
            output.push_str(&format!("</h{}>\n", level));
        }
        node::PARAGRAPH => {
            output.push_str("<p>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</p>\n");
        }
        node::CODE_BLOCK => {
            output.push_str("<pre>");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_html(content));
            }
            output.push_str("</pre>\n");
        }
        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let tag = if ordered { "ol" } else { "ul" };
            output.push_str(&format!("<{}>\n", tag));
            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str("<li>");
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                        }
                    }
                    output.push_str("</li>\n");
                }
            }
            output.push_str(&format!("</{}>\n", tag));
        }
        _ => {
            for child in &node.children {
                emit_node(child, output);
            }
        }
    }
}

fn emit_inline_nodes(nodes: &[Node], output: &mut String) {
    for node in nodes {
        emit_inline_node(node, output);
    }
}

fn emit_inline_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_html(content));
            }
        }
        node::STRONG => {
            output.push_str("<strong>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</strong>");
        }
        node::EMPHASIS => {
            output.push_str("<em>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</em>");
        }
        node::CODE => {
            output.push_str("<code>");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_html(content));
            }
            output.push_str("</code>");
        }
        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!("<a href=\"{}\">", escape_html(url)));
            }
            emit_inline_nodes(&node.children, output);
            if node.props.get_str(prop::URL).is_some() {
                output.push_str("</a>");
            }
        }
        _ => emit_inline_nodes(&node.children, output),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::*;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_basic() {
        let doc = doc(|d| d.heading(1, |h| h.text("Slide 1")));
        let output = emit_str(&doc);
        assert!(output.contains("class=\"slide\""));
        assert!(output.contains("class=\"presentation\""));
    }
}
