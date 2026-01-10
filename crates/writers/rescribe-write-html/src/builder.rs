//! Type-safe HTML document builder.
//!
//! HTML is flexible but this builder provides a structured API
//! that maps cleanly to semantic HTML5.
//!
//! # Example
//!
//! ```
//! use rescribe_write_html::builder::*;
//!
//! let doc = html(|d| {
//!     d.h1(|i| i.text("Welcome"))
//!         .p(|i| {
//!             i.text("This is ")
//!                 .em(|i| i.text("emphasized"))
//!                 .text(" text.")
//!         })
//!         .ul(|l| {
//!             l.li(|i| i.text("First"))
//!                 .li(|i| i.text("Second"))
//!         })
//! });
//! ```

use rescribe_core::{Document, Node};
use rescribe_std::{node, prop};

/// Build an HTML document with type-safe structure.
pub fn html<F>(f: F) -> Document
where
    F: FnOnce(HtmlBuilder) -> HtmlBuilder,
{
    let builder = f(HtmlBuilder::new());
    Document::new().with_content(builder.build())
}

/// Builder for HTML document structure.
#[derive(Default)]
pub struct HtmlBuilder {
    children: Vec<Node>,
}

impl HtmlBuilder {
    fn new() -> Self {
        Self::default()
    }

    // Headings

    /// Add an h1 heading.
    pub fn h1<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 1i64)
                .children(inner.children),
        );
        self
    }

    /// Add an h2 heading.
    pub fn h2<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 2i64)
                .children(inner.children),
        );
        self
    }

    /// Add an h3 heading.
    pub fn h3<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 3i64)
                .children(inner.children),
        );
        self
    }

    /// Add an h4 heading.
    pub fn h4<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 4i64)
                .children(inner.children),
        );
        self
    }

    /// Add an h5 heading.
    pub fn h5<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 5i64)
                .children(inner.children),
        );
        self
    }

    /// Add an h6 heading.
    pub fn h6<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 6i64)
                .children(inner.children),
        );
        self
    }

    /// Add a paragraph.
    pub fn p<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::PARAGRAPH).children(inner.children));
        self
    }

    /// Add an unordered list.
    pub fn ul<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlList) -> HtmlList,
    {
        let list = f(HtmlList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, false)
                .children(list.items),
        );
        self
    }

    /// Add an ordered list.
    pub fn ol<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlList) -> HtmlList,
    {
        let list = f(HtmlList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .children(list.items),
        );
        self
    }

    /// Add an ordered list with start value.
    pub fn ol_start<F>(mut self, start: i64, f: F) -> Self
    where
        F: FnOnce(HtmlList) -> HtmlList,
    {
        let list = f(HtmlList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .prop(prop::START, start)
                .children(list.items),
        );
        self
    }

    /// Add a definition list.
    pub fn dl<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlDefList) -> HtmlDefList,
    {
        let list = f(HtmlDefList::new());
        self.children
            .push(Node::new(node::DEFINITION_LIST).children(list.items));
        self
    }

    /// Add a blockquote.
    pub fn blockquote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlBuilder) -> HtmlBuilder,
    {
        let inner = f(HtmlBuilder::new());
        self.children
            .push(Node::new(node::BLOCKQUOTE).children(inner.children));
        self
    }

    /// Add a preformatted code block.
    pub fn pre(mut self, code: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code.into()));
        self
    }

    /// Add a code block with language class.
    pub fn pre_lang(mut self, language: impl Into<String>, code: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::CODE_BLOCK)
                .prop(prop::LANGUAGE, language.into())
                .prop(prop::CONTENT, code.into()),
        );
        self
    }

    /// Add a horizontal rule.
    pub fn hr(mut self) -> Self {
        self.children.push(Node::new(node::HORIZONTAL_RULE));
        self
    }

    /// Add a figure with optional caption.
    pub fn figure<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlFigure) -> HtmlFigure,
    {
        let fig = f(HtmlFigure::new());
        self.children
            .push(Node::new(node::FIGURE).children(fig.children));
        self
    }

    /// Add a table.
    pub fn table<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlTable) -> HtmlTable,
    {
        let table = f(HtmlTable::new());
        self.children.push(table.build());
        self
    }

    /// Add a div container.
    pub fn container<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlBuilder) -> HtmlBuilder,
    {
        let inner = f(HtmlBuilder::new());
        self.children
            .push(Node::new(node::DIV).children(inner.children));
        self
    }

    /// Add a div with CSS class.
    pub fn div_class<F>(mut self, class: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(HtmlBuilder) -> HtmlBuilder,
    {
        let inner = f(HtmlBuilder::new());
        self.children.push(
            Node::new(node::DIV)
                .prop("html:class", class.into())
                .children(inner.children),
        );
        self
    }

    /// Add a div with ID.
    pub fn div_id<F>(mut self, id: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(HtmlBuilder) -> HtmlBuilder,
    {
        let inner = f(HtmlBuilder::new());
        self.children.push(
            Node::new(node::DIV)
                .prop(prop::ID, id.into())
                .children(inner.children),
        );
        self
    }

    /// Add raw HTML.
    pub fn raw(mut self, html_content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "html")
                .prop(prop::CONTENT, html_content.into()),
        );
        self
    }

    fn build(self) -> Node {
        Node::new(node::DOCUMENT).children(self.children)
    }
}

