//! Markdown reader for rescribe.
//!
//! Parses CommonMark (with extensions) into rescribe's document IR.
//!
//! This crate supports multiple parser backends:
//! - `pulldown` (default) - Uses pulldown-cmark, pure Rust, CommonMark compliant
//! - `tree-sitter` - Uses tree-sitter-md, better error recovery and precise spans

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};

#[cfg(feature = "pulldown")]
mod pulldown;

#[cfg(feature = "tree-sitter")]
mod treesitter;

/// Parse markdown text into a rescribe Document.
///
/// Uses the default parser backend (pulldown-cmark if available, else tree-sitter).
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse markdown with custom options.
#[cfg(feature = "pulldown")]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    pulldown::parse_with_options(input, options)
}

/// Parse markdown with custom options.
#[cfg(all(feature = "tree-sitter", not(feature = "pulldown")))]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    treesitter::parse_with_options(input, options)
}

/// Parse using specifically the pulldown-cmark backend.
#[cfg(feature = "pulldown")]
pub mod backend_pulldown {
    pub use crate::pulldown::{parse, parse_with_options};
}

/// Parse using specifically the tree-sitter backend.
#[cfg(feature = "tree-sitter")]
pub mod backend_treesitter {
    pub use crate::treesitter::{parse, parse_with_options};
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::{node, prop};

    fn root_children(doc: &Document) -> &[rescribe_std::Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_paragraph() {
        let result = parse("Hello, world!").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_heading() {
        let result = parse("# Heading 1\n\n## Heading 2").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(1));
        assert_eq!(children[1].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_emphasis() {
        let result = parse("*italic* and **bold**").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        let para = &children[0];
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
    fn test_parse_link() {
        let result = parse("[example](https://example.com)").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        let para = &children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_code_block() {
        let result = parse("```rust\nfn main() {}\n```").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(children[0].props.get_str(prop::LANGUAGE), Some("rust"));
    }

    #[test]
    fn test_parse_list() {
        let result = parse("- item 1\n- item 2").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(children[0].children.len(), 2);
    }

    #[test]
    fn test_parse_ordered_list() {
        let result = parse("1. first\n2. second").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    #[cfg(feature = "pulldown")]
    fn test_parse_yaml_frontmatter() {
        let input = r#"---
title: My Document
author: John Doe
date: 2024-01-15
draft: true
tags:
  - rust
  - markdown
---

# Hello

Content here."#;
        let result = parse(input).unwrap();
        let doc = result.value;

        assert_eq!(doc.metadata.get_str("title"), Some("My Document"));
        assert_eq!(doc.metadata.get_str("author"), Some("John Doe"));
        assert_eq!(doc.metadata.get_str("date"), Some("2024-01-15"));
        assert_eq!(doc.metadata.get_bool("draft"), Some(true));
        assert_eq!(doc.metadata.get_str("tags"), Some("rust, markdown"));

        let children = root_children(&doc);
        assert!(!children.is_empty());
        assert_eq!(children[0].kind.as_str(), node::HEADING);
    }

    #[test]
    fn test_preserve_source_info() {
        let input = "# Hello\n\nWorld!";
        let options = ParseOptions {
            preserve_source_info: true,
            ..Default::default()
        };
        let result = parse_with_options(input, &options).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        let heading = &children[0];
        assert!(heading.span.is_some());
        let span = heading.span.unwrap();
        assert_eq!(span.start, 0);
        assert!(span.end > span.start);

        let para = &children[1];
        assert!(para.span.is_some());
    }

    #[test]
    fn test_no_spans_by_default() {
        let input = "# Hello";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        assert!(children[0].span.is_none());
    }

    #[test]
    fn test_parse_task_list() {
        let result = parse("- [ ] unchecked\n- [x] checked").unwrap();
        let doc = result.value;
        let children = root_children(&doc);
        let list = &children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);

        // First item should be unchecked
        let item1 = &list.children[0];
        assert_eq!(item1.kind.as_str(), node::LIST_ITEM);
        assert_eq!(item1.props.get_bool(prop::CHECKED), Some(false));

        // Second item should be checked
        let item2 = &list.children[1];
        assert_eq!(item2.kind.as_str(), node::LIST_ITEM);
        assert_eq!(item2.props.get_bool(prop::CHECKED), Some(true));
    }
}
