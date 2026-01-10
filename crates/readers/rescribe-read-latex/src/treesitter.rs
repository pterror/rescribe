//! LaTeX parser using tree-sitter.
//!
//! This backend provides:
//! - Precise source spans (tree-sitter's core strength)
//! - Better error recovery for malformed input

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Severity, Span,
    WarningKind,
};
use rescribe_std::{Node, node, prop};
use tree_sitter::Parser as TsParser;

/// Parse LaTeX text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse LaTeX with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = TsParser::new();
    let lang: tree_sitter::Language = tree_sitter_latex::LANGUAGE.into();
    parser
        .set_language(&lang)
        .map_err(|e| ParseError::Invalid(format!("Failed to load LaTeX grammar: {}", e)))?;

    let tree = parser
        .parse(input.as_bytes(), None)
        .ok_or_else(|| ParseError::Invalid("Failed to parse LaTeX".to_string()))?;

    let mut converter = Converter::new(input, options);
    let children = converter.convert_document(&tree.root_node());

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root);

    Ok(ConversionResult::with_warnings(doc, converter.warnings))
}

/// Converts tree-sitter nodes to rescribe nodes.
struct Converter<'a> {
    source: &'a str,
    options: &'a ParseOptions,
    warnings: Vec<FidelityWarning>,
}

impl<'a> Converter<'a> {
    fn new(source: &'a str, options: &'a ParseOptions) -> Self {
        Self {
            source,
            options,
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

    fn convert_document(&mut self, root: &tree_sitter::Node) -> Vec<Node> {
        let mut children = Vec::new();
        let mut current_para = Vec::new();

        // Find document body (content after \begin{document})
        let mut in_document = false;
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            let kind = child.kind();

            // Check for document environment
            if kind == "generic_environment"
                && let Some(begin) = self.find_child_by_kind(&child, "begin")
                && let Some(name) = self.find_child_by_kind(&begin, "curly_group")
            {
                let env_name = self.node_text(&name).trim_matches(['{', '}']);
                if env_name == "document" {
                    in_document = true;
                    // Process document body
                    children.extend(self.convert_children(&child));
                    continue;
                }
            }

            // If no \begin{document}, treat all content as body
            if !in_document && let Some(nodes) = self.convert_node(&child) {
                for n in nodes {
                    if is_block_node(&n) {
                        if !current_para.is_empty() {
                            children.push(
                                Node::new(node::PARAGRAPH)
                                    .children(std::mem::take(&mut current_para)),
                            );
                        }
                        children.push(n);
                    } else {
                        current_para.push(n);
                    }
                }
            }
        }

        // Flush remaining paragraph
        if !current_para.is_empty() {
            children.push(Node::new(node::PARAGRAPH).children(current_para));
        }

        children
    }

    fn find_child_by_kind<'b>(
        &self,
        node: &'b tree_sitter::Node<'b>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'b>> {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|child| child.kind() == kind)
    }

    fn convert_children(&mut self, parent: &tree_sitter::Node) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current_para = Vec::new();
        let mut cursor = parent.walk();

        for child in parent.children(&mut cursor) {
            if child.kind() == "begin" || child.kind() == "end" {
                continue;
            }

            if let Some(converted) = self.convert_node(&child) {
                for n in converted {
                    if is_block_node(&n) {
                        if !current_para.is_empty() {
                            nodes.push(
                                Node::new(node::PARAGRAPH)
                                    .children(std::mem::take(&mut current_para)),
                            );
                        }
                        nodes.push(n);
                    } else {
                        current_para.push(n);
                    }
                }
            }
        }

        if !current_para.is_empty() {
            nodes.push(Node::new(node::PARAGRAPH).children(current_para));
        }

