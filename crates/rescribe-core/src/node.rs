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
///
/// This is a newtype wrapper around String to allow any node kind.
/// Standard node kinds are defined in `rescribe-std`.
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
    /// Create a new node kind from a string.
    pub fn new(s: impl Into<String>) -> Self {
        NodeKind(s.into())
    }

    /// Get the kind as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
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

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Re-export PropValue for the prop() method
pub use crate::properties::PropValue;
