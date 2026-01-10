//! LaTeX reader for rescribe.
//!
//! Parses LaTeX source into rescribe's document IR.
//!
//! This crate supports multiple parser backends:
//! - `handwritten` (default) - Hand-rolled parser, good coverage
//! - `tree-sitter` - Uses tree-sitter-latex, better for precise spans

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};

#[cfg(feature = "handwritten")]
mod handwritten;

#[cfg(feature = "tree-sitter")]
mod treesitter;

/// Parse LaTeX text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse LaTeX with custom options.
#[cfg(feature = "handwritten")]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    handwritten::parse_with_options(input, options)
}

/// Parse LaTeX with custom options.
#[cfg(all(feature = "tree-sitter", not(feature = "handwritten")))]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    treesitter::parse_with_options(input, options)
}

/// Parse using specifically the handwritten backend.
#[cfg(feature = "handwritten")]
pub mod backend_handwritten {
    pub use crate::handwritten::{parse, parse_with_options};
}

/// Parse using specifically the tree-sitter backend.
#[cfg(feature = "tree-sitter")]
pub mod backend_treesitter {
    pub use crate::treesitter::{parse, parse_with_options};
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::{Node, node, prop};

    fn root_children(doc: &Document) -> &[Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_section() {
        let input = "\\section{Hello World}";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(1));
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
        let input = "\\textit{italic} and \\textbf{bold}";
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
        let input = r"
\begin{itemize}
\item First item
\item Second item
\end{itemize}
";
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
    fn test_parse_verbatim() {
        let input = r#"
\begin{verbatim}
fn main() {
    println!("Hello");
}
\end{verbatim}
"#;
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        let code = &children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
        assert!(code.props.get_str(prop::CONTENT).is_some());
    }
}
