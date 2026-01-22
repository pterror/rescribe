//! Slideous writer for rescribe.
//!
//! Generates Slideous HTML slideshow output from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to Slideous HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to Slideous HTML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let title = doc
        .metadata
        .get_str("title")
        .unwrap_or("Presentation")
        .to_string();

    let mut output = String::new();

    // Slideous HTML header
    output.push_str(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{}</title>
<meta name="generator" content="rescribe">
<link rel="stylesheet" href="slideous.css" type="text/css">
<script src="slideous.js" type="text/javascript"></script>
<style>
body {{ font-family: sans-serif; }}
.slide {{ padding: 20px 40px; }}
h1 {{ font-size: 2em; margin-bottom: 0.5em; }}
h2 {{ font-size: 1.5em; }}
pre {{ background: #f5f5f5; padding: 10px; overflow-x: auto; }}
code {{ background: #f0f0f0; padding: 2px 4px; font-family: monospace; }}
blockquote {{ border-left: 3px solid #ccc; margin-left: 0; padding-left: 15px; color: #666; }}
ul, ol {{ margin-left: 1.5em; }}
table {{ border-collapse: collapse; margin: 1em 0; }}
th, td {{ border: 1px solid #ddd; padding: 8px; }}
th {{ background: #f5f5f5; }}
img {{ max-width: 100%; height: auto; }}
</style>
</head>
<body>
<div id="statusbar">
<span id="pagenr">1</span>/<span id="pagecount">1</span>
</div>
"#,
        escape_html(&title)
    ));

    // Collect slides (split on h1 headings)
    let mut slides: Vec<Vec<&Node>> = Vec::new();
    let mut current_slide: Vec<&Node> = Vec::new();

    for child in &doc.content.children {
        if child.kind.as_str() == node::HEADING {
            let level = child.props.get_int(prop::LEVEL).unwrap_or(1);
            if level == 1 && !current_slide.is_empty() {
                slides.push(current_slide);
                current_slide = Vec::new();
            }
        }
        current_slide.push(child);
    }
    if !current_slide.is_empty() {
        slides.push(current_slide);
    }

    // Title slide
    output.push_str("<div class=\"slide\">\n");
    output.push_str(&format!("<h1>{}</h1>\n", escape_html(&title)));
    if let Some(author) = doc.metadata.get_str("author") {
        output.push_str(&format!("<p>{}</p>\n", escape_html(author)));
    }
    if let Some(date) = doc.metadata.get_str("date") {
        output.push_str(&format!("<p>{}</p>\n", escape_html(date)));
    }
    output.push_str("</div>\n\n");

    // Content slides
    for slide in &slides {
        output.push_str("<div class=\"slide\">\n");
        for node in slide {
            emit_node(node, &mut output);
        }
        output.push_str("</div>\n\n");
    }

    output.push_str("</body>\n</html>\n");

    Ok(ConversionResult::ok(output.into_bytes()))
}

fn emit_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1).clamp(1, 6);
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
            let lang = node.props.get_str(prop::LANGUAGE).unwrap_or("");
            if lang.is_empty() {
                output.push_str("<pre><code>");
            } else {
                output.push_str(&format!(
                    "<pre><code class=\"language-{}\">",
                    escape_html(lang)
                ));
            }
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_html(content));
            }
            output.push_str("</code></pre>\n");
        }

        node::BLOCKQUOTE => {
            output.push_str("<blockquote>\n");
            for child in &node.children {
                emit_node(child, output);
            }
            output.push_str("</blockquote>\n");
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

        node::TABLE => {
            output.push_str("<table>\n");
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    output.push_str("<tr>");
                    for cell in &row.children {
                        let tag = if cell.kind.as_str() == node::TABLE_HEADER {
                            "th"
                        } else {
                            "td"
                        };
                        output.push_str(&format!("<{}>", tag));
                        emit_inline_nodes(&cell.children, output);
                        output.push_str(&format!("</{}>", tag));
                    }
                    output.push_str("</tr>\n");
                }
            }
            output.push_str("</table>\n");
        }

        node::HORIZONTAL_RULE => {
            output.push_str("<hr>\n");
        }

        node::DIV | node::FIGURE => {
            output.push_str("<div>\n");
            for child in &node.children {
                emit_node(child, output);
            }
            output.push_str("</div>\n");
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

        node::UNDERLINE => {
            output.push_str("<u>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</u>");
        }

        node::STRIKEOUT => {
            output.push_str("<del>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</del>");
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

        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                let alt = node.props.get_str(prop::ALT).unwrap_or("");
                output.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\">",
                    escape_html(url),
                    escape_html(alt)
                ));
            }
        }

        node::SUBSCRIPT => {
            output.push_str("<sub>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</sub>");
        }

        node::SUPERSCRIPT => {
            output.push_str("<sup>");
            emit_inline_nodes(&node.children, output);
            output.push_str("</sup>");
        }

        node::LINE_BREAK => output.push_str("<br>\n"),
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
                .heading(1, |h| h.text("Slide 2"))
                .para(|p| p.text("More content"))
        });
        let output = emit_str(&doc);
        assert!(output.contains("class=\"slide\""));
        assert!(output.contains("<h1>Slide 1</h1>"));
        assert!(output.contains("<h1>Slide 2</h1>"));
    }

    #[test]
    fn test_emit_formatting() {
        let doc = doc(|d| {
            d.para(|p| {
                p.strong(|s| s.text("bold"))
                    .text(" and ")
                    .em(|e| e.text("italic"))
            })
        });
        let output = emit_str(&doc);
        assert!(output.contains("<strong>bold</strong>"));
        assert!(output.contains("<em>italic</em>"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item"))));
        let output = emit_str(&doc);
        assert!(output.contains("<ul>"));
        assert!(output.contains("<li>"));
    }
}
