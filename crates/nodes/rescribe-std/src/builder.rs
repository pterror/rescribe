//! Type-safe document builders.
//!
//! This module provides builders that enforce valid document structure at compile time.
//! For example, you cannot add a heading inside a paragraph because `ParagraphBuilder`
//! only has methods for inline elements.
//!
//! # Example
//!
//! ```rust
//! use rescribe_std::builder::*;
//!
//! let doc = doc(|d| d
//!     .h1(|h| h.text("Hello World"))
//!     .para(|p| p
//!         .text("This is ")
//!         .strong(|s| s.text("bold"))
//!         .text(" text.")
//!     )
//!     .bullet_list(|l| l
//!         .item(|i| i.text("First item"))
//!         .item(|i| i.text("Second item"))
//!     )
//! );
//! ```

use crate::{Document, Node, node, prop};

/// Build a document with type-safe structure.
pub fn doc<F>(f: F) -> Document
where
    F: FnOnce(DocumentBuilder) -> DocumentBuilder,
{
    let builder = f(DocumentBuilder::new());
    Document::new().with_content(builder.build())
}

/// Builder for the document root (accepts block elements).
#[derive(Default)]
pub struct DocumentBuilder {
    children: Vec<Node>,
}

impl DocumentBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a level 1 heading.
    pub fn h1<F>(self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        self.heading(1, f)
    }

    /// Add a level 2 heading.
    pub fn h2<F>(self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        self.heading(2, f)
    }

    /// Add a level 3 heading.
    pub fn h3<F>(self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        self.heading(3, f)
    }

    /// Add a level 4 heading.
    pub fn h4<F>(self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        self.heading(4, f)
    }

    /// Add a level 5 heading.
    pub fn h5<F>(self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        self.heading(5, f)
    }

    /// Add a level 6 heading.
    pub fn h6<F>(self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        self.heading(6, f)
    }

    /// Add a heading with a specific level.
    pub fn heading<F>(mut self, level: i64, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inline = f(InlineBuilder::new());
        let heading = Node::new(node::HEADING)
            .prop(prop::LEVEL, level)
            .children(inline.children);
        self.children.push(heading);
        self
    }

    /// Add a paragraph.
    pub fn para<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inline = f(InlineBuilder::new());
        let para = Node::new(node::PARAGRAPH).children(inline.children);
        self.children.push(para);
        self
    }

    /// Add a code block.
    pub fn code_block(mut self, code: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code.into()));
        self
    }

    /// Add a code block with language.
    pub fn code_block_lang(mut self, code: impl Into<String>, lang: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::CODE_BLOCK)
                .prop(prop::CONTENT, code.into())
                .prop(prop::LANGUAGE, lang.into()),
        );
        self
    }

    /// Add a blockquote.
    pub fn blockquote<F>(mut self, f: F) -> Self
    where
        F: FnOnce(DocumentBuilder) -> DocumentBuilder,
    {
        let inner = f(DocumentBuilder::new());
        let quote = Node::new(node::BLOCKQUOTE).children(inner.children);
        self.children.push(quote);
        self
    }

    /// Add an unordered (bullet) list.
    pub fn bullet_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(ListBuilder) -> ListBuilder,
    {
        let list = f(ListBuilder::new(false));
        self.children.push(list.build());
        self
    }

    /// Add an ordered (numbered) list.
    pub fn ordered_list<F>(mut self, f: F) -> Self
    where
        F: FnOnce(ListBuilder) -> ListBuilder,
    {
        let list = f(ListBuilder::new(true));
        self.children.push(list.build());
        self
    }

    /// Add an ordered list starting at a specific number.
    pub fn ordered_list_from<F>(mut self, start: i64, f: F) -> Self
    where
        F: FnOnce(ListBuilder) -> ListBuilder,
    {
        let mut list = f(ListBuilder::new(true));
        list.start = Some(start);
        self.children.push(list.build());
        self
    }

    /// Add a horizontal rule.
    pub fn hr(mut self) -> Self {
        self.children.push(Node::new(node::HORIZONTAL_RULE));
        self
    }

    /// Add a table.
    pub fn table<F>(mut self, f: F) -> Self
    where
        F: FnOnce(TableBuilder) -> TableBuilder,
    {
        let table = f(TableBuilder::new());
        self.children.push(table.build());
        self
    }

    /// Add a div container.
    pub fn container<F>(mut self, f: F) -> Self
    where
        F: FnOnce(DocumentBuilder) -> DocumentBuilder,
    {
        let inner = f(DocumentBuilder::new());
        let div = Node::new(node::DIV).children(inner.children);
        self.children.push(div);
        self
    }

    /// Add a raw block of a specific format.
    pub fn raw_block(mut self, format: impl Into<String>, content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, format.into())
                .prop(prop::CONTENT, content.into()),
        );
        self
    }

    fn build(self) -> Node {
        Node::new(node::DOCUMENT).children(self.children)
    }
}

