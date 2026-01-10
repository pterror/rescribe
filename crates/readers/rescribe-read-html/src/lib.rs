//! HTML reader for rescribe.
//!
//! Parses HTML5 into rescribe's document IR using html5ever.

use html5ever::tendril::TendrilSink;
use html5ever::{Attribute, QualName, parse_document};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Properties, Severity,
    WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse HTML text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse HTML with custom options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut warnings = Vec::new();
    let mut metadata = Properties::new();

    // Parse HTML using html5ever
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut input.as_bytes())
        .map_err(|e| ParseError::Invalid(format!("HTML parse error: {:?}", e)))?;

    // Extract metadata from <head>
    extract_metadata(&dom.document, &mut metadata);

    // Convert DOM to rescribe nodes
    let children = convert_children(&dom.document, &mut warnings);

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root).with_metadata(metadata);

    Ok(ConversionResult::with_warnings(doc, warnings))
}

/// Extract metadata from HTML head element.
fn extract_metadata(handle: &Handle, metadata: &mut Properties) {
    // Recursively search for head element and extract metadata
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag = name.local.as_ref();

        match tag {
            "title" => {
                // Extract title text content
                let title = extract_element_text(handle);
                if !title.is_empty() {
                    metadata.set("title", title);
                }
            }
            "meta" => {
                let attrs = attrs.borrow();
                // Handle <meta name="..." content="...">
                if let Some(name) = get_attr(&attrs, "name")
                    && let Some(content) = get_attr(&attrs, "content")
                {
                    metadata.set(&name, content);
                }
                // Handle <meta property="..." content="..."> (Open Graph)
                if let Some(property) = get_attr(&attrs, "property")
                    && let Some(content) = get_attr(&attrs, "content")
                {
                    // Normalize og: prefix
                    let key = property.strip_prefix("og:").unwrap_or(&property);
                    metadata.set(key, content);
                }
            }
            _ => {}
        }
    }

    // Recurse into children
    for child in handle.children.borrow().iter() {
        extract_metadata(child, metadata);
    }
}

/// Extract text content from an element.
fn extract_element_text(handle: &Handle) -> String {
    let mut text = String::new();
    for child in handle.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            text.push_str(&contents.borrow());
        }
        text.push_str(&extract_element_text(child));
    }
    text
}

/// Convert child nodes of a DOM node.
fn convert_children(handle: &Handle, warnings: &mut Vec<FidelityWarning>) -> Vec<Node> {
    let mut nodes = Vec::new();

    for child in handle.children.borrow().iter() {
        nodes.extend(convert_node(child, warnings));
    }

    // Flatten and merge adjacent text nodes, remove empty paragraphs
    merge_text_nodes(&mut nodes);

    nodes
}

/// Convert a single DOM node to rescribe Node(s).
/// Returns a Vec because some elements (html/body) may expand to multiple nodes.
fn convert_node(handle: &Handle, warnings: &mut Vec<FidelityWarning>) -> Vec<Node> {
    match &handle.data {
        NodeData::Document => {
            // Document node - just return children
            let children = convert_children(handle, warnings);
            vec![Node::new(node::DOCUMENT).children(children)]
        }

        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            // Skip whitespace-only text nodes between elements
            if text.trim().is_empty() {
                return vec![];
            }
            vec![Node::new(node::TEXT).prop(prop::CONTENT, text)]
        }

        NodeData::Element { name, attrs, .. } => {
            let attrs_borrowed = attrs.borrow();
            convert_element(name, &attrs_borrowed, handle, warnings)
        }

        NodeData::Comment { .. } => {
            // Skip comments
            vec![]
        }

        NodeData::Doctype { .. } => {
            // Skip doctype
            vec![]
        }

        NodeData::ProcessingInstruction { .. } => {
            // Skip processing instructions
            vec![]
        }
    }
}

