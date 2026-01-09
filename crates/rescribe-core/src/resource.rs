//! Resource management for embedded content (images, fonts, etc.).

use std::collections::HashMap;
use crate::Properties;

/// Unique identifier for an embedded resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(String);

/// Map of resource IDs to resources.
pub type ResourceMap = HashMap<ResourceId, Resource>;

/// An embedded resource (image, font, data file, etc.).
#[derive(Debug, Clone)]
pub struct Resource {
    /// Original filename or identifier.
    pub name: Option<String>,
    /// MIME type.
    pub mime_type: String,
    /// Raw data.
    pub data: Vec<u8>,
    /// Resource metadata.
    pub metadata: Properties,
}

impl ResourceId {
    /// Generate a new unique resource ID.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(format!("res_{id}"))
    }

    /// Create a resource ID from a string.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl Resource {
    /// Create a new resource.
    pub fn new(mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            name: None,
            mime_type: mime_type.into(),
            data,
            metadata: Properties::new(),
        }
    }

    /// Set the resource name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Create an image resource.
    pub fn image(mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self::new(mime_type, data)
    }

    /// Create a PNG image resource.
    pub fn png(data: Vec<u8>) -> Self {
        Self::new("image/png", data)
    }

    /// Create a JPEG image resource.
    pub fn jpeg(data: Vec<u8>) -> Self {
        Self::new("image/jpeg", data)
    }
}
