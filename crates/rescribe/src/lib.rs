//! Rescribe - Universal document conversion library
//!
//! Rescribe is a document conversion library inspired by Pandoc, with:
//! - Open node kinds (not fixed enum)
//! - Property bags for extensibility
//! - Fidelity tracking (know what was lost in conversion)
//! - Embedded resource handling
//! - Roundtrip-friendly design
//!
//! # Quick Start
//!
//! ```rust
//! use rescribe::prelude::*;
//!
//! // Parse markdown
//! let doc = rescribe::markdown::parse("# Hello\n\nWorld!").unwrap();
//!
//! // Convert to HTML
//! let html = rescribe::html::emit(&doc.value).unwrap();
//! let html_str = String::from_utf8(html.value).unwrap();
//!
//! assert!(html_str.contains("<h1>Hello</h1>"));
//! ```
//!
//! # Features
//!
//! Enable format support with Cargo features:
//!
//! - `markdown` - Markdown reader/writer (default)
//! - `html` - HTML reader/writer (default)
//! - `latex` - LaTeX reader/writer
//! - `org` - Org-mode reader/writer
//! - `plaintext` - Plain text writer
//! - `pdf` - PDF reader
//! - `docx` - DOCX (Word) reader/writer
//! - `std` - Standard node kinds (default)
//! - `math` - Math node kinds
//! - `all` - Enable all formats
//!
//! # Architecture
//!
//! Documents are represented as trees of `Node`s with:
//! - `kind`: A string identifying the node type (e.g., "paragraph", "heading")
//! - `props`: A property bag with typed values
//! - `children`: Child nodes
//!
//! Format-specific crates implement parsers (readers) and emitters (writers)
//! that convert between bytes and the document IR.

// Re-export core types
pub use rescribe_core::*;

/// Standard node kinds and helpers.
#[cfg(feature = "std")]
pub mod std {
    pub use rescribe_std::*;
}

/// Math node kinds.
#[cfg(feature = "math")]
pub mod math {
    pub use rescribe_math::*;
}

/// Markdown format support.
#[cfg(feature = "markdown")]
pub mod markdown {
    #[cfg(feature = "markdown")]
    pub use rescribe_read_markdown::parse;
    #[cfg(feature = "markdown")]
    pub use rescribe_read_markdown::parse_with_options;

    #[cfg(feature = "markdown")]
    pub use rescribe_write_markdown::emit;
    #[cfg(feature = "markdown")]
    pub use rescribe_write_markdown::emit_with_options;
}

/// HTML format support.
#[cfg(feature = "html")]
pub mod html {
    #[cfg(feature = "html")]
    pub use rescribe_read_html::parse;
    #[cfg(feature = "html")]
    pub use rescribe_read_html::parse_with_options;

    #[cfg(feature = "html")]
    pub use rescribe_write_html::emit;
    #[cfg(feature = "html")]
    pub use rescribe_write_html::emit_full_document;
    #[cfg(feature = "html")]
    pub use rescribe_write_html::emit_with_options;
}

/// LaTeX format support.
#[cfg(feature = "latex")]
pub mod latex {
    pub use rescribe_read_latex::parse;
    pub use rescribe_read_latex::parse_with_options;
    pub use rescribe_write_latex::emit;
    pub use rescribe_write_latex::emit_full_document;
    pub use rescribe_write_latex::emit_with_options;
}

/// Org-mode format support.
#[cfg(feature = "org")]
pub mod org {
    pub use rescribe_read_org::parse;
    pub use rescribe_read_org::parse_with_options;
    pub use rescribe_write_org::emit;
    pub use rescribe_write_org::emit_with_options;
}

/// Plain text format support.
#[cfg(feature = "plaintext")]
pub mod plaintext {
    pub use rescribe_write_plaintext::emit;
    pub use rescribe_write_plaintext::emit_with_options;
}

/// PDF format support (reader only).
#[cfg(feature = "pdf")]
pub mod pdf {
    pub use rescribe_read_pdf::parse;
    pub use rescribe_read_pdf::parse_with_options;
}