/// Builder for HTML inline content.
#[derive(Default)]
pub struct HtmlInline {
    children: Vec<Node>,
}

impl HtmlInline {
    fn new() -> Self {
        Self::default()
    }

    /// Add plain text.
    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::TEXT).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add emphasized text (<em>).
    pub fn em<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::EMPHASIS).children(inner.children));
        self
    }

    /// Add strong text (<strong>).
    pub fn strong<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::STRONG).children(inner.children));
        self
    }

    /// Add strikethrough text (<del> or <s>).
    pub fn del<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::STRIKEOUT).children(inner.children));
        self
    }

    /// Add underlined text (<u>).
    pub fn u<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::UNDERLINE).children(inner.children));
        self
    }

    /// Add subscript text (<sub>).
    pub fn subscript<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::SUBSCRIPT).children(inner.children));
        self
    }

    /// Add superscript text (<sup>).
    pub fn sup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::SUPERSCRIPT).children(inner.children));
        self
    }

    /// Add small caps.
    pub fn smallcaps<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::SMALL_CAPS).children(inner.children));
        self
    }

    /// Add inline code (<code>).
    pub fn code(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add a link (<a>).
    pub fn a<F>(mut self, href: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::LINK)
                .prop(prop::URL, href.into())
                .children(inner.children),
        );
        self
    }

    /// Add a link with title.
    pub fn a_titled<F>(mut self, href: impl Into<String>, title: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::LINK)
                .prop(prop::URL, href.into())
                .prop(prop::TITLE, title.into())
                .children(inner.children),
        );
        self
    }

    /// Add an image (<img>).
    pub fn img(mut self, src: impl Into<String>, alt: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::IMAGE)
                .prop(prop::URL, src.into())
                .prop(prop::ALT, alt.into()),
        );
        self
    }

    /// Add a line break (<br>).
    pub fn br(mut self) -> Self {
        self.children.push(Node::new(node::LINE_BREAK));
        self
    }

    /// Add a span.
    pub fn span<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::SPAN).children(inner.children));
        self
    }

    /// Add a span with CSS class.
    pub fn span_class<F>(mut self, class: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children.push(
            Node::new(node::SPAN)
                .prop("html:class", class.into())
                .children(inner.children),
        );
        self
    }

    /// Add raw inline HTML.
    pub fn raw(mut self, html_content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_INLINE)
                .prop(prop::FORMAT, "html")
                .prop(prop::CONTENT, html_content.into()),
        );
        self
    }
}

/// Builder for HTML lists.
#[derive(Default)]
pub struct HtmlList {
    items: Vec<Node>,
}

impl HtmlList {
    fn new() -> Self {
        Self::default()
    }

    /// Add a list item with inline content.
    pub fn li<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inline = f(HtmlInline::new());
        let item = Node::new(node::LIST_ITEM)
            .children(vec![Node::new(node::PARAGRAPH).children(inline.children)]);
        self.items.push(item);
        self
    }

    /// Add a list item with block content.
    pub fn li_block<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlBuilder) -> HtmlBuilder,
    {
        let inner = f(HtmlBuilder::new());
        let item = Node::new(node::LIST_ITEM).children(inner.children);
        self.items.push(item);
        self
    }
}

/// Builder for HTML definition lists.
#[derive(Default)]
pub struct HtmlDefList {
    items: Vec<Node>,
}

impl HtmlDefList {
    fn new() -> Self {
        Self::default()
    }

    /// Add a term and definition.
    pub fn item<F, G>(mut self, term: F, def: G) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
        G: FnOnce(HtmlBuilder) -> HtmlBuilder,
    {
        let term_content = term(HtmlInline::new());
        let def_content = def(HtmlBuilder::new());

        self.items
            .push(Node::new(node::DEFINITION_TERM).children(term_content.children));
        self.items
            .push(Node::new(node::DEFINITION_DESC).children(def_content.children));
        self
    }
}

/// Builder for HTML figures.
#[derive(Default)]
pub struct HtmlFigure {
    children: Vec<Node>,
}

impl HtmlFigure {
    fn new() -> Self {
        Self::default()
    }

    /// Add an image to the figure.
    pub fn img(mut self, src: impl Into<String>, alt: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::IMAGE)
                .prop(prop::URL, src.into())
                .prop(prop::ALT, alt.into()),
        );
        self
    }

    /// Add a figcaption.
    pub fn figcaption<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        self.children
            .push(Node::new(node::CAPTION).children(inner.children));
        self
    }
}

/// Builder for HTML tables.
#[derive(Default)]
pub struct HtmlTable {
    thead: Vec<Node>,
    tbody: Vec<Node>,
    tfoot: Vec<Node>,
}

impl HtmlTable {
    fn new() -> Self {
        Self::default()
    }

