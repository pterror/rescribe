//! reveal.js presentation writer for rescribe.
//!
//! Generates reveal.js HTML presentations from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to reveal.js HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to reveal.js HTML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();
    let title = doc.metadata.get_str("title").unwrap_or("Presentation");

    // HTML header
    output.push_str(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{}</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.css">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/theme/black.css">
</head>
<body>
<div class="reveal">
<div class="slides">
"#,
        escape_html(title)
    ));

    // Split content into slides (h1/h2 starts new slide)
    let mut current_slide = Vec::new();
    let mut in_slide = false;

    for child in &doc.content.children {
        if child.kind.as_str() == node::HEADING {
            let level = child.props.get_int(prop::LEVEL).unwrap_or(1);
            if level <= 2 {
                // Close previous slide
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

    // Emit last slide
    if !current_slide.is_empty() {
        emit_slide(&current_slide, &mut output);
    }

    // HTML footer
    output.push_str(
        r#"</div>
</div>
<script src="https://cdn.jsdelivr.net/npm/reveal.js@4/dist/reveal.js"></script>
<script>Reveal.initialize();</script>
</body>
</html>
"#,
    );

    Ok(ConversionResult::ok(output.into_bytes()))
}

fn emit_slide(nodes: &[Node], output: &mut String) {
    output.push_str("<section>\n");
    for node in nodes {
        emit_node(node, output);
    }
    output.push_str("</section>\n");
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
            output.push_str("<pre><code");
            if let Some(lang) = node.props.get_str(prop::LANGUAGE) {
                output.push_str(&format!(" class=\"language-{}\"", escape_html(lang)));
            }
            output.push('>');
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_html(content));
            }
            output.push_str("</code></pre>\n");
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
                        } else {
                            emit_node(item_child, output);
                        }
                    }
                    output.push_str("</li>\n");
                }
            }
            output.push_str(&format!("</{}>\n", tag));
        }

        node::BLOCKQUOTE => {
            output.push_str("<blockquote>\n");
            for child in &node.children {
                emit_node(child, output);
            }
            output.push_str("</blockquote>\n");
        }

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!("<img src=\"{}\"", escape_html(url)));
                if let Some(alt) = node.props.get_str(prop::ALT) {
                    output.push_str(&format!(" alt=\"{}\"", escape_html(alt)));
                }
                output.push_str(">\n");
            }
        }

        node::DIV | node::FIGURE => {
            for child in &node.children {
                emit_node(child, output);
            }
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
            emit_inline_nodes(&node.children, output);
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
        node::LINE_BREAK => output.push_str("<br>"),
        node::SOFT_BREAK => output.push(' '),
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
        let doc = doc(|d| {
            d.heading(1, |h| h.text("Slide 1"))
                .para(|p| p.text("Content"))
        });
        let output = emit_str(&doc);
        assert!(output.contains("<section>"));
        assert!(output.contains("<h1>Slide 1</h1>"));
        assert!(output.contains("reveal.js"));
    }

    #[test]
    fn test_emit_multiple_slides() {
        let doc = doc(|d| {
            d.heading(1, |h| h.text("Slide 1"))
                .para(|p| p.text("Content 1"))
                .heading(1, |h| h.text("Slide 2"))
                .para(|p| p.text("Content 2"))
        });
        let output = emit_str(&doc);
        assert!(output.matches("<section>").count() == 2);
    }
}
