//! Parser, Emitter, and Transformer traits.

use crate::{ConversionResult, Document};

/// Options for parsing.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Preserve format-specific properties.
    pub preserve_source_info: bool,
    /// Embed external resources (images, etc.).
    pub embed_resources: bool,
}

/// Options for emitting.
#[derive(Debug, Clone, Default)]
pub struct EmitOptions {
    /// Pretty-print output where applicable.
    pub pretty: bool,
    /// Include format-specific properties from source.
    pub use_source_info: bool,
}

/// Error during parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Error during emitting.
#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("unsupported node kind: {0}")]
    UnsupportedNode(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Error during transformation.
#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("transform failed: {0}")]
    Failed(String),
}

/// Parse a format into the document IR.
pub trait Parser: Send + Sync {
    /// Formats this parser can handle.
    fn formats(&self) -> &[&str];

    /// Parse bytes into a document.
    fn parse(&self, input: &[u8], options: &ParseOptions) -> Result<ConversionResult<Document>, ParseError>;
}

/// Emit the document IR to a format.
pub trait Emitter: Send + Sync {
    /// Formats this emitter can produce.
    fn formats(&self) -> &[&str];

    /// Emit a document to bytes.
    fn emit(&self, doc: &Document, options: &EmitOptions) -> Result<ConversionResult<Vec<u8>>, EmitError>;
}

/// Transform a document (same IR, modified content).
pub trait Transformer: Send + Sync {
    /// Name of this transformer.
    fn name(&self) -> &str;

    /// Transform a document.
    fn transform(&self, doc: Document) -> Result<Document, TransformError>;
}
