//! Fidelity tracking - know what was lost in conversion.

use crate::{ResourceId, Span};

/// Result of a conversion operation, including fidelity warnings.
#[derive(Debug)]
pub struct ConversionResult<T> {
    /// The conversion output.
    pub value: T,
    /// Warnings about information that was lost or transformed.
    pub warnings: Vec<FidelityWarning>,
}

impl<T> ConversionResult<T> {
    /// Create a successful result with no warnings.
    pub fn ok(value: T) -> Self {
        Self {
            value,
            warnings: Vec::new(),
        }
    }

    /// Create a result with warnings.
    pub fn with_warnings(value: T, warnings: Vec<FidelityWarning>) -> Self {
        Self { value, warnings }
    }

    /// Add a warning.
    pub fn warn(mut self, warning: FidelityWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Check if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Check if there are any major or error-level warnings.
    pub fn has_errors(&self) -> bool {
        self.warnings
            .iter()
            .any(|w| matches!(w.severity, Severity::Major | Severity::Error))
    }
}

/// A warning about fidelity loss during conversion.
#[derive(Debug, Clone)]
pub struct FidelityWarning {
    /// How severe is this warning?
    pub severity: Severity,
    /// What kind of issue?
    pub kind: WarningKind,
    /// Human-readable message.
    pub message: String,
    /// Where in the source this occurred.
    pub span: Option<Span>,
}

impl FidelityWarning {
    /// Create a new warning.
    pub fn new(severity: Severity, kind: WarningKind, message: impl Into<String>) -> Self {
        Self {
            severity,
            kind,
            message: message.into(),
            span: None,
        }
    }

    /// Set the source span.
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

/// Severity of a fidelity warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Information only, no data lost.
    Info,
    /// Minor formatting may differ.
    Minor,
    /// Significant information lost.
    Major,
    /// Conversion may be incorrect.
    Error,
}

/// Kind of fidelity issue.
#[derive(Debug, Clone)]
pub enum WarningKind {
    /// Property not supported by target format.
    UnsupportedProperty(String),
    /// Node kind not supported, using fallback.
    UnsupportedNode(String),
    /// Complex structure simplified.
    Simplified(String),
    /// Resource could not be embedded.
    ResourceFailed(ResourceId),
    /// Format-specific feature lost.
    FeatureLost(String),
}
