//! Node types for the document tree.

use crate::Properties;

/// A content node in the document tree.
#[derive(Debug, Clone)]
pub struct Node {
    /// Node type (e.g., "paragraph", "heading", "table").
    pub kind: NodeKind,
    /// Extensible properties for this node.
    pub props: Properties,
    /// Child nodes.
    pub children: Vec<Node>,
    /// Source location for error reporting.
    pub span: Option<Span>,
}

/// Node kind - open enum for extensibility.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeKind(pub String);

/// Source span for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Node {
    /// Create a new node with the given kind.
    pub fn new(kind: impl Into<NodeKind>) -> Self {
        Self {
            kind: kind.into(),
            props: Properties::new(),
            children: Vec::new(),
            span: None,
        }
    }

    /// Create a text node.
    pub fn text(content: impl Into<String>) -> Self {
        Self::new(NodeKind::TEXT).prop("content", content.into())
    }

    /// Add a property.
    pub fn prop(mut self, key: impl Into<String>, value: impl Into<PropValue>) -> Self {
        self.props.set(key, value);
        self
    }

    /// Add a child node.
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// Add multiple child nodes.
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    /// Set the source span.
    pub fn span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl NodeKind {
    // Standard block kinds
    pub const DOCUMENT: &'static str = "document";
    pub const PARAGRAPH: &'static str = "paragraph";
    pub const HEADING: &'static str = "heading";
    pub const CODE_BLOCK: &'static str = "code_block";
    pub const BLOCKQUOTE: &'static str = "blockquote";
    pub const LIST: &'static str = "list";
    pub const LIST_ITEM: &'static str = "list_item";
    pub const TABLE: &'static str = "table";
    pub const TABLE_ROW: &'static str = "table_row";
    pub const TABLE_CELL: &'static str = "table_cell";
    pub const FIGURE: &'static str = "figure";
    pub const HORIZONTAL_RULE: &'static str = "horizontal_rule";

    // Standard inline kinds
    pub const TEXT: &'static str = "text";
    pub const EMPHASIS: &'static str = "emphasis";
    pub const STRONG: &'static str = "strong";
    pub const CODE: &'static str = "code";
    pub const LINK: &'static str = "link";
    pub const IMAGE: &'static str = "image";
    pub const LINE_BREAK: &'static str = "line_break";

    // Format-specific kinds (examples)
    pub const LATEX_MATH: &'static str = "latex:math";
    pub const HTML_DIV: &'static str = "html:div";
    pub const DOCX_COMMENT: &'static str = "docx:comment";
}

impl From<&str> for NodeKind {
    fn from(s: &str) -> Self {
        NodeKind(s.to_string())
    }
}

impl From<String> for NodeKind {
    fn from(s: String) -> Self {
        NodeKind(s)
    }
}

// Re-export PropValue for the prop() method
pub use crate::properties::PropValue;
