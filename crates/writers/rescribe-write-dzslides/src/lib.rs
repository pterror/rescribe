//! DZSlides presentation writer for rescribe.
//!
//! Generates DZSlides HTML presentations from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to DZSlides HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to DZSlides HTML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();
    let title = doc.metadata.get_str("title").unwrap_or("Presentation");

    output.push_str(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{}</title>
<style>
html, .view body {{ background-color: black; counter-reset: slideidx; }}
body, .view section {{ background-color: white; border-radius: 12px; }}
section, .view head > title {{ display: none; }}
section, .view section {{
  font-size: 2em;
  padding: 1em;
  min-height: 100%;
  box-sizing: border-box;
}}
.view section {{
  position: absolute; top: 0; left: 0;
  width: 100%; height: 100%;
  transform-origin: 0 0;
}}
.view section[aria-selected] {{ transform: scale(1); }}
.view section:not([aria-selected]) {{ display: block; }}
h1 {{ font-size: 1.5em; margin: 0.5em 0; }}
ul, ol {{ margin: 0.5em 0; padding-left: 1.5em; }}
</style>
</head>
<body>
"#,
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

    // DZSlides inline script
    output.push_str(
        r#"<script>
var defined = function(x) { return x !== undefined; };
var defined_ = function(x) { return defined(x) ? x : ''; };
var Dz = {
  idx: -1,
  step: 0,
  slides: document.querySelectorAll('body > section'),
  go: function(idx) {
    if (idx < 0 || idx >= this.slides.length) return;
    this.slides[this.idx] && (this.slides[this.idx].style.display = 'none');
    this.idx = idx;
    this.slides[this.idx].style.display = '';
  }
};
Dz.go(0);
document.onkeydown = function(e) {
  var k = e.keyCode;
  if (k === 39 || k === 40 || k === 34) Dz.go(Dz.idx + 1);
  if (k === 37 || k === 38 || k === 33) Dz.go(Dz.idx - 1);
};
</script>
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
        assert!(output.contains("<section>"));
        assert!(output.contains("DZSlides") || output.contains("Dz.go"));
    }
}
