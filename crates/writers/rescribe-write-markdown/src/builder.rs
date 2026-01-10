//! Type-safe Markdown document builder.
//!
//! This builder only exposes elements that CommonMark and GFM handle well.
//! Using this builder guarantees your document will emit cleanly to Markdown.
//!
//! # Example
//!
//! ```
//! use rescribe_write_markdown::builder::*;
//!
//! let doc = markdown(|d| {
//!     d.h1(|i| i.text("Hello World"))
//!         .para(|i| {
//!             i.text("This is ")
//!                 .em(|i| i.text("emphasized"))
//!                 .text(" and ")
//!                 .strong(|i| i.text("bold"))
//!                 .text(".")
//!         })
//!         .bullet_list(|l| {
//!             l.item(|i| i.text("First"))
//!                 .item(|i| i.text("Second"))
//!         })
//! });
//! ```

use rescribe_core::{Document, Node};
use rescribe_std::{node, prop};

/// Build a Markdown document with type-safe structure.
pub fn markdown<F>(f: F) -> Document
where
    F: FnOnce(MarkdownBuilder) -> MarkdownBuilder,
{
    let builder = f(MarkdownBuilder::new());
    Document::new().with_content(builder.build())
}

/// Builder for Markdown document structure.
/// Only exposes elements that CommonMark/GFM supports.
#[derive(Default)]
pub struct MarkdownBuilder {
    children: Vec<Node>,
}

impl MarkdownBuilder {
    fn new() -> Self {
        Self::default()
    }

    // Headings (ATX style: # through ######)

    /// Add a level 1 heading (#).
    pub fn h1<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 1i64)
                .children(inner.children),
        );
        self
    }

    /// Add a level 2 heading (##).
    pub fn h2<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 2i64)
                .children(inner.children),
        );
        self
    }

    /// Add a level 3 heading (###).
    pub fn h3<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 3i64)
                .children(inner.children),
        );
        self
    }

    /// Add a level 4 heading (####).
    pub fn h4<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 4i64)
                .children(inner.children),
        );
        self
    }

    /// Add a level 5 heading (#####).
    pub fn h5<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 5i64)
                .children(inner.children),
        );
        self
    }

    /// Add a level 6 heading (######).
    pub fn h6<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 6i64)
                .children(inner.children),
        );
        self
    }

    /// Add a paragraph.
    pub fn para<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children
            .push(Node::new(node::PARAGRAPH).children(inner.children));
        self
    }

    /// Add a bullet (unordered) list.
    pub fn bullet_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdList) -> MdList,
    {
        let list = f(MdList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, false)
                .prop(prop::TIGHT, list.tight)
                .children(list.items),
        );
        self
    }

    /// Add an ordered (numbered) list.
    pub fn ordered_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdList) -> MdList,
    {
        let list = f(MdList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .prop(prop::TIGHT, list.tight)
                .children(list.items),
        );
        self
    }

    /// Add an ordered list starting at a specific number.
    pub fn ordered_list_from<F>(mut self, start: i64, f: F) -> Self
    where
        F: FnOnce(MdList) -> MdList,
    {
        let list = f(MdList::new());
        self.children.push(
            Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .prop(prop::START, start)
                .prop(prop::TIGHT, list.tight)
                .children(list.items),
        );
        self
    }

    /// Add a blockquote.
    pub fn blockquote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MarkdownBuilder) -> MarkdownBuilder,
    {
        let inner = f(MarkdownBuilder::new());
        self.children
            .push(Node::new(node::BLOCKQUOTE).children(inner.children));
        self
    }

    /// Add a fenced code block.
    pub fn code_block(mut self, code: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code.into()));
        self
    }

    /// Add a fenced code block with language.
    pub fn code_block_lang(mut self, language: impl Into<String>, code: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::CODE_BLOCK)
                .prop(prop::LANGUAGE, language.into())
                .prop(prop::CONTENT, code.into()),
        );
        self
    }

    /// Add a horizontal rule (---).
    pub fn hr(mut self) -> Self {
        self.children.push(Node::new(node::HORIZONTAL_RULE));
        self
    }

    /// Add a GFM table.
    pub fn table<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdTable) -> MdTable,
    {
        let table = f(MdTable::new());
        self.children.push(table.build());
        self
    }

    /// Add raw Markdown (use sparingly).
    pub fn raw(mut self, content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "markdown")
                .prop(prop::CONTENT, content.into()),
        );
        self
    }

    fn build(self) -> Node {
        Node::new(node::DOCUMENT).children(self.children)
    }
}

