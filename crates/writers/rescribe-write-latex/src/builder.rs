//! Type-safe LaTeX document builder.
//!
//! This builder only exposes elements that LaTeX handles well.
//! Using this builder guarantees your document will emit cleanly to LaTeX.
//!
//! # Example
//!
//! ```
//! use rescribe_write_latex::builder::*;
//!
//! let doc = latex(|d| {
//!     d.section(|i| i.text("Introduction"))
//!         .para(|i| {
//!             i.text("This is ")
//!                 .emph(|i| i.text("emphasized"))
//!                 .text(" and ")
//!                 .bold(|i| i.text("bold"))
//!                 .text(" text.")
//!         })
//!         .math_display("E = mc^2")
//! });
//! ```

use rescribe_core::{Document, Node};
use rescribe_std::{node, prop};

/// Build a LaTeX document with type-safe structure.
pub fn latex<F>(f: F) -> Document
where
    F: FnOnce(LatexBuilder) -> LatexBuilder,
{
    let builder = f(LatexBuilder::new());
    Document::new().with_content(builder.build())
}

/// Builder for LaTeX document structure.
/// Only exposes block-level elements that LaTeX supports well.
#[derive(Default)]
pub struct LatexBuilder {
    children: Vec<Node>,
}

impl LatexBuilder {
    fn new() -> Self {
        Self::default()
    }

    // Sectioning commands (LaTeX's strength)

    /// Add a section (\\section{}).
    pub fn section<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 1i64)
                .children(inner.children),
        );
        self
    }

    /// Add a subsection (\\subsection{}).
    pub fn subsection<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 2i64)
                .children(inner.children),
        );
        self
    }

    /// Add a subsubsection (\\subsubsection{}).
    pub fn subsubsection<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 3i64)
                .children(inner.children),
        );
        self
    }

    /// Add a paragraph heading (\\paragraph{}).
    pub fn paragraph_heading<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 4i64)
                .children(inner.children),
        );
        self
    }

    /// Add a subparagraph heading (\\subparagraph{}).
    pub fn subparagraph<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 5i64)
                .children(inner.children),
        );
        self
    }

    /// Add a paragraph of text.
    pub fn para<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::PARAGRAPH).children(inner.children));
        self
    }

    // Environments

    /// Add an itemize (bullet) list.
    pub fn itemize<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexList) -> LatexList,
    {
        let list = f(LatexList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, false)
                .children(list.items),
        );
        self
    }

    /// Add an enumerate (numbered) list.
    pub fn enumerate<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexList) -> LatexList,
    {
        let list = f(LatexList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .children(list.items),
        );
        self
    }

    /// Add a description list.
    pub fn description<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexDescList) -> LatexDescList,
    {
        let list = f(LatexDescList::new());
        self.children
            .push(Node::new(node::DEFINITION_LIST).children(list.items));
        self
    }

    /// Add a quote environment.
    pub fn quote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexBuilder) -> LatexBuilder,
    {
        let inner = f(LatexBuilder::new());
        self.children
            .push(Node::new(node::BLOCKQUOTE).children(inner.children));
        self
    }

    /// Add a verbatim code block (no syntax highlighting).
    pub fn verbatim(mut self, code: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code.into()));
        self
    }

    /// Add a lstlisting code block with language.
    pub fn lstlisting(mut self, language: impl Into<String>, code: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::CODE_BLOCK)
                .prop(prop::LANGUAGE, language.into())
                .prop(prop::CONTENT, code.into()),
        );
        self
    }

    // Math (LaTeX's specialty)

    /// Add display math (\\[ ... \\]).
    pub fn math_display(mut self, latex_src: impl Into<String>) -> Self {
        self.children
            .push(Node::new("math_display").prop("math:source", latex_src.into()));
        self
    }

    // Floats

    /// Add a figure environment.
    pub fn figure<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexFigure) -> LatexFigure,
    {
        let fig = f(LatexFigure::new());
        self.children
            .push(Node::new(node::FIGURE).children(fig.children));
        self
    }

    /// Add a basic table (tabular environment).
    pub fn tabular<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexTable) -> LatexTable,
    {
        let table = f(LatexTable::new());
        self.children.push(table.build());
        self
    }

    /// Add a horizontal rule (\\hrulefill).
    pub fn hrule(mut self) -> Self {
        self.children.push(Node::new(node::HORIZONTAL_RULE));
        self
    }

    /// Add raw LaTeX code.
    pub fn raw(mut self, latex_code: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "latex")
                .prop(prop::CONTENT, latex_code.into()),
        );
        self
    }

    fn build(self) -> Node {
        Node::new(node::DOCUMENT).children(self.children)
    }
}

