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

    /// Remove a property and return its value.
    pub fn remove(&mut self, key: &str) -> Option<PropValue> {
        self.0.remove(key)
    }

    /// Iterate over properties.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &PropValue)> {
        self.0.iter()
    }

    /// Check if the property set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the number of properties.
    pub fn len(&self) -> usize {
        self.0.len()
    }
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
