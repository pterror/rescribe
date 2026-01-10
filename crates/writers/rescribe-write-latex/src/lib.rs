//! LaTeX writer for rescribe.
//!
//! Emits documents as LaTeX source.

pub mod builder;

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a document as LaTeX fragment (body content only).
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as LaTeX with custom options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit a complete LaTeX document with preamble.
pub fn emit_full_document(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Preamble
    ctx.write("\\documentclass{article}\n");
    ctx.write("\\usepackage[utf8]{inputenc}\n");
    ctx.write("\\usepackage{graphicx}\n");
    ctx.write("\\usepackage{hyperref}\n");
    ctx.write("\\usepackage{listings}\n");
    ctx.write("\\usepackage{amsmath}\n");
    ctx.write("\\usepackage{amssymb}\n");
    ctx.write("\\usepackage{ulem}\n"); // For strikethrough
    ctx.write("\n\\begin{document}\n\n");

    emit_nodes(&doc.content.children, &mut ctx);

    ctx.write("\n\\end{document}\n");

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

/// Emit context for tracking state during emission.
struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
    in_verbatim: bool,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
            in_verbatim: false,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_escaped(&mut self, s: &str) {
        if self.in_verbatim {
            self.output.push_str(s);
        } else {
            self.output.push_str(&escape_latex(s));
        }
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

        node::PARAGRAPH => {
            emit_nodes(&node.children, ctx);
            ctx.write("\n\n");
        }

        node::HEADING => emit_heading(node, ctx),
        node::CODE_BLOCK => emit_code_block(node, ctx),
        node::BLOCKQUOTE => emit_blockquote(node, ctx),
        node::LIST => emit_list(node, ctx),
        node::LIST_ITEM => emit_list_item(node, ctx),
        node::TABLE => emit_table(node, ctx),
        node::FIGURE => emit_figure(node, ctx),
        node::CAPTION => emit_caption(node, ctx),
        node::HORIZONTAL_RULE => ctx.write("\\hrulefill\n\n"),

        node::DIV | node::SPAN => emit_nodes(&node.children, ctx),

        node::RAW_BLOCK | node::RAW_INLINE => {
            let format = node.props.get_str(prop::FORMAT).unwrap_or("");
            if (format == "latex" || format == "tex")
                && let Some(content) = node.props.get_str(prop::CONTENT)
            {
                ctx.write(content);
            }
        }

        node::DEFINITION_LIST => emit_definition_list(node, ctx),
        node::DEFINITION_TERM => emit_definition_term(node, ctx),
        node::DEFINITION_DESC => emit_definition_desc(node, ctx),

        // Inline elements
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write_escaped(content);
            }
        }

        node::EMPHASIS => {
            ctx.write("\\emph{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::STRONG => {
            ctx.write("\\textbf{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::STRIKEOUT => {
            ctx.write("\\sout{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::UNDERLINE => {
            ctx.write("\\underline{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::SUBSCRIPT => {
            ctx.write("\\textsubscript{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::SUPERSCRIPT => {
            ctx.write("\\textsuperscript{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::CODE => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write("\\texttt{");
                ctx.write_escaped(content);
                ctx.write("}");
            }
        }

        node::LINK => emit_link(node, ctx),
        node::IMAGE => emit_image(node, ctx),
        node::LINE_BREAK => ctx.write("\\\\\n"),
        node::SOFT_BREAK => ctx.write("\n"),

        node::FOOTNOTE_REF => emit_footnote_ref(node, ctx),
        node::FOOTNOTE_DEF => emit_footnote_def(node, ctx),

        node::SMALL_CAPS => {
            ctx.write("\\textsc{");
            emit_nodes(&node.children, ctx);
            ctx.write("}");
        }

        node::QUOTED => emit_quoted(node, ctx),

        "math_inline" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write("$");
                ctx.write(source);
                ctx.write("$");
            }
        }

        "math_display" => {
            if let Some(source) = node.props.get_str("math:source") {
                ctx.write("\\[\n");
                ctx.write(source);
                ctx.write("\n\\]\n");
            }
        }

        _ => {
            ctx.warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                format!("Unknown node type for LaTeX: {}", node.kind.as_str()),
            ));
            emit_nodes(&node.children, ctx);
        }
    }
}

/// Emit a heading.
fn emit_heading(node: &Node, ctx: &mut EmitContext) {
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
    let cmd = match level {
        1 => "\\section{",
        2 => "\\subsection{",
        3 => "\\subsubsection{",
        4 => "\\paragraph{",
        5 => "\\subparagraph{",
        _ => "\\subparagraph{",
    };

    ctx.write(cmd);
    emit_nodes(&node.children, ctx);
    ctx.write("}\n\n");
}

