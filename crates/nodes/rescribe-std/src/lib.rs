//! Standard node kinds and property keys for rescribe.
//!
//! This crate provides the standard vocabulary for document representation.
//! It re-exports `rescribe-core` so users only need one import.

pub use rescribe_core::*;

/// Type-safe document builder API.
pub mod builder;

/// Standard node kind constants.
pub mod node {
    // Block-level nodes
    /// Root document container.
    pub const DOCUMENT: &str = "document";
    /// A paragraph of text.
    pub const PARAGRAPH: &str = "paragraph";
    /// A heading (use `level` property for h1-h6).
    pub const HEADING: &str = "heading";
    /// A fenced or indented code block.
    pub const CODE_BLOCK: &str = "code_block";
    /// A block quotation.
    pub const BLOCKQUOTE: &str = "blockquote";
    /// A list (use `ordered` property to distinguish).
    pub const LIST: &str = "list";
    /// An item in a list.
    pub const LIST_ITEM: &str = "list_item";
    /// A table.
    pub const TABLE: &str = "table";
    /// A row in a table.
    pub const TABLE_ROW: &str = "table_row";
    /// A cell in a table row.
    pub const TABLE_CELL: &str = "table_cell";
    /// A header cell in a table.
    pub const TABLE_HEADER: &str = "table_header";
    /// A figure with caption.
    pub const FIGURE: &str = "figure";
    /// A thematic break / horizontal rule.
    pub const HORIZONTAL_RULE: &str = "horizontal_rule";
    /// A generic block container (like HTML div).
    pub const DIV: &str = "div";
    /// Raw format-specific block content.
    pub const RAW_BLOCK: &str = "raw_block";
    /// A definition list.
    pub const DEFINITION_LIST: &str = "definition_list";
    /// A term in a definition list.
    pub const DEFINITION_TERM: &str = "definition_term";
    /// A description in a definition list.
    pub const DEFINITION_DESC: &str = "definition_desc";
    /// Caption for figures/tables.
    pub const CAPTION: &str = "caption";
    /// Table head section.
    pub const TABLE_HEAD: &str = "table_head";
    /// Table body section.
    pub const TABLE_BODY: &str = "table_body";
    /// Table foot section.
    pub const TABLE_FOOT: &str = "table_foot";

    // Inline-level nodes
    /// Plain text content (use `content` property).
    pub const TEXT: &str = "text";
    /// Emphasized text (typically italic).
    pub const EMPHASIS: &str = "emphasis";
    /// Strong text (typically bold).
    pub const STRONG: &str = "strong";
    /// Strikethrough text.
    pub const STRIKEOUT: &str = "strikeout";
    /// Underlined text.
    pub const UNDERLINE: &str = "underline";
    /// Subscript text.
    pub const SUBSCRIPT: &str = "subscript";
    /// Superscript text.
    pub const SUPERSCRIPT: &str = "superscript";
    /// Inline code.
    pub const CODE: &str = "code";
    /// A hyperlink (use `url` and optional `title` properties).
    pub const LINK: &str = "link";
    /// An image (use `url`, `alt`, optional `title` properties).
    pub const IMAGE: &str = "image";
    /// A hard line break.
    pub const LINE_BREAK: &str = "line_break";
    /// A soft line break (may render as space).
    pub const SOFT_BREAK: &str = "soft_break";
    /// A generic inline container (like HTML span).
    pub const SPAN: &str = "span";
    /// Raw format-specific inline content.
    pub const RAW_INLINE: &str = "raw_inline";
    /// A footnote reference.
    pub const FOOTNOTE_REF: &str = "footnote_ref";
    /// A footnote definition.
    pub const FOOTNOTE_DEF: &str = "footnote_def";
    /// Small caps text.
    pub const SMALL_CAPS: &str = "small_caps";
    /// Quoted text (use `quote_type` property: single/double).
    pub const QUOTED: &str = "quoted";
    /// A citation.
    pub const CITE: &str = "cite";
}