    /// Add a header row.
    pub fn thead<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlTableSection) -> HtmlTableSection,
    {
        let section = f(HtmlTableSection::new(true));
        self.thead = section.rows;
        self
    }

    /// Add body rows.
    pub fn tbody<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlTableSection) -> HtmlTableSection,
    {
        let section = f(HtmlTableSection::new(false));
        self.tbody = section.rows;
        self
    }

    /// Add footer rows.
    pub fn tfoot<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlTableSection) -> HtmlTableSection,
    {
        let section = f(HtmlTableSection::new(false));
        self.tfoot = section.rows;
        self
    }

    fn build(self) -> Node {
        let mut children = Vec::new();

        if !self.thead.is_empty() {
            children.push(Node::new(node::TABLE_HEAD).children(self.thead));
        }
        if !self.tbody.is_empty() {
            children.push(Node::new(node::TABLE_BODY).children(self.tbody));
        }
        if !self.tfoot.is_empty() {
            children.push(Node::new(node::TABLE_FOOT).children(self.tfoot));
        }

        Node::new(node::TABLE).children(children)
    }
}

/// Builder for table sections (thead, tbody, tfoot).
pub struct HtmlTableSection {
    rows: Vec<Node>,
    is_header: bool,
}

impl HtmlTableSection {
    fn new(is_header: bool) -> Self {
        Self {
            rows: Vec::new(),
            is_header,
        }
    }

    /// Add a row.
    pub fn tr<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlTableRow) -> HtmlTableRow,
    {
        let row = f(HtmlTableRow::new(self.is_header));
        self.rows
            .push(Node::new(node::TABLE_ROW).children(row.cells));
        self
    }
}

/// Builder for table rows.
pub struct HtmlTableRow {
    cells: Vec<Node>,
    is_header: bool,
}

impl HtmlTableRow {
    fn new(is_header: bool) -> Self {
        Self {
            cells: Vec::new(),
            is_header,
        }
    }

    /// Add a cell.
    pub fn cell<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        let kind = if self.is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };
        self.cells.push(Node::new(kind).children(inner.children));
        self
    }

    /// Add a cell with colspan.
    pub fn cell_colspan<F>(mut self, colspan: i64, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        let kind = if self.is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };
        self.cells.push(
            Node::new(kind)
                .prop(prop::COLSPAN, colspan)
                .children(inner.children),
        );
        self
    }

    /// Add a cell with rowspan.
    pub fn cell_rowspan<F>(mut self, rowspan: i64, f: F) -> Self
    where
        F: FnOnce(HtmlInline) -> HtmlInline,
    {
        let inner = f(HtmlInline::new());
        let kind = if self.is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };
        self.cells.push(
            Node::new(kind)
                .prop(prop::ROWSPAN, rowspan)
                .children(inner.children),
        );
        self
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
    fn test_heading_and_para() {
        let doc = html(|d| d.h1(|i| i.text("Hello")).p(|i| i.text("World")));

        let output = emit_str(&doc);
        assert!(output.contains("<h1>Hello</h1>"));
        assert!(output.contains("<p>World</p>"));
    }

    #[test]
    fn test_inline_formatting() {
        let doc = html(|d| {
            d.p(|i| {
                i.text("This is ")
                    .em(|i| i.text("italic"))
                    .text(" and ")
                    .strong(|i| i.text("bold"))
                    .text(".")
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("<em>italic</em>"));
        assert!(output.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_links() {
        let doc = html(|d| d.p(|i| i.a("https://example.com", |i| i.text("Example"))));

        let output = emit_str(&doc);
        assert!(output.contains("<a href=\"https://example.com\">Example</a>"));
    }

    #[test]
    fn test_lists() {
        let doc = html(|d| {
            d.ul(|l| l.li(|i| i.text("First")).li(|i| i.text("Second")))
                .ol(|l| l.li(|i| i.text("One")).li(|i| i.text("Two")))
        });

        let output = emit_str(&doc);
        assert!(output.contains("<ul>"));
        assert!(output.contains("<li>"));
        assert!(output.contains("<ol>"));
    }

    #[test]
    fn test_table() {
        let doc = html(|d| {
            d.table(|t| {
                t.thead(|s| s.tr(|r| r.cell(|i| i.text("A")).cell(|i| i.text("B"))))
                    .tbody(|s| s.tr(|r| r.cell(|i| i.text("1")).cell(|i| i.text("2"))))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("<table>"));
        assert!(output.contains("<thead>"));
        assert!(output.contains("<th>A</th>"));
        assert!(output.contains("<td>1</td>"));
    }

    #[test]
    fn test_figure() {
        let doc = html(|d| {
            d.figure(|f| {
                f.img("photo.jpg", "A photo")
                    .figcaption(|i| i.text("Caption"))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("<figure>"));
        assert!(output.contains("<img"));
        assert!(output.contains("<figcaption>Caption</figcaption>"));
    }

    #[test]
    fn test_sub_sup() {
        let doc = html(|d| {
            d.p(|i| {
                i.text("H")
                    .subscript(|i| i.text("2"))
                    .text("O and x")
                    .sup(|i| i.text("2"))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("<sub>2</sub>"));
        assert!(output.contains("<sup>2</sup>"));
    }
}