/// Emit a code block.
fn emit_code_block(node: &Node, ctx: &mut EmitContext) {
    let lang = node.props.get_str(prop::LANGUAGE);

    if let Some(lang) = lang {
        ctx.write("\\begin{lstlisting}[language=");
        ctx.write(lang);
        ctx.write("]\n");
    } else {
        ctx.write("\\begin{verbatim}\n");
    }

    ctx.in_verbatim = true;
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        ctx.write(content);
        if !content.ends_with('\n') {
            ctx.write("\n");
        }
    }
    ctx.in_verbatim = false;

    if lang.is_some() {
        ctx.write("\\end{lstlisting}\n\n");
    } else {
        ctx.write("\\end{verbatim}\n\n");
    }
}

/// Emit a blockquote.
fn emit_blockquote(node: &Node, ctx: &mut EmitContext) {
    ctx.write("\\begin{quote}\n");
    emit_nodes(&node.children, ctx);
    ctx.write("\\end{quote}\n\n");
}

/// Emit a list.
fn emit_list(node: &Node, ctx: &mut EmitContext) {
    let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
    let env = if ordered { "enumerate" } else { "itemize" };

    ctx.write("\\begin{");
    ctx.write(env);
    ctx.write("}\n");

    emit_nodes(&node.children, ctx);

    ctx.write("\\end{");
    ctx.write(env);
    ctx.write("}\n");
}

/// Emit a list item.
fn emit_list_item(node: &Node, ctx: &mut EmitContext) {
    ctx.write("\\item ");
    // Emit children inline, but handle nested lists
    for child in &node.children {
        if child.kind.as_str() == node::PARAGRAPH {
            emit_nodes(&child.children, ctx);
            ctx.write("\n");
        } else {
            emit_node(child, ctx);
        }
    }
}

/// Emit a table.
fn emit_table(node: &Node, ctx: &mut EmitContext) {
    // Count columns by finding first row
    let num_cols = count_table_columns(node);

    ctx.write("\\begin{tabular}{");
    for _ in 0..num_cols {
        ctx.write("l");
    }
    ctx.write("}\n\\hline\n");

    emit_table_contents(&node.children, ctx);

    ctx.write("\\hline\n\\end{tabular}\n\n");
}

/// Count the number of columns in a table.
fn count_table_columns(table: &Node) -> usize {
    for child in &table.children {
        match child.kind.as_str() {
            node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
                if let Some(row) = child.children.first() {
                    return row.children.len();
                }
            }
            node::TABLE_ROW => {
                return child.children.len();
            }
            _ => {}
        }
    }
    1
}

/// Emit table contents.
fn emit_table_contents(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        match node.kind.as_str() {
            node::TABLE_HEAD | node::TABLE_BODY | node::TABLE_FOOT => {
                emit_table_contents(&node.children, ctx);
            }
            node::TABLE_ROW => {
                let mut first = true;
                for cell in &node.children {
                    if !first {
                        ctx.write(" & ");
                    }
                    first = false;
                    emit_nodes(&cell.children, ctx);
                }
                ctx.write(" \\\\\n");
            }
            _ => emit_node(node, ctx),
        }
    }
}

/// Emit a figure.
fn emit_figure(node: &Node, ctx: &mut EmitContext) {
    ctx.write("\\begin{figure}[h]\n\\centering\n");
    emit_nodes(&node.children, ctx);
    ctx.write("\\end{figure}\n\n");
}

/// Emit a caption.
fn emit_caption(node: &Node, ctx: &mut EmitContext) {
    ctx.write("\\caption{");
    emit_nodes(&node.children, ctx);
    ctx.write("}\n");
}

/// Emit a definition list.
fn emit_definition_list(node: &Node, ctx: &mut EmitContext) {
    ctx.write("\\begin{description}\n");
    emit_nodes(&node.children, ctx);
    ctx.write("\\end{description}\n");
}

/// Emit a definition term.
fn emit_definition_term(node: &Node, ctx: &mut EmitContext) {
    ctx.write("\\item[");
    emit_nodes(&node.children, ctx);
    ctx.write("] ");
}

/// Emit a definition description.
fn emit_definition_desc(node: &Node, ctx: &mut EmitContext) {
    emit_nodes(&node.children, ctx);
    ctx.write("\n");
}

/// Emit a link.
fn emit_link(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("\\href{");
        ctx.write(url);
        ctx.write("}{");
        emit_nodes(&node.children, ctx);
        ctx.write("}");
    } else {
        emit_nodes(&node.children, ctx);
    }
}

/// Emit an image.
fn emit_image(node: &Node, ctx: &mut EmitContext) {
    if let Some(url) = node.props.get_str(prop::URL) {
        ctx.write("\\includegraphics{");
        ctx.write(url);
        ctx.write("}");
    }
}

/// Emit a footnote reference.
fn emit_footnote_ref(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write("\\footnotemark[");
        ctx.write(label);
        ctx.write("]");
    }
}

/// Emit a footnote definition.
fn emit_footnote_def(node: &Node, ctx: &mut EmitContext) {
    if let Some(label) = node.props.get_str(prop::LABEL) {
        ctx.write("\\footnotetext[");
        ctx.write(label);
        ctx.write("]{");
        emit_nodes(&node.children, ctx);
        ctx.write("}\n");
    }
}