/// Convert an HTML element to a rescribe Node.
/// Returns a Vec because html/body can return multiple nodes.
fn convert_element(
    name: &QualName,
    attrs: &[Attribute],
    handle: &Handle,
    warnings: &mut Vec<FidelityWarning>,
) -> Vec<Node> {
    let tag = name.local.as_ref();
    let children = convert_children(handle, warnings);

    let node = match tag {
        // Block elements
        "html" | "body" => {
            // Transparent wrapper - return children directly
            return children;
        }

        "head" | "script" | "style" | "meta" | "link" | "title" => {
            // Skip these elements
            return vec![];
        }

        "p" => Node::new(node::PARAGRAPH).children(children),

        "h1" => Node::new(node::HEADING)
            .prop(prop::LEVEL, 1i64)
            .children(children),
        "h2" => Node::new(node::HEADING)
            .prop(prop::LEVEL, 2i64)
            .children(children),
        "h3" => Node::new(node::HEADING)
            .prop(prop::LEVEL, 3i64)
            .children(children),
        "h4" => Node::new(node::HEADING)
            .prop(prop::LEVEL, 4i64)
            .children(children),
        "h5" => Node::new(node::HEADING)
            .prop(prop::LEVEL, 5i64)
            .children(children),
        "h6" => Node::new(node::HEADING)
            .prop(prop::LEVEL, 6i64)
            .children(children),

        "pre" => {
            // Extract text content for code blocks
            let content = extract_text_content(&children);
            let lang = get_code_language(&children);
            let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
            if let Some(l) = lang {
                node = node.prop(prop::LANGUAGE, l);
            }
            node
        }

        "blockquote" => Node::new(node::BLOCKQUOTE).children(children),

        "ul" => Node::new(node::LIST)
            .prop(prop::ORDERED, false)
            .children(children),

        "ol" => {
            let mut list = Node::new(node::LIST).prop(prop::ORDERED, true);
            if let Some(start) = get_attr(attrs, "start")
                && let Ok(n) = start.parse::<i64>()
            {
                list = list.prop(prop::START, n);
            }
            list.children(children)
        }

        "li" => Node::new(node::LIST_ITEM).children(children),

        "dl" => Node::new(node::DEFINITION_LIST).children(children),
        "dt" => Node::new(node::DEFINITION_TERM).children(children),
        "dd" => Node::new(node::DEFINITION_DESC).children(children),

        "table" => Node::new(node::TABLE).children(children),
        "thead" => Node::new(node::TABLE_HEAD).children(children),
        "tbody" => Node::new(node::TABLE_BODY).children(children),
        "tfoot" => Node::new(node::TABLE_FOOT).children(children),
        "tr" => Node::new(node::TABLE_ROW).children(children),
        "th" => {
            let mut cell = Node::new(node::TABLE_HEADER).children(children);
            if let Some(colspan) = get_attr(attrs, "colspan")
                && let Ok(n) = colspan.parse::<i64>()
            {
                cell = cell.prop(prop::COLSPAN, n);
            }
            if let Some(rowspan) = get_attr(attrs, "rowspan")
                && let Ok(n) = rowspan.parse::<i64>()
            {
                cell = cell.prop(prop::ROWSPAN, n);
            }
            cell
        }
        "td" => {
            let mut cell = Node::new(node::TABLE_CELL).children(children);
            if let Some(colspan) = get_attr(attrs, "colspan")
                && let Ok(n) = colspan.parse::<i64>()
            {
                cell = cell.prop(prop::COLSPAN, n);
            }
            if let Some(rowspan) = get_attr(attrs, "rowspan")
                && let Ok(n) = rowspan.parse::<i64>()
            {
                cell = cell.prop(prop::ROWSPAN, n);
            }
            cell
        }

        "figure" => Node::new(node::FIGURE).children(children),
        "figcaption" => Node::new(node::CAPTION).children(children),

        "hr" => Node::new(node::HORIZONTAL_RULE),

        "div" | "section" | "article" | "main" | "aside" | "nav" | "header" | "footer" => {
            let mut div = Node::new(node::DIV).children(children);
            if let Some(id) = get_attr(attrs, "id") {
                div = div.prop(prop::ID, id);
            }
            if let Some(class) = get_attr(attrs, "class") {
                div = div.prop(prop::CLASSES, class);
            }
            div
        }

        // Inline elements
        "em" | "i" => Node::new(node::EMPHASIS).children(children),

        "strong" | "b" => Node::new(node::STRONG).children(children),

        "s" | "strike" | "del" => Node::new(node::STRIKEOUT).children(children),

        "u" | "ins" => Node::new(node::UNDERLINE).children(children),

        "sub" => Node::new(node::SUBSCRIPT).children(children),

        "sup" => Node::new(node::SUPERSCRIPT).children(children),

        "code" => {
            let content = extract_text_content(&children);
            Node::new(node::CODE).prop(prop::CONTENT, content)
        }

        "a" => {
            let mut link = Node::new(node::LINK).children(children);
            if let Some(href) = get_attr(attrs, "href") {
                link = link.prop(prop::URL, href);
            }
            if let Some(title) = get_attr(attrs, "title") {
                link = link.prop(prop::TITLE, title);
            }
            link
        }

        "img" => {
            let mut img = Node::new(node::IMAGE);
            if let Some(src) = get_attr(attrs, "src") {
                img = img.prop(prop::URL, src);
            }
            if let Some(alt) = get_attr(attrs, "alt") {
                img = img.prop(prop::ALT, alt);
            }
            if let Some(title) = get_attr(attrs, "title") {
                img = img.prop(prop::TITLE, title);
            }
            img
        }

        "br" => Node::new(node::LINE_BREAK),

        "span" => {
            let mut span = Node::new(node::SPAN).children(children);
            if let Some(id) = get_attr(attrs, "id") {
                span = span.prop(prop::ID, id);
            }
            if let Some(class) = get_attr(attrs, "class") {
                span = span.prop(prop::CLASSES, class);
            }
            span
        }

        "q" => Node::new(node::QUOTED)
            .prop(prop::QUOTE_TYPE, "double")
            .children(children),

        "small" => Node::new(node::SMALL_CAPS).children(children),

        // Unknown elements - wrap in div/span
        _ => {
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::UnsupportedNode(format!("html:{}", tag)),
                format!("Unknown HTML element: {}", tag),
            ));

            // Guess block vs inline based on common patterns
            if is_block_element(tag) {
                Node::new(node::DIV).children(children)
            } else {
                Node::new(node::SPAN).children(children)
            }
        }
    };

    vec![node]
}

