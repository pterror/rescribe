//! Markdown parser using tree-sitter.
//!
//! This backend provides:
//! - Precise source spans (tree-sitter's core strength)
//! - Better error recovery for malformed input
//! - Incremental parsing capability (future)

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Severity, Span,
    WarningKind,
};
use rescribe_std::{Node, node, prop};
use std::collections::HashMap;
use tree_sitter::{Parser as TsParser, Tree};

/// Key for inline tree lookup: (start_byte, end_byte)
type InlineKey = (usize, usize);

/// Map of inline node positions to their parsed inline trees
type InlineTrees = HashMap<InlineKey, Tree>;

/// Find all inline nodes in block tree and parse them with inline grammar
fn parse_inline_nodes(
    block_tree: &Tree,
    source: &[u8],
    inline_parser: &mut TsParser,
) -> InlineTrees {
    let mut trees = HashMap::new();
    collect_inline_nodes(&block_tree.root_node(), source, inline_parser, &mut trees);
    trees
}

fn collect_inline_nodes(
    node: &tree_sitter::Node,
    source: &[u8],
    inline_parser: &mut TsParser,
    trees: &mut InlineTrees,
) {
    if node.kind() == "inline" {
        let start = node.start_byte();
        let end = node.end_byte();
        let inline_source = &source[start..end];
        if let Some(inline_tree) = inline_parser.parse(inline_source, None) {
            trees.insert((start, end), inline_tree);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_inline_nodes(&child, source, inline_parser, trees);
    }
}

/// Parse markdown text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse markdown with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    // tree-sitter-md requires trailing newline for proper parsing
    let normalized = if input.ends_with('\n') {
        std::borrow::Cow::Borrowed(input)
    } else {
        std::borrow::Cow::Owned(format!("{}\n", input))
    };

    // Parse block structure
    let mut block_parser = TsParser::new();
    let block_lang: tree_sitter::Language = tree_sitter_md::LANGUAGE.into();
    block_parser
        .set_language(&block_lang)
        .map_err(|e| ParseError::Invalid(format!("Failed to load block grammar: {}", e)))?;

    let block_tree = block_parser
        .parse(normalized.as_bytes(), None)
        .ok_or_else(|| ParseError::Invalid("Failed to parse markdown blocks".to_string()))?;

    // Parse inline content for each inline node
    let mut inline_parser = TsParser::new();
    let inline_lang: tree_sitter::Language = tree_sitter_md::INLINE_LANGUAGE.into();
    inline_parser
        .set_language(&inline_lang)
        .map_err(|e| ParseError::Invalid(format!("Failed to load inline grammar: {}", e)))?;

    let inline_trees = parse_inline_nodes(&block_tree, normalized.as_bytes(), &mut inline_parser);

    // Use normalized source for text extraction (byte offsets match the tree)
    // but original input length for span clamping
    let mut converter = Converter::new(
        normalized.as_ref(),
        options.preserve_source_info,
        input.len(),
        inline_trees,
    );
    let children = converter.convert_block_tree(&block_tree);

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root);

    Ok(ConversionResult::with_warnings(doc, converter.warnings))
}

/// Converts tree-sitter nodes to rescribe nodes.
struct Converter<'a> {
    source: &'a str,
    preserve_spans: bool,
    /// Original input length (before trailing newline normalization)
    original_len: usize,
    /// Parsed inline trees keyed by (start, end) byte positions
    inline_trees: InlineTrees,
    warnings: Vec<FidelityWarning>,
}

impl<'a> Converter<'a> {
    fn new(
        source: &'a str,
        preserve_spans: bool,
        original_len: usize,
        inline_trees: InlineTrees,
    ) -> Self {
        Self {
            source,
            preserve_spans,
            original_len,
            inline_trees,
            warnings: Vec::new(),
        }
    }

    /// Get the text content of a node.
    fn node_text(&self, node: &tree_sitter::Node) -> &'a str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Create a span from a tree-sitter node.
    /// Clamps end byte to original input length (handles added trailing newline).
    fn make_span(&self, node: &tree_sitter::Node) -> Option<Span> {
        if self.preserve_spans {
            Some(Span {
                start: node.start_byte().min(self.original_len),
                end: node.end_byte().min(self.original_len),
            })
        } else {
            None
        }
    }

    /// Add span to a rescribe node.
    fn with_span(&self, mut rnode: Node, tsnode: &tree_sitter::Node) -> Node {
        rnode.span = self.make_span(tsnode);
        rnode
    }

