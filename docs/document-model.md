# Document Model

rescribe uses a flexible document model designed for lossless conversion between formats.

## Core Types

### Document

The root container holding content and resources:

```rust
pub struct Document {
    /// Root content node.
    pub content: Node,
    /// Embedded resources (images, fonts, etc.).
    pub resources: ResourceMap,
    /// Document-level metadata.
    pub metadata: Properties,
    /// Source format information (for roundtrip fidelity).
    pub source: Option<SourceInfo>,
}
```

### Node

A content node in the document tree:

```rust
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
```

### NodeKind

Unlike Pandoc's fixed enum, `NodeKind` is an open string type:

```rust
pub struct NodeKind(pub String);
```

This allows format-specific node types without changing the core library:

```rust
// Standard kinds
NodeKind::PARAGRAPH  // "paragraph"
NodeKind::HEADING    // "heading"
NodeKind::TABLE      // "table"

// Format-specific kinds
"latex:math"
"html:div"
"docx:comment"
```

## Building Documents

### Programmatic Construction

```rust
use rescribe::{Document, Node, NodeKind};

let doc = Document::new()
    .with_metadata(props! {
        "title" => "My Document",
        "author" => "Alice",
    })
    .with_content(Node::new(NodeKind::DOCUMENT).children(vec![
        Node::new(NodeKind::HEADING)
            .prop("level", 1)
            .child(Node::text("Introduction")),
        Node::new(NodeKind::PARAGRAPH)
            .child(Node::text("This is the first paragraph.")),
    ]));
```

### From Parsing

```rust
let parser = MarkdownParser::new();
let result = parser.parse(input, &ParseOptions::default())?;
let doc = result.value;
```

## Tree Traversal

```rust
fn count_headings(doc: &Document) -> usize {
    count_nodes(&doc.content, |n| n.kind.0 == NodeKind::HEADING)
}

fn extract_links(doc: &Document) -> Vec<String> {
    collect_props(&doc.content, NodeKind::LINK, "url")
}
```
