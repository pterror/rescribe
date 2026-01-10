//! HTML parser using tree-sitter.
//!
//! This backend provides:
//! - Precise source spans (tree-sitter's core strength)
//! - Better error recovery for malformed input

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Properties, Resource,
    ResourceId, ResourceMap, Severity, Span, WarningKind,
};
use rescribe_std::{Node, node, prop};
use tree_sitter::Parser as TsParser;

use crate::{
    extract_text_content, get_code_language, is_block_element, merge_text_nodes, parse_data_uri,
};

/// Parse HTML text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse HTML with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = TsParser::new();
    let lang: tree_sitter::Language = tree_sitter_html::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|e| ParseError::Invalid(format!("Failed to load HTML grammar: {}", e)))?;

    let tree = parser
        .parse(input.as_bytes(), None)
        .ok_or_else(|| ParseError::Invalid("Failed to parse HTML".to_string()))?;

    let mut converter = Converter::new(input, options);
    let children = converter.convert_children(&tree.root_node());

    let root = Node::new(node::DOCUMENT).children(children);
    let mut doc = Document::new()
        .with_content(root)
        .with_metadata(converter.metadata);
    doc.resources = converter.resources;

    Ok(ConversionResult::with_warnings(doc, converter.warnings))
}

/// Converts tree-sitter nodes to rescribe nodes.
struct Converter<'a> {
    source: &'a str,
    options: &'a ParseOptions,
    metadata: Properties,
    resources: ResourceMap,
    warnings: Vec<FidelityWarning>,
}

impl<'a> Converter<'a> {
    fn new(source: &'a str, options: &'a ParseOptions) -> Self {
        Self {
            source,
            options,
            metadata: Properties::new(),
            resources: ResourceMap::new(),
            warnings: Vec::new(),
        }
    }

