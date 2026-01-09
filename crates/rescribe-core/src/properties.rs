//! Property system for extensible node attributes.

use std::collections::HashMap;

/// A collection of properties (key-value pairs).
#[derive(Debug, Clone, Default)]
pub struct Properties(HashMap<String, PropValue>);

/// A property value.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<PropValue>),
    Map(HashMap<String, PropValue>),
}

impl Properties {
    /// Create an empty property set.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Set a property.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<PropValue>) {
        self.0.insert(key.into(), value.into());
    }

    /// Get a property.
    pub fn get(&self, key: &str) -> Option<&PropValue> {
        self.0.get(key)
    }

    /// Get a string property.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(PropValue::String(s)) => Some(s),
            _ => None,
        }
    }

    /// Get an integer property.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        match self.0.get(key) {
            Some(PropValue::Int(i)) => Some(*i),
            _ => None,
        }
    }

    /// Get a boolean property.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.0.get(key) {
            Some(PropValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// Check if a property exists.
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Iterate over properties.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &PropValue)> {
        self.0.iter()
    }
}

// Standard property keys
pub mod prop {
    // Semantic properties (format-agnostic)
    pub const LEVEL: &str = "level";
    pub const ORDERED: &str = "ordered";
    pub const LANGUAGE: &str = "language";
    pub const URL: &str = "url";
    pub const TITLE: &str = "title";
    pub const ALT: &str = "alt";
    pub const CONTENT: &str = "content";
    pub const RESOURCE_ID: &str = "resource";

    // Style properties (presentational)
    pub const STYLE_FONT: &str = "style:font";
    pub const STYLE_SIZE: &str = "style:size";
    pub const STYLE_COLOR: &str = "style:color";
    pub const STYLE_ALIGN: &str = "style:align";

    // Layout properties (positioning)
    pub const LAYOUT_PAGE_BREAK: &str = "layout:page_break";
    pub const LAYOUT_COLUMN: &str = "layout:column";
    pub const LAYOUT_FLOAT: &str = "layout:float";

    // Format-specific prefixes
    pub const HTML_PREFIX: &str = "html:";
    pub const LATEX_PREFIX: &str = "latex:";
    pub const DOCX_PREFIX: &str = "docx:";
}

// Conversions
impl From<String> for PropValue {
    fn from(s: String) -> Self {
        PropValue::String(s)
    }
}

impl From<&str> for PropValue {
    fn from(s: &str) -> Self {
        PropValue::String(s.to_string())
    }
}

impl From<i64> for PropValue {
    fn from(i: i64) -> Self {
        PropValue::Int(i)
    }
}

impl From<i32> for PropValue {
    fn from(i: i32) -> Self {
        PropValue::Int(i as i64)
    }
}

impl From<f64> for PropValue {
    fn from(f: f64) -> Self {
        PropValue::Float(f)
    }
}

impl From<bool> for PropValue {
    fn from(b: bool) -> Self {
        PropValue::Bool(b)
    }
}
