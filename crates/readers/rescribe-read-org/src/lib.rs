//! Org-mode reader for rescribe.
//!
//! Parses Org-mode source into rescribe's document IR.
//!
//! Currently uses a handwritten parser. Tree-sitter support is pending
//! an update to tree-sitter-org for tree-sitter 0.26 compatibility.

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};

#[cfg(feature = "handwritten")]
mod handwritten;

/// Parse Org-mode text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Org-mode with custom options.
#[cfg(feature = "handwritten")]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    handwritten::parse_with_options(input, options)
}

/// Parse using specifically the handwritten backend.
#[cfg(feature = "handwritten")]
pub mod backend_handwritten {
    pub use crate::handwritten::{parse, parse_with_options};
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::{Node, node, prop};

    fn root_children(doc: &Document) -> &[Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_heading() {
        let input = "* Hello World\n** Subheading";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(1));
        assert_eq!(children[1].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_paragraph() {
        let input = "This is a paragraph.\n\nThis is another.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind.as_str(), node::PARAGRAPH);
        assert_eq!(children[1].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_emphasis() {
        let input = "/italic/ and *bold*";
        let result = parse(input).unwrap();
        let doc = result.value;
        let para = &root_children(&doc)[0];

        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::STRONG)
        );
    }

    #[test]
    fn test_parse_list() {
        let input = "- First item\n- Second item";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        let list = &children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_code_block() {
        let input = "#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        let code = &children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
        assert_eq!(code.props.get_str(prop::LANGUAGE), Some("rust"));
    }

    #[test]
    #[cfg(feature = "handwritten")]
    fn test_parse_metadata() {
        let input = "#+TITLE: My Document\n#+AUTHOR: Jane Doe\n\nContent here.";
        let result = parse(input).unwrap();
        let doc = result.value;

        assert_eq!(doc.metadata.get_str("title"), Some("My Document"));
        assert_eq!(doc.metadata.get_str("author"), Some("Jane Doe"));
    }
}
