//! reStructuredText (RST) reader for rescribe.
//!
//! Parses reStructuredText source into rescribe's document IR.
//! Uses a handwritten parser for core RST features.

#![allow(clippy::collapsible_if)]

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Severity, Span,
    WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse RST text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse RST with custom options.
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

/// RST heading character priority (lower = higher level).
/// The actual level is determined by order of appearance in the document.
const HEADING_CHARS: &[char] = &['=', '-', '~', '^', '"', '`', '#', '*', '+', '_'];

/// RST parser state.
struct Parser<'a> {
    input: &'a str,
    lines: Vec<&'a str>,
    line_idx: usize,
    warnings: Vec<FidelityWarning>,
    preserve_spans: bool,
    /// Maps underline character to heading level (assigned in order of appearance).
    heading_levels: Vec<char>,
    /// Link targets: name -> url
    link_targets: std::collections::HashMap<String, String>,
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
            heading_levels: Vec::new(),
            link_targets: std::collections::HashMap::new(),
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

    fn peek_line(&self) -> Option<&'a str> {
        self.lines.get(self.line_idx + 1).copied()
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

    /// First pass: collect link targets (.. _name: url)
    fn collect_link_targets(&mut self) {
        let mut idx = 0;
        while idx < self.lines.len() {
            let line = self.lines[idx];
            if let Some(rest) = line.strip_prefix(".. _") {
                if let Some(colon_idx) = rest.find(':') {
                    let name = rest[..colon_idx].trim().to_lowercase();
                    let url = rest[colon_idx + 1..].trim().to_string();
                    self.link_targets.insert(name, url);
                }
            }
            idx += 1;
        }
    }

    fn parse_document(&mut self) -> (Vec<Node>, Vec<FidelityWarning>) {
        // First pass: collect link targets
        self.collect_link_targets();

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
        // Skip link target definitions (already collected)
        if let Some(line) = self.current_line() {
            if line.starts_with(".. _") && line.contains(':') {
                self.advance_line();
                return self.try_parse_block();
            }
        }

        // Check for heading (text followed by underline)
        if let Some(heading) = self.try_parse_heading() {
            return Some(heading);
        }

        // Check for directive
        if let Some(directive) = self.try_parse_directive() {
            return Some(directive);
        }

        // Check for list
        if let Some(list) = self.try_parse_list() {
            return Some(list);
        }

        // Check for definition list
        if let Some(deflist) = self.try_parse_definition_list() {
            return Some(deflist);
        }

        // Check for literal block (ends with ::)
        if let Some(literal) = self.try_parse_literal_block() {
            return Some(literal);
        }

        // Check for block quote (indented paragraph)
        if let Some(bq) = self.try_parse_blockquote() {
            return Some(bq);
        }

        // Regular paragraph
        self.parse_paragraph()
    }

    fn try_parse_heading(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // Check if this line is all underline chars (possible overline)
        if self.is_underline(line) && !line.is_empty() {
            // Overlined heading: === then title then ===
            let overline_char = line.chars().next()?;
            let next_line = self.peek_line()?;
            if !next_line.trim().is_empty() && !self.is_underline(next_line) {
                // Check for underline after title
                let title = next_line.trim();
                if let Some(underline) = self.lines.get(self.line_idx + 2) {
                    if self.is_underline(underline) && underline.starts_with(overline_char) {
                        self.advance_line(); // skip overline
                        self.advance_line(); // skip title
                        self.advance_line(); // skip underline
                        let level = self.get_heading_level(overline_char);
                        let children = self.parse_inline_content(title);
                        let mut node = Node::new(node::HEADING)
                            .prop(prop::LEVEL, level)
                            .children(children);
                        node.span = self.make_span(start_line, self.line_idx);
                        return Some(node);
                    }
                }
            }
        }

        // Underlined heading: title then ===
        if !line.trim().is_empty() && !self.is_underline(line) {
            if let Some(underline) = self.peek_line() {
                if self.is_underline(underline) && underline.len() >= line.trim().len() {
                    let title = line.trim();
                    let underline_char = underline.chars().next()?;
                    self.advance_line(); // skip title
                    self.advance_line(); // skip underline
                    let level = self.get_heading_level(underline_char);
                    let children = self.parse_inline_content(title);
                    let mut node = Node::new(node::HEADING)
                        .prop(prop::LEVEL, level)
                        .children(children);
                    node.span = self.make_span(start_line, self.line_idx);
                    return Some(node);
                }
            }
        }

        None
    }

    fn is_underline(&self, line: &str) -> bool {
        if line.is_empty() {
            return false;
        }
        let first = line.chars().next().unwrap();
        HEADING_CHARS.contains(&first) && line.chars().all(|c| c == first)
    }

    fn get_heading_level(&mut self, ch: char) -> i64 {
        if let Some(pos) = self.heading_levels.iter().position(|&c| c == ch) {
            (pos + 1) as i64
        } else {
            self.heading_levels.push(ch);
            self.heading_levels.len() as i64
        }
    }

    fn try_parse_directive(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        if !line.starts_with(".. ") {
            return None;
        }

        let rest = &line[3..];

        // Check for comment (just .. with optional text but no ::)
        if !rest.contains("::") {
            // It's a comment, skip it
            self.advance_line();
            // Skip indented continuation
            while !self.is_eof() {
                let content_line = self.current_line().unwrap_or("");
                if content_line.is_empty()
                    || content_line.starts_with(' ')
                    || content_line.starts_with('\t')
                {
                    self.advance_line();
                } else {
                    break;
                }
            }
            return self.try_parse_block();
        }

        // Parse directive: .. name:: argument
        let colon_idx = rest.find("::")?;
        let directive_name = rest[..colon_idx].trim();
        let argument = rest[colon_idx + 2..].trim();

        self.advance_line();

        // Collect directive content (indented lines)
        let mut content_lines = Vec::new();
        let mut options = std::collections::HashMap::new();

        // First, collect field list options (:option: value)
        while !self.is_eof() {
            let Some(content_line) = self.current_line() else {
                break;
            };
            let trimmed = content_line.trim();
            if trimmed.is_empty() {
                self.advance_line();
                continue;
            }
            if (content_line.starts_with(' ') || content_line.starts_with('\t'))
                && trimmed.starts_with(':')
                && trimmed.len() > 1
            {
                // Option line
                if let Some(end_colon) = trimmed[1..].find(':') {
                    let opt_name = &trimmed[1..end_colon + 1];
                    let opt_value = trimmed[end_colon + 2..].trim();
                    options.insert(opt_name.to_string(), opt_value.to_string());
                    self.advance_line();
                    continue;
                }
            }
            break;
        }

        // Then collect content
        while !self.is_eof() {
            let Some(content_line) = self.current_line() else {
                break;
            };
            if content_line.is_empty() {
                content_lines.push("");
                self.advance_line();
            } else if content_line.starts_with(' ') || content_line.starts_with('\t') {
                content_lines.push(content_line.trim());
                self.advance_line();
            } else {
                break;
            }
        }

        // Handle specific directives
        match directive_name {
            "code" | "code-block" | "sourcecode" => {
                let language = if argument.is_empty() {
                    None
                } else {
                    Some(argument.to_string())
                };
                let content = content_lines.join("\n");
                let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
                if let Some(lang) = language {
                    node = node.prop(prop::LANGUAGE, lang);
                }
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            "note" | "warning" | "tip" | "important" | "caution" | "danger" | "error" | "hint"
            | "attention" => {
                let content = content_lines.join("\n");
                let children = self.parse_inline_content(&content);
                let mut node = Node::new(node::DIV)
                    .prop("class", directive_name.to_string())
                    .children(vec![Node::new(node::PARAGRAPH).children(children)]);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            "image" => {
                let mut node = Node::new(node::IMAGE).prop(prop::URL, argument.to_string());
                if let Some(alt) = options.get("alt") {
                    node = node.prop(prop::ALT, alt.clone());
                }
                if let Some(title) = options.get("title") {
                    node = node.prop(prop::TITLE, title.clone());
                }
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            "figure" => {
                let mut children =
                    vec![Node::new(node::IMAGE).prop(prop::URL, argument.to_string())];
                if !content_lines.is_empty() {
                    let caption_text = content_lines.join(" ");
                    let caption_children = self.parse_inline_content(&caption_text);
                    children.push(Node::new(node::CAPTION).children(caption_children));
                }
                let mut node = Node::new(node::FIGURE).children(children);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            "raw" => {
                let content = content_lines.join("\n");
                let format = argument.to_string();
                let mut node = Node::new(node::RAW_BLOCK)
                    .prop(prop::CONTENT, content)
                    .prop("format", format);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            "contents" | "toc" => {
                // Table of contents - represent as a div with class
                let mut node = Node::new(node::DIV).prop("class", "toc".to_string());
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
            _ => {
                // Unknown directive - emit warning and create generic div
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(format!("rst:{}", directive_name)),
                    format!("Unknown directive: {}", directive_name),
                ));
                let content = content_lines.join("\n");
                let children = if content.is_empty() {
                    vec![]
                } else {
                    vec![Node::new(node::PARAGRAPH).children(self.parse_inline_content(&content))]
                };
                let mut node = Node::new(node::DIV)
                    .prop("rst:directive", directive_name.to_string())
                    .children(children);
                node.span = self.make_span(start_line, self.line_idx);
                Some(node)
            }
        }
    }

    fn try_parse_list(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;
        let trimmed = line.trim_start();

        // Bullet list: *, -, +
        if trimmed
            .strip_prefix("* ")
            .or_else(|| trimmed.strip_prefix("- "))
            .or_else(|| trimmed.strip_prefix("+ "))
            .is_some()
        {
            let bullet_char = trimmed.chars().next().unwrap();
            return Some(self.parse_bullet_list(bullet_char, start_line));
        }

        // Numbered list: 1. or #.
        if let Some(idx) = trimmed.find(". ") {
            let prefix = &trimmed[..idx];
            if prefix.chars().all(|c| c.is_ascii_digit()) || prefix == "#" {
                return Some(self.parse_numbered_list(start_line));
            }
        }

        None
    }

    fn parse_bullet_list(&mut self, bullet: char, start_line: usize) -> Node {
        let mut items = Vec::new();
        let indent = self.get_indent();

        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");
            let current_indent = self.get_line_indent(line);
            let trimmed = line.trim_start();

            if trimmed.is_empty() {
                // Blank line - check if list continues
                let next_idx = self.line_idx + 1;
                if next_idx < self.lines.len() {
                    let next_line = self.lines[next_idx];
                    let next_trimmed = next_line.trim_start();
                    if !next_trimmed.starts_with(&format!("{} ", bullet)) {
                        break;
                    }
                }
                self.advance_line();
                continue;
            }

            if current_indent < indent && indent > 0 {
                break;
            }

            if let Some(rest) = trimmed.strip_prefix(&format!("{} ", bullet)) {
                let item = self.parse_list_item(rest, indent + 2);
                items.push(item);
            } else if current_indent > indent {
                // Continuation of previous item - skip for now
                self.advance_line();
            } else {
                break;
            }
        }

        let mut node = Node::new(node::LIST)
            .prop(prop::ORDERED, false)
            .children(items);
        node.span = self.make_span(start_line, self.line_idx);
        node
    }

    fn parse_numbered_list(&mut self, start_line: usize) -> Node {
        let mut items = Vec::new();
        let indent = self.get_indent();

        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");
            let current_indent = self.get_line_indent(line);
            let trimmed = line.trim_start();

            if trimmed.is_empty() {
                self.advance_line();
                continue;
            }

            if current_indent < indent && indent > 0 {
                break;
            }

            // Check for numbered item
            if let Some(idx) = trimmed.find(". ") {
                let prefix = &trimmed[..idx];
                if prefix.chars().all(|c| c.is_ascii_digit()) || prefix == "#" {
                    let rest = &trimmed[idx + 2..];
                    let item = self.parse_list_item(rest, indent + idx + 2);
                    items.push(item);
                    continue;
                }
            }

            if current_indent > indent {
                // Continuation
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

    fn parse_list_item(&mut self, first_line: &str, _content_indent: usize) -> Node {
        let start_line = self.line_idx;
        self.advance_line();

        let mut content = first_line.to_string();

        // Collect continuation lines
        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");
            if line.trim().is_empty() {
                break;
            }
            // Check if it's a new list item
            let trimmed = line.trim_start();
            if trimmed.starts_with("* ") || trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
                break;
            }
            if let Some(idx) = trimmed.find(". ") {
                let prefix = &trimmed[..idx];
                if prefix.chars().all(|c| c.is_ascii_digit()) || prefix == "#" {
                    break;
                }
            }
            // Check if indented (continuation)
            if line.starts_with(' ') || line.starts_with('\t') {
                content.push(' ');
                content.push_str(trimmed);
                self.advance_line();
            } else {
                break;
            }
        }

        let children = self.parse_inline_content(&content);
        let mut node = Node::new(node::LIST_ITEM).children(children);
        node.span = self.make_span(start_line, self.line_idx);
        node
    }

    fn try_parse_definition_list(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // Definition list: term at start of line, definition indented on next line
        if !line.is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && !line.starts_with(".. ")
        {
            if let Some(next_line) = self.peek_line() {
                if (next_line.starts_with(' ') || next_line.starts_with('\t'))
                    && !next_line.trim().is_empty()
                {
                    return Some(self.parse_definition_list(start_line));
                }
            }
        }

        None
    }

    fn parse_definition_list(&mut self, start_line: usize) -> Node {
        let mut items = Vec::new();

        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");

            // Skip blank lines
            if line.trim().is_empty() {
                self.advance_line();
                continue;
            }

            // Check if it's a term (non-indented)
            if !line.starts_with(' ') && !line.starts_with('\t') && !line.starts_with(".. ") {
                // Check if next line is definition (indented)
                if let Some(next_line) = self.peek_line() {
                    if (next_line.starts_with(' ') || next_line.starts_with('\t'))
                        && !next_line.trim().is_empty()
                    {
                        let term = line.trim();
                        let term_children = self.parse_inline_content(term);
                        let term_node = Node::new(node::DEFINITION_TERM).children(term_children);

                        self.advance_line();

                        // Collect definition
                        let mut def_content = String::new();
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
                            } else {
                                break;
                            }
                        }

                        let def_children = self.parse_inline_content(&def_content);
                        let def_node = Node::new(node::DEFINITION_DESC).children(def_children);

                        items.push(term_node);
                        items.push(def_node);
                        continue;
                    }
                }
            }

            break;
        }

        let mut node = Node::new(node::DEFINITION_LIST).children(items);
        node.span = self.make_span(start_line, self.line_idx);
        node
    }

    fn try_parse_literal_block(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // Check for :: at end of line (paragraph ending with ::)
        if line.trim_end().ends_with("::") {
            let _intro = line.trim_end().strip_suffix("::").unwrap();
            self.advance_line();
            self.skip_blank_lines();

            // Collect indented content
            let mut content_lines = Vec::new();
            let base_indent = self.get_indent();

            while !self.is_eof() {
                let content_line = self.current_line().unwrap_or("");
                if content_line.trim().is_empty() {
                    content_lines.push("");
                    self.advance_line();
                } else if self.get_line_indent(content_line) >= base_indent {
                    // Remove base indentation
                    let dedented = if content_line.len() > base_indent {
                        &content_line[base_indent..]
                    } else {
                        ""
                    };
                    content_lines.push(dedented);
                    self.advance_line();
                } else {
                    break;
                }
            }

            // Trim trailing empty lines
            while content_lines.last() == Some(&"") {
                content_lines.pop();
            }

            let content = content_lines.join("\n");

            let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
            node.span = self.make_span(start_line, self.line_idx);
            return Some(node);
        }

        // Check for standalone :: on its own line
        if line.trim() == "::" {
            self.advance_line();
            self.skip_blank_lines();

            let mut content_lines = Vec::new();
            let base_indent = self.get_indent();

            while !self.is_eof() {
                let content_line = self.current_line().unwrap_or("");
                if content_line.trim().is_empty() {
                    content_lines.push("");
                    self.advance_line();
                } else if self.get_line_indent(content_line) >= base_indent {
                    let dedented = if content_line.len() > base_indent {
                        &content_line[base_indent..]
                    } else {
                        ""
                    };
                    content_lines.push(dedented);
                    self.advance_line();
                } else {
                    break;
                }
            }

            while content_lines.last() == Some(&"") {
                content_lines.pop();
            }

            let content = content_lines.join("\n");
            let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
            node.span = self.make_span(start_line, self.line_idx);
            return Some(node);
        }

        None
    }

    fn try_parse_blockquote(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let line = self.current_line()?;

        // Block quote: indented text that's not a list or literal block
        if (line.starts_with(' ') || line.starts_with('\t')) && !line.trim().is_empty() {
            let trimmed = line.trim();
            // Make sure it's not a list item
            if trimmed.starts_with("* ") || trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
                return None;
            }
            if let Some(idx) = trimmed.find(". ") {
                let prefix = &trimmed[..idx];
                if prefix.chars().all(|c| c.is_ascii_digit()) || prefix == "#" {
                    return None;
                }
            }

            // Collect block quote content
            let mut content = String::new();
            while !self.is_eof() {
                let bq_line = self.current_line().unwrap_or("");
                if bq_line.trim().is_empty() {
                    break;
                }
                if bq_line.starts_with(' ') || bq_line.starts_with('\t') {
                    if !content.is_empty() {
                        content.push(' ');
                    }
                    content.push_str(bq_line.trim());
                    self.advance_line();
                } else {
                    break;
                }
            }

            let children = self.parse_inline_content(&content);
            let mut node = Node::new(node::BLOCKQUOTE)
                .children(vec![Node::new(node::PARAGRAPH).children(children)]);
            node.span = self.make_span(start_line, self.line_idx);
            return Some(node);
        }

        None
    }

    fn parse_paragraph(&mut self) -> Option<Node> {
        let start_line = self.line_idx;
        let mut content = String::new();

        while !self.is_eof() {
            let line = self.current_line().unwrap_or("");

            if line.trim().is_empty() {
                break;
            }

            // Check if next line is an underline (making this a heading)
            if let Some(next) = self.peek_line() {
                if self.is_underline(next) {
                    break;
                }
            }

            // Check for start of block elements
            if line.starts_with(".. ") {
                break;
            }

            if !content.is_empty() {
                content.push(' ');
            }
            content.push_str(line.trim());
            self.advance_line();
        }

        if content.is_empty() {
            return None;
        }

        // Check for trailing :: (literal block indicator)
        let content = if content.ends_with("::") && content.len() > 2 {
            content[..content.len() - 1].trim_end().to_string()
        } else {
            content
        };

        let children = self.parse_inline_content(&content);
        let mut node = Node::new(node::PARAGRAPH).children(children);
        node.span = self.make_span(start_line, self.line_idx);

        Some(node)
    }

    fn get_indent(&self) -> usize {
        self.current_line()
            .map(|l| self.get_line_indent(l))
            .unwrap_or(0)
    }

    fn get_line_indent(&self, line: &str) -> usize {
        line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
    }

    fn parse_inline_content(&self, content: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = content.chars().collect();

        while pos < chars.len() {
            // Strong: **text**
            if pos + 1 < chars.len() && chars[pos] == '*' && chars[pos + 1] == '*' {
                if let Some((end, text)) = self.find_closing(&chars, pos + 2, "**") {
                    nodes.push(Node::new(node::STRONG).children(self.parse_inline_content(&text)));
                    pos = end + 2;
                    continue;
                }
            }

            // Emphasis: *text*
            if chars[pos] == '*' {
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '*') {
                    if !text.is_empty() && !text.starts_with('*') {
                        nodes.push(
                            Node::new(node::EMPHASIS).children(self.parse_inline_content(&text)),
                        );
                        pos = end + 1;
                        continue;
                    }
                }
            }

            // Inline literal: ``text``
            if pos + 1 < chars.len() && chars[pos] == '`' && chars[pos + 1] == '`' {
                if let Some((end, text)) = self.find_closing(&chars, pos + 2, "``") {
                    nodes.push(Node::new(node::CODE).prop(prop::CONTENT, text));
                    pos = end + 2;
                    continue;
                }
            }

            // Interpreted text with role: :role:`text`
            if chars[pos] == ':' {
                if let Some((role_end, role)) = self.find_closing_char(&chars, pos + 1, ':') {
                    if role_end + 1 < chars.len() && chars[role_end + 1] == '`' {
                        if let Some((text_end, text)) =
                            self.find_closing_char(&chars, role_end + 2, '`')
                        {
                            // Handle common roles
                            let node = match role.as_str() {
                                "emphasis" | "em" => Node::new(node::EMPHASIS)
                                    .children(self.parse_inline_content(&text)),
                                "strong" => Node::new(node::STRONG)
                                    .children(self.parse_inline_content(&text)),
                                "code" | "literal" => {
                                    Node::new(node::CODE).prop(prop::CONTENT, text)
                                }
                                "subscript" | "sub" => Node::new(node::SUBSCRIPT)
                                    .children(self.parse_inline_content(&text)),
                                "superscript" | "sup" => Node::new(node::SUPERSCRIPT)
                                    .children(self.parse_inline_content(&text)),
                                "title-reference" | "title" | "t" => Node::new(node::EMPHASIS)
                                    .children(self.parse_inline_content(&text)),
                                "ref" | "doc" => {
                                    // Internal reference - treat as link
                                    Node::new(node::LINK)
                                        .prop(prop::URL, format!("#{}", text))
                                        .children(vec![
                                            Node::new(node::TEXT).prop(prop::CONTENT, text.clone()),
                                        ])
                                }
                                "math" => Node::new("math_inline").prop("math:source", text),
                                _ => {
                                    // Unknown role - just use the text
                                    Node::new(node::SPAN)
                                        .prop("rst:role", role)
                                        .children(self.parse_inline_content(&text))
                                }
                            };
                            nodes.push(node);
                            pos = text_end + 1;
                            continue;
                        }
                    }
                }
            }

            // Inline link: `text <url>`_
            if chars[pos] == '`' {
                if let Some((end, text)) = self.find_closing_char(&chars, pos + 1, '`') {
                    // Check for trailing _
                    if end + 1 < chars.len() && chars[end + 1] == '_' {
                        // Check if it's an inline link with URL
                        if let Some(angle_start) = text.rfind('<') {
                            if text.ends_with('>') {
                                let link_text = text[..angle_start].trim();
                                let url = &text[angle_start + 1..text.len() - 1];
                                nodes.push(
                                    Node::new(node::LINK)
                                        .prop(prop::URL, url.to_string())
                                        .children(vec![
                                            Node::new(node::TEXT)
                                                .prop(prop::CONTENT, link_text.to_string()),
                                        ]),
                                );
                                pos = end + 2;
                                continue;
                            }
                        }

                        // Reference link - look up in link_targets
                        let ref_name = text.to_lowercase();
                        if let Some(url) = self.link_targets.get(&ref_name) {
                            nodes.push(
                                Node::new(node::LINK)
                                    .prop(prop::URL, url.clone())
                                    .children(vec![
                                        Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()),
                                    ]),
                            );
                            pos = end + 2;
                            continue;
                        }
                    }

                    // Plain interpreted text (default role, usually emphasis)
                    nodes.push(Node::new(node::EMPHASIS).children(vec![
                        Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()),
                    ]));
                    pos = end + 1;
                    continue;
                }
            }

            // Simple reference link: word_
            if chars[pos].is_alphanumeric() {
                let mut word_end = pos;
                while word_end < chars.len()
                    && (chars[word_end].is_alphanumeric()
                        || chars[word_end] == '_'
                        || chars[word_end] == '-')
                {
                    word_end += 1;
                }
                if word_end < chars.len() && chars[word_end] == '_' {
                    // Check it's not __ (anonymous reference)
                    if word_end + 1 >= chars.len() || chars[word_end + 1] != '_' {
                        let word: String = chars[pos..word_end].iter().collect();
                        let ref_name = word.to_lowercase();
                        if let Some(url) = self.link_targets.get(&ref_name) {
                            nodes.push(
                                Node::new(node::LINK)
                                    .prop(prop::URL, url.clone())
                                    .children(vec![
                                        Node::new(node::TEXT).prop(prop::CONTENT, word),
                                    ]),
                            );
                            pos = word_end + 1;
                            continue;
                        }
                    }
                }
            }

            // Regular text
            let mut text = String::new();
            while pos < chars.len() {
                let c = chars[pos];
                // Stop at potential inline markup starts
                if c == '*' || c == '`' || c == ':' || c == '[' {
                    break;
                }
                // Stop at potential reference (word followed by _)
                if c.is_alphanumeric() {
                    let mut word_end = pos;
                    while word_end < chars.len()
                        && (chars[word_end].is_alphanumeric()
                            || chars[word_end] == '_'
                            || chars[word_end] == '-')
                    {
                        word_end += 1;
                    }
                    if word_end < chars.len()
                        && chars[word_end] == '_'
                        && (word_end + 1 >= chars.len() || chars[word_end + 1] != '_')
                    {
                        let word: String = chars[pos..word_end].iter().collect();
                        if self.link_targets.contains_key(&word.to_lowercase()) {
                            break;
                        }
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

    fn find_closing(&self, chars: &[char], start: usize, pattern: &str) -> Option<(usize, String)> {
        let pat_chars: Vec<char> = pattern.chars().collect();
        let mut pos = start;
        let mut text = String::new();

        while pos + pat_chars.len() <= chars.len() {
            let mut matches = true;
            for (i, pc) in pat_chars.iter().enumerate() {
                if chars[pos + i] != *pc {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some((pos, text));
            }
            text.push(chars[pos]);
            pos += 1;
        }

        None
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
        let input = "Hello World\n===========\n\nSome text.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 2);
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
        let input = "This is *emphasized* text.";
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
    fn test_parse_strong() {
        let input = "This is **strong** text.";
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
        let input = "1. First item\n2. Second item\n3. Third item";
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
        let input = "Example::\n\n    def hello():\n        print('Hello')";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        // Should have paragraph and code block
        assert!(children.iter().any(|n| n.kind.as_str() == node::CODE_BLOCK));
    }

    #[test]
    fn test_parse_inline_code() {
        let input = "Use ``code here`` in text.";
        let result = parse(input).unwrap();
        let doc = result.value;
        let para = &root_children(&doc)[0];

        assert!(para.children.iter().any(|n| n.kind.as_str() == node::CODE));
    }

    #[test]
    fn test_parse_link() {
        let input = "Click `here <https://example.com>`_ for more.";
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
    fn test_parse_directive() {
        let input = ".. code-block:: python\n\n   print('hello')";
        let result = parse(input).unwrap();
        let doc = result.value;
        let children = root_children(&doc);

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(children[0].props.get_str(prop::LANGUAGE), Some("python"));
    }
}