/// Builder for Markdown inline content.
#[derive(Default)]
pub struct MdInline {
    children: Vec<Node>,
}

impl MdInline {
    fn new() -> Self {
        Self::default()
    }

    /// Add plain text.
    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::TEXT).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add emphasized text (*italic*).
    pub fn em<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children
            .push(Node::new(node::EMPHASIS).children(inner.children));
        self
    }

    /// Add strong text (**bold**).
    pub fn strong<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children
            .push(Node::new(node::STRONG).children(inner.children));
        self
    }

    /// Add strikethrough text (~~deleted~~) - GFM extension.
    pub fn strikethrough<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children
            .push(Node::new(node::STRIKEOUT).children(inner.children));
        self
    }

    /// Add inline code (`code`).
    pub fn code(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add a link.
    pub fn link<F>(mut self, url: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::LINK)
                .prop(prop::URL, url.into())
                .children(inner.children),
        );
        self
    }

    /// Add a link with title.
    pub fn link_titled<F>(mut self, url: impl Into<String>, title: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
        self.children.push(
            Node::new(node::LINK)
                .prop(prop::URL, url.into())
                .prop(prop::TITLE, title.into())
                .children(inner.children),
        );
        self
    }

    /// Add an image.
    pub fn image(mut self, url: impl Into<String>, alt: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::IMAGE)
                .prop(prop::URL, url.into())
                .prop(prop::ALT, alt.into()),
        );
        self
    }

    /// Add an image with title.
    pub fn image_titled(
        mut self,
        url: impl Into<String>,
        alt: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        self.children.push(
            Node::new(node::IMAGE)
                .prop(prop::URL, url.into())
                .prop(prop::ALT, alt.into())
                .prop(prop::TITLE, title.into()),
        );
        self
    }

    /// Add a hard line break (two trailing spaces).
    pub fn linebreak(mut self) -> Self {
        self.children.push(Node::new(node::LINE_BREAK));
        self
    }

    /// Add a soft line break.
    pub fn softbreak(mut self) -> Self {
        self.children.push(Node::new(node::SOFT_BREAK));
        self
    }
}

/// Builder for Markdown lists.
pub struct MdList {
    items: Vec<Node>,
    tight: bool,
}

impl MdList {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            tight: true,
        }
    }

    /// Make this a loose list (blank lines between items).
    pub fn loose(mut self) -> Self {
        self.tight = false;
        self
    }

    /// Add a simple list item with inline content.
    pub fn item<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inline = f(MdInline::new());
        let item = Node::new(node::LIST_ITEM)
            .children(vec![Node::new(node::PARAGRAPH).children(inline.children)]);
        self.items.push(item);
        self
    }

    /// Add a list item with block content (can contain sublists, paragraphs, etc.).
    pub fn item_block<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MarkdownBuilder) -> MarkdownBuilder,
    {
        let inner = f(MarkdownBuilder::new());
        let item = Node::new(node::LIST_ITEM).children(inner.children);
        self.items.push(item);
        self
    }
}

/// Builder for GFM tables.
#[derive(Default)]
pub struct MdTable {
    header: Option<Vec<Node>>,
    rows: Vec<Vec<Node>>,
}

impl MdTable {
    fn new() -> Self {
        Self::default()
    }