        nodes
    }

    fn convert_node(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let kind = tsnode.kind();

        match kind {
            "text" | "word" => {
                let text = self.node_text(tsnode);
                if text.trim().is_empty() {
                    return None;
                }
                Some(vec![self.with_span(
                    Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()),
                    tsnode,
                )])
            }

            // Section commands (codebook-tree-sitter-latex specific)
            "section" => self.convert_section_node(tsnode, 1),
            "subsection" => self.convert_section_node(tsnode, 2),
            "subsubsection" => self.convert_section_node(tsnode, 3),
            "paragraph" => self.convert_section_node(tsnode, 4),

            "command" | "generic_command" => self.convert_command(tsnode),

            "generic_environment" | "environment" => self.convert_environment(tsnode),

            "inline_formula" | "math_environment" => {
                let text = self.node_text(tsnode);
                let content = text.trim_start_matches('$').trim_end_matches('$');
                Some(vec![self.with_span(
                    Node::new("math_inline").prop("math:source", content.to_string()),
                    tsnode,
                )])
            }

            "displayed_equation" => {
                let text = self.node_text(tsnode);
                let content = text
                    .trim_start_matches("$$")
                    .trim_end_matches("$$")
                    .trim_start_matches("\\[")
                    .trim_end_matches("\\]");
                Some(vec![self.with_span(
                    Node::new("math_display").prop("math:source", content.to_string()),
                    tsnode,
                )])
            }

            "curly_group" | "curly_group_text" | "curly_group_text_list" => {
                // Process contents of braces
                let children = self.convert_children(tsnode);
                if children.is_empty() {
                    None
                } else {
                    Some(
                        children
                            .into_iter()
                            .flat_map(|n| {
                                if n.kind.as_str() == node::PARAGRAPH {
                                    n.children
                                } else {
                                    vec![n]
                                }
                            })
                            .collect(),
                    )
                }
            }

            "comment" => None,

            "ERROR" => {
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode("latex:ERROR".to_string()),
                    "Parse error in LaTeX".to_string(),
                ));
                None
            }

            _ => {
                // Try to convert children for unknown nodes
                let children: Vec<_> = self.convert_children(tsnode);
                if children.is_empty() {
                    None
                } else {
                    Some(children)
                }
            }
        }
    }

    fn convert_command(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let text = self.node_text(tsnode);

        // Get command name from first child
        let cmd_name = {
            let mut cursor = tsnode.walk();
            tsnode
                .children(&mut cursor)
                .find(|c| c.kind() == "command_name")
                .map(|n| self.node_text(&n))
                .unwrap_or(text)
        };

        let cmd = cmd_name.trim_start_matches('\\');

        match cmd {
            "section" => self.convert_section(tsnode, 1),
            "subsection" => self.convert_section(tsnode, 2),
            "subsubsection" => self.convert_section(tsnode, 3),
            "paragraph" => self.convert_section(tsnode, 4),

            "textbf" | "bf" => self.convert_inline_command(tsnode, node::STRONG),
            "textit" | "it" | "emph" => self.convert_inline_command(tsnode, node::EMPHASIS),
            "texttt" => self.convert_code_command(tsnode),
            "underline" => self.convert_inline_command(tsnode, node::UNDERLINE),
            "sout" | "st" => self.convert_inline_command(tsnode, node::STRIKEOUT),
            "textsc" => self.convert_inline_command(tsnode, node::SMALL_CAPS),
            "textsuperscript" | "textsup" => self.convert_inline_command(tsnode, node::SUPERSCRIPT),
            "textsubscript" | "textsub" => self.convert_inline_command(tsnode, node::SUBSCRIPT),

            "href" => self.convert_href(tsnode),
            "url" => self.convert_url(tsnode),

            "includegraphics" => self.convert_includegraphics(tsnode),

            "item" => None, // Handled in list processing

            "hrule" | "rule" => Some(vec![
                self.with_span(Node::new(node::HORIZONTAL_RULE), tsnode),
            ]),

            "\\" => Some(vec![Node::new(node::LINE_BREAK)]),

            _ => {
                // Check for escaped characters
                if cmd.len() == 1 && "&%$_#{}".contains(cmd) {
                    return Some(vec![
                        Node::new(node::TEXT).prop(prop::CONTENT, cmd.to_string()),
                    ]);
                }
                None
            }
        }
    }

    fn convert_section(&mut self, tsnode: &tree_sitter::Node, level: i64) -> Option<Vec<Node>> {
        let children = self.get_command_content(tsnode);
        Some(vec![
            self.with_span(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level)
                    .children(children),
                tsnode,
            ),
        ])
    }

    fn convert_section_node(
        &mut self,
        tsnode: &tree_sitter::Node,
        level: i64,
    ) -> Option<Vec<Node>> {
        // For codebook-tree-sitter-latex section nodes
        let mut children = Vec::new();

        // Find curly_group which contains the section title
        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            if child.kind() == "curly_group" {
                children = self
                    .convert_children(&child)
                    .into_iter()
                    .flat_map(|n| {
                        if n.kind.as_str() == node::PARAGRAPH {
                            n.children
                        } else {
                            vec![n]
                        }
                    })
                    .collect();
                break;
            }
        }

        Some(vec![
            self.with_span(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level)
                    .children(children),
                tsnode,
            ),
        ])
    }

    fn convert_inline_command(
        &mut self,
        tsnode: &tree_sitter::Node,
        kind: &str,
    ) -> Option<Vec<Node>> {
        let children = self.get_command_content(tsnode);
        Some(vec![
            self.with_span(Node::new(kind).children(children), tsnode),
        ])
    }

    fn convert_code_command(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let content = self.get_command_text(tsnode);
        Some(vec![self.with_span(
            Node::new(node::CODE).prop(prop::CONTENT, content),
            tsnode,
        )])
    }

    fn convert_href(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let mut cursor = tsnode.walk();
        let mut url = String::new();
        let mut children = Vec::new();

        let groups: Vec<_> = tsnode
            .children(&mut cursor)
            .filter(|c| c.kind().starts_with("curly_group"))
            .collect();

        if let Some(url_group) = groups.first() {
            url = self
                .node_text(url_group)
                .trim_matches(|c| c == '{' || c == '}')
                .to_string();
        }
        if let Some(text_group) = groups.get(1) {
            children = self
                .convert_children(text_group)
                .into_iter()
                .flat_map(|n| {
                    if n.kind.as_str() == node::PARAGRAPH {
                        n.children
                    } else {
                        vec![n]
                    }
                })
                .collect();
        }

        Some(vec![
            self.with_span(
                Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .children(children),
                tsnode,
            ),
        ])
    }

    fn convert_url(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let url = self.get_command_text(tsnode);
        Some(vec![
            self.with_span(
                Node::new(node::LINK)
                    .prop(prop::URL, url.clone())
                    .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, url)]),
                tsnode,
            ),
        ])
    }

    fn convert_includegraphics(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let path = self.get_command_text(tsnode);
        Some(vec![self.with_span(
            Node::new(node::IMAGE).prop(prop::URL, path),
            tsnode,
        )])
    }

    fn get_command_content(&mut self, tsnode: &tree_sitter::Node) -> Vec<Node> {
        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            if child.kind().starts_with("curly_group") {
                return self
                    .convert_children(&child)
                    .into_iter()
                    .flat_map(|n| {
                        if n.kind.as_str() == node::PARAGRAPH {
                            n.children
                        } else {
                            vec![n]
                        }
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    fn get_command_text(&self, tsnode: &tree_sitter::Node) -> String {
        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            if child.kind().starts_with("curly_group") {
                return self
                    .node_text(&child)
                    .trim_matches(|c| c == '{' || c == '}')
                    .to_string();
            }
        }
        String::new()
    }

    fn convert_environment(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        // Get environment name
        let env_name = {
            if let Some(begin) = self.find_child_by_kind(tsnode, "begin") {
                if let Some(name_group) = self.find_child_by_kind(&begin, "curly_group") {
                    self.node_text(&name_group)
                        .trim_matches(|c| c == '{' || c == '}')
                        .to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        };

        match env_name.as_str() {
            "itemize" => self.convert_list(tsnode, false),
            "enumerate" => self.convert_list(tsnode, true),
            "verbatim" | "lstlisting" => self.convert_verbatim(tsnode),
            "quote" | "quotation" => self.convert_blockquote(tsnode),
            "equation" | "equation*" | "align" | "align*" => self.convert_math_env(tsnode),
            "figure" => self.convert_figure(tsnode),
            "tabular" => self.convert_tabular(tsnode),
            "document" => Some(self.convert_children(tsnode)),
            _ => {
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(format!("latex:{}", env_name)),
                    format!("Unknown environment: {}", env_name),
                ));
                None
            }
        }
    }

    fn convert_list(&mut self, tsnode: &tree_sitter::Node, ordered: bool) -> Option<Vec<Node>> {
        let mut items = Vec::new();
        let mut cursor = tsnode.walk();

        for child in tsnode.children(&mut cursor) {
            if child.kind() == "enum_item" {
                let content = self.convert_children(&child);
                items.push(self.with_span(Node::new(node::LIST_ITEM).children(content), &child));
            }
        }

        Some(vec![
            self.with_span(
                Node::new(node::LIST)
                    .prop(prop::ORDERED, ordered)
                    .children(items),
                tsnode,
            ),
        ])
    }

    fn convert_verbatim(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        // Get raw text content between \begin{...} and \end{...}
        let text = self.node_text(tsnode);

        // Extract content between begin and end
        let content = if let Some(start) = text.find('}') {
            if let Some(end) = text.rfind("\\end") {
                text[start + 1..end].trim_matches('\n').to_string()
            } else {
                text[start + 1..].to_string()
            }
        } else {
            text.to_string()
        };

        Some(vec![self.with_span(
            Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content),
            tsnode,
        )])
    }

    fn convert_blockquote(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let children = self.convert_children(tsnode);
        Some(vec![self.with_span(
            Node::new(node::BLOCKQUOTE).children(children),
            tsnode,
        )])
    }

    fn convert_math_env(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let text = self.node_text(tsnode);

        // Extract content between begin and end
        let content = if let Some(start) = text.find('}') {
            if let Some(end) = text.rfind("\\end") {
                text[start + 1..end].trim().to_string()
            } else {
                text[start + 1..].to_string()
            }
        } else {
            text.to_string()
        };

        Some(vec![self.with_span(
            Node::new("math_display").prop("math:source", content),
            tsnode,
        )])
    }

    fn convert_figure(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        let mut children = Vec::new();
        let mut cursor = tsnode.walk();

        for child in tsnode.children(&mut cursor) {
            if let Some(nodes) = self.convert_node(&child) {
                children.extend(nodes);
            }
        }

        Some(vec![self.with_span(
            Node::new(node::FIGURE).children(children),
            tsnode,
        )])
    }

    fn convert_tabular(&mut self, tsnode: &tree_sitter::Node) -> Option<Vec<Node>> {
        // Simplified tabular conversion - just extract content
        let children = self.convert_children(tsnode);
        Some(vec![self.with_span(
            Node::new(node::TABLE).children(children),
            tsnode,
        )])
    }
}

fn is_block_node(n: &Node) -> bool {
    matches!(
        n.kind.as_str(),
        node::HEADING
            | node::PARAGRAPH
            | node::LIST
            | node::CODE_BLOCK
            | node::BLOCKQUOTE
            | node::FIGURE
            | node::TABLE
            | "math_display"
            | node::HORIZONTAL_RULE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_children(doc: &Document) -> &[Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_section() {
        let input = "\\section{Hello World}";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_with_spans() {
        let input = "\\section{Title}";
        let options = ParseOptions {
            preserve_source_info: true,
            ..Default::default()
        };
        let result = parse_with_options(input, &options).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        let section = &children[0];
        assert!(section.span.is_some());
    }
}