/// Get an attribute value by name.
fn get_attr(attrs: &[Attribute], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name.local.as_ref() == name)
        .map(|a| a.value.to_string())
}

/// Extract text content from a list of nodes.
fn extract_text_content(nodes: &[Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        // Check for content property first (text nodes, code nodes, etc.)
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            text.push_str(content);
        }
        // Also recursively extract from children
        text.push_str(&extract_text_content(&node.children));
    }
    text
}

/// Try to get the language from a code element inside pre.
fn get_code_language(children: &[Node]) -> Option<String> {
    for child in children {
        if child.kind.as_str() == node::CODE
            && let Some(classes) = child.props.get_str(prop::CLASSES)
        {
            // Look for language-xxx or xxx class
            for class in classes.split_whitespace() {
                if let Some(lang) = class.strip_prefix("language-") {
                    return Some(lang.to_string());
                }
            }
        }
    }
    None
}

/// Check if an element is a block-level element.
fn is_block_element(tag: &str) -> bool {
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
fn merge_text_nodes(nodes: &mut Vec<Node>) {
    if nodes.is_empty() {
        return;
    }

    let mut i = 0;
    while i < nodes.len() {
        // Recursively process children
        merge_text_nodes(&mut nodes[i].children);

        // Remove empty text nodes
        if nodes[i].kind.as_str() == node::TEXT
            && let Some(content) = nodes[i].props.get_str(prop::CONTENT)
            && content.is_empty()
        {
            nodes.remove(i);
            continue;
        }

        // Merge adjacent text nodes
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
}
