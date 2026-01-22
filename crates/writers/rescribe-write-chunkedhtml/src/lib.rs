//! Chunked HTML writer for rescribe.
//!
//! Generates multiple HTML files split by sections from rescribe's document IR.
//! Returns a zip-like structure with multiple files.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// A chunk of HTML output.
#[derive(Debug, Clone)]
pub struct HtmlChunk {
    pub filename: String,
    pub title: String,
    pub content: Vec<u8>,
}

/// Emit a document to chunked HTML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<HtmlChunk>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to chunked HTML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<HtmlChunk>>, EmitError> {
    let doc_title = doc
        .metadata
        .get_str("title")
        .unwrap_or("Document")
        .to_string();
    let mut chunks = Vec::new();
    let mut current_content = Vec::new();
    let mut current_title = doc_title.clone();
    let mut chunk_index = 0;

    // Collect table of contents
    let toc = collect_toc(&doc.content.children);

    for child in &doc.content.children {
        if child.kind.as_str() == node::HEADING {
            let level = child.props.get_int(prop::LEVEL).unwrap_or(1);
            if level == 1 {
                // Save previous chunk if it has content
                if !current_content.is_empty() {
                    let html = generate_html_page(
                        &current_title,
                        &current_content,
                        &toc,
                        chunk_index,
                        chunks.len(),
                    );
                    chunks.push(HtmlChunk {
                        filename: format!("chunk{:03}.html", chunk_index),
                        title: current_title.clone(),
                        content: html.into_bytes(),
                    });
                    chunk_index += 1;
                    current_content.clear();
                }
                current_title = get_text_content(child);
            }
        }
        current_content.push(child.clone());
    }

    // Save final chunk
    if !current_content.is_empty() {
        let html = generate_html_page(
            &current_title,
            &current_content,
            &toc,
            chunk_index,
            chunk_index + 1,
        );
        chunks.push(HtmlChunk {
            filename: format!("chunk{:03}.html", chunk_index),
            title: current_title,
            content: html.into_bytes(),
        });
    }

    // Generate index page
    let index_html = generate_index_page(&doc_title, &toc);
    chunks.insert(
        0,
        HtmlChunk {
            filename: "index.html".to_string(),
            title: doc_title,
            content: index_html.into_bytes(),
        },
    );

    Ok(ConversionResult::ok(chunks))
}

fn collect_toc(nodes: &[Node]) -> Vec<(i64, String, usize)> {
    let mut toc = Vec::new();
    let mut chunk_index = 0;

    for child in nodes {
        if child.kind.as_str() == node::HEADING {
            let level = child.props.get_int(prop::LEVEL).unwrap_or(1);
            let title = get_text_content(child);
            if level == 1 {
                chunk_index += 1;
            }
            toc.push((level, title, chunk_index.max(1) - 1));
        }
    }

    toc
}

