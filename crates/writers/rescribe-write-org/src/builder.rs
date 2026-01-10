//! Type-safe Org-mode document builder.
//!
//! This builder exposes elements that Org-mode supports natively.
//!
//! # Example
//!
//! ```
//! use rescribe_write_org::builder::*;
//!
//! let doc = org(|d| {
//!     d.heading(1, |i| i.text("Introduction"))
//!         .para(|i| {
//!             i.text("This is ")
//!                 .italic(|i| i.text("emphasized"))
//!                 .text(" and ")
//!                 .bold(|i| i.text("strong"))
//!                 .text(".")
//!         })
//!         .src_block("python", "print('hello')")
//! });
//! ```

use rescribe_core::{Document, Node};
use rescribe_std::{node, prop};

/// Build an Org-mode document with type-safe structure.
pub fn org<F>(f: F) -> Document
where
    F: FnOnce(OrgBuilder) -> OrgBuilder,
{
    let builder = f(OrgBuilder::new());
    Document::new().with_content(builder.build())
}

/// Builder for Org-mode document structure.
#[derive(Default)]
pub struct OrgBuilder {
    children: Vec<Node>,
}

impl OrgBuilder {
    fn new() -> Self {
        Self::default()
    }

    /// Add a heading at the given level (1-based, uses * prefix).
    pub fn heading<F>(mut self, level: i64, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level)
                .children(inner.children),
        );
        self
    }

    /// Add a paragraph.
    pub fn para<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::PARAGRAPH).children(inner.children));
        self
    }

    /// Add an unordered list.
    pub fn unordered_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgList) -> OrgList,
    {
        let list = f(OrgList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, false)
                .children(list.items),
        );
        self
    }

    /// Add an ordered list.
    pub fn ordered_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgList) -> OrgList,
    {
        let list = f(OrgList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .children(list.items),
        );
        self
    }

    /// Add a description list.
    pub fn description_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgDescList) -> OrgDescList,
    {
        let list = f(OrgDescList::new());
        self.children
            .push(Node::new(node::DEFINITION_LIST).children(list.items));
        self
    }

    /// Add a blockquote (#+BEGIN_QUOTE).
    pub fn quote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgBuilder) -> OrgBuilder,
    {
        let inner = f(OrgBuilder::new());
        self.children
            .push(Node::new(node::BLOCKQUOTE).children(inner.children));
        self
    }

    /// Add a source code block (#+BEGIN_SRC).
    pub fn src_block(mut self, language: impl Into<String>, code: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::CODE_BLOCK)
                .prop(prop::LANGUAGE, language.into())
                .prop(prop::CONTENT, code.into()),
        );
        self
    }

    /// Add an example block (#+BEGIN_EXAMPLE).
    pub fn example_block(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add a horizontal rule (-----).
    pub fn hr(mut self) -> Self {
        self.children.push(Node::new(node::HORIZONTAL_RULE));
        self
    }

    /// Add a table.
    pub fn table<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgTable) -> OrgTable,
    {
        let table = f(OrgTable::new());
        self.children.push(table.build());
        self
    }

    /// Add display math (\[...\]).
    pub fn math_display(mut self, latex_src: impl Into<String>) -> Self {
        self.children
            .push(Node::new("math_display").prop("math:source", latex_src.into()));
        self
    }

    /// Add raw Org content.
    pub fn raw(mut self, content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "org")
                .prop(prop::CONTENT, content.into()),
        );
        self
    }

    fn build(self) -> Node {
        Node::new(node::DOCUMENT).children(self.children)
    }
}

/// Builder for Org-mode inline content.
#[derive(Default)]
pub struct OrgInline {
    children: Vec<Node>,
}

impl OrgInline {
    fn new() -> Self {
        Self::default()
    }

    /// Add plain text.
    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::TEXT).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add italic text (/italic/).
    pub fn italic<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::EMPHASIS).children(inner.children));
        self
    }

    /// Add bold text (*bold*).
    pub fn bold<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::STRONG).children(inner.children));
        self
    }

    /// Add strikethrough text (+deleted+).
    pub fn strikethrough<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::STRIKEOUT).children(inner.children));
        self
    }

    /// Add underlined text (_underline_).
    pub fn underline<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::UNDERLINE).children(inner.children));
        self
    }

    /// Add verbatim/code text (=code=).
    pub fn verbatim(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add subscript (_{sub}).
    pub fn subscript<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::SUBSCRIPT).children(inner.children));
        self
    }

    /// Add superscript (^{sup}).
    pub fn superscript<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children
            .push(Node::new(node::SUPERSCRIPT).children(inner.children));
        self
    }

    /// Add a link ([[url][text]]).
    pub fn link<F>(mut self, url: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        self.children.push(
            Node::new(node::LINK)
                .prop(prop::URL, url.into())
                .children(inner.children),
        );
        self
    }

    /// Add an image ([[file:path]]).
    pub fn image(mut self, path: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::IMAGE)
                .prop(prop::URL, path.into())
                .prop(prop::ALT, ""),
        );
        self
    }

    /// Add inline math ($...$).
    pub fn math(mut self, latex_src: impl Into<String>) -> Self {
        self.children
            .push(Node::new("math_inline").prop("math:source", latex_src.into()));
        self
    }

    /// Add a footnote reference ([fn:label]).
    pub fn footnote_ref(mut self, label: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::FOOTNOTE_REF).prop(prop::LABEL, label.into()));
        self
    }

    /// Add a line break.
    pub fn linebreak(mut self) -> Self {
        self.children.push(Node::new(node::LINE_BREAK));
        self
    }

    /// Add raw inline Org.
    pub fn raw(mut self, content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_INLINE)
                .prop(prop::FORMAT, "org")
                .prop(prop::CONTENT, content.into()),
        );
        self
    }
}

