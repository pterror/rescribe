//! Document type - the root container for content and resources.

use crate::{Node, Properties, Resource, ResourceId, ResourceMap};

/// A document with content and embedded resources.
#[derive(Debug, Clone)]
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

/// Information about the source format, for better roundtrip fidelity.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Source format identifier (e.g., "markdown", "html", "docx").
    pub format: String,
    /// Format-specific metadata preserved for roundtrip.
    pub metadata: Properties,
}

impl Document {
    /// Create a new empty document.
    pub fn new() -> Self {
        Self {
            content: Node::new("document"),
            resources: ResourceMap::new(),
            metadata: Properties::new(),
            source: None,
        }
    }

    /// Set the root content node.
    pub fn with_content(mut self, content: Node) -> Self {
        self.content = content;
        self
    }

    /// Set document metadata.
    pub fn with_metadata(mut self, metadata: Properties) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set source format info.
    pub fn with_source(mut self, source: SourceInfo) -> Self {
        self.source = Some(source);
        self
    }

    /// Embed a resource and return its ID.
    pub fn embed(&mut self, resource: Resource) -> ResourceId {
        let id = ResourceId::new();
        self.resources.insert(id.clone(), resource);
        id
    }

    /// Get a resource by ID.
    pub fn resource(&self, id: &ResourceId) -> Option<&Resource> {
        self.resources.get(id)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
