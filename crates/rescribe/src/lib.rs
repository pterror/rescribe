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

/// XLSX (Excel) format support.
#[cfg(feature = "xlsx")]
pub mod xlsx {
    pub use rescribe_read_xlsx::parse;
    pub use rescribe_read_xlsx::parse_bytes;
    pub use rescribe_read_xlsx::parse_file;
    pub use rescribe_write_xlsx::emit;
    pub use rescribe_write_xlsx::emit_with_options;
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

/// BibTeX format support.
#[cfg(feature = "bibtex")]
pub mod bibtex {
    pub use rescribe_read_bibtex::parse;
    pub use rescribe_write_bibtex::emit;
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

/// reStructuredText format support.
#[cfg(feature = "rst")]
pub mod rst {
    pub use rescribe_read_rst::parse;
    pub use rescribe_read_rst::parse_with_options;
    pub use rescribe_write_rst::emit;
    pub use rescribe_write_rst::emit_with_options;
}

/// AsciiDoc format support.
#[cfg(feature = "asciidoc")]
pub mod asciidoc {
    pub use rescribe_read_asciidoc::parse;
    pub use rescribe_read_asciidoc::parse_with_options;
    pub use rescribe_write_asciidoc::emit;
    pub use rescribe_write_asciidoc::emit_with_options;
}

/// Typst format support.
#[cfg(feature = "typst")]
pub mod typst {
    pub use rescribe_read_typst::parse;
    pub use rescribe_read_typst::parse_with_options;
    pub use rescribe_write_typst::emit;
    pub use rescribe_write_typst::emit_with_options;
}

/// ANSI terminal format support (writer only).
#[cfg(feature = "ansi")]
pub mod ansi {
    pub use rescribe_write_ansi::emit;
    pub use rescribe_write_ansi::emit_with_options;
}

/// DokuWiki format support.
#[cfg(feature = "dokuwiki")]
pub mod dokuwiki {
    pub use rescribe_read_dokuwiki::parse;
    pub use rescribe_read_dokuwiki::parse_with_options;
    pub use rescribe_write_dokuwiki::emit;
    pub use rescribe_write_dokuwiki::emit_with_options;
}

/// JATS (Journal Article Tag Suite) format support.
#[cfg(feature = "jats")]
pub mod jats {
    pub use rescribe_read_jats::parse;
    pub use rescribe_write_jats::emit;
}

/// TEI (Text Encoding Initiative) format support.
#[cfg(feature = "tei")]
pub mod tei {
    pub use rescribe_read_tei::parse;
    pub use rescribe_write_tei::emit;
}

/// Man page (roff/troff) format support.
#[cfg(feature = "man")]
pub mod man {
    pub use rescribe_read_man::parse;
    pub use rescribe_write_man::emit;
    pub use rescribe_write_man::emit_with_options;
}

/// Jira/Confluence markup format support.
#[cfg(feature = "jira")]
pub mod jira {
    pub use rescribe_read_jira::parse;
    pub use rescribe_read_jira::parse_with_options;
    pub use rescribe_write_jira::emit;
    pub use rescribe_write_jira::emit_with_options;
}

/// Creole wiki markup format support.
#[cfg(feature = "creole")]
pub mod creole {
    pub use rescribe_read_creole::parse;
    pub use rescribe_read_creole::parse_with_options;
    pub use rescribe_write_creole::emit;
    pub use rescribe_write_creole::emit_with_options;
}

/// Textile markup format support.
#[cfg(feature = "textile")]
pub mod textile {
    pub use rescribe_read_textile::parse;
    pub use rescribe_read_textile::parse_with_options;
    pub use rescribe_write_textile::emit;
    pub use rescribe_write_textile::emit_with_options;
}

/// Haddock (Haskell documentation) format support.
#[cfg(feature = "haddock")]
pub mod haddock {
    pub use rescribe_read_haddock::parse;
    pub use rescribe_read_haddock::parse_with_options;
    pub use rescribe_write_haddock::emit;
    pub use rescribe_write_haddock::emit_with_options;
}

/// Muse (Emacs Muse) format support.
#[cfg(feature = "muse")]
pub mod muse {
    pub use rescribe_read_muse::parse;
    pub use rescribe_read_muse::parse_with_options;
    pub use rescribe_write_muse::emit;
    pub use rescribe_write_muse::emit_with_options;
}

/// txt2tags (t2t) format support.
#[cfg(feature = "t2t")]
pub mod t2t {
    pub use rescribe_read_t2t::parse;
    pub use rescribe_read_t2t::parse_with_options;
    pub use rescribe_write_t2t::emit;
    pub use rescribe_write_t2t::emit_with_options;
}

/// RTF (Rich Text Format) support.
#[cfg(feature = "rtf")]
pub mod rtf {
    pub use rescribe_read_rtf::parse;
    pub use rescribe_read_rtf::parse_with_options;
    pub use rescribe_write_rtf::emit;
    pub use rescribe_write_rtf::emit_with_options;
}

/// VimWiki format support.
#[cfg(feature = "vimwiki")]
pub mod vimwiki {
    pub use rescribe_read_vimwiki::parse;
    pub use rescribe_read_vimwiki::parse_with_options;
    pub use rescribe_write_vimwiki::emit;
    pub use rescribe_write_vimwiki::emit_with_options;
}

/// ZimWiki (Zim Desktop Wiki) format support.
#[cfg(feature = "zimwiki")]
pub mod zimwiki {
    pub use rescribe_read_zimwiki::parse;
    pub use rescribe_read_zimwiki::parse_with_options;
    pub use rescribe_write_zimwiki::emit;
    pub use rescribe_write_zimwiki::emit_with_options;
}

/// POD (Plain Old Documentation) format support.
#[cfg(feature = "pod")]
pub mod pod {
    pub use rescribe_read_pod::parse;
    pub use rescribe_read_pod::parse_with_options;
    pub use rescribe_write_pod::emit;
    pub use rescribe_write_pod::emit_with_options;
}

/// Markua (Leanpub) format support.
#[cfg(feature = "markua")]
pub mod markua {
    pub use rescribe_read_markua::parse;
    pub use rescribe_read_markua::parse_with_options;
    pub use rescribe_write_markua::emit;
    pub use rescribe_write_markua::emit_with_options;
}

/// FictionBook 2 (FB2) format support.
#[cfg(feature = "fb2")]
pub mod fb2 {
    pub use rescribe_read_fb2::parse;
    pub use rescribe_read_fb2::parse_with_options;
    pub use rescribe_write_fb2::emit;
    pub use rescribe_write_fb2::emit_with_options;
}

/// Texinfo (GNU documentation) format support.
#[cfg(feature = "texinfo")]
pub mod texinfo {
    pub use rescribe_read_texinfo::parse;
    pub use rescribe_read_texinfo::parse_with_options;
    pub use rescribe_write_texinfo::emit;
    pub use rescribe_write_texinfo::emit_with_options;
}

/// TikiWiki format support.
#[cfg(feature = "tikiwiki")]
pub mod tikiwiki {
    pub use rescribe_read_tikiwiki::parse;
    pub use rescribe_read_tikiwiki::parse_with_options;
    pub use rescribe_write_tikiwiki::emit;
    pub use rescribe_write_tikiwiki::emit_with_options;
}

/// TWiki format support.
#[cfg(feature = "twiki")]
pub mod twiki {
    pub use rescribe_read_twiki::parse;
    pub use rescribe_read_twiki::parse_with_options;
    pub use rescribe_write_twiki::emit;
    pub use rescribe_write_twiki::emit_with_options;
}

/// XWiki format support.
#[cfg(feature = "xwiki")]
pub mod xwiki {
    pub use rescribe_read_xwiki::parse;
    pub use rescribe_read_xwiki::parse_with_options;
    pub use rescribe_write_xwiki::emit;
    pub use rescribe_write_xwiki::emit_with_options;
}

/// reveal.js HTML presentation format support (writer only).
#[cfg(feature = "revealjs")]
pub mod revealjs {
    pub use rescribe_write_revealjs::emit;
    pub use rescribe_write_revealjs::emit_with_options;
}

/// W3C Slidy HTML presentation format support (writer only).
#[cfg(feature = "slidy")]
pub mod slidy {
    pub use rescribe_write_slidy::emit;
    pub use rescribe_write_slidy::emit_with_options;
}

/// S5 HTML presentation format support (writer only).
#[cfg(feature = "s5")]
pub mod s5 {
    pub use rescribe_write_s5::emit;
    pub use rescribe_write_s5::emit_with_options;
}

/// DZSlides HTML presentation format support (writer only).
#[cfg(feature = "dzslides")]
pub mod dzslides {
    pub use rescribe_write_dzslides::emit;
    pub use rescribe_write_dzslides::emit_with_options;
}

/// BBCode forum markup format support.
#[cfg(feature = "bbcode")]
pub mod bbcode {
    pub use rescribe_read_bbcode::parse;
    pub use rescribe_read_bbcode::parse_with_options;
    pub use rescribe_write_bbcode::emit;
    pub use rescribe_write_bbcode::emit_with_options;
}

/// ANSI escape sequence format support (reader only).
#[cfg(feature = "ansi-read")]
pub mod ansi_read {
    pub use rescribe_read_ansi::parse;
    pub use rescribe_read_ansi::parse_with_options;
}

/// Beamer (LaTeX presentation) format support (writer only).
#[cfg(feature = "beamer")]
pub mod beamer {
    pub use rescribe_write_beamer::emit;
    pub use rescribe_write_beamer::emit_with_options;
}

/// CSV (Comma-Separated Values) format support.
#[cfg(feature = "csv")]
pub mod csv {
    pub use rescribe_read_csv::parse;
    pub use rescribe_read_csv::parse_with_options;
    pub use rescribe_write_csv::emit;
    pub use rescribe_write_csv::emit_with_options;
}

/// ConTeXt format support (writer only).
#[cfg(feature = "context")]
pub mod context {
    pub use rescribe_write_context::emit;
    pub use rescribe_write_context::emit_with_options;
}

/// Groff ms macro format support (writer only).
#[cfg(feature = "ms")]
pub mod ms {
    pub use rescribe_write_ms::emit;
    pub use rescribe_write_ms::emit_with_options;
}

/// Chunked HTML format support (writer only).
#[cfg(feature = "chunkedhtml")]
pub mod chunkedhtml {
    pub use rescribe_write_chunkedhtml::HtmlChunk;
    pub use rescribe_write_chunkedhtml::emit;
    pub use rescribe_write_chunkedhtml::emit_with_options;
}

/// TSV (Tab-Separated Values) format support.
#[cfg(feature = "tsv")]
pub mod tsv {
    pub use rescribe_read_tsv::parse;
    pub use rescribe_read_tsv::parse_with_options;
    pub use rescribe_write_tsv::emit;
    pub use rescribe_write_tsv::emit_with_options;
}

/// ICML (InCopy Markup Language) format support (writer only).
#[cfg(feature = "icml")]
pub mod icml {
    pub use rescribe_write_icml::emit;
    pub use rescribe_write_icml::emit_with_options;
}

/// Slideous HTML slideshow format support (writer only).
#[cfg(feature = "slideous")]
pub mod slideous {
    pub use rescribe_write_slideous::emit;
    pub use rescribe_write_slideous::emit_with_options;
}

/// ODT (OpenDocument Text) format support.
#[cfg(feature = "odt")]
pub mod odt {
    pub use rescribe_read_odt::parse;
    pub use rescribe_read_odt::parse_with_options;
    pub use rescribe_write_odt::emit;
    pub use rescribe_write_odt::emit_with_options;
}

/// Native debug format support.
#[cfg(feature = "native")]
pub mod native {
    pub use rescribe_read_native::parse;
    pub use rescribe_read_native::parse_with_options;
    pub use rescribe_write_native::emit;
    pub use rescribe_write_native::emit_with_options;
}

/// PPTX (PowerPoint) format support.
#[cfg(feature = "pptx")]
pub mod pptx {
    pub use rescribe_read_pptx::parse;
    pub use rescribe_read_pptx::parse_with_options;
    pub use rescribe_write_pptx::emit;
    pub use rescribe_write_pptx::emit_with_options;
}

/// CommonMark format support.
#[cfg(feature = "commonmark")]
pub mod commonmark {
    pub use rescribe_read_commonmark::parse;
    pub use rescribe_read_commonmark::parse_with_options;
    pub use rescribe_write_commonmark::emit;
    pub use rescribe_write_commonmark::emit_with_options;
}

/// GitHub Flavored Markdown (GFM) format support.
#[cfg(feature = "gfm")]
pub mod gfm {
    pub use rescribe_read_gfm::parse;
    pub use rescribe_read_gfm::parse_with_options;
    pub use rescribe_write_gfm::emit;
    pub use rescribe_write_gfm::emit_with_options;
}

/// RIS (Research Information Systems) bibliographic format support.
#[cfg(feature = "ris")]
pub mod ris {
    pub use rescribe_read_ris::parse;
    pub use rescribe_read_ris::parse_with_options;
    pub use rescribe_write_ris::emit;
    pub use rescribe_write_ris::emit_with_options;
}

/// EndNote XML bibliographic format support.
#[cfg(feature = "endnotexml")]
pub mod endnotexml {
    pub use rescribe_read_endnotexml::parse;
    pub use rescribe_read_endnotexml::parse_with_options;
    pub use rescribe_write_endnotexml::emit;
    pub use rescribe_write_endnotexml::emit_with_options;
}

/// BibLaTeX bibliographic format support.
#[cfg(feature = "biblatex")]
pub mod biblatex {
    pub use rescribe_read_biblatex::parse;
    pub use rescribe_read_biblatex::parse_with_options;
    pub use rescribe_write_biblatex::emit;
    pub use rescribe_write_biblatex::emit_with_options;
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