/// Builder for LaTeX inline content.
/// Only exposes inline elements that LaTeX supports well.
#[derive(Default)]
pub struct LatexInline {
    children: Vec<Node>,
}

impl LatexInline {
    fn new() -> Self {
        Self::default()
    }

    /// Add plain text.
    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::TEXT).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add emphasized text (\\emph{}).
    pub fn emph<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::EMPHASIS).children(inner.children));
        self
    }

    /// Add bold text (\\textbf{}).
    pub fn bold<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::STRONG).children(inner.children));
        self
    }

    /// Add typewriter/monospace text (\\texttt{}).
    pub fn texttt(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add small caps (\\textsc{}).
    pub fn smallcaps<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::SMALL_CAPS).children(inner.children));
        self
    }

    /// Add subscript (\\textsubscript{}).
    pub fn textsubscript<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::SUBSCRIPT).children(inner.children));
        self
    }

    /// Add superscript (\\textsuperscript{}).
    pub fn textsuperscript<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::SUPERSCRIPT).children(inner.children));
        self
    }

    /// Add inline math ($...$).
    pub fn math(mut self, latex_src: impl Into<String>) -> Self {
        self.children
            .push(Node::new("math_inline").prop("math:source", latex_src.into()));
        self
    }

    /// Add a hyperlink (\\href{}{}).
    pub fn href<F>(mut self, url: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::LINK)
                .prop(prop::URL, url.into())
                .children(inner.children),
        );
        self
    }

    /// Add a footnote.
    pub fn footnote<F>(mut self, label: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let label = label.into();
        let inner = f(LatexInline::new());

        // Add reference
        self.children
            .push(Node::new(node::FOOTNOTE_REF).prop(prop::LABEL, label.clone()));

        // Add definition (will be collected at end)
        self.children.push(
            Node::new(node::FOOTNOTE_DEF)
                .prop(prop::LABEL, label)
                .children(inner.children),
        );
        self
    }

    /// Add double quotes (``...'').
    pub fn enquote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::QUOTED)
                .prop(prop::QUOTE_TYPE, "double")
                .children(inner.children),
        );
        self
    }

    /// Add single quotes (`...').
    pub fn enquote_single<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children.push(
            Node::new(node::QUOTED)
                .prop(prop::QUOTE_TYPE, "single")
                .children(inner.children),
        );
        self
    }

    /// Add a line break (\\\\).
    pub fn linebreak(mut self) -> Self {
        self.children.push(Node::new(node::LINE_BREAK));
        self
    }

    /// Add raw inline LaTeX.
    pub fn raw(mut self, latex_code: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_INLINE)
                .prop(prop::FORMAT, "latex")
                .prop(prop::CONTENT, latex_code.into()),
        );
        self
    }
}

/// Builder for LaTeX itemize/enumerate lists.
#[derive(Default)]
pub struct LatexList {
    items: Vec<Node>,
}

impl LatexList {
    fn new() -> Self {
        Self::default()
    }

    /// Add a list item.
    pub fn item<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inline = f(LatexInline::new());
        let item = Node::new(node::LIST_ITEM)
            .children(vec![Node::new(node::PARAGRAPH).children(inline.children)]);
        self.items.push(item);
        self
    }

    /// Add a list item with nested content (can contain sublists).
    pub fn item_block<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexBuilder) -> LatexBuilder,
    {
        let inner = f(LatexBuilder::new());
        let item = Node::new(node::LIST_ITEM).children(inner.children);
        self.items.push(item);
        self
    }
}

/// Builder for LaTeX description lists.
#[derive(Default)]
pub struct LatexDescList {
    items: Vec<Node>,
}

impl LatexDescList {
    fn new() -> Self {
        Self::default()
    }

    /// Add a description item with term and definition.
    pub fn item<F, G>(mut self, term: F, desc: G) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
        G: FnOnce(LatexInline) -> LatexInline,
    {
        let term_content = term(LatexInline::new());
        let desc_content = desc(LatexInline::new());

        self.items
            .push(Node::new(node::DEFINITION_TERM).children(term_content.children));
        self.items
            .push(Node::new(node::DEFINITION_DESC).children(vec![
                Node::new(node::PARAGRAPH).children(desc_content.children),
            ]));
        self
    }
}

/// Builder for LaTeX figures.
#[derive(Default)]
pub struct LatexFigure {
    children: Vec<Node>,
}

impl LatexFigure {
    fn new() -> Self {
        Self::default()
    }

    /// Add an image (\\includegraphics{}).
    pub fn includegraphics(mut self, path: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::IMAGE)
                .prop(prop::URL, path.into())
                .prop(prop::ALT, ""),
        );
        self
    }

    /// Add a caption (\\caption{}).
    pub fn caption<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        self.children
            .push(Node::new(node::CAPTION).children(inner.children));
        self
    }
}

