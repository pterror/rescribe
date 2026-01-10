//! Type-safe plain text document builder.
//!
//! Plain text has minimal formatting. This builder provides structure
//! through headings (underlined), paragraphs, and lists.
//!
//! # Example
//!
//! ```
//! use rescribe_write_plaintext::builder::*;
//!
//! let doc = plaintext(|d| {
//!     d.heading(1, "Introduction")
//!         .para("This is a paragraph of text.")
//!         .list(|l| {
//!             l.item("First item")
//!                 .item("Second item")
//!         })
//! });
//! ```

use rescribe_core::{Document, Node};
use rescribe_std::{node, prop};

/// Build a plain text document.
pub fn plaintext<F>(f: F) -> Document
where
    F: FnOnce(PlainBuilder) -> PlainBuilder,
{
    let builder = f(PlainBuilder::new());
    Document::new().with_content(builder.build())
}

/// Builder for plain text documents.
/// Focuses on structural elements since text has no inline formatting.
#[derive(Default)]
pub struct PlainBuilder {
    children: Vec<Node>,
}

impl PlainBuilder {
    fn new() -> Self {
        Self::default()
    }

    /// Add a heading (rendered with underlines in output).
    pub fn heading(mut self, level: i64, text: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level)
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text.into())]),
        );
        self
    }

    /// Add a paragraph.
    pub fn para(mut self, text: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::PARAGRAPH)
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text.into())]),
        );
        self
    }

    /// Add a bullet list.
    pub fn list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(PlainList) -> PlainList,
    {
        let list = f(PlainList::new(false));
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, false)
                .children(list.items),
        );
        self
    }

    /// Add a numbered list.
    pub fn numbered_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(PlainList) -> PlainList,
    {
        let list = f(PlainList::new(true));
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .children(list.items),
        );
        self
    }

    /// Add a blockquote.
    pub fn quote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(PlainBuilder) -> PlainBuilder,
    {
        let inner = f(PlainBuilder::new());
        self.children
            .push(Node::new(node::BLOCKQUOTE).children(inner.children));
        self
    }

    /// Add a code block (rendered as-is with indentation).
    pub fn code_block(mut self, code: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code.into()));
        self
    }

    /// Add a horizontal rule.
    pub fn hr(mut self) -> Self {
        self.children.push(Node::new(node::HORIZONTAL_RULE));
        self
    }

    /// Add a simple table.
    pub fn table<F>(mut self, f: F) -> Self
    where
        F: FnOnce(PlainTable) -> PlainTable,
    {
        let table = f(PlainTable::new());
        self.children.push(table.build());
        self
    }

    fn build(self) -> Node {
        Node::new(node::DOCUMENT).children(self.children)
    }
}

/// Builder for plain text lists.
pub struct PlainList {
    items: Vec<Node>,
    #[allow(dead_code)]
    ordered: bool,
}

impl PlainList {
    fn new(ordered: bool) -> Self {
        Self {
            items: Vec::new(),
            ordered,
        }
    }

    /// Add a list item.
    pub fn item(mut self, text: impl Into<String>) -> Self {
        let item = Node::new(node::LIST_ITEM)
            .children(vec![Node::new(node::PARAGRAPH).children(vec![
                Node::new(node::TEXT).prop(prop::CONTENT, text.into()),
            ])]);
        self.items.push(item);
        self
    }

    /// Add a list item with nested content.
    pub fn item_block<F>(mut self, f: F) -> Self
    where
        F: FnOnce(PlainBuilder) -> PlainBuilder,
    {
        let inner = f(PlainBuilder::new());
        let item = Node::new(node::LIST_ITEM).children(inner.children);
        self.items.push(item);
        self
    }
}

/// Builder for plain text tables.
#[derive(Default)]
pub struct PlainTable {
    rows: Vec<Vec<String>>,
}

impl PlainTable {
    fn new() -> Self {
        Self::default()
    }

    /// Add a row of cells.
    pub fn row(mut self, cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.rows
            .push(cells.into_iter().map(|s| s.into()).collect());
        self
    }

    fn build(self) -> Node {
        let body_rows: Vec<Node> = self
            .rows
            .into_iter()
            .map(|cells| {
                let cell_nodes: Vec<Node> = cells
                    .into_iter()
                    .map(|text| {
                        Node::new(node::TABLE_CELL)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text)])
                    })
                    .collect();
                Node::new(node::TABLE_ROW).children(cell_nodes)
            })
            .collect();

        Node::new(node::TABLE).children(vec![Node::new(node::TABLE_BODY).children(body_rows)])
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
        let doc = plaintext(|d| d.heading(1, "Title").para("Hello world."));

        let output = emit_str(&doc);
        assert!(output.contains("Title"));
        assert!(output.contains("Hello world."));
    }

    #[test]
    fn test_list() {
        let doc = plaintext(|d| d.list(|l| l.item("First").item("Second")));

        let output = emit_str(&doc);
        assert!(output.contains("- First"));
        assert!(output.contains("- Second"));
    }

    #[test]
    fn test_numbered_list() {
        let doc = plaintext(|d| d.numbered_list(|l| l.item("One").item("Two")));

        let output = emit_str(&doc);
        assert!(output.contains("1.") || output.contains("1)"));
        assert!(output.contains("One"));
        assert!(output.contains("Two"));
    }

    #[test]
    fn test_code_block() {
        let doc = plaintext(|d| d.code_block("let x = 42;"));

        let output = emit_str(&doc);
        assert!(output.contains("let x = 42;"));
    }

    #[test]
    fn test_table() {
        let doc = plaintext(|d| d.table(|t| t.row(["A", "B"]).row(["1", "2"])));

        let output = emit_str(&doc);
        assert!(output.contains("A"));
        assert!(output.contains("B"));
        assert!(output.contains("1"));
        assert!(output.contains("2"));
    }
}
