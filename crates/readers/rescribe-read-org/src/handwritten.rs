//! Handwritten Org-mode parser.

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Properties, Severity,
    Span, WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse Org-mode text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Org-mode with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input, options.preserve_source_info);
    let (children, metadata, warnings) = parser.parse_document();

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root).with_metadata(metadata);

    Ok(ConversionResult::with_warnings(doc, warnings))
}

/// Org-mode parser state.
struct Parser<'a> {
    input: &'a str,
    lines: Vec<&'a str>,
    line_offsets: Vec<usize>,
    pos: usize,
    warnings: Vec<FidelityWarning>,
    preserve_spans: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, preserve_spans: bool) -> Self {
        let lines: Vec<&str> = input.lines().collect();
        // Calculate byte offsets for each line
        let mut line_offsets = Vec::with_capacity(lines.len());
        let mut offset = 0;
        for line in &lines {
            line_offsets.push(offset);
            offset += line.len() + 1; // +1 for newline
        }
        Self {
            input,
            lines,
            line_offsets,
            pos: 0,
            warnings: Vec::new(),
            preserve_spans,
        }
    }

    /// Get the byte offset of the current line.
    fn current_offset(&self) -> usize {
        self.line_offsets
            .get(self.pos)
            .copied()
            .unwrap_or(self.input.len())
    }

    /// Get the byte offset of a specific line.
    fn line_offset(&self, line_idx: usize) -> usize {
        self.line_offsets
            .get(line_idx)
            .copied()
            .unwrap_or(self.input.len())
    }

    /// Create a span from start line to current line.
    fn make_span(&self, start_line: usize) -> Option<Span> {
        if self.preserve_spans {
            Some(Span {
                start: self.line_offset(start_line),
                end: self.current_offset(),
            })
        } else {
            None
        }
    }

    /// Add span to node if preserving spans.
    fn with_span(&self, mut node: Node, start_line: usize) -> Node {
        node.span = self.make_span(start_line);
        node
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.lines.len()
    }

    fn current_line(&self) -> Option<&'a str> {
        self.lines.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn parse_document(&mut self) -> (Vec<Node>, Properties, Vec<FidelityWarning>) {
        let mut children = Vec::new();
        let mut metadata = Properties::new();
        let mut current_para: Vec<String> = Vec::new();

        while !self.is_eof() {
            let line = self.current_line().unwrap();

            // Parse metadata (#+KEY: value)
            if line.starts_with("#+") && !line.to_uppercase().starts_with("#+BEGIN") {
                if let Some((key, value)) = self.parse_metadata_line(line) {
                    metadata.set(key, value);
                }
                self.advance();
                continue;
            }

            // Blank line - end paragraph
            if line.trim().is_empty() {
                if !current_para.is_empty() {
                    let content = current_para.join(" ");
                    children.push(
                        Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content)),
                    );
                    current_para.clear();
                }
                self.advance();
                continue;
            }

            // Heading
            if line.starts_with('*') && line.chars().find(|&c| c != '*') == Some(' ') {
                if !current_para.is_empty() {
                    let content = current_para.join(" ");
                    children.push(
                        Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content)),
                    );
                    current_para.clear();
                }
                children.push(self.parse_heading());
                continue;
            }

            // Block elements
            if line.to_uppercase().starts_with("#+BEGIN_") {
                if !current_para.is_empty() {
                    let content = current_para.join(" ");
                    children.push(
                        Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content)),
                    );
                    current_para.clear();
                }
                if let Some(block) = self.parse_block() {
                    children.push(block);
                }
                continue;
            }

            // List item
            if self.is_list_item(line) {
                if !current_para.is_empty() {
                    let content = current_para.join(" ");
                    children.push(
                        Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content)),
                    );
                    current_para.clear();
                }
                children.push(self.parse_list());
                continue;
            }

            // Horizontal rule
            if line.trim() == "-----" || (line.chars().all(|c| c == '-') && line.len() >= 5) {
                if !current_para.is_empty() {
                    let content = current_para.join(" ");
                    children.push(
                        Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content)),
                    );
                    current_para.clear();
                }
                children.push(Node::new(node::HORIZONTAL_RULE));
                self.advance();
                continue;
            }

            // Regular text - add to current paragraph
            current_para.push(line.to_string());
            self.advance();
        }

        // Flush remaining paragraph
        if !current_para.is_empty() {
            let content = current_para.join(" ");
            children.push(Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content)));
        }

        (children, metadata, std::mem::take(&mut self.warnings))
    }

    fn parse_metadata_line(&self, line: &str) -> Option<(String, String)> {
        let line = line.strip_prefix("#+")?.trim();
        let (key, value) = line.split_once(':')?;
        Some((key.trim().to_lowercase(), value.trim().to_string()))
    }

    fn parse_heading(&mut self) -> Node {
        let start_line = self.pos;
        let line = self.current_line().unwrap();
        let level = line.chars().take_while(|&c| c == '*').count();
        let text = &line[level..];
        let text = text.trim();

        // Remove TODO/DONE keywords and tags
        let text = self.strip_heading_metadata(text);

        self.advance();

        self.with_span(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level as i64)
                .children(self.parse_inline_content(&text)),
            start_line,
        )
    }

    fn strip_heading_metadata(&self, text: &str) -> String {
        let text = text.trim();

        // Remove TODO keywords
        let text = if text.starts_with("TODO ") || text.starts_with("DONE ") {
            &text[5..]
        } else {
            text
        };

        // Remove tags (like :tag1:tag2:)
        if let Some(idx) = text.rfind(" :")
            && text.ends_with(':')
        {
            return text[..idx].trim().to_string();
        }

        text.trim().to_string()
    }

    fn parse_block(&mut self) -> Option<Node> {
        let start_line = self.pos;
        let orig_line = self.current_line()?;
        let line_upper = orig_line.to_uppercase();
        let block_type = line_upper
            .strip_prefix("#+BEGIN_")?
            .split_whitespace()
            .next()?
            .to_uppercase();

        // Get language for SRC blocks
        let lang = if block_type == "SRC" {
            orig_line
                .to_uppercase()
                .strip_prefix("#+BEGIN_SRC")
                .and_then(|s| s.split_whitespace().next())
                .map(|s| s.to_lowercase())
        } else {
            None
        };

        self.advance();

        let end_marker = format!("#+END_{}", block_type);
        let mut content = Vec::new();

        while !self.is_eof() {
            let line = self.current_line().unwrap();
            if line.to_uppercase().starts_with(&end_marker) {
                self.advance();
                break;
            }
            content.push(line);
            self.advance();
        }

        let content_str = content.join("\n");

        match block_type.as_str() {
            "SRC" => {
                let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content_str);
                if let Some(l) = lang
                    && !l.is_empty()
                {
                    node = node.prop(prop::LANGUAGE, l);
                }
                Some(self.with_span(node, start_line))
            }
            "QUOTE" => {
                let children = self.parse_inline_content(&content_str);
                Some(
                    self.with_span(
                        Node::new(node::BLOCKQUOTE)
                            .children(vec![Node::new(node::PARAGRAPH).children(children)]),
                        start_line,
                    ),
                )
            }
            "EXAMPLE" | "VERSE" => Some(self.with_span(
                Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content_str),
                start_line,
            )),
            "CENTER" => {
                let children = self.parse_inline_content(&content_str);
                Some(self.with_span(Node::new(node::DIV).children(children), start_line))
            }
            _ => {
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(format!("org:{}", block_type)),
                    format!("Unknown block type: {}", block_type),
                ));
                None
            }
        }
    }

    fn is_list_item(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        // Unordered: - item, + item
        if trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
            return true;
        }
        // Ordered: 1. item, 1) item
        if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            return rest.starts_with(". ") || rest.starts_with(") ");
        }
        false
    }

    fn parse_list(&mut self) -> Node {
        let start_line = self.pos;
        let first_line = self.current_line().unwrap();
        let indent = first_line.len() - first_line.trim_start().len();
        let ordered = self.is_ordered_list_item(first_line);

        let mut items = Vec::new();

        while !self.is_eof() {
            let line = self.current_line().unwrap();
            let line_indent = line.len() - line.trim_start().len();

            // Check if still part of list
            if line.trim().is_empty() {
                // Blank line might end the list or be between items
                self.advance();
                if self.is_eof() {
                    break;
                }
                let next = self.current_line().unwrap();
                let next_indent = next.len() - next.trim_start().len();
                if !self.is_list_item(next) || next_indent < indent {
                    break;
                }
                continue;
            }

            if line_indent < indent && !line.trim().is_empty() {
                break;
            }

            if self.is_list_item(line) && line_indent == indent {
                items.push(self.parse_list_item(indent));
            } else {
                break;
            }
        }

        self.with_span(
            Node::new(node::LIST)
                .prop(prop::ORDERED, ordered)
                .children(items),
            start_line,
        )
    }

    fn is_ordered_list_item(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            rest.starts_with(". ") || rest.starts_with(") ")
        } else {
            false
        }
    }

    fn parse_list_item(&mut self, base_indent: usize) -> Node {
        let line = self.current_line().unwrap();
        let trimmed = line.trim_start();

        // Extract item content (skip marker)
        let content = if trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
            &trimmed[2..]
        } else {
            // Ordered list: skip "1. " or "1) "
            let idx = trimmed.find(['.', ')']).map(|i| i + 2).unwrap_or(0);
            if idx < trimmed.len() {
                &trimmed[idx..]
            } else {
                trimmed
            }
        };

        self.advance();

        // Collect continuation lines
        let mut full_content = content.to_string();
        while !self.is_eof() {
            let line = self.current_line().unwrap();
            if line.trim().is_empty() {
                break;
            }
            let line_indent = line.len() - line.trim_start().len();
            if line_indent <= base_indent && self.is_list_item(line) {
                break;
            }
            if line_indent > base_indent {
                full_content.push(' ');
                full_content.push_str(line.trim());
                self.advance();
            } else {
                break;
            }
        }

        let children = self.parse_inline_content(&full_content);
        Node::new(node::LIST_ITEM).children(children)
    }

    fn parse_inline_content(&self, text: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = text.chars().collect();

        while pos < chars.len() {
            let c = chars[pos];

            match c {
                // Bold: *text*
                '*' => {
                    if let Some((content, end)) = self.find_inline_span(&chars, pos, '*') {
                        nodes.push(
                            Node::new(node::STRONG).children(self.parse_inline_content(&content)),
                        );
                        pos = end + 1;
                        continue;
                    }
                }
                // Italic: /text/
                '/' => {
                    if let Some((content, end)) = self.find_inline_span(&chars, pos, '/') {
                        nodes.push(
                            Node::new(node::EMPHASIS).children(self.parse_inline_content(&content)),
                        );
                        pos = end + 1;
                        continue;
                    }
                }
                // Underline: _text_
                '_' => {
                    if let Some((content, end)) = self.find_inline_span(&chars, pos, '_') {
                        nodes.push(
                            Node::new(node::UNDERLINE)
                                .children(self.parse_inline_content(&content)),
                        );
                        pos = end + 1;
                        continue;
                    }
                }
                // Strikethrough: +text+
                '+' => {
                    if let Some((content, end)) = self.find_inline_span(&chars, pos, '+') {
                        nodes.push(
                            Node::new(node::STRIKEOUT)
                                .children(self.parse_inline_content(&content)),
                        );
                        pos = end + 1;
                        continue;
                    }
                }
                // Code: ~text~ or =text=
                '~' | '=' => {
                    if let Some((content, end)) = self.find_inline_span(&chars, pos, c) {
                        nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
                        pos = end + 1;
                        continue;
                    }
                }
                // Link: [[url]] or [[url][description]]
                '[' => {
                    if pos + 1 < chars.len()
                        && chars[pos + 1] == '['
                        && let Some((link_node, end)) = self.parse_link(&chars, pos)
                    {
                        nodes.push(link_node);
                        pos = end;
                        continue;
                    }
                }
                _ => {}
            }

            // Regular character - add to text node or create new one
            if let Some(Node { kind, props, .. }) = nodes.last_mut()
                && kind.as_str() == node::TEXT
                && let Some(existing) = props.get_str(prop::CONTENT)
            {
                let mut new_text = existing.to_string();
                new_text.push(c);
                props.set(prop::CONTENT, new_text);
                pos += 1;
                continue;
            }

            nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, c.to_string()));
            pos += 1;
        }

        // Merge adjacent text nodes
        merge_text_nodes(&mut nodes);

        nodes
    }

    fn find_inline_span(
        &self,
        chars: &[char],
        start: usize,
        marker: char,
    ) -> Option<(String, usize)> {
        if start + 2 >= chars.len() {
            return None;
        }

        // Opening marker must not be followed by whitespace
        if chars[start + 1].is_whitespace() {
            return None;
        }

        // Find closing marker
        for i in (start + 2)..chars.len() {
            if chars[i] == marker {
                // Closing marker must not be preceded by whitespace
                if !chars[i - 1].is_whitespace() {
                    let content: String = chars[(start + 1)..i].iter().collect();
                    return Some((content, i));
                }
            }
        }

        None
    }

    fn parse_link(&self, chars: &[char], start: usize) -> Option<(Node, usize)> {
        // Skip [[
        let mut pos = start + 2;
        let mut url = String::new();
        let mut description = String::new();
        let mut in_description = false;

        while pos < chars.len() {
            let c = chars[pos];
            if c == ']' {
                if pos + 1 < chars.len() && chars[pos + 1] == ']' {
                    // End of link
                    let children = if description.is_empty() {
                        vec![Node::new(node::TEXT).prop(prop::CONTENT, url.clone())]
                    } else {
                        self.parse_inline_content(&description)
                    };
                    return Some((
                        Node::new(node::LINK)
                            .prop(prop::URL, url)
                            .children(children),
                        pos + 2,
                    ));
                } else if pos + 1 < chars.len() && chars[pos + 1] == '[' {
                    // Start of description
                    in_description = true;
                    pos += 2;
                    continue;
                }
            }

            if in_description {
                description.push(c);
            } else {
                url.push(c);
            }
            pos += 1;
        }

        None
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