/// Builder for Org-mode lists.
#[derive(Default)]
pub struct OrgList {
    items: Vec<Node>,
}

impl OrgList {
    fn new() -> Self {
        Self::default()
    }

    /// Add a list item with inline content.
    pub fn item<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inline = f(OrgInline::new());
        let item = Node::new(node::LIST_ITEM)
            .children(vec![Node::new(node::PARAGRAPH).children(inline.children)]);
        self.items.push(item);
        self
    }

    /// Add a list item with block content.
    pub fn item_block<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgBuilder) -> OrgBuilder,
    {
        let inner = f(OrgBuilder::new());
        let item = Node::new(node::LIST_ITEM).children(inner.children);
        self.items.push(item);
        self
    }
}

/// Builder for Org-mode description lists.
#[derive(Default)]
pub struct OrgDescList {
    items: Vec<Node>,
}

impl OrgDescList {
    fn new() -> Self {
        Self::default()
    }

    /// Add a term :: definition entry.
    pub fn item<F, G>(mut self, term: F, desc: G) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
        G: FnOnce(OrgInline) -> OrgInline,
    {
        let term_content = term(OrgInline::new());
        let desc_content = desc(OrgInline::new());

        self.items
            .push(Node::new(node::DEFINITION_TERM).children(term_content.children));
        self.items
            .push(Node::new(node::DEFINITION_DESC).children(vec![
                Node::new(node::PARAGRAPH).children(desc_content.children),
            ]));
        self
    }
}

/// Builder for Org-mode tables.
#[derive(Default)]
pub struct OrgTable {
    rows: Vec<Vec<Node>>,
    has_header: bool,
}

impl OrgTable {
    fn new() -> Self {
        Self::default()
    }

    /// Add a header row (with separator after).
    pub fn header<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgTableRow) -> OrgTableRow,
    {
        let row = f(OrgTableRow::new(true));
        self.rows.push(row.cells);
        self.has_header = true;
        self
    }

    /// Add a body row.
    pub fn row<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgTableRow) -> OrgTableRow,
    {
        let row = f(OrgTableRow::new(false));
        self.rows.push(row.cells);
        self
    }

    fn build(self) -> Node {
        let mut children = Vec::new();

        if self.has_header && !self.rows.is_empty() {
            // First row is header
            let header_row = Node::new(node::TABLE_ROW).children(self.rows[0].clone());
            children.push(Node::new(node::TABLE_HEAD).children(vec![header_row]));

            // Rest are body
            if self.rows.len() > 1 {
                let body_rows: Vec<Node> = self.rows[1..]
                    .iter()
                    .map(|cells| Node::new(node::TABLE_ROW).children(cells.clone()))
                    .collect();
                children.push(Node::new(node::TABLE_BODY).children(body_rows));
            }
        } else {
            // All rows are body
            let body_rows: Vec<Node> = self
                .rows
                .into_iter()
                .map(|cells| Node::new(node::TABLE_ROW).children(cells))
                .collect();
            if !body_rows.is_empty() {
                children.push(Node::new(node::TABLE_BODY).children(body_rows));
            }
        }

        Node::new(node::TABLE).children(children)
    }
}

/// Builder for table rows.
pub struct OrgTableRow {
    cells: Vec<Node>,
    is_header: bool,
}

impl OrgTableRow {
    fn new(is_header: bool) -> Self {
        Self {
            cells: Vec::new(),
            is_header,
        }
    }

    /// Add a cell.
    pub fn cell<F>(mut self, f: F) -> Self
    where
        F: FnOnce(OrgInline) -> OrgInline,
    {
        let inner = f(OrgInline::new());
        let kind = if self.is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };
        self.cells.push(Node::new(kind).children(inner.children));
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
        let doc = org(|d| d.heading(1, |i| i.text("Title")).para(|i| i.text("Hello")));

        let output = emit_str(&doc);
        assert!(output.contains("* Title"));
        assert!(output.contains("Hello"));
    }

    #[test]
    fn test_inline_formatting() {
        let doc = org(|d| {
            d.para(|i| {
                i.text("This is ")
                    .italic(|i| i.text("italic"))
                    .text(" and ")
                    .bold(|i| i.text("bold"))
                    .text(".")
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("/italic/"));
        assert!(output.contains("*bold*"));
    }

    #[test]
    fn test_code_block() {
        let doc = org(|d| d.src_block("python", "print('hello')"));

        let output = emit_str(&doc);
        assert!(output.contains("#+BEGIN_SRC python"));
        assert!(output.contains("print('hello')"));
        assert!(output.contains("#+END_SRC"));
    }

    #[test]
    fn test_lists() {
        let doc =
            org(|d| d.unordered_list(|l| l.item(|i| i.text("First")).item(|i| i.text("Second"))));

        let output = emit_str(&doc);
        assert!(output.contains("- First"));
        assert!(output.contains("- Second"));
    }

    #[test]
    fn test_links() {
        let doc = org(|d| d.para(|i| i.link("https://example.com", |i| i.text("Example"))));

        let output = emit_str(&doc);
        assert!(output.contains("[[https://example.com][Example]]"));
    }

    #[test]
    fn test_table() {
        let doc = org(|d| {
            d.table(|t| {
                t.header(|r| r.cell(|i| i.text("A")).cell(|i| i.text("B")))
                    .row(|r| r.cell(|i| i.text("1")).cell(|i| i.text("2")))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("| A | B |"));
        assert!(output.contains("| 1 | 2 |"));
    }
}