/// Standard property key constants.
pub mod prop {
    // Semantic properties (format-agnostic)
    /// Heading level (1-6).
    pub const LEVEL: &str = "level";
    /// Whether a list is ordered.
    pub const ORDERED: &str = "ordered";
    /// Programming language for code blocks.
    pub const LANGUAGE: &str = "language";
    /// URL for links and images.
    pub const URL: &str = "url";
    /// Title attribute for links and images.
    pub const TITLE: &str = "title";
    /// Alt text for images.
    pub const ALT: &str = "alt";
    /// Text content for text nodes.
    pub const CONTENT: &str = "content";
    /// Reference to an embedded resource.
    pub const RESOURCE_ID: &str = "resource";
    /// Identifier/anchor name.
    pub const ID: &str = "id";
    /// CSS classes (as list).
    pub const CLASSES: &str = "classes";
    /// Start number for ordered lists.
    pub const START: &str = "start";
    /// List style type (decimal, lower-alpha, etc.).
    pub const LIST_STYLE: &str = "list_style";
    /// Tight list (no paragraph wrapping).
    pub const TIGHT: &str = "tight";
    /// Format for raw blocks/inlines.
    pub const FORMAT: &str = "format";
    /// Quote type (single, double).
    pub const QUOTE_TYPE: &str = "quote_type";
    /// Footnote/reference label.
    pub const LABEL: &str = "label";
    /// Column alignment (left, center, right).
    pub const ALIGN: &str = "align";
    /// Column span for table cells.
    pub const COLSPAN: &str = "colspan";
    /// Row span for table cells.
    pub const ROWSPAN: &str = "rowspan";

    // Style properties (presentational)
    /// Font family.
    pub const STYLE_FONT: &str = "style:font";
    /// Font size.
    pub const STYLE_SIZE: &str = "style:size";
    /// Text color.
    pub const STYLE_COLOR: &str = "style:color";
    /// Text alignment.
    pub const STYLE_ALIGN: &str = "style:align";
    /// Background color.
    pub const STYLE_BG_COLOR: &str = "style:bg_color";
    /// Font weight.
    pub const STYLE_WEIGHT: &str = "style:weight";

    // Layout properties (positioning)
    /// Page break before.
    pub const LAYOUT_PAGE_BREAK: &str = "layout:page_break";
    /// Column specification.
    pub const LAYOUT_COLUMN: &str = "layout:column";
    /// Float positioning.
    pub const LAYOUT_FLOAT: &str = "layout:float";

    // Format-specific prefixes (for dynamic property names)
    /// HTML-specific properties prefix.
    pub const HTML_PREFIX: &str = "html:";
    /// LaTeX-specific properties prefix.
    pub const LATEX_PREFIX: &str = "latex:";
    /// DOCX-specific properties prefix.
    pub const DOCX_PREFIX: &str = "docx:";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::doc;

    #[test]
    fn test_create_text_node() {
        let document = doc(|d| d.para(|i| i.text("Hello, world!")));
        let para = &document.content.children[0];
        let text_node = &para.children[0];
        assert_eq!(text_node.kind.as_str(), node::TEXT);
        assert_eq!(
            text_node.props.get_str(prop::CONTENT),
            Some("Hello, world!")
        );
    }

    #[test]
    fn test_create_heading() {
        let document = doc(|d| d.heading(1, |i| i.text("Title")));
        let h1 = &document.content.children[0];
        assert_eq!(h1.kind.as_str(), node::HEADING);
        assert_eq!(h1.props.get_int(prop::LEVEL), Some(1));
        assert_eq!(h1.children.len(), 1);
    }

    #[test]
    fn test_create_link() {
        let document = doc(|d| d.para(|i| i.link("https://example.com", |i| i.text("Example"))));
        let para = &document.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_create_list() {
        let document =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));
        let list = &document.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }
}
