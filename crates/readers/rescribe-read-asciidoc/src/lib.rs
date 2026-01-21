//! AsciiDoc reader for rescribe.
//!
//! Parses AsciiDoc source into rescribe's document IR.
//! Uses a handwritten parser for core AsciiDoc features.

#![allow(clippy::collapsible_if)]

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Severity, Span,
    WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse AsciiDoc text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse AsciiDoc with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input, options.preserve_source_info);
    let (children, warnings) = parser.parse_document();

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root);

    Ok(ConversionResult::with_warnings(doc, warnings))
}

/// AsciiDoc parser state.
struct Parser<'a> {
    input: &'a str,
    lines: Vec<&'a str>,
    line_idx: usize,
    warnings: Vec<FidelityWarning>,
    preserve_spans: bool,
    /// Document attributes
    attributes: std::collections::HashMap<String, String>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, preserve_spans: bool) -> Self {
        let lines: Vec<&str> = input.lines().collect();
        Self {
            input,
            lines,
            line_idx: 0,
            warnings: Vec::new(),
            preserve_spans,
            attributes: std::collections::HashMap::new(),
        }
    }

    /// Create a span for the given line range.
    fn make_span(&self, start_line: usize, end_line: usize) -> Option<Span> {
        if self.preserve_spans {
            let start = self.line_offset(start_line);
            let end = self.line_offset(end_line);
            Some(Span { start, end })
        } else {
            None
        }
    }

    /// Get byte offset of line start.
    fn line_offset(&self, line_idx: usize) -> usize {
        let mut offset = 0;
        for (i, line) in self.lines.iter().enumerate() {
            if i >= line_idx {
                break;
            }
            offset += line.len() + 1; // +1 for newline
        }
        offset.min(self.input.len())
    }

    fn current_line(&self) -> Option<&'a str> {
        self.lines.get(self.line_idx).copied()
    }

    fn advance_line(&mut self) {
        self.line_idx += 1;
    }

    fn is_eof(&self) -> bool {
        self.line_idx >= self.lines.len()
    }

    fn is_blank_line(&self) -> bool {
        self.current_line()
            .map(|l| l.trim().is_empty())
            .unwrap_or(true)
    }

    fn skip_blank_lines(&mut self) {
        while !self.is_eof() && self.is_blank_line() {
            self.advance_line();
        }
    }

    fn parse_document(&mut self) -> (Vec<Node>, Vec<FidelityWarning>) {
        let mut children = Vec::new();

        while !self.is_eof() {
            self.skip_blank_lines();
            if self.is_eof() {
                break;
            }

            if let Some(block) = self.try_parse_block() {
                children.push(block);
            } else {
                // Fallback: skip line to prevent infinite loop
                self.advance_line();
            }
        }

        (children, std::mem::take(&mut self.warnings))
    }

    fn try_parse_block(&mut self) -> Option<Node> {
        let line = self.current_line()?;

        // Document attributes (:attr: value)
        if line.starts_with(':') && line.len() > 1 {
            if let Some((attr, value)) = self.try_parse_attribute(line) {
                self.attributes.insert(attr, value);
                self.advance_line();
                return self.try_parse_block();
            }
        }

        // Section titles (= Title, == Title, etc.)
        if line.starts_with('=') {
            if let Some(heading) = self.try_parse_heading() {
                return Some(heading);
            }
        }

        // Block attributes/admonitions ([NOTE], [source,lang], etc.)
        if line.starts_with('[') && line.ends_with(']') {
            return self.parse_block_with_attributes();
        }

        // Delimited blocks
        if self.is_delimiter_line(line) {
            return self.parse_delimited_block();
        }

        // Lists
        if let Some(list) = self.try_parse_list() {
            return Some(list);
        }

        // Horizontal rule
        if line == "'''" || line == "---" || line == "***" {
            self.advance_line();
            return Some(Node::new(node::HORIZONTAL_RULE));
        }

        // Page break
        if line == "<<<" {
            self.advance_line();
            return Some(Node::new(node::DIV).prop("class", "page-break".to_string()));
        }

        // Block image (image::path[alt])
        if line.starts_with("image::") {
            return self.parse_block_image();
        }

        // Regular paragraph
        self.parse_paragraph()
    }

    fn try_parse_attribute(&self, line: &str) -> Option<(String, String)> {
        if !line.starts_with(':') {
            return None;
        }

        let rest = &line[1..];
        // Find the closing : and ensure there's content after
        if let Some(colon_idx) = rest.find(':') {
            let attr_name = rest[..colon_idx].trim();
            let value = rest[colon_idx + 1..].trim();

            // Attribute unset (:!attr:)
            if let Some(stripped) = attr_name.strip_prefix('!') {
                return Some((stripped.to_string(), String::new()));
            }

            return Some((attr_name.to_string(), value.to_string()));
        }

        None
    }

    fn try_parse_heading(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // Count leading = characters
        let level = line.chars().take_while(|&c| c == '=').count();

        // Must have at least one = and be followed by space
        if level == 0 || level > 6 {
            return None;
        }

        let rest = &line[level..];
        if !rest.starts_with(' ') && !rest.is_empty() {
            return None;
        }

        let title = rest.trim();
        self.advance_line();

        let children = self.parse_inline_content(title);
        let mut node = Node::new(node::HEADING)
            .prop(prop::LEVEL, level as i64)
            .children(children);
        node.span = self.make_span(start_line, self.line_idx);
        Some(node)
    }

    fn parse_block_with_attributes(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // Parse attributes inside [...]
        let attrs_content = &line[1..line.len() - 1];
        let attrs: Vec<&str> = attrs_content.split(',').map(|s| s.trim()).collect();

        self.advance_line();

        // Check for admonition blocks
        let first_attr = attrs.first().map(|s| s.to_uppercase());

        match first_attr.as_deref() {
            Some("NOTE") | Some("TIP") | Some("WARNING") | Some("IMPORTANT") | Some("CAUTION") => {
                // Admonition block
                let admonition_type = first_attr.unwrap().to_lowercase();
                let content = self.collect_paragraph_content();
                let children = self.parse_inline_content(&content);
                let mut node = Node::new(node::DIV)
                    .prop("class", format!("admonition {}", admonition_type))
                    .children(vec![Node::new(node::PARAGRAPH).children(children)]);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            Some("SOURCE") => {
                // Source code block
                let language = attrs.get(1).map(|s| s.to_string());

                // Expect delimiter
                self.skip_blank_lines();
                if let Some(delim_line) = self.current_line() {
                    if delim_line.starts_with("----") {
                        let content = self.collect_delimited_content("----");
                        let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
                        if let Some(lang) = language {
                            node = node.prop(prop::LANGUAGE, lang);
                        }
                        node.span = self.make_span(start_line, self.line_idx);
                        return Some(node);
                    }
                }

                // Fallback: parse as regular content
                let content = self.collect_paragraph_content();
                let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
                if let Some(lang) = language {
                    node = node.prop(prop::LANGUAGE, lang);
                }
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            Some("QUOTE") | Some("VERSE") => {
                // Quote or verse block
                self.skip_blank_lines();
                let content = self.collect_delimited_content_or_paragraph();
                let children = self.parse_inline_content(&content);
                let attribution = attrs.get(1).map(|s| s.to_string());
                let mut bq = Node::new(node::BLOCKQUOTE)
                    .children(vec![Node::new(node::PARAGRAPH).children(children)]);
                if let Some(attr) = attribution {
                    bq = bq.prop("attribution", attr);
                }
                bq.span = self.make_span(start_line, self.line_idx);
                Some(bq)
            }
            Some("EXAMPLE") => {
                self.skip_blank_lines();
                let content = self.collect_delimited_content_or_paragraph();
                let children = self.parse_inline_content(&content);
                let mut node = Node::new(node::DIV)
                    .prop("class", "example".to_string())
                    .children(vec![Node::new(node::PARAGRAPH).children(children)]);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            Some("SIDEBAR") => {
                self.skip_blank_lines();
                let content = self.collect_delimited_content_or_paragraph();
                let children = self.parse_inline_content(&content);
                let mut node = Node::new(node::DIV)
                    .prop("class", "sidebar".to_string())
                    .children(vec![Node::new(node::PARAGRAPH).children(children)]);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            _ => {
                // Unknown attribute - apply to next block and parse
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(format!("asciidoc:{}", attrs_content)),
                    format!("Unknown block attribute: [{}]", attrs_content),
                ));
                // Try parsing next element
                self.try_parse_block()
            }
        }
    }

    fn is_delimiter_line(&self, line: &str) -> bool {
        if line.len() < 4 {
            return false;
        }

        let patterns = ["----", "====", "****", "____", "++++", "...."];

        for pattern in patterns {
            if line.starts_with(pattern)
                && line.chars().all(|c| c == pattern.chars().next().unwrap())
            {
                return true;
            }
        }

        false
    }

    fn parse_delimited_block(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;
        let delimiter = line.to_string();
        let delim_char = delimiter.chars().next()?;

        self.advance_line();

        // Collect content until matching delimiter
        let mut content_lines = Vec::new();
        while !self.is_eof() {
            let content_line = self.current_line().unwrap_or("");
            if content_line == delimiter {
                self.advance_line();
                break;
            }
            content_lines.push(content_line);
            self.advance_line();
        }

        let content = content_lines.join("\n");

        let node = match delim_char {
            '-' => {
                // Listing block (code)
                Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
            }
            '=' => {
                // Example block
                let children = self.parse_inline_content(&content);
                Node::new(node::DIV)
                    .prop("class", "example".to_string())
                    .children(vec![Node::new(node::PARAGRAPH).children(children)])
            }
            '*' => {
                // Sidebar block
                let children = self.parse_inline_content(&content);
                Node::new(node::DIV)
                    .prop("class", "sidebar".to_string())
                    .children(vec![Node::new(node::PARAGRAPH).children(children)])
            }
            '_' => {
                // Quote block
                let children = self.parse_inline_content(&content);
                Node::new(node::BLOCKQUOTE)
                    .children(vec![Node::new(node::PARAGRAPH).children(children)])
            }
            '+' => {
                // Passthrough block (raw)
                Node::new(node::RAW_BLOCK)
                    .prop(prop::CONTENT, content)
                    .prop("format", "asciidoc".to_string())
            }
            '.' => {
                // Literal block
                Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
            }
            _ => {
                // Unknown delimiter
                let children = self.parse_inline_content(&content);
                Node::new(node::DIV).children(vec![Node::new(node::PARAGRAPH).children(children)])
            }
        };

        let mut node = node;
        node.span = self.make_span(start_line, self.line_idx);
        Some(node)
    }

    fn collect_delimited_content(&mut self, delimiter: &str) -> String {
        self.advance_line(); // Skip opening delimiter

        let mut lines = Vec::new();
        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");
            if line.starts_with(delimiter)
                && line.chars().all(|c| c == delimiter.chars().next().unwrap())
            {
                self.advance_line();
                break;
            }
            lines.push(line);
            self.advance_line();
        }

        lines.join("\n")
    }

    fn collect_delimited_content_or_paragraph(&mut self) -> String {
        if let Some(line) = self.current_line() {
            if self.is_delimiter_line(line) {
                let delimiter = line.to_string();
                return self.collect_delimited_content(&delimiter);
            }
        }

        self.collect_paragraph_content()
    }

    fn collect_paragraph_content(&mut self) -> String {
        let mut content = String::new();

        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");

            if line.trim().is_empty() {
                break;
            }

            // Check for start of new block
            if line.starts_with('=')
                || line.starts_with('[')
                || self.is_delimiter_line(line)
                || line.starts_with("image::")
            {
                break;
            }

            if !content.is_empty() {
                content.push(' ');
            }
            content.push_str(line.trim());
            self.advance_line();
        }

        content
    }

    fn try_parse_list(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;
        let trimmed = line.trim_start();

        // Unordered list: *, **, ***, -, --
        if let Some(rest) = trimmed.strip_prefix("* ") {
            return Some(self.parse_unordered_list("* ", rest, start_line));
        }
        if let Some(rest) = trimmed.strip_prefix("** ") {
            return Some(self.parse_unordered_list("** ", rest, start_line));
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            return Some(self.parse_unordered_list("- ", rest, start_line));
        }

        // Ordered list: ., .., 1., a., i.
        if let Some(rest) = trimmed.strip_prefix(". ") {
            return Some(self.parse_ordered_list(". ", rest, start_line));
        }
        if let Some(rest) = trimmed.strip_prefix(".. ") {
            return Some(self.parse_ordered_list(".. ", rest, start_line));
        }

        // Numbered: 1. 2. etc.
        if let Some(idx) = trimmed.find(". ") {
            let prefix = &trimmed[..idx];
            if prefix.chars().all(|c| c.is_ascii_digit()) {
                let rest = &trimmed[idx + 2..];
                return Some(self.parse_ordered_list_numbered(rest, start_line));
            }
        }

        // Description list: term:: definition (but not image::)
        if trimmed.contains("::") && !trimmed.starts_with("::") && !trimmed.starts_with("image::") {
            return self.parse_description_list(start_line);
        }

        None
    }

    fn parse_unordered_list(
        &mut self,
        marker: &str,
        first_content: &str,
        start_line: usize,
    ) -> Node {
        let mut items = Vec::new();

        // First item
        let first_children = self.parse_inline_content(first_content);
        items.push(Node::new(node::LIST_ITEM).children(first_children));
        self.advance_line();

        // Subsequent items
        while !self.is_eof() {
            self.skip_blank_lines();
            let Some(line) = self.current_line() else {
                break;
            };
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix(marker) {
                let children = self.parse_inline_content(rest);
                items.push(Node::new(node::LIST_ITEM).children(children));
                self.advance_line();
            } else {
                // Check for continuation
                if line.starts_with(' ') || line.starts_with('\t') {
                    // Continuation - append to last item
                    if let Some(last_item) = items.last_mut() {
                        let continuation = self.parse_inline_content(trimmed);
                        last_item.children.extend(continuation);
                    }
                    self.advance_line();
                } else {
                    break;
                }
            }
        }

        let mut node = Node::new(node::LIST)
            .prop(prop::ORDERED, false)
            .children(items);
        node.span = self.make_span(start_line, self.line_idx);
        node
    }

    fn parse_ordered_list(&mut self, marker: &str, first_content: &str, start_line: usize) -> Node {
        let mut items = Vec::new();

        // First item
        let first_children = self.parse_inline_content(first_content);
        items.push(Node::new(node::LIST_ITEM).children(first_children));
        self.advance_line();

        // Subsequent items
        while !self.is_eof() {
            self.skip_blank_lines();
            let Some(line) = self.current_line() else {
                break;
            };
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix(marker) {
                let children = self.parse_inline_content(rest);
                items.push(Node::new(node::LIST_ITEM).children(children));
                self.advance_line();
            } else {
                break;
            }
        }

        let mut node = Node::new(node::LIST)
            .prop(prop::ORDERED, true)
            .children(items);
        node.span = self.make_span(start_line, self.line_idx);
        node
    }

    fn parse_ordered_list_numbered(&mut self, first_content: &str, start_line: usize) -> Node {
        let mut items = Vec::new();

        // First item
        let first_children = self.parse_inline_content(first_content);
        items.push(Node::new(node::LIST_ITEM).children(first_children));
        self.advance_line();

        // Subsequent items
        while !self.is_eof() {
            self.skip_blank_lines();
            let Some(line) = self.current_line() else {
                break;
            };
            let trimmed = line.trim_start();

            if let Some(idx) = trimmed.find(". ") {
                let prefix = &trimmed[..idx];
                if prefix.chars().all(|c| c.is_ascii_digit()) {
                    let rest = &trimmed[idx + 2..];
                    let children = self.parse_inline_content(rest);
                    items.push(Node::new(node::LIST_ITEM).children(children));
                    self.advance_line();
                    continue;
                }
            }
            break;
        }

        let mut node = Node::new(node::LIST)
            .prop(prop::ORDERED, true)
            .children(items);
        node.span = self.make_span(start_line, self.line_idx);
        node
    }

    fn parse_description_list(&mut self, start_line: usize) -> Option<Node> {
        let mut items = Vec::new();

        while !self.is_eof() {
            let Some(line) = self.current_line() else {
                break;
            };

            if line.trim().is_empty() {
                self.advance_line();
                continue;
            }

            // Find :: separator
            if let Some(sep_idx) = line.find("::") {
                let term = line[..sep_idx].trim();
                let def_start = &line[sep_idx + 2..].trim();

                let term_children = self.parse_inline_content(term);
                items.push(Node::new(node::DEFINITION_TERM).children(term_children));

                self.advance_line();

                // Definition may continue on next lines if indented
                let mut def_content = def_start.to_string();
                while !self.is_eof() {
                    let def_line = self.current_line().unwrap_or("");
                    if def_line.trim().is_empty() {
                        break;
                    }
                    if def_line.starts_with(' ') || def_line.starts_with('\t') {
                        if !def_content.is_empty() {
                            def_content.push(' ');
                        }
                        def_content.push_str(def_line.trim());
                        self.advance_line();
                    } else if def_line.contains("::") && !def_line.starts_with("::") {
                        // Another term
                        break;
                    } else {
                        break;
                    }
                }

                let def_children = self.parse_inline_content(&def_content);
                items.push(Node::new(node::DEFINITION_DESC).children(def_children));
            } else {
                break;
            }
        }

        if items.is_empty() {
            return None;
        }

        let mut node = Node::new(node::DEFINITION_LIST).children(items);
        node.span = self.make_span(start_line, self.line_idx);
        Some(node)
    }

    fn parse_block_image(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // image::path[alt,width,height]
        let rest = line.strip_prefix("image::")?;

        let (path, attrs) = if let Some(bracket_start) = rest.find('[') {
            let path = &rest[..bracket_start];
            let attrs_str = &rest[bracket_start + 1..];
            let attrs_end = attrs_str.find(']').unwrap_or(attrs_str.len());
            (path.to_string(), attrs_str[..attrs_end].to_string())
        } else {
            (rest.to_string(), String::new())
        };

        self.advance_line();

        let attrs_parts: Vec<&str> = attrs.split(',').map(|s| s.trim()).collect();
        let alt = attrs_parts.first().map(|s| s.to_string());
        let width = attrs_parts.get(1).map(|s| s.to_string());
        let height = attrs_parts.get(2).map(|s| s.to_string());

        let mut node = Node::new(node::IMAGE).prop(prop::URL, path);
        if let Some(alt_text) = alt {
            if !alt_text.is_empty() {
                node = node.prop(prop::ALT, alt_text);
            }
        }
        if let Some(w) = width {
            if !w.is_empty() {
                node = node.prop("width", w);
            }
        }
        if let Some(h) = height {
            if !h.is_empty() {
                node = node.prop("height", h);
            }
        }

        // Wrap in figure for block images
        let mut figure = Node::new(node::FIGURE).children(vec![node]);
        figure.span = self.make_span(start_line, self.line_idx);
        Some(figure)
    }

    fn parse_paragraph(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let content = self.collect_paragraph_content();

        if content.is_empty() {
            return None;
        }

        let children = self.parse_inline_content(&content);
        let mut node = Node::new(node::PARAGRAPH).children(children);
        node.span = self.make_span(start_line, self.line_idx);
        Some(node)
    }

    fn parse_inline_content(&self, content: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = content.chars().collect();

        while pos < chars.len() {
            // Strong: *text* or **text**
            if chars[pos] == '*' {
                // Check for **text** first
                if pos + 1 < chars.len() && chars[pos + 1] == '*' {
                    if let Some((end, text)) = self.find_double_closing(&chars, pos + 2, '*') {
                        nodes.push(
                            Node::new(node::STRONG).children(self.parse_inline_content(&text)),
                        );
                        pos = end + 2;
                        continue;
                    }
                }
                // Single *text*
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '*') {
                    nodes.push(Node::new(node::STRONG).children(self.parse_inline_content(&text)));
                    pos = end + 1;
                    continue;
                }
            }

            // Emphasis: _text_ or __text__
            if chars[pos] == '_' {
                // Check for __text__ first
                if pos + 1 < chars.len() && chars[pos + 1] == '_' {
                    if let Some((end, text)) = self.find_double_closing(&chars, pos + 2, '_') {
                        nodes.push(
                            Node::new(node::EMPHASIS).children(self.parse_inline_content(&text)),
                        );
                        pos = end + 2;
                        continue;
                    }
                }
                // Single _text_
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '_') {
                    nodes
                        .push(Node::new(node::EMPHASIS).children(self.parse_inline_content(&text)));
                    pos = end + 1;
                    continue;
                }
            }

            // Monospace: `text` or ``text``
            if chars[pos] == '`' {
                // Check for ``text`` first
                if pos + 1 < chars.len() && chars[pos + 1] == '`' {
                    if let Some((end, text)) = self.find_double_closing(&chars, pos + 2, '`') {
                        nodes.push(Node::new(node::CODE).prop(prop::CONTENT, text));
                        pos = end + 2;
                        continue;
                    }
                }
                // Single `text`
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '`') {
                    nodes.push(Node::new(node::CODE).prop(prop::CONTENT, text));
                    pos = end + 1;
                    continue;
                }
            }

            // Superscript: ^text^
            if chars[pos] == '^' {
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '^') {
                    nodes.push(
                        Node::new(node::SUPERSCRIPT).children(self.parse_inline_content(&text)),
                    );
                    pos = end + 1;
                    continue;
                }
            }

            // Subscript: ~text~
            if chars[pos] == '~' {
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '~') {
                    nodes.push(
                        Node::new(node::SUBSCRIPT).children(self.parse_inline_content(&text)),
                    );
                    pos = end + 1;
                    continue;
                }
            }

            // Highlight: #text#
            if chars[pos] == '#' {
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '#') {
                    nodes.push(
                        Node::new(node::SPAN)
                            .prop("class", "highlight".to_string())
                            .children(self.parse_inline_content(&text)),
                    );
                    pos = end + 1;
                    continue;
                }
            }

            // Inline image: image:path[alt]
            if pos + 6 < chars.len() {
                let prefix: String = chars[pos..pos + 6].iter().collect();
                if prefix == "image:" && !chars.get(pos + 6).map(|c| *c == ':').unwrap_or(false) {
                    if let Some(node) = self.parse_inline_image(&chars, pos) {
                        let end = self.find_image_end(&chars, pos + 6);
                        nodes.push(node);
                        pos = end;
                        continue;
                    }
                }
            }

            // Link: https://url or link:url[text] or <<anchor>>
            if pos + 4 < chars.len() {
                let prefix: String = chars[pos..].iter().take(8).collect();
                if prefix.starts_with("https://") || prefix.starts_with("http://") {
                    if let Some((end, node)) = self.parse_url_link(&chars, pos) {
                        nodes.push(node);
                        pos = end;
                        continue;
                    }
                }
            }

            if pos + 5 < chars.len() {
                let prefix: String = chars[pos..pos + 5].iter().collect();
                if prefix == "link:" {
                    if let Some((end, node)) = self.parse_link_macro(&chars, pos) {
                        nodes.push(node);
                        pos = end;
                        continue;
                    }
                }
            }

            if pos + 2 < chars.len() && chars[pos] == '<' && chars[pos + 1] == '<' {
                if let Some((end, node)) = self.parse_xref(&chars, pos) {
                    nodes.push(node);
                    pos = end;
                    continue;
                }
            }

            // Line break: + at end of line (represented by +\n or just + before end)
            if chars[pos] == '+' && (pos + 1 >= chars.len() || chars[pos + 1] == ' ') {
                nodes.push(Node::new(node::LINE_BREAK));
                pos += 1;
                continue;
            }

            // Regular text
            let mut text = String::new();
            while pos < chars.len() {
                let c = chars[pos];
                // Stop at potential markup starts
                if c == '*'
                    || c == '_'
                    || c == '`'
                    || c == '^'
                    || c == '~'
                    || c == '#'
                    || c == '+'
                    || c == '<'
                {
                    break;
                }
                // Check for image: or link:
                if c == 'i' || c == 'l' || c == 'h' {
                    let remaining: String = chars[pos..].iter().take(8).collect();
                    if remaining.starts_with("image:")
                        || remaining.starts_with("link:")
                        || remaining.starts_with("https://")
                        || remaining.starts_with("http://")
                    {
                        break;
                    }
                }
                text.push(c);
                pos += 1;
            }

            if !text.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, text));
            }
        }

        // Merge adjacent text nodes
        merge_text_nodes(&mut nodes);

        nodes
    }

    fn find_closing_char(
        &self,
        chars: &[char],
        start: usize,
        close: char,
    ) -> Option<(usize, String)> {
        let mut pos = start;
        let mut text = String::new();

        while pos < chars.len() {
            if chars[pos] == close {
                return Some((pos, text));
            }
            text.push(chars[pos]);
            pos += 1;
        }

        None
    }

    fn find_double_closing(
        &self,
        chars: &[char],
        start: usize,
        close: char,
    ) -> Option<(usize, String)> {
        let mut pos = start;
        let mut text = String::new();

        while pos + 1 < chars.len() {
            if chars[pos] == close && chars[pos + 1] == close {
                return Some((pos, text));
            }
            text.push(chars[pos]);
            pos += 1;
        }

        None
    }

    fn parse_inline_image(&self, chars: &[char], start: usize) -> Option<Node> {
        // image:path[alt]
        let pos = start + 6; // Skip "image:"

        let mut path_end = pos;
        while path_end < chars.len() && chars[path_end] != '[' {
            path_end += 1;
        }

        if path_end >= chars.len() {
            return None;
        }

        let path: String = chars[pos..path_end].iter().collect();

        // Find closing ]
        let mut attr_end = path_end + 1;
        while attr_end < chars.len() && chars[attr_end] != ']' {
            attr_end += 1;
        }

        let attrs: String = chars[path_end + 1..attr_end].iter().collect();

        let mut node = Node::new(node::IMAGE).prop(prop::URL, path);
        if !attrs.is_empty() {
            node = node.prop(prop::ALT, attrs);
        }

        Some(node)
    }

    fn find_image_end(&self, chars: &[char], start: usize) -> usize {
        let mut pos = start;
        // Find [
        while pos < chars.len() && chars[pos] != '[' {
            pos += 1;
        }
        // Find ]
        while pos < chars.len() && chars[pos] != ']' {
            pos += 1;
        }
        if pos < chars.len() {
            pos + 1
        } else {
            chars.len()
        }
    }

    fn parse_url_link(&self, chars: &[char], start: usize) -> Option<(usize, Node)> {
        let mut pos = start;

        // Collect URL until space or [ or end
        while pos < chars.len() && !chars[pos].is_whitespace() && chars[pos] != '[' {
            pos += 1;
        }

        let url: String = chars[start..pos].iter().collect();

        // Check for [text]
        if pos < chars.len() && chars[pos] == '[' {
            let text_start = pos + 1;
            while pos < chars.len() && chars[pos] != ']' {
                pos += 1;
            }
            let link_text: String = chars[text_start..pos].iter().collect();
            pos += 1; // Skip ]

            let node = Node::new(node::LINK)
                .prop(prop::URL, url)
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, link_text)]);
            return Some((pos, node));
        }

        // No text - URL is the text
        let node = Node::new(node::LINK)
            .prop(prop::URL, url.clone())
            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, url)]);
        Some((pos, node))
    }

    fn parse_link_macro(&self, chars: &[char], start: usize) -> Option<(usize, Node)> {
        // link:url[text]
        let url_start = start + 5; // Skip "link:"
        let mut pos = url_start;

        while pos < chars.len() && chars[pos] != '[' {
            pos += 1;
        }

        if pos >= chars.len() {
            return None;
        }

        let url: String = chars[url_start..pos].iter().collect();

        let text_start = pos + 1;
        while pos < chars.len() && chars[pos] != ']' {
            pos += 1;
        }
        let link_text: String = chars[text_start..pos].iter().collect();
        pos += 1; // Skip ]

        let text = if link_text.is_empty() {
            url.clone()
        } else {
            link_text
        };

        let node = Node::new(node::LINK)
            .prop(prop::URL, url)
            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text)]);
        Some((pos, node))
    }

    fn parse_xref(&self, chars: &[char], start: usize) -> Option<(usize, Node)> {
        // <<anchor>> or <<anchor,text>>
        let anchor_start = start + 2; // Skip "<<"
        let mut pos = anchor_start;

        while pos + 1 < chars.len() && !(chars[pos] == '>' && chars[pos + 1] == '>') {
            pos += 1;
        }

        if pos + 1 >= chars.len() {
            return None;
        }

        let content: String = chars[anchor_start..pos].iter().collect();
        pos += 2; // Skip ">>"

        let (anchor, text) = if let Some(comma_idx) = content.find(',') {
            (
                content[..comma_idx].to_string(),
                content[comma_idx + 1..].trim().to_string(),
            )
        } else {
            (content.clone(), content)
        };

        let node = Node::new(node::LINK)
            .prop(prop::URL, format!("#{}", anchor))
            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text)]);
        Some((pos, node))
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
        let input = "== Hello World\n\nSome text.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind.as_str(), node::HEADING);
        assert_eq!(children[0].props.get_int(prop::LEVEL), Some(2));
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
    fn test_parse_strong() {
        let input = "This is *strong* text.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let para = &root_children(&doc)[0];

        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::STRONG)
        );
    }

    #[test]
    fn test_parse_emphasis() {
        let input = "This is _emphasized_ text.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let para = &root_children(&doc)[0];

        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
    }

    #[test]
    fn test_parse_bullet_list() {
        let input = "* First item\n* Second item\n* Third item";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(children[0].children.len(), 3);
    }

    #[test]
    fn test_parse_numbered_list() {
        let input = ". First item\n. Second item\n. Third item";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::LIST);
        assert_eq!(children[0].props.get_bool(prop::ORDERED), Some(true));
        assert_eq!(children[0].children.len(), 3);
    }

    #[test]
    fn test_parse_code_block() {
        let input = "[source,python]\n----\nprint('hello')\n----";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(children[0].props.get_str(prop::LANGUAGE), Some("python"));
    }

    #[test]
    fn test_parse_inline_code() {
        let input = "Use `code here` in text.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let para = &root_children(&doc)[0];

        assert!(para.children.iter().any(|n| n.kind.as_str() == node::CODE));
    }

    #[test]
    fn test_parse_link() {
        let input = "Visit https://example.com[Example Site] for more.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let para = &root_children(&doc)[0];

        let link = para.children.iter().find(|n| n.kind.as_str() == node::LINK);
        assert!(link.is_some());
        assert_eq!(
            link.unwrap().props.get_str(prop::URL),
            Some("https://example.com")
        );
    }

    #[test]
    fn test_parse_block_image() {
        let input = "image::path/to/image.png[Alt text]";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::FIGURE);

        let img = &children[0].children[0];
        assert_eq!(img.kind.as_str(), node::IMAGE);
        assert_eq!(img.props.get_str(prop::URL), Some("path/to/image.png"));
    }
}