/// Builder for inline content (text, emphasis, links, etc.).
#[derive(Default)]
pub struct InlineBuilder {
    children: Vec<Node>,
}

impl InlineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add plain text.
    pub fn text(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::TEXT).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add emphasized (italic) text.
    pub fn em<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::EMPHASIS).children(inner.children));
        self
    }

    /// Add strong (bold) text.
    pub fn strong<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::STRONG).children(inner.children));
        self
    }

    /// Add strikethrough text.
    pub fn strike<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::STRIKEOUT).children(inner.children));
        self
    }

    /// Add underlined text.
    pub fn underline<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::UNDERLINE).children(inner.children));
        self
    }

    /// Add subscript text.
    pub fn subscript<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::SUBSCRIPT).children(inner.children));
        self
    }

    /// Add superscript text.
    pub fn sup<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::SUPERSCRIPT).children(inner.children));
        self
    }

    /// Add inline code.
    pub fn code(mut self, content: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::CODE).prop(prop::CONTENT, content.into()));
        self
    }

    /// Add a link.
    pub fn link<F>(mut self, url: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
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
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
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

    /// Add a hard line break.
    pub fn br(mut self) -> Self {
        self.children.push(Node::new(node::LINE_BREAK));
        self
    }

    /// Add a soft break (renders as space in most formats).
    pub fn soft_break(mut self) -> Self {
        self.children.push(Node::new(node::SOFT_BREAK));
        self
    }

    /// Add a span container.
    pub fn span<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inner = f(InlineBuilder::new());
        self.children
            .push(Node::new(node::SPAN).children(inner.children));
        self
    }

    /// Add raw inline content of a specific format.
    pub fn raw(mut self, format: impl Into<String>, content: impl Into<String>) -> Self {
        self.children.push(
            Node::new(node::RAW_INLINE)
                .prop(prop::FORMAT, format.into())
                .prop(prop::CONTENT, content.into()),
        );
        self
    }

    /// Add a footnote reference.
    pub fn footnote_ref(mut self, label: impl Into<String>) -> Self {
        self.children
            .push(Node::new(node::FOOTNOTE_REF).prop(prop::LABEL, label.into()));
        self
    }
}

/// Builder for lists.
pub struct ListBuilder {
    ordered: bool,
    start: Option<i64>,
    items: Vec<Node>,
}

impl ListBuilder {
    fn new(ordered: bool) -> Self {
        Self {
            ordered,
            start: None,
            items: Vec::new(),
        }
    }

    /// Add a list item with inline content.
    pub fn item<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inline = f(InlineBuilder::new());
        let item = Node::new(node::LIST_ITEM)
            .children(vec![Node::new(node::PARAGRAPH).children(inline.children)]);
        self.items.push(item);
        self
    }

    /// Add a list item with block content (for multi-paragraph items).
    pub fn item_block<F>(mut self, f: F) -> Self
    where
        F: FnOnce(DocumentBuilder) -> DocumentBuilder,
    {
        let inner = f(DocumentBuilder::new());
        let item = Node::new(node::LIST_ITEM).children(inner.children);
        self.items.push(item);
        self
    }

    fn build(self) -> Node {
        let mut list = Node::new(node::LIST)
            .prop(prop::ORDERED, self.ordered)
            .children(self.items);
        if let Some(start) = self.start {
            list = list.prop(prop::START, start);
        }
        list
    }
}