/// Emit quoted text.
fn emit_quoted(node: &Node, ctx: &mut EmitContext) {
    let quote_type = node.props.get_str(prop::QUOTE_TYPE).unwrap_or("double");
    if quote_type == "single" {
        ctx.write("`");
        emit_nodes(&node.children, ctx);
        ctx.write("'");
    } else {
        ctx.write("``");
        emit_nodes(&node.children, ctx);
        ctx.write("''");
    }
}

/// Escape special LaTeX characters.
fn escape_latex(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            '\\' => result.push_str("\\textbackslash{}"),
            '{' => result.push_str("\\{"),
            '}' => result.push_str("\\}"),
            '$' => result.push_str("\\$"),
            '&' => result.push_str("\\&"),
            '#' => result.push_str("\\#"),
            '%' => result.push_str("\\%"),
            '_' => result.push_str("\\_"),
            '^' => result.push_str("\\textasciicircum{}"),
            '~' => result.push_str("\\textasciitilde{}"),
            '<' => result.push_str("\\textless{}"),
            '>' => result.push_str("\\textgreater{}"),
            '|' => result.push_str("\\textbar{}"),
            '"' => result.push_str("\\textquotedbl{}"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::latex;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let doc = latex(|d| d.para(|i| i.text("Hello, world!")));
        let output = emit_str(&doc);
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_emit_heading() {
        let doc = latex(|d| d.section(|i| i.text("Main Title")));
        let output = emit_str(&doc);
        assert!(output.contains("\\section{Main Title}"));
    }

    #[test]
    fn test_emit_heading_levels() {
        let doc = latex(|d| {
            d.section(|i| i.text("Level 1"))
                .subsection(|i| i.text("Level 2"))
                .subsubsection(|i| i.text("Level 3"))
        });
        let output = emit_str(&doc);
        assert!(output.contains("\\section{Level 1}"));
        assert!(output.contains("\\subsection{Level 2}"));
        assert!(output.contains("\\subsubsection{Level 3}"));
    }

    #[test]
    fn test_emit_emphasis() {
        let doc = latex(|d| d.para(|i| i.emph(|i| i.text("italic"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\emph{italic}"));
    }

    #[test]
    fn test_emit_strong() {
        let doc = latex(|d| d.para(|i| i.bold(|i| i.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\textbf{bold}"));
    }

    #[test]
    fn test_emit_link() {
        let doc = latex(|d| d.para(|i| i.href("https://example.com", |i| i.text("click"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\href{https://example.com}{click}"));
    }

    #[test]
    fn test_emit_list() {
        let doc = latex(|d| d.itemize(|l| l.item(|i| i.text("item 1")).item(|i| i.text("item 2"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\begin{itemize}"));
        assert!(output.contains("\\item item 1"));
        assert!(output.contains("\\item item 2"));
        assert!(output.contains("\\end{itemize}"));
    }

    #[test]
    fn test_emit_ordered_list() {
        let doc =
            latex(|d| d.enumerate(|l| l.item(|i| i.text("first")).item(|i| i.text("second"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\begin{enumerate}"));
        assert!(output.contains("\\item first"));
        assert!(output.contains("\\item second"));
        assert!(output.contains("\\end{enumerate}"));
    }

    #[test]
    fn test_emit_code_block() {
        let doc = latex(|d| d.lstlisting("rust", "fn main() {}"));
        let output = emit_str(&doc);
        assert!(output.contains("\\begin{lstlisting}[language=rust]"));
        assert!(output.contains("fn main() {}"));
        assert!(output.contains("\\end{lstlisting}"));
    }

    #[test]
    fn test_emit_blockquote() {
        let doc = latex(|d| d.quote(|b| b.para(|i| i.text("A quote"))));
        let output = emit_str(&doc);
        assert!(output.contains("\\begin{quote}"));
        assert!(output.contains("A quote"));
        assert!(output.contains("\\end{quote}"));
    }

    #[test]
    fn test_emit_image() {
        let doc = latex(|d| d.figure(|f| f.includegraphics("test.png")));
        let output = emit_str(&doc);
        assert!(output.contains("\\includegraphics{test.png}"));
    }

    #[test]
    fn test_escape_latex() {
        let doc = latex(|d| d.para(|i| i.text("$100 & 50% off #1")));
        let output = emit_str(&doc);
        assert!(output.contains("\\$100 \\& 50\\% off \\#1"));
    }

    #[test]
    fn test_emit_full_document() {
        let doc = latex(|d| d.para(|i| i.text("Hello")));
        let result = emit_full_document(&doc).unwrap();
        let output = String::from_utf8(result.value).unwrap();
        assert!(output.contains("\\documentclass{article}"));
        assert!(output.contains("\\begin{document}"));
        assert!(output.contains("Hello"));
        assert!(output.contains("\\end{document}"));
    }
}