/// Builder for LaTeX tables (tabular environment).
/// Note: LaTeX tables without extra packages have limited colspan/rowspan support.
#[derive(Default)]
pub struct LatexTable {
    rows: Vec<Node>,
}

impl LatexTable {
    fn new() -> Self {
        Self::default()
    }

    /// Add a header row.
    pub fn header<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexTableRow) -> LatexTableRow,
    {
        let row = f(LatexTableRow::new_header());
        self.rows
            .push(Node::new(node::TABLE_HEAD).children(vec![row.build()]));
        self
    }

    /// Add a body row.
    pub fn row<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexTableRow) -> LatexTableRow,
    {
        let row = f(LatexTableRow::new());
        self.rows.push(row.build());
        self
    }

    fn build(self) -> Node {
        Node::new(node::TABLE).children(self.rows)
    }
}

/// Builder for LaTeX table rows.
pub struct LatexTableRow {
    cells: Vec<Node>,
    is_header: bool,
}

impl LatexTableRow {
    fn new() -> Self {
        Self {
            cells: Vec::new(),
            is_header: false,
        }
    }

    fn new_header() -> Self {
        Self {
            cells: Vec::new(),
            is_header: true,
        }
    }

    /// Add a cell.
    pub fn cell<F>(mut self, f: F) -> Self
    where
        F: FnOnce(LatexInline) -> LatexInline,
    {
        let inner = f(LatexInline::new());
        let kind = if self.is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };
        self.cells.push(Node::new(kind).children(inner.children));
        self
    }

    fn build(self) -> Node {
        Node::new(node::TABLE_ROW).children(self.cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit;

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    #[test]
    fn test_section_and_para() {
        let doc = latex(|d| {
            d.section(|i| i.text("Introduction"))
                .para(|i| i.text("Hello, world!"))
        });

        let output = emit_str(&doc);
        assert!(output.contains("\\section{Introduction}"));
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_inline_formatting() {
        let doc = latex(|d| {
            d.para(|i| {
                i.text("This is ")
                    .emph(|i| i.text("emphasized"))
                    .text(" and ")
                    .bold(|i| i.text("bold"))
                    .text(".")
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("\\emph{emphasized}"));
        assert!(output.contains("\\textbf{bold}"));
    }

    #[test]
    fn test_math() {
        let doc = latex(|d| {
            d.para(|i| i.text("The equation ").math("E = mc^2").text(" is famous."))
                .math_display("\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}")
        });

        let output = emit_str(&doc);
        assert!(output.contains("$E = mc^2$"));
        assert!(output.contains("\\["));
        assert!(output.contains("\\int_0^\\infty"));
    }

    #[test]
    fn test_lists() {
        let doc = latex(|d| {
            d.itemize(|l| l.item(|i| i.text("First")).item(|i| i.text("Second")))
                .enumerate(|l| l.item(|i| i.text("One")).item(|i| i.text("Two")))
        });

        let output = emit_str(&doc);
        assert!(output.contains("\\begin{itemize}"));
        assert!(output.contains("\\begin{enumerate}"));
        assert!(output.contains("\\item First"));
        assert!(output.contains("\\item One"));
    }

    #[test]
    fn test_description_list() {
        let doc = latex(|d| {
            d.description(|l| {
                l.item(|t| t.text("Term1"), |d| d.text("Definition 1"))
                    .item(|t| t.text("Term2"), |d| d.text("Definition 2"))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("\\begin{description}"));
        assert!(output.contains("\\item[Term1]"));
        assert!(output.contains("Definition 1"));
    }

    #[test]
    fn test_code() {
        let doc = latex(|d| d.verbatim("fn main() {}").lstlisting("rust", "let x = 42;"));

        let output = emit_str(&doc);
        assert!(output.contains("\\begin{verbatim}"));
        assert!(output.contains("\\begin{lstlisting}[language=rust]"));
    }

    #[test]
    fn test_figure() {
        let doc = latex(|d| {
            d.figure(|f| {
                f.includegraphics("image.png")
                    .caption(|i| i.text("A figure"))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("\\begin{figure}"));
        assert!(output.contains("\\includegraphics{image.png}"));
        assert!(output.contains("\\caption{A figure}"));
    }

    #[test]
    fn test_table() {
        let doc = latex(|d| {
            d.tabular(|t| {
                t.header(|r| r.cell(|i| i.text("A")).cell(|i| i.text("B")))
                    .row(|r| r.cell(|i| i.text("1")).cell(|i| i.text("2")))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("\\begin{tabular}"));
        assert!(output.contains("A & B"));
        assert!(output.contains("1 & 2"));
    }
}
