//! HTML reader for rescribe.
//!
//! Parses HTML5 into rescribe's document IR.
//!
//! This crate supports multiple parser backends:
//! - `html5ever` (default) - Uses html5ever, full HTML5 compliance
//! - `tree-sitter` - Uses tree-sitter-html, better for precise spans

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};

#[cfg(feature = "html5ever")]
mod html5ever_backend;

#[cfg(feature = "tree-sitter")]
mod treesitter;

/// Parse HTML text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse HTML with custom options.
#[cfg(feature = "html5ever")]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    html5ever_backend::parse_with_options(input, options)
}

/// Parse HTML with custom options.
#[cfg(all(feature = "tree-sitter", not(feature = "html5ever")))]
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    treesitter::parse_with_options(input, options)
}

/// Parse using specifically the html5ever backend.
#[cfg(feature = "html5ever")]
pub mod backend_html5ever {
    pub use crate::html5ever_backend::{parse, parse_with_options};
}

/// Parse using specifically the tree-sitter backend.
#[cfg(feature = "tree-sitter")]
pub mod backend_treesitter {
    pub use crate::treesitter::{parse, parse_with_options};
}

// Common utilities used by both backends
use rescribe_std::{Node, node, prop};

/// Extract text content from a list of nodes.
pub(crate) fn extract_text_content(nodes: &[Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            text.push_str(content);
        }
        text.push_str(&extract_text_content(&node.children));
    }
    text
}

/// Try to get the language from a code element inside pre.
pub(crate) fn get_code_language(children: &[Node]) -> Option<String> {
    for child in children {
        if child.kind.as_str() == node::CODE
            && let Some(classes) = child.props.get_str(prop::CLASSES)
        {
            for class in classes.split_whitespace() {
                if let Some(lang) = class.strip_prefix("language-") {
                    return Some(lang.to_string());
                }
            }
        }
    }
    None
}

/// Parse a data URI into mime type and data.
pub(crate) fn parse_data_uri(uri: &str) -> Option<(String, Vec<u8>)> {
    let uri = uri.strip_prefix("data:")?;
    let (header, data) = uri.split_once(',')?;

    let is_base64 = header.ends_with(";base64");
    let mime_type = if is_base64 {
        header
            .strip_suffix(";base64")
            .unwrap_or("application/octet-stream")
    } else if header.is_empty() {
        "text/plain;charset=US-ASCII"
    } else {
        header
    };

    let decoded = if is_base64 {
        base64_decode(data)?
    } else {
        percent_decode(data)
    };

    Some((mime_type.to_string(), decoded))
}

/// Simple base64 decoder.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let input: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r' && b != b' ')
        .collect();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        let mut buf = [0u8; 4];
        let mut len = 0;

        for (i, &byte) in chunk.iter().enumerate() {
            if byte == b'=' {
                break;
            }
            buf[i] = ALPHABET.iter().position(|&c| c == byte)? as u8;
            len = i + 1;
        }

        if len >= 2 {
            output.push((buf[0] << 2) | (buf[1] >> 4));
        }
        if len >= 3 {
            output.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if len >= 4 {
            output.push((buf[2] << 6) | buf[3]);
        }
    }

    Some(output)
}

/// Simple percent-decoding for data URIs.
fn percent_decode(input: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut chars = input.bytes().peekable();

    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let high = chars.next().and_then(|c| (c as char).to_digit(16));
            let low = chars.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h), Some(l)) = (high, low) {
                output.push((h * 16 + l) as u8);
            }
        } else {
            output.push(byte);
        }
    }

    output
}

/// Check if an element is a block-level element.
pub(crate) fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "canvas"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "noscript"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "tfoot"
            | "ul"
            | "video"
    )
}