fn generate_index_page(title: &str, toc: &[(i64, String, usize)]) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{}</title>
<style>
body {{ font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
h1 {{ border-bottom: 2px solid #333; }}
ul {{ list-style: none; padding-left: 0; }}
ul ul {{ padding-left: 20px; }}
a {{ color: #0066cc; text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
</style>
</head>
<body>
<h1>{}</h1>
<h2>Table of Contents</h2>
<ul>
"#,
        escape_html(title),
        escape_html(title)
    ));

    for (level, section_title, chunk_idx) in toc {
        let indent = "  ".repeat(*level as usize);
        html.push_str(&format!(
            "{}<li><a href=\"chunk{:03}.html\">{}</a></li>\n",
            indent,
            chunk_idx,
            escape_html(section_title)
        ));
    }

    html.push_str("</ul>\n</body>\n</html>\n");
    html
}

fn generate_html_page(
    title: &str,
    nodes: &[Node],
    toc: &[(i64, String, usize)],
    current_chunk: usize,
    total_chunks: usize,
) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{}</title>
<style>
body {{ font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
nav {{ background: #f5f5f5; padding: 10px; margin-bottom: 20px; }}
nav a {{ margin-right: 15px; color: #0066cc; text-decoration: none; }}
nav a:hover {{ text-decoration: underline; }}
pre {{ background: #f5f5f5; padding: 10px; overflow-x: auto; }}
code {{ background: #f0f0f0; padding: 2px 4px; }}
blockquote {{ border-left: 3px solid #ccc; margin-left: 0; padding-left: 15px; color: #666; }}
table {{ border-collapse: collapse; }}
th, td {{ border: 1px solid #ddd; padding: 8px; }}
</style>
</head>
<body>
<nav>
<a href="index.html">Index</a>
"#,
        escape_html(title)
    ));

    // Navigation links
    if current_chunk > 0 {
        html.push_str(&format!(
            "<a href=\"chunk{:03}.html\">&laquo; Previous</a>",
            current_chunk - 1
        ));
    }
    if current_chunk + 1 < total_chunks {
        html.push_str(&format!(
            "<a href=\"chunk{:03}.html\">Next &raquo;</a>",
            current_chunk + 1
        ));
    }

    html.push_str("</nav>\n");

    // Content
    let mut content = String::new();
    emit_nodes(nodes, &mut content);
    html.push_str(&content);

    // Footer navigation
    html.push_str(
        "<nav style=\"margin-top: 40px; border-top: 1px solid #ddd; padding-top: 10px;\">\n",
    );
    if current_chunk > 0 {
        let prev_title = toc
            .iter()
            .find(|(l, _, idx)| *l == 1 && *idx == current_chunk - 1)
            .map(|(_, t, _)| t.as_str())
            .unwrap_or("Previous");
        html.push_str(&format!(
            "<a href=\"chunk{:03}.html\">&laquo; {}</a> ",
            current_chunk - 1,
            escape_html(prev_title)
        ));
    }
    if current_chunk + 1 < total_chunks {
        let next_title = toc
            .iter()
            .find(|(l, _, idx)| *l == 1 && *idx == current_chunk + 1)
            .map(|(_, t, _)| t.as_str())
            .unwrap_or("Next");
        html.push_str(&format!(
            "<a href=\"chunk{:03}.html\">{} &raquo;</a>",
            current_chunk + 1,
            escape_html(next_title)
        ));
    }
    html.push_str("\n</nav>\n</body>\n</html>\n");

    html
}

fn emit_nodes(nodes: &[Node], output: &mut String) {
    for node in nodes {
        emit_node(node, output);
    }
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
            emit_nodes(&node.children, output);
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
        _ => emit_nodes(&node.children, output),
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
        node::LINE_BREAK => output.push_str("<br>\n"),
        node::SOFT_BREAK => output.push(' '),
        _ => emit_inline_nodes(&node.children, output),
    }
}

fn get_text_content(node: &Node) -> String {
    let mut text = String::new();
    collect_text(node, &mut text);
    text
}

fn collect_text(node: &Node, output: &mut String) {
    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        output.push_str(content);
    }
    for child in &node.children {
        collect_text(child, output);
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

    #[test]
    fn test_emit_basic() {
        let document = doc(|d| {
            d.heading(1, |h| h.text("Chapter 1"))
                .para(|p| p.text("Content 1"))
                .heading(1, |h| h.text("Chapter 2"))
                .para(|p| p.text("Content 2"))
        });
        let result = emit(&document).unwrap();
        assert!(result.value.len() >= 3); // index + 2 chapters
    }

    #[test]
    fn test_index_generation() {
        let toc = vec![
            (1, "Chapter 1".to_string(), 0),
            (2, "Section 1.1".to_string(), 0),
            (1, "Chapter 2".to_string(), 1),
        ];
        let index = generate_index_page("Test", &toc);
        assert!(index.contains("Chapter 1"));
        assert!(index.contains("Chapter 2"));
        assert!(index.contains("chunk000.html"));
    }
}