    fn node_text(&self, node: &tree_sitter::Node) -> &'a str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    fn make_span(&self, node: &tree_sitter::Node) -> Option<Span> {
        if self.options.preserve_source_info {
            Some(Span {
                start: node.start_byte(),
                end: node.end_byte(),
            })
        } else {
            None
        }
    }

    fn with_span(&self, mut rnode: Node, tsnode: &tree_sitter::Node) -> Node {
        rnode.span = self.make_span(tsnode);
        rnode
    }

    fn convert_children(&mut self, parent: &tree_sitter::Node) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut cursor = parent.walk();

        for child in parent.children(&mut cursor) {
            nodes.extend(self.convert_node(&child));
        }

        merge_text_nodes(&mut nodes);
        nodes
    }

    fn convert_node(&mut self, tsnode: &tree_sitter::Node) -> Vec<Node> {
        let kind = tsnode.kind();

        match kind {
            "document" | "fragment" => self.convert_children(tsnode),

            "doctype" | "comment" => vec![],

            "text" => {
                let text = self.node_text(tsnode);
                if text.trim().is_empty() {
                    return vec![];
                }
                vec![self.with_span(
                    Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()),
                    tsnode,
                )]
            }

            "element" | "script_element" | "style_element" => self.convert_element(tsnode),

            "self_closing_tag" => self.convert_self_closing(tsnode),

            "erroneous_end_tag" => vec![],

            _ => {
                // Try to extract children for unknown node types
                self.convert_children(tsnode)
            }
        }
    }

    fn convert_element(&mut self, tsnode: &tree_sitter::Node) -> Vec<Node> {
        // Find start tag to get element name and attributes
        let mut tag_name = String::new();
        let mut attrs = Vec::new();
        let mut content_children = Vec::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            match child.kind() {
                "start_tag" => {
                    self.parse_tag(&child, &mut tag_name, &mut attrs);
                }
                "end_tag" => {
                    // Ignore end tag
                }
                _ => {
                    content_children.extend(self.convert_node(&child));
                }
            }
        }

        self.create_element(&tag_name, &attrs, content_children, tsnode)
    }

    fn convert_self_closing(&mut self, tsnode: &tree_sitter::Node) -> Vec<Node> {
        let mut tag_name = String::new();
        let mut attrs = Vec::new();

        self.parse_tag(tsnode, &mut tag_name, &mut attrs);
        self.create_element(&tag_name, &attrs, Vec::new(), tsnode)
    }

    fn parse_tag(
        &self,
        tag_node: &tree_sitter::Node,
        tag_name: &mut String,
        attrs: &mut Vec<(String, String)>,
    ) {
        let mut cursor = tag_node.walk();
        for child in tag_node.children(&mut cursor) {
            match child.kind() {
                "tag_name" => {
                    *tag_name = self.node_text(&child).to_lowercase();
                }
                "attribute" => {
                    let mut attr_name = String::new();
                    let mut attr_value = String::new();

                    let mut attr_cursor = child.walk();
                    for attr_child in child.children(&mut attr_cursor) {
                        match attr_child.kind() {
                            "attribute_name" => {
                                attr_name = self.node_text(&attr_child).to_lowercase();
                            }
                            "attribute_value" | "quoted_attribute_value" => {
                                attr_value = self
                                    .node_text(&attr_child)
                                    .trim_matches('"')
                                    .trim_matches('\'')
                                    .to_string();
                            }
                            _ => {}
                        }
                    }

                    if !attr_name.is_empty() {
                        attrs.push((attr_name, attr_value));
                    }
                }
                _ => {}
            }
        }
    }

    fn get_attr<'b>(attrs: &'b [(String, String)], name: &str) -> Option<&'b str> {
        attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn create_element(
        &mut self,
        tag: &str,
        attrs: &[(String, String)],
        children: Vec<Node>,
        tsnode: &tree_sitter::Node,
    ) -> Vec<Node> {
        // Handle metadata elements
        match tag {
            "title" => {
                let title = extract_text_content(&children);
                if !title.is_empty() {
                    self.metadata.set("title", title);
                }
                return vec![];
            }
            "meta" => {
                if let Some(name) = Self::get_attr(attrs, "name")
                    && let Some(content) = Self::get_attr(attrs, "content")
                {
                    self.metadata.set(name, content.to_string());
                }
                if let Some(property) = Self::get_attr(attrs, "property")
                    && let Some(content) = Self::get_attr(attrs, "content")
                {
                    let key = property.strip_prefix("og:").unwrap_or(property);
                    self.metadata.set(key, content.to_string());
                }
                return vec![];
            }
            "head" | "script" | "style" | "link" => return vec![],
            "html" | "body" => return children,
            _ => {}
        }

        let node = match tag {
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
                if let Some(start) = Self::get_attr(attrs, "start")
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
                if let Some(colspan) = Self::get_attr(attrs, "colspan")
                    && let Ok(n) = colspan.parse::<i64>()
                {
                    cell = cell.prop(prop::COLSPAN, n);
                }
                if let Some(rowspan) = Self::get_attr(attrs, "rowspan")
                    && let Ok(n) = rowspan.parse::<i64>()
                {
                    cell = cell.prop(prop::ROWSPAN, n);
                }
                cell
            }
            "td" => {
                let mut cell = Node::new(node::TABLE_CELL).children(children);
                if let Some(colspan) = Self::get_attr(attrs, "colspan")
                    && let Ok(n) = colspan.parse::<i64>()
                {
                    cell = cell.prop(prop::COLSPAN, n);
                }
                if let Some(rowspan) = Self::get_attr(attrs, "rowspan")
                    && let Ok(n) = rowspan.parse::<i64>()
                {
                    cell = cell.prop(prop::ROWSPAN, n);
                }
                cell
            }

            "figure" => Node::new(node::FIGURE).children(children),
            "figcaption" => Node::new(node::CAPTION).children(children),

            "hr" => Node::new(node::HORIZONTAL_RULE),
            "br" => Node::new(node::LINE_BREAK),

            "div" | "section" | "article" | "main" | "aside" | "nav" | "header" | "footer" => {
                let mut div = Node::new(node::DIV).children(children);
                if let Some(id) = Self::get_attr(attrs, "id") {
                    div = div.prop(prop::ID, id.to_string());
                }
                if let Some(class) = Self::get_attr(attrs, "class") {
                    div = div.prop(prop::CLASSES, class.to_string());
                }
                div
            }

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
                if let Some(href) = Self::get_attr(attrs, "href") {
                    link = link.prop(prop::URL, href.to_string());
                }
                if let Some(title) = Self::get_attr(attrs, "title") {
                    link = link.prop(prop::TITLE, title.to_string());
                }
                link
            }

            "img" => {
                let mut img = Node::new(node::IMAGE);
                if let Some(src) = Self::get_attr(attrs, "src") {
                    if self.options.embed_resources {
                        if let Some((mime_type, data)) = parse_data_uri(src) {
                            let resource = Resource::new(mime_type, data);
                            let id = ResourceId::new();
                            self.resources.insert(id.clone(), resource);
                            img = img.prop(prop::RESOURCE_ID, id.as_str().to_string());
                        } else {
                            img = img.prop(prop::URL, src.to_string());
                        }
                    } else {
                        img = img.prop(prop::URL, src.to_string());
                    }
                }
                if let Some(alt) = Self::get_attr(attrs, "alt") {
                    img = img.prop(prop::ALT, alt.to_string());
                }
                if let Some(title) = Self::get_attr(attrs, "title") {
                    img = img.prop(prop::TITLE, title.to_string());
                }
                img
            }

            "span" => {
                let mut span = Node::new(node::SPAN).children(children);
                if let Some(id) = Self::get_attr(attrs, "id") {
                    span = span.prop(prop::ID, id.to_string());
                }
                if let Some(class) = Self::get_attr(attrs, "class") {
                    span = span.prop(prop::CLASSES, class.to_string());
                }
                span
            }

            "q" => Node::new(node::QUOTED)
                .prop(prop::QUOTE_TYPE, "double")
                .children(children),

            "small" => Node::new(node::SMALL_CAPS).children(children),

            _ => {
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(format!("html:{}", tag)),
                    format!("Unknown HTML element: {}", tag),
                ));

                if is_block_element(tag) {
                    Node::new(node::DIV).children(children)
                } else {
                    Node::new(node::SPAN).children(children)
                }
            }
        };

        vec![self.with_span(node, tsnode)]
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
        assert_eq!(children[0].kind.as_str(), node::PARAGRAPH);
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
    fn test_parse_with_spans() {
        let input = "<p>Hello</p>";
        let options = ParseOptions {
            preserve_source_info: true,
            ..Default::default()
        };
        let result = parse_with_options(input, &options).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        let para = &children[0];
        assert!(para.span.is_some());
    }
}