/// Merge adjacent text nodes and clean up whitespace.
pub(crate) fn merge_text_nodes(nodes: &mut Vec<Node>) {
    if nodes.is_empty() {
        return;
    }

    let mut i = 0;
    while i < nodes.len() {
        merge_text_nodes(&mut nodes[i].children);

        if nodes[i].kind.as_str() == node::TEXT
            && let Some(content) = nodes[i].props.get_str(prop::CONTENT)
            && content.is_empty()
        {
            nodes.remove(i);
            continue;
        }

        if i + 1 < nodes.len()
            && nodes[i].kind.as_str() == node::TEXT
            && nodes[i + 1].kind.as_str() == node::TEXT
        {
            let next_content = nodes[i + 1]
                .props
                .get_str(prop::CONTENT)
                .unwrap_or("")
                .to_string();
            let current_content = nodes[i]
                .props
                .get_str(prop::CONTENT)
                .unwrap_or("")
                .to_string();

            nodes[i] = Node::new(node::TEXT).prop(prop::CONTENT, current_content + &next_content);
            nodes.remove(i + 1);
            continue;
        }

        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::ResourceId;

    fn root_children(doc: &Document) -> &[Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_paragraph() {
        let result = parse("<p>Hello, world!</p>").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        let para = &children[0];
        assert_eq!(para.kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_heading() {
        let result = parse("<h1>Title</h1><h2>Subtitle</h2>").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(1));
        assert_eq!(children[1].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_emphasis() {
        let result = parse("<p><em>italic</em> and <strong>bold</strong></p>").unwrap();
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
        let result = parse(r#"<a href="https://example.com">link</a>"#).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        let link = &children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_list() {
        let result = parse("<ul><li>item 1</li><li>item 2</li></ul>").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(children[0].children.len(), 2);
    }

    #[test]
    fn test_parse_ordered_list() {
        let result = parse("<ol><li>first</li><li>second</li></ol>").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    fn test_parse_code_block() {
        let result = parse("<pre><code>fn main() {}</code></pre>").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(
            children[0].props.get_str(prop::CONTENT),
            Some("fn main() {}")
        );
    }

    #[test]
    fn test_parse_table() {
        let result =
            parse("<table><tr><th>Header</th></tr><tr><td>Cell</td></tr></table>").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children[0].kind.as_str(), node::TABLE);
    }

    #[test]
    fn test_parse_image() {
        let result = parse(r#"<img src="test.png" alt="Test image">"#).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        let img = &children[0];
        assert_eq!(img.kind.as_str(), node::IMAGE);
        assert_eq!(img.props.get_str(prop::URL), Some("test.png"));
        assert_eq!(img.props.get_str(prop::ALT), Some("Test image"));
    }

    #[test]
    #[cfg(feature = "html5ever")]
    fn test_parse_html_metadata() {
        let input = r#"<!DOCTYPE html>
<html>
<head>
    <title>My Page Title</title>
    <meta name="author" content="Jane Doe">
    <meta name="description" content="A test page">
    <meta name="keywords" content="test, html, metadata">
    <meta property="og:image" content="https://example.com/image.png">
</head>
<body>
    <h1>Hello</h1>
    <p>Content here.</p>
</body>
</html>"#;
        let result = parse(input).unwrap();
        let doc = result.value;

        // Check metadata was extracted
        assert_eq!(doc.metadata.get_str("title"), Some("My Page Title"));
        assert_eq!(doc.metadata.get_str("author"), Some("Jane Doe"));
        assert_eq!(doc.metadata.get_str("description"), Some("A test page"));
        assert_eq!(
            doc.metadata.get_str("keywords"),
            Some("test, html, metadata")
        );
        // Open Graph metadata (og: prefix stripped)
        assert_eq!(
            doc.metadata.get_str("image"),
            Some("https://example.com/image.png")
        );

        // Content should still be parsed
        let children = root_children(&doc);
        assert!(!children.is_empty());
    }

    #[test]
    #[cfg(feature = "html5ever")]
    fn test_parse_data_uri_image() {
        // A small 1x1 red PNG as base64
        let data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
        let input = format!(r#"<p><img src="{}" alt="red pixel"></p>"#, data_uri);

        let options = ParseOptions {
            embed_resources: true,
            ..Default::default()
        };
        let result = parse_with_options(&input, &options).unwrap();
        let doc = result.value;

        // Should have extracted the resource
        assert_eq!(doc.resources.len(), 1);

        // The image node should have a resource_id, not a URL
        let para = &doc.content.children[0];
        let img = &para.children[0];
        assert_eq!(img.kind.as_str(), node::IMAGE);
        assert!(img.props.get_str(prop::RESOURCE_ID).is_some());
        assert!(img.props.get_str(prop::URL).is_none());
        assert_eq!(img.props.get_str(prop::ALT), Some("red pixel"));

        // Resource should have correct mime type
        let resource_id = img.props.get_str(prop::RESOURCE_ID).unwrap();
        let id = ResourceId::from_string(resource_id);
        let resource = doc.resources.get(&id).unwrap();
        assert_eq!(resource.mime_type, "image/png");
    }

    #[test]
    #[cfg(feature = "html5ever")]
    fn test_data_uri_roundtrip() {
        // A small 1x1 red PNG as base64
        let original_data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
        let input = format!(r#"<img src="{}" alt="red pixel">"#, original_data_uri);

        // Parse with embed_resources enabled
        let options = ParseOptions {
            embed_resources: true,
            ..Default::default()
        };
        let result = parse_with_options(&input, &options).unwrap();
        let doc = result.value;

        // Emit back to HTML
        let output = rescribe_write_html::emit(&doc).unwrap();
        let html = String::from_utf8(output.value).unwrap();

        // Should contain a data URI
        assert!(html.contains("data:image/png;base64,"));
        assert!(html.contains("alt=\"red pixel\""));

        // The base64 data should roundtrip correctly
        assert!(html.contains("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="));
    }
}