    /// Set the header row.
    pub fn header<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdTableRow) -> MdTableRow,
    {
        let row = f(MdTableRow::new(true));
        self.header = Some(row.cells);
        self
    }

    /// Add a body row.
    pub fn row<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdTableRow) -> MdTableRow,
    {
        let row = f(MdTableRow::new(false));
        self.rows.push(row.cells);
        self
    }

    fn build(self) -> Node {
        let mut children = Vec::new();

        if let Some(header_cells) = self.header {
            let header_row = Node::new(node::TABLE_ROW).children(header_cells);
            children.push(Node::new(node::TABLE_HEAD).children(vec![header_row]));
        }

        if !self.rows.is_empty() {
            let body_rows: Vec<Node> = self
                .rows
                .into_iter()
                .map(|cells| Node::new(node::TABLE_ROW).children(cells))
                .collect();
            children.push(Node::new(node::TABLE_BODY).children(body_rows));
        }

        Node::new(node::TABLE).children(children)
    }
}

/// Builder for table rows.
pub struct MdTableRow {
    cells: Vec<Node>,
    is_header: bool,
}

impl MdTableRow {
    fn new(is_header: bool) -> Self {
        Self {
            cells: Vec::new(),
            is_header,
        }
    }

    /// Add a cell with inline content.
    pub fn cell<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MdInline) -> MdInline,
    {
        let inner = f(MdInline::new());
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
        let doc = markdown(|d| d.h1(|i| i.text("Hello")).para(|i| i.text("World")));

        let output = emit_str(&doc);
        assert!(output.contains("# Hello"));
        assert!(output.contains("World"));
    }

    #[test]
    fn test_inline_formatting() {
        let doc = markdown(|d| {
            d.para(|i| {
                i.text("This is ")
                    .em(|i| i.text("italic"))
                    .text(" and ")
                    .strong(|i| i.text("bold"))
                    .text(".")
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("*italic*"));
        assert!(output.contains("**bold**"));
    }

    #[test]
    fn test_code() {
        let doc = markdown(|d| {
            d.para(|i| i.text("Use ").code("println!").text(" to print."))
                .code_block_lang("rust", "fn main() {}")
        });

        let output = emit_str(&doc);
        assert!(output.contains("`println!`"));
        assert!(output.contains("```rust"));
    }

    #[test]
    fn test_lists() {
        let doc = markdown(|d| {
            d.bullet_list(|l| l.item(|i| i.text("First")).item(|i| i.text("Second")))
                .ordered_list(|l| l.item(|i| i.text("One")).item(|i| i.text("Two")))
        });

        let output = emit_str(&doc);
        assert!(output.contains("- First"));
        assert!(output.contains("- Second"));
        assert!(output.contains("1. One"));
        assert!(output.contains("2. Two"));
    }

    #[test]
    fn test_links_and_images() {
        let doc = markdown(|d| {
            d.para(|i| {
                i.link("https://example.com", |i| i.text("Example"))
                    .text(" ")
                    .image("img.png", "An image")
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("[Example](https://example.com)"));
        assert!(output.contains("![An image](img.png)"));
    }

    #[test]
    fn test_blockquote() {
        let doc = markdown(|d| d.blockquote(|b| b.para(|i| i.text("A quote"))));

        let output = emit_str(&doc);
        assert!(output.contains("> A quote"));
    }

    #[test]
    fn test_table() {
        let doc = markdown(|d| {
            d.table(|t| {
                t.header(|r| r.cell(|i| i.text("A")).cell(|i| i.text("B")))
                    .row(|r| r.cell(|i| i.text("1")).cell(|i| i.text("2")))
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("| A"));
        assert!(output.contains("| B"));
        assert!(output.contains("---"));
        assert!(output.contains("| 1"));
    }

    #[test]
    fn test_nested_list() {
        let doc = markdown(|d| {
            d.bullet_list(|l| {
                l.item_block(|b| {
                    b.para(|i| i.text("Parent"))
                        .bullet_list(|l| l.item(|i| i.text("Child")))
                })
            })
        });

        let output = emit_str(&doc);
        assert!(output.contains("Parent"));
        assert!(output.contains("Child"));
    }
}
