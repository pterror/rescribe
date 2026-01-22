//! Beamer (LaTeX) presentation writer for rescribe.
//!
//! Generates Beamer LaTeX presentations from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to Beamer LaTeX.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to Beamer LaTeX with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();
    let title = doc.metadata.get_str("title").unwrap_or("Presentation");
    let author = doc.metadata.get_str("author").unwrap_or("");

    // Beamer preamble
    output.push_str("\\documentclass{beamer}\n");
    output.push_str("\\usetheme{default}\n");
    output.push_str("\\usepackage[utf8]{inputenc}\n");
    output.push_str("\\usepackage{hyperref}\n");
    output.push_str("\\usepackage{graphicx}\n\n");

    output.push_str(&format!("\\title{{{}}}\n", escape_latex(title)));
    if !author.is_empty() {
        output.push_str(&format!("\\author{{{}}}\n", escape_latex(author)));
    }
    output.push_str("\\date{\\today}\n\n");

    output.push_str("\\begin{document}\n\n");
    output.push_str("\\begin{frame}\n\\titlepage\n\\end{frame}\n\n");

    // Process content into frames
    let mut current_frame = Vec::new();
    let mut frame_title = String::new();
    let mut in_frame = false;

    for child in &doc.content.children {
        if child.kind.as_str() == node::HEADING {
            let level = child.props.get_int(prop::LEVEL).unwrap_or(1);
            if level <= 2 {
                if in_frame {
                    emit_frame(&frame_title, &current_frame, &mut output);
                    current_frame.clear();
                }
                frame_title = get_text_content(child);
                in_frame = true;
                continue;
            }
        }
        if in_frame {
            current_frame.push(child.clone());
        }
    }

    if !current_frame.is_empty() || in_frame {
        emit_frame(&frame_title, &current_frame, &mut output);
    }

    output.push_str("\\end{document}\n");

    Ok(ConversionResult::ok(output.into_bytes()))
}

fn get_text_content(node: &Node) -> String {
    let mut text = String::new();
    for child in &node.children {
        if child.kind.as_str() == node::TEXT {
            if let Some(content) = child.props.get_str(prop::CONTENT) {
                text.push_str(content);
            }
        } else {
            text.push_str(&get_text_content(child));
        }
    }
    text
}

fn emit_frame(title: &str, nodes: &[Node], output: &mut String) {
    output.push_str("\\begin{frame}\n");
    if !title.is_empty() {
        output.push_str(&format!("\\frametitle{{{}}}\n", escape_latex(title)));
    }
    for node in nodes {
        emit_node(node, output);
    }
    output.push_str("\\end{frame}\n\n");
}

fn emit_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::HEADING => {
            // Sub-headings within frame
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            if level >= 3 {
                output.push_str("\\textbf{");
                emit_inline_nodes(&node.children, output);
                output.push_str("}\n\n");
            }
        }
        node::PARAGRAPH => {
            emit_inline_nodes(&node.children, output);
            output.push_str("\n\n");
        }
        node::CODE_BLOCK => {
            let lang = node
                .props
                .get_str(prop::LANGUAGE)
                .map(|s| s.to_string())
                .unwrap_or_default();
            if lang.is_empty() {
                output.push_str("\\begin{verbatim}\n");
            } else {
                output.push_str(&format!("\\begin{{verbatim}}% {}\n", lang));
            }
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.push_str("\\end{verbatim}\n\n");
        }
        node::BLOCKQUOTE => {
            output.push_str("\\begin{quote}\n");
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, output);
                    output.push('\n');
                } else {
                    emit_node(child, output);
                }
            }
            output.push_str("\\end{quote}\n\n");
        }
        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let env = if ordered { "enumerate" } else { "itemize" };
            output.push_str(&format!("\\begin{{{}}}\n", env));

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str("\\item ");
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                        } else {
                            emit_node(item_child, output);
                        }
                    }
                    output.push('\n');
                }
            }

            output.push_str(&format!("\\end{{{}}}\n\n", env));
        }
        node::TABLE => {
            // Simple table support
            let col_count = node
                .children
                .first()
                .map(|row| row.children.len())
                .unwrap_or(0);
            if col_count > 0 {
                let cols = "l".repeat(col_count);
                output.push_str(&format!("\\begin{{tabular}}{{{}}}\n", cols));
                output.push_str("\\hline\n");
                for (row_idx, row) in node.children.iter().enumerate() {
                    if row.kind.as_str() == node::TABLE_ROW {
                        let cells: Vec<String> = row
                            .children
                            .iter()
                            .map(|cell| {
                                let mut cell_content = String::new();
                                emit_inline_nodes(&cell.children, &mut cell_content);
                                cell_content.trim().to_string()
                            })
                            .collect();
                        output.push_str(&cells.join(" & "));
                        output.push_str(" \\\\\n");
                        if row_idx == 0 {
                            output.push_str("\\hline\n");
                        }
                    }
                }
                output.push_str("\\hline\n");
                output.push_str("\\end{tabular}\n\n");
            }
        }
        node::HORIZONTAL_RULE => {
            output.push_str("\\hrulefill\n\n");
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
                output.push_str(&escape_latex(content));
            }
        }
        node::STRONG => {
            output.push_str("\\textbf{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::EMPHASIS => {
            output.push_str("\\emph{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::UNDERLINE => {
            output.push_str("\\underline{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::STRIKEOUT => {
            output.push_str("\\sout{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::CODE => {
            output.push_str("\\texttt{");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_latex(content));
            }
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!("\\href{{{}}}", escape_latex(url)));
                output.push('{');
                emit_inline_nodes(&node.children, output);
                output.push('}');
            } else {
                emit_inline_nodes(&node.children, output);
            }
        }
        node::IMAGE => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!(
                    "\\includegraphics[width=\\textwidth]{{{}}}",
                    escape_latex(url)
                ));
            }
        }
        node::SUBSCRIPT => {
            output.push_str("\\textsubscript{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::SUPERSCRIPT => {
            output.push_str("\\textsuperscript{");
            emit_inline_nodes(&node.children, output);
            output.push('}');
        }
        node::LINE_BREAK => output.push_str("\\\\\n"),
        node::SOFT_BREAK => output.push(' '),
        _ => emit_inline_nodes(&node.children, output),
    }
}

fn escape_latex(s: &str) -> String {
    s.replace('\\', "\\textbackslash{}")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('$', "\\$")
        .replace('&', "\\&")
        .replace('#', "\\#")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('^', "\\^{}")
        .replace('~', "\\textasciitilde{}")
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
        assert!(output.contains("\\documentclass{beamer}"));
        assert!(output.contains("\\begin{frame}"));
    }

    #[test]
    fn test_emit_list() {
        let doc = doc(|d| {
            d.heading(1, |h| h.text("Title"))
                .bullet_list(|l| l.item(|i| i.text("Item 1")))
        });
        let output = emit_str(&doc);
        assert!(output.contains("\\begin{itemize}"));
        assert!(output.contains("\\item"));
    }

    #[test]
    fn test_escape_latex() {
        assert_eq!(escape_latex("$100"), "\\$100");
        assert_eq!(escape_latex("50%"), "50\\%");
        assert_eq!(escape_latex("a_b"), "a\\_b");
    }
}