/// Builder for tables.
#[derive(Default)]
pub struct TableBuilder {
    rows: Vec<Node>,
}

impl TableBuilder {
    fn new() -> Self {
        Self::default()
    }

    /// Add a header row.
    pub fn header<F>(mut self, f: F) -> Self
    where
        F: FnOnce(TableRowBuilder) -> TableRowBuilder,
    {
        let row = f(TableRowBuilder::new(true));
        self.rows.push(row.build());
        self
    }

    /// Add a data row.
    pub fn row<F>(mut self, f: F) -> Self
    where
        F: FnOnce(TableRowBuilder) -> TableRowBuilder,
    {
        let row = f(TableRowBuilder::new(false));
        self.rows.push(row.build());
        self
    }

    fn build(self) -> Node {
        Node::new(node::TABLE).children(self.rows)
    }
}

/// Builder for table rows.
pub struct TableRowBuilder {
    is_header: bool,
    cells: Vec<Node>,
}

impl TableRowBuilder {
    fn new(is_header: bool) -> Self {
        Self {
            is_header,
            cells: Vec::new(),
        }
    }

    /// Add a cell with inline content.
    pub fn cell<F>(mut self, f: F) -> Self
    where
        F: FnOnce(InlineBuilder) -> InlineBuilder,
    {
        let inline = f(InlineBuilder::new());
        let kind = if self.is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };
        self.cells.push(Node::new(kind).children(inline.children));
        self
    }

    fn build(self) -> Node {
        Node::new(node::TABLE_ROW).children(self.cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_document() {
        let document = doc(|d| {
            d.h1(|h| h.text("Hello World"))
                .para(|p| p.text("This is a paragraph."))
        });

        assert_eq!(document.content.children.len(), 2);
        assert_eq!(document.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(document.content.children[1].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_inline_formatting() {
        let document = doc(|d| {
            d.para(|p| {
                p.text("Normal ")
                    .strong(|s| s.text("bold"))
                    .text(" and ")
                    .em(|e| e.text("italic"))
            })
        });

        let para = &document.content.children[0];
        assert_eq!(para.children.len(), 4);
        assert_eq!(para.children[0].kind.as_str(), node::TEXT);
        assert_eq!(para.children[1].kind.as_str(), node::STRONG);
        assert_eq!(para.children[2].kind.as_str(), node::TEXT);
        assert_eq!(para.children[3].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_lists() {
        let document =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("First")).item(|i| i.text("Second"))));

        let list = &document.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_links() {
        let document = doc(|d| {
            d.para(|p| {
                p.text("Visit ")
                    .link("https://example.com", |l| l.text("Example"))
            })
        });

        let para = &document.content.children[0];
        let link = &para.children[1];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_code_block() {
        let document = doc(|d| d.code_block_lang("fn main() {}", "rust"));

        let code = &document.content.children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
        assert_eq!(code.props.get_str(prop::CONTENT), Some("fn main() {}"));
        assert_eq!(code.props.get_str(prop::LANGUAGE), Some("rust"));
    }

    #[test]
    fn test_table() {
        let document = doc(|d| {
            d.table(|t| {
                t.header(|r| r.cell(|c| c.text("Name")).cell(|c| c.text("Value")))
                    .row(|r| r.cell(|c| c.text("foo")).cell(|c| c.text("42")))
            })
        });

        let table = &document.content.children[0];
        assert_eq!(table.kind.as_str(), node::TABLE);
        assert_eq!(table.children.len(), 2);
    }

    #[test]
    fn test_nested_blockquote() {
        let document = doc(|d| {
            d.blockquote(|q| {
                q.para(|p| p.text("A wise quote"))
                    .para(|p| p.text("-- Author"))
            })
        });

        let quote = &document.content.children[0];
        assert_eq!(quote.kind.as_str(), node::BLOCKQUOTE);
        assert_eq!(quote.children.len(), 2);
    }
}