/// DOCX (Word) format support.
#[cfg(feature = "docx")]
pub mod docx {
    pub use rescribe_read_docx::parse;
    pub use rescribe_read_docx::parse_bytes;
    pub use rescribe_read_docx::parse_file;
    pub use rescribe_write_docx::emit;
}

/// Jupyter notebook (ipynb) format support.
#[cfg(feature = "ipynb")]
pub mod ipynb {
    pub use rescribe_read_ipynb::parse;
    pub use rescribe_read_ipynb::parse_bytes;
    pub use rescribe_write_ipynb::emit;
}

/// XLSX (Excel) format support (reader only).
#[cfg(feature = "xlsx")]
pub mod xlsx {
    pub use rescribe_read_xlsx::parse;
    pub use rescribe_read_xlsx::parse_bytes;
    pub use rescribe_read_xlsx::parse_file;
}

/// EPUB format support.
#[cfg(feature = "epub")]
pub mod epub {
    pub use rescribe_read_epub::parse;
    pub use rescribe_read_epub::parse_bytes;
    pub use rescribe_read_epub::parse_file;
    pub use rescribe_write_epub::emit;
}

/// Djot format support.
#[cfg(feature = "djot")]
pub mod djot {
    pub use rescribe_read_djot::parse;
    pub use rescribe_write_djot::emit;
}

/// OPML format support.
#[cfg(feature = "opml")]
pub mod opml {
    pub use rescribe_read_opml::parse;
    pub use rescribe_write_opml::emit;
}

/// MediaWiki format support.
#[cfg(feature = "mediawiki")]
pub mod mediawiki {
    pub use rescribe_read_mediawiki::parse;
    pub use rescribe_write_mediawiki::emit;
}

/// BibTeX format support (reader only).
#[cfg(feature = "bibtex")]
pub mod bibtex {
    pub use rescribe_read_bibtex::parse;
}

/// CSL JSON format support.
#[cfg(feature = "csl-json")]
pub mod csl_json {
    pub use rescribe_read_csl_json::parse;
    pub use rescribe_write_csl_json::emit;
}

/// DocBook format support.
#[cfg(feature = "docbook")]
pub mod docbook {
    pub use rescribe_read_docbook::parse;
    pub use rescribe_write_docbook::emit;
}

/// reStructuredText format support (reader only).
#[cfg(feature = "rst")]
pub mod rst {
    pub use rescribe_read_rst::parse;
    pub use rescribe_read_rst::parse_with_options;
}

/// Common imports for typical usage.
pub mod prelude {
    pub use crate::{ConversionResult, Document, Node, PropValue, Properties};

    #[cfg(feature = "std")]
    pub use crate::std::{builder, node, prop};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(all(feature = "markdown", feature = "html", feature = "std"))]
    fn test_markdown_to_html() {
        let result = markdown::parse("# Hello\n\nWorld!").unwrap();
        let doc = result.value;

        let html_result = html::emit(&doc).unwrap();
        let html = String::from_utf8(html_result.value).unwrap();

        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
        assert!(html.contains("<p>"));
        assert!(html.contains("World!"));
    }

    #[test]
    #[cfg(all(feature = "markdown", feature = "latex"))]
    fn test_markdown_to_latex() {
        let result = markdown::parse("# Title\n\n**Bold** text").unwrap();
        let doc = result.value;

        let latex_result = latex::emit(&doc).unwrap();
        let latex = String::from_utf8(latex_result.value).unwrap();

        assert!(latex.contains("\\section{Title}"));
        assert!(latex.contains("\\textbf{Bold}"));
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_build_document_manually() {
        use crate::std::builder::doc;

        let document = doc(|d| {
            d.heading(1, |i| i.text("Manual Document"))
                .para(|i| i.text("This is ").strong(|i| i.text("bold")).text(" text."))
        });

        assert_eq!(document.content.children.len(), 2);
    }
}
