//! Standard document transformers for rescribe.
//!
//! This crate provides common document transformations:
//! - Heading level adjustment
//! - Empty node removal
//! - Document structure normalization
//! - Visitor utilities for custom transforms

use rescribe_core::{Document, TransformError, Transformer};
use rescribe_std::{Node, node, prop};

/// Shift all heading levels by a fixed amount.
///
/// Useful for embedding documents where heading hierarchy needs adjustment.
pub struct ShiftHeadings {
    /// Amount to shift (positive = deeper, negative = shallower).
    pub delta: i64,
    /// Minimum heading level (default: 1).
    pub min_level: i64,
    /// Maximum heading level (default: 6).
    pub max_level: i64,
}

impl ShiftHeadings {
    /// Create a new heading shifter.
    pub fn new(delta: i64) -> Self {
        Self {
            delta,
            min_level: 1,
            max_level: 6,
        }
    }

    /// Set the minimum heading level.
    pub fn with_min(mut self, min: i64) -> Self {
        self.min_level = min;
        self
    }

    /// Set the maximum heading level.
    pub fn with_max(mut self, max: i64) -> Self {
        self.max_level = max;
        self
    }

    fn transform_node(&self, mut node: Node) -> Node {
        if node.kind.as_str() == node::HEADING
            && let Some(level) = node.props.get_int(prop::LEVEL)
        {
            let new_level = (level + self.delta).clamp(self.min_level, self.max_level);
            node.props.set(prop::LEVEL, new_level);
        }

        node.children = node
            .children
            .into_iter()
            .map(|c| self.transform_node(c))
            .collect();

        node
    }
}

impl Transformer for ShiftHeadings {
    fn name(&self) -> &str {
        "shift_headings"
    }

    fn transform(&self, doc: Document) -> Result<Document, TransformError> {
        let content = self.transform_node(doc.content);
        Ok(Document {
            content,
            metadata: doc.metadata,
            resources: doc.resources,
            source: doc.source,
        })
    }
}

/// Remove empty text nodes and paragraphs with no content.
pub struct StripEmpty;

impl StripEmpty {
    fn is_empty_node(node: &Node) -> bool {
        match node.kind.as_str() {
            node::TEXT => node
                .props
                .get_str(prop::CONTENT)
                .map(|s| s.trim().is_empty())
                .unwrap_or(true),
            node::PARAGRAPH | node::SPAN | node::DIV => node.children.is_empty(),
            _ => false,
        }
    }

    fn transform_node(mut node: Node) -> Node {
        node.children = node
            .children
            .into_iter()
            .filter(|c| !Self::is_empty_node(c))
            .map(Self::transform_node)
            .collect();
        node
    }
}

impl Transformer for StripEmpty {
    fn name(&self) -> &str {
        "strip_empty"
    }

    fn transform(&self, doc: Document) -> Result<Document, TransformError> {
        let content = Self::transform_node(doc.content);
        Ok(Document {
            content,
            metadata: doc.metadata,
            resources: doc.resources,
            source: doc.source,
        })
    }
}

/// Merge adjacent text nodes.
pub struct MergeText;

impl MergeText {
    fn transform_node(mut node: Node) -> Node {
        // First transform children recursively
        node.children = node
            .children
            .into_iter()
            .map(Self::transform_node)
            .collect();

        // Then merge adjacent text nodes
        let mut merged: Vec<Node> = Vec::new();
        for child in node.children {
            if child.kind.as_str() == node::TEXT
                && let Some(last) = merged.last_mut()
                && last.kind.as_str() == node::TEXT
            {
                // Merge with previous text node
                let prev_content = last.props.get_str(prop::CONTENT).unwrap_or("").to_string();
                let this_content = child.props.get_str(prop::CONTENT).unwrap_or("").to_string();
                last.props.set(prop::CONTENT, prev_content + &this_content);
                continue;
            }
            merged.push(child);
        }

        node.children = merged;
        node
    }
}

impl Transformer for MergeText {
    fn name(&self) -> &str {
        "merge_text"
    }

    fn transform(&self, doc: Document) -> Result<Document, TransformError> {
        let content = Self::transform_node(doc.content);
        Ok(Document {
            content,
            metadata: doc.metadata,
            resources: doc.resources,
            source: doc.source,
        })
    }
}

/// Unwrap single-child wrapper nodes (divs, spans with no properties).
pub struct UnwrapSingleChild;

impl UnwrapSingleChild {
    fn is_unwrappable(node: &Node) -> bool {
        let kind = node.kind.as_str();
        (kind == node::DIV || kind == node::SPAN)
            && node.children.len() == 1
            && node.props.is_empty()
            && node.span.is_none()
    }

    fn transform_node(mut node: Node) -> Node {
        // Recursively transform children first
        node.children = node
            .children
            .into_iter()
            .map(Self::transform_node)
            .collect();

        // Then check if this node should be unwrapped
        if Self::is_unwrappable(&node) {
            return node.children.into_iter().next().unwrap();
        }

        node
    }
}