    /// Get text for an inline node. The inline tree has local byte offsets,
    /// so we add the base offset to get the position in the source.
    fn inline_text(&self, node: &tree_sitter::Node, base_offset: usize) -> &'a str {
        let start = base_offset + node.start_byte();
        let end = base_offset + node.end_byte();
        &self.source[start..end.min(self.source.len())]
    }

    /// Add span to a rescribe node for inline content.
    fn with_inline_span(
        &self,
        mut rnode: Node,
        tsnode: &tree_sitter::Node,
        base_offset: usize,
    ) -> Node {
        if self.preserve_spans {
            let start = (base_offset + tsnode.start_byte()).min(self.original_len);
            let end = (base_offset + tsnode.end_byte()).min(self.original_len);
            rnode.span = Some(Span { start, end });
        }
        rnode
    }

    fn convert_block_tree(&mut self, block_tree: &Tree) -> Vec<Node> {
        let root = block_tree.root_node();
        self.convert_block_children(&root)
    }

    fn convert_block_children(&mut self, parent: &tree_sitter::Node) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut cursor = parent.walk();

        for child in parent.children(&mut cursor) {
            if child.kind() == "section" {
                // Flatten sections - recursively get their children
                nodes.extend(self.convert_block_children(&child));
            } else if let Some(node) = self.convert_block_node(&child) {
                nodes.push(node);
            }
        }

        nodes
    }

    fn convert_block_node(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let kind = tsnode.kind();

        match kind {
            // Document structure
            "document" => {
                let children = self.convert_block_children(tsnode);
                Some(self.with_span(Node::new(node::DOCUMENT).children(children), tsnode))
            }

            // "section" is handled in convert_block_children by flattening

            // Block elements
            "atx_heading" => self.convert_heading(tsnode),
            "setext_heading" => self.convert_heading(tsnode),
            "paragraph" => self.convert_paragraph(tsnode),
            "fenced_code_block" => self.convert_fenced_code(tsnode),
            "indented_code_block" => self.convert_indented_code(tsnode),
            "block_quote" => self.convert_blockquote(tsnode),
            "list" => self.convert_list(tsnode),
            "list_item" => self.convert_list_item(tsnode),
            "thematic_break" => Some(self.with_span(Node::new(node::HORIZONTAL_RULE), tsnode)),
            "html_block" => self.convert_html_block(tsnode),

            // Inline node in block tree - get inline tree and process
            "inline" => {
                let children = self.convert_inline_from_block(tsnode);
                if children.len() == 1 {
                    return Some(children.into_iter().next().unwrap());
                }
                None
            }

            // Table elements
            "pipe_table" => self.convert_table(tsnode),

            // Skip certain nodes
            "link_destination" | "link_title" | "code_fence_content" | "info_string" => None,

            // Unknown - warn and skip
            _ => {
                // Only warn for named nodes, not anonymous punctuation
                if !kind.starts_with('_') && tsnode.is_named() {
                    self.warnings.push(FidelityWarning::new(
                        Severity::Minor,
                        WarningKind::UnsupportedNode(format!("md:{}", kind)),
                        format!("Unknown markdown node type: {}", kind),
                    ));
                }
                None
            }
        }
    }

    /// Get inline content from a block node by looking up its inline tree
    fn convert_inline_from_block(&mut self, inline_block_node: &tree_sitter::Node) -> Vec<Node> {
        let key = (inline_block_node.start_byte(), inline_block_node.end_byte());
        let offset = inline_block_node.start_byte();

        // Check if we have an inline tree for this node
        let has_tree = self.inline_trees.contains_key(&key);

        if has_tree {
            // Get the tree and process inline content
            let inline_tree = self.inline_trees.get(&key).unwrap();
            let root = inline_tree.root_node();

            let mut nodes = Vec::new();

            // Check if root has children - if so, process them
            // But also extract text from gaps between children, since
            // plain text is not represented as nodes in the inline grammar
            if root.child_count() > 0 {
                let mut current_pos = 0usize;
                let root_end = root.end_byte();

                let mut cursor = root.walk();
                for child in root.children(&mut cursor) {
                    let child_start = child.start_byte();
                    let child_end = child.end_byte();

                    // Extract text before this child (gap between current_pos and child_start)
                    if child_start > current_pos {
                        let gap_text = &self.source[offset + current_pos..offset + child_start];
                        if !gap_text.is_empty() {
                            nodes.push(
                                Node::new(node::TEXT).prop(prop::CONTENT, gap_text.to_string()),
                            );
                        }
                    }

                    // Process the child node
                    if let Some(n) = self.process_inline_node(&child, offset) {
                        nodes.push(n);
                    }

                    current_pos = child_end;
                }

                // Extract text after the last child
                if current_pos < root_end {
                    let gap_text = &self.source[offset + current_pos..offset + root_end];
                    if !gap_text.is_empty() {
                        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, gap_text.to_string()));
                    }
                }
            } else {
                // Root has no children - extract text directly from the root node
                // This happens for simple text content like "Hello World"
                let text = self.inline_text(&root, offset).to_string();
                if !text.is_empty() {
                    nodes.push(self.with_inline_span(
                        Node::new(node::TEXT).prop(prop::CONTENT, text),
                        &root,
                        offset,
                    ));
                }
            }

            merge_text_nodes(&mut nodes);
            nodes
        } else {
            // Fallback: treat as plain text
            let text = self.node_text(inline_block_node).to_string();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![Node::new(node::TEXT).prop(prop::CONTENT, text)]
            }
        }
    }

    /// Process a single inline node - wrapper to handle borrowing
    fn process_inline_node(&self, tsnode: &tree_sitter::Node, offset: usize) -> Option<Node> {
        // This is a non-mutable version for initial pass
        let kind = tsnode.kind();

        match kind {
            "text" | "soft_line_break" => {
                let text = self.inline_text(tsnode, offset);
                let text = if kind == "soft_line_break" {
                    " ".to_string()
                } else {
                    text.to_string()
                };
                if text.is_empty() {
                    return None;
                }
                Some(self.with_inline_span(
                    Node::new(node::TEXT).prop(prop::CONTENT, text),
                    tsnode,
                    offset,
                ))
            }

            "hard_line_break" => {
                Some(self.with_inline_span(Node::new(node::LINE_BREAK), tsnode, offset))
            }

            "emphasis" => {
                let children = self.process_inline_children(tsnode, offset);
                Some(self.with_inline_span(
                    Node::new(node::EMPHASIS).children(children),
                    tsnode,
                    offset,
                ))
            }

            "strong_emphasis" => {
                let children = self.process_inline_children(tsnode, offset);
                Some(self.with_inline_span(
                    Node::new(node::STRONG).children(children),
                    tsnode,
                    offset,
                ))
            }

            "strikethrough" => {
                let children = self.process_inline_children(tsnode, offset);
                Some(self.with_inline_span(
                    Node::new(node::STRIKEOUT).children(children),
                    tsnode,
                    offset,
                ))
            }

            "code_span" => {
                let text = self.inline_text(tsnode, offset);
                let content = text
                    .trim_start_matches('`')
                    .trim_end_matches('`')
                    .to_string();
                Some(self.with_inline_span(
                    Node::new(node::CODE).prop(prop::CONTENT, content),
                    tsnode,
                    offset,
                ))
            }

            "inline_link"
            | "full_reference_link"
            | "collapsed_reference_link"
            | "shortcut_link" => self.process_link(tsnode, offset),

            "image" => self.process_image(tsnode, offset),

            "html_tag" => {
                let content = self.inline_text(tsnode, offset).to_string();
                Some(
                    self.with_inline_span(
                        Node::new(node::RAW_INLINE)
                            .prop(prop::FORMAT, "html")
                            .prop(prop::CONTENT, content),
                        tsnode,
                        offset,
                    ),
                )
            }

            "backslash_escape" => {
                let text = self.inline_text(tsnode, offset);
                let escaped = text.strip_prefix('\\').unwrap_or(text);
                Some(self.with_inline_span(
                    Node::new(node::TEXT).prop(prop::CONTENT, escaped.to_string()),
                    tsnode,
                    offset,
                ))
            }

            "emphasis_delimiter" | "link_text" | "link_destination" | "link_title"
            | "image_description" => None,

            _ => {
                if tsnode.child_count() == 0 {
                    let text = self.inline_text(tsnode, offset).to_string();
                    if !text.is_empty() {
                        return Some(self.with_inline_span(
                            Node::new(node::TEXT).prop(prop::CONTENT, text),
                            tsnode,
                            offset,
                        ));
                    }
                }
                let children = self.process_inline_children(tsnode, offset);
                if children.len() == 1 {
                    return Some(children.into_iter().next().unwrap());
                }
                None
            }
        }
    }

    fn process_inline_children(&self, parent: &tree_sitter::Node, offset: usize) -> Vec<Node> {
        let mut nodes = Vec::new();

        if parent.child_count() == 0 {
            // No children - extract text directly
            let text = self.inline_text(parent, offset).to_string();
            if !text.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, text));
            }
            return nodes;
        }

        // Extract text from gaps between children
        // Positions are absolute within the inline tree (0-indexed from inline content start)
        let mut current_pos = parent.start_byte();
        let parent_end = parent.end_byte();

        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            let child_start = child.start_byte();
            let child_end = child.end_byte();

            // Extract text before this child (gap between current_pos and child_start)
            if child_start > current_pos {
                let gap_text = &self.source[offset + current_pos..offset + child_start];
                if !gap_text.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, gap_text.to_string()));
                }
            }

            // Process the child node
            if let Some(n) = self.process_inline_node(&child, offset) {
                nodes.push(n);
            }

            current_pos = child_end;
        }

        // Extract text after the last child
        if current_pos < parent_end {
            let gap_text = &self.source[offset + current_pos..offset + parent_end];
            if !gap_text.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, gap_text.to_string()));
            }
        }

        nodes
    }

    fn process_link(&self, tsnode: &tree_sitter::Node, offset: usize) -> Option<Node> {
        let mut url = String::new();
        let mut title = String::new();
        let mut children = Vec::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            match child.kind() {
                "link_text" => {
                    children = self.process_inline_children(&child, offset);
                }
                "link_destination" => {
                    url = self
                        .inline_text(&child, offset)
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_string();
                }
                "link_title" => {
                    let t = self.inline_text(&child, offset);
                    title = t
                        .trim_start_matches(['"', '\'', '('])
                        .trim_end_matches(['"', '\'', ')'])
                        .to_string();
                }
                _ => {}
            }
        }

        let mut link = Node::new(node::LINK)
            .prop(prop::URL, url)
            .children(children);
        if !title.is_empty() {
            link = link.prop(prop::TITLE, title);
        }
        Some(self.with_inline_span(link, tsnode, offset))
    }

    fn process_image(&self, tsnode: &tree_sitter::Node, offset: usize) -> Option<Node> {
        let mut url = String::new();
        let mut alt = String::new();
        let mut title = String::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            match child.kind() {
                "image_description" | "link_text" => {
                    alt = self
                        .inline_text(&child, offset)
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .to_string();
                }
                "link_destination" => {
                    url = self
                        .inline_text(&child, offset)
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_string();
                }
                "link_title" => {
                    let t = self.inline_text(&child, offset);
                    title = t
                        .trim_start_matches(['"', '\'', '('])
                        .trim_end_matches(['"', '\'', ')'])
                        .to_string();
                }
                _ => {}
            }
        }

        let mut img = Node::new(node::IMAGE)
            .prop(prop::URL, url)
            .prop(prop::ALT, alt);
        if !title.is_empty() {
            img = img.prop(prop::TITLE, title);
        }
        Some(self.with_inline_span(img, tsnode, offset))
    }

    fn convert_heading(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let mut level = 1i64;
        let mut content_nodes = Vec::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            match child.kind() {
                "atx_h1_marker" => level = 1,
                "atx_h2_marker" => level = 2,
                "atx_h3_marker" => level = 3,
                "atx_h4_marker" => level = 4,
                "atx_h5_marker" => level = 5,
                "atx_h6_marker" => level = 6,
                "heading_content" | "inline" => {
                    content_nodes.extend(self.convert_inline_from_block(&child));
                }
                "setext_h1_underline" => level = 1,
                "setext_h2_underline" => level = 2,
                "paragraph" => {
                    // Setext heading content is in a paragraph
                    let mut para_cursor = child.walk();
                    for para_child in child.children(&mut para_cursor) {
                        if para_child.kind() == "inline" {
                            content_nodes.extend(self.convert_inline_from_block(&para_child));
                        }
                    }
                }
                _ => {}
            }
        }

        Some(
            self.with_span(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level)
                    .children(content_nodes),
                tsnode,
            ),
        )
    }

    fn convert_paragraph(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let mut children = Vec::new();
        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            if child.kind() == "inline" {
                children.extend(self.convert_inline_from_block(&child));
            }
        }
        if children.is_empty() {
            return None;
        }
        Some(self.with_span(Node::new(node::PARAGRAPH).children(children), tsnode))
    }

    fn convert_fenced_code(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let mut language = String::new();
        let mut content = String::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            match child.kind() {
                "info_string" | "language" => {
                    language = self.node_text(&child).trim().to_string();
                }
                "code_fence_content" => {
                    content = self.node_text(&child).to_string();
                }
                _ => {}
            }
        }

        // Trim trailing newline from content
        if content.ends_with('\n') {
            content.pop();
        }

        let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
        if !language.is_empty() {
            node = node.prop(prop::LANGUAGE, language);
        }

        Some(self.with_span(node, tsnode))
    }

    fn convert_indented_code(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        // For indented code blocks, extract the text and remove 4-space indent
        let text = self.node_text(tsnode);
        let content: String = text
            .lines()
            .map(|line| line.strip_prefix("    ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");

        Some(self.with_span(
            Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content),
            tsnode,
        ))
    }

    fn convert_blockquote(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let children = self.convert_block_children(tsnode);
        Some(self.with_span(Node::new(node::BLOCKQUOTE).children(children), tsnode))
    }

    fn convert_list(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        // Determine if ordered by checking first list item marker
        let mut ordered = false;
        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            if child.kind() == "list_item" {
                // Check for list_marker_dot or list_marker_paren
                let mut item_cursor = child.walk();
                if let Some(item_child) = child.children(&mut item_cursor).next() {
                    let marker_kind = item_child.kind();
                    if marker_kind.contains("dot") || marker_kind.contains("paren") {
                        // Check if it starts with a digit
                        let marker_text = self.node_text(&item_child);
                        if marker_text
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit())
                        {
                            ordered = true;
                        }
                    }
                }
                break;
            }
        }

        let children = self.convert_block_children(tsnode);
        Some(
            self.with_span(
                Node::new(node::LIST)
                    .prop(prop::ORDERED, ordered)
                    .children(children),
                tsnode,
            ),
        )
    }

    fn convert_list_item(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let children = self.convert_block_children(tsnode);
        Some(self.with_span(Node::new(node::LIST_ITEM).children(children), tsnode))
    }

    fn convert_html_block(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let content = self.node_text(tsnode).to_string();
        Some(
            self.with_span(
                Node::new(node::RAW_BLOCK)
                    .prop(prop::FORMAT, "html")
                    .prop(prop::CONTENT, content),
                tsnode,
            ),
        )
    }

    fn convert_table(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let mut rows = Vec::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            match child.kind() {
                "pipe_table_header" => {
                    if let Some(row) = self.convert_table_row(&child) {
                        rows.push(row);
                    }
                }
                "pipe_table_row" => {
                    if let Some(row) = self.convert_table_row(&child) {
                        rows.push(row);
                    }
                }
                "pipe_table_delimiter_row" => {
                    // Skip delimiter row
                }
                _ => {}
            }
        }

        Some(self.with_span(Node::new(node::TABLE).children(rows), tsnode))
    }

    fn convert_table_row(&mut self, tsnode: &tree_sitter::Node) -> Option<Node> {
        let mut cells = Vec::new();

        let mut cursor = tsnode.walk();
        for child in tsnode.children(&mut cursor) {
            if child.kind() == "pipe_table_cell" {
                // Table cells have inline content
                let content = self.convert_inline_from_block(&child);
                cells.push(self.with_span(Node::new(node::TABLE_CELL).children(content), &child));
            }
        }

        Some(self.with_span(Node::new(node::TABLE_ROW).children(cells), tsnode))
    }
}

/// Merge adjacent text nodes.
fn merge_text_nodes(nodes: &mut Vec<Node>) {
    let mut i = 0;
    while i + 1 < nodes.len() {
        if nodes[i].kind.as_str() == node::TEXT && nodes[i + 1].kind.as_str() == node::TEXT {
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
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_children(doc: &Document) -> &[Node] {
        &doc.content.children
    }

    #[test]
    fn test_parse_heading() {
        let result = parse("# Hello").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        let heading = &children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
        assert_eq!(heading.props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_paragraph() {
        let result = parse("Hello, world!").unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert!(!children.is_empty());
        assert_eq!(children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_spans_preserved() {
        let input = "# Hello";
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
        assert_eq!(span.end, 7);
    }
}