impl Transformer for UnwrapSingleChild {
    fn name(&self) -> &str {
        "unwrap_single_child"
    }

    fn transform(&self, doc: Document) -> Result<Document, TransformError> {
        let content = Self::transform_node(doc.content);
        Ok(Document {
            content,
            metadata: doc.metadata,
            resources: doc.resources,
            source: doc.source,
        })
    }
}

/// A transform pipeline that applies multiple transforms in sequence.
pub struct Pipeline {
    transforms: Vec<Box<dyn Transformer>>,
}

impl Pipeline {
    /// Create a new empty pipeline.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    /// Add a transform to the pipeline.
    pub fn then<T: Transformer + 'static>(mut self, transform: T) -> Self {
        self.transforms.push(Box::new(transform));
        self
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Transformer for Pipeline {
    fn name(&self) -> &str {
        "pipeline"
    }

    fn transform(&self, mut doc: Document) -> Result<Document, TransformError> {
        for transform in &self.transforms {
            doc = transform.transform(doc)?;
        }
        Ok(doc)
    }
}

/// Walk a document tree, calling a function on each node.
pub fn walk<F>(node: &Node, f: &mut F)
where
    F: FnMut(&Node),
{
    f(node);
    for child in &node.children {
        walk(child, f);
    }
}

/// Walk a document tree mutably, calling a function on each node.
pub fn walk_mut<F>(node: &mut Node, f: &mut F)
where
    F: FnMut(&mut Node),
{
    f(node);
    for child in &mut node.children {
        walk_mut(child, f);
    }
}

/// Map a function over all nodes in a tree, building a new tree.
pub fn map<F>(node: Node, f: &mut F) -> Node
where
    F: FnMut(Node) -> Node,
{
    let mut node = f(node);
    node.children = node.children.into_iter().map(|c| map(c, f)).collect();
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::doc;

    #[test]
    fn test_shift_headings_positive() {
        let document = doc(|d| {
            d.heading(1, |i| i.text("Title"))
                .heading(2, |i| i.text("Sub"))
        });

        let transform = ShiftHeadings::new(1);
        let result = transform.transform(document).unwrap();

        assert_eq!(
            result.content.children[0].props.get_int(prop::LEVEL),
            Some(2)
        );
        assert_eq!(
            result.content.children[1].props.get_int(prop::LEVEL),
            Some(3)
        );
    }

    #[test]
    fn test_shift_headings_clamped() {
        let document = doc(|d| d.heading(6, |i| i.text("Deep")));

        let transform = ShiftHeadings::new(2);
        let result = transform.transform(document).unwrap();

        // Should be clamped to max (6)
        assert_eq!(
            result.content.children[0].props.get_int(prop::LEVEL),
            Some(6)
        );
    }

    #[test]
    fn test_strip_empty() {
        // Create a document with an empty paragraph by using raw node construction
        let mut document = doc(|d| d.para(|i| i.text("Hello")).para(|i| i.text("World")));
        // Insert an empty paragraph in the middle
        let empty_para = Node::new(node::PARAGRAPH);
        document.content.children.insert(1, empty_para);

        assert_eq!(document.content.children.len(), 3);

        let transform = StripEmpty;
        let result = transform.transform(document).unwrap();

        assert_eq!(result.content.children.len(), 2);
    }

    #[test]
    fn test_merge_text() {
        let document = doc(|d| d.para(|i| i.text("Hello ").text("World")));

        // Verify we have two text nodes before
        assert_eq!(document.content.children[0].children.len(), 2);

        let transform = MergeText;
        let result = transform.transform(document).unwrap();

        // Should be merged to one text node
        assert_eq!(result.content.children[0].children.len(), 1);
        assert_eq!(
            result.content.children[0].children[0]
                .props
                .get_str(prop::CONTENT),
            Some("Hello World")
        );
    }

    #[test]
    fn test_pipeline() {
        // Create document with heading, empty paragraph, and content paragraph
        let mut document = doc(|d| {
            d.heading(1, |i| i.text("Title"))
                .para(|i| i.text("Content"))
        });
        let empty_para = Node::new(node::PARAGRAPH);
        document.content.children.insert(1, empty_para);

        assert_eq!(document.content.children.len(), 3);

        let pipeline = Pipeline::new().then(StripEmpty).then(ShiftHeadings::new(1));

        let result = pipeline.transform(document).unwrap();

        // Empty paragraph removed
        assert_eq!(result.content.children.len(), 2);
        // Heading level shifted
        assert_eq!(
            result.content.children[0].props.get_int(prop::LEVEL),
            Some(2)
        );
    }

    #[test]
    fn test_walk() {
        let document = doc(|d| d.para(|i| i.text("Hello").em(|i| i.text("World"))));

        let mut count = 0;
        walk(&document.content, &mut |_| count += 1);

        // document -> paragraph -> text, emphasis -> text
        assert_eq!(count, 5);
    }
}
