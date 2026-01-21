//! Typst reader for rescribe.
//!
//! Parses Typst markup into rescribe documents.

#![allow(clippy::collapsible_if)]

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Typst source into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Typst source with custom options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    let root = parser.parse_document();
    let doc = Document::new().with_content(root);
    Ok(ConversionResult::ok(doc))
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let lines: Vec<&str> = input.lines().collect();
        Self { lines, pos: 0 }
    }

    fn parse_document(&mut self) -> Node {
        let mut children = Vec::new();

        while self.pos < self.lines.len() {
            if let Some(node) = self.parse_block() {
                children.push(node);
            }
        }

        Node::new(node::DOCUMENT).children(children)
    }

    fn current_line(&self) -> Option<&'a str> {
        self.lines.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn parse_block(&mut self) -> Option<Node> {
        let line = self.current_line()?;

        // Skip blank lines
        if line.trim().is_empty() {
            self.advance();
            return None;
        }

        let trimmed = line.trim();

        // Heading: = Title, == Subtitle, etc.
        if trimmed.starts_with('=') && !trimmed.starts_with("==") {
            return Some(self.parse_heading());
        }
        if trimmed.starts_with("==") {
            return Some(self.parse_heading());
        }

        // Code block: ```
        if trimmed.starts_with("```") {
            return Some(self.parse_code_block());
        }

        // List: - item or + item (numbered)
        if trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
            return Some(self.parse_list());
        }

        // Function calls
        if trimmed.starts_with('#') {
            return self.parse_function_call();
        }

        // Blockquote: > text
        if trimmed.starts_with("> ") || trimmed == ">" {
            return Some(self.parse_blockquote());
        }

        // Default: paragraph
        Some(self.parse_paragraph())
    }

    fn parse_heading(&mut self) -> Node {
        let line = self.current_line().unwrap();
        self.advance();

        let trimmed = line.trim();

        // Count = signs for level
        let level_count = trimmed.chars().take_while(|c| *c == '=').count();
        let content = trimmed[level_count..].trim();
        let level = level_count as i64;

        Node::new(node::HEADING)
            .prop(prop::LEVEL, level)
            .children(self.parse_inline(content))
    }

    fn parse_code_block(&mut self) -> Node {
        let line = self.current_line().unwrap();
        self.advance();

        let trimmed = line.trim();

        // Extract language if present: ```rust
        let lang = if trimmed.len() > 3 {
            Some(trimmed[3..].trim())
        } else {
            None
        };

        let mut content = String::new();
        while let Some(line) = self.current_line() {
            if line.trim().starts_with("```") {
                self.advance();
                break;
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(line);
            self.advance();
        }

        let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
        if let Some(lang) = lang {
            if !lang.is_empty() {
                node = node.prop(prop::LANGUAGE, lang);
            }
        }
        node
    }

    fn parse_list(&mut self) -> Node {
        let mut items = Vec::new();
        let ordered = self.current_line().map(|l| l.trim().starts_with('+')) == Some(true);

        while let Some(line) = self.current_line() {
            let trimmed = line.trim();
            if !trimmed.starts_with("- ") && !trimmed.starts_with("+ ") {
                break;
            }

            // Get content after marker
            let content = &trimmed[2..];
            let item_children = self.parse_inline(content);
            let item = Node::new(node::LIST_ITEM)
                .children(vec![Node::new(node::PARAGRAPH).children(item_children)]);
            items.push(item);
            self.advance();
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items)
    }

    fn parse_function_call(&mut self) -> Option<Node> {
        let line = self.current_line().unwrap();
        self.advance();

        let trimmed = line.trim();

        // Parse common Typst functions
        if trimmed.starts_with("#image(") {
            return Some(self.parse_image_function(trimmed));
        }

        if trimmed.starts_with("#link(") {
            return Some(self.parse_link_function(trimmed));
        }

        if trimmed.starts_with("#raw(") {
            return Some(self.parse_raw_function(trimmed));
        }

        if trimmed.starts_with("#quote(") || trimmed.starts_with("#quote[") {
            return Some(self.parse_quote_function(trimmed));
        }

        if trimmed.starts_with("#figure(") {
            return Some(self.parse_figure_function(trimmed));
        }

        if trimmed.starts_with("#table(") {
            return Some(self.parse_table_function(trimmed));
        }

        // Generic: treat as raw block
        Some(
            Node::new(node::RAW_BLOCK)
                .prop(prop::FORMAT, "typst")
                .prop(prop::CONTENT, trimmed),
        )
    }

    fn parse_image_function(&self, s: &str) -> Node {
        // #image("path.png")
        if let Some(path) = extract_string_arg(s, "#image(") {
            Node::new(node::IMAGE).prop(prop::URL, path)
        } else {
            Node::new(node::IMAGE)
        }
    }

    fn parse_link_function(&self, s: &str) -> Node {
        // #link("url")[text] or #link("url")
        if let Some(url) = extract_string_arg(s, "#link(") {
            let text = extract_bracket_content(s).unwrap_or(&url);
            Node::new(node::LINK)
                .prop(prop::URL, url.clone())
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text)])
        } else {
            Node::new(node::LINK)
        }
    }

    fn parse_raw_function(&self, s: &str) -> Node {
        // #raw("content")
        if let Some(content) = extract_string_arg(s, "#raw(") {
            Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
        } else {
            Node::new(node::CODE_BLOCK)
        }
    }

    fn parse_quote_function(&mut self, s: &str) -> Node {
        // #quote[content] or #quote(attribution: "...")[content]
        let content = extract_bracket_content(s).unwrap_or("");
        Node::new(node::BLOCKQUOTE)
            .children(vec![Node::new(node::PARAGRAPH).children(vec![
                Node::new(node::TEXT).prop(prop::CONTENT, content),
            ])])
    }

    fn parse_figure_function(&self, s: &str) -> Node {
        // #figure(image("path"), caption: [...])
        let figure = Node::new(node::FIGURE);
        let mut children = Vec::new();

        // Try to extract image
        if let Some(img_start) = s.find("image(") {
            let after_img = &s[img_start..];
            if let Some(path) = extract_string_arg(after_img, "image(") {
                children.push(Node::new(node::IMAGE).prop(prop::URL, path));
            }
        }

        // Try to extract caption
        if let Some(cap_start) = s.find("caption:") {
            let after_cap = &s[cap_start..];
            if let Some(caption_text) = extract_bracket_content(after_cap) {
                children.push(Node::new(node::CAPTION).children(vec![
                    Node::new(node::TEXT).prop(prop::CONTENT, caption_text),
                ]));
            }
        }

        figure.children(children)
    }

    fn parse_table_function(&mut self, _s: &str) -> Node {
        // Tables in Typst are complex - simplified parsing
        // #table(columns: 2, [...], [...], ...)
        Node::new(node::TABLE)
    }

    fn parse_blockquote(&mut self) -> Node {
        let mut lines = Vec::new();

        while let Some(line) = self.current_line() {
            let trimmed = line.trim();
            if !trimmed.starts_with('>') {
                break;
            }
            let content = if trimmed.len() > 1 {
                trimmed[1..].trim_start()
            } else {
                ""
            };
            lines.push(content);
            self.advance();
        }

        let text = lines.join("\n");
        Node::new(node::BLOCKQUOTE).children(vec![
            Node::new(node::PARAGRAPH)
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, text)]),
        ])
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut lines = Vec::new();

        while let Some(line) = self.current_line() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            // Stop at block-level elements
            if trimmed.starts_with('=')
                || trimmed.starts_with("```")
                || trimmed.starts_with("- ")
                || trimmed.starts_with("+ ")
                || trimmed.starts_with('#')
                || trimmed.starts_with('>')
            {
                break;
            }
            lines.push(trimmed);
            self.advance();
        }

        let text = lines.join(" ");
        Node::new(node::PARAGRAPH).children(self.parse_inline(&text))
    }

    fn parse_inline(&self, text: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            // Bold: *text*
            if c == '*' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_matching(&chars, i + 1, '*') {
                    nodes.push(
                        Node::new(node::STRONG)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, content)]),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Italic: _text_
            if c == '_' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_matching(&chars, i + 1, '_') {
                    nodes.push(
                        Node::new(node::EMPHASIS)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, content)]),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Inline code: `code`
            if c == '`' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_matching(&chars, i + 1, '`') {
                    nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
                    i = end + 1;
                    continue;
                }
            }

            // Math: $...$
            if c == '$' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_matching(&chars, i + 1, '$') {
                    // Display math has spaces: $ x^2 $
                    let is_display = content.starts_with(' ') && content.ends_with(' ');
                    let kind = if is_display {
                        "math_display"
                    } else {
                        "math_inline"
                    };
                    let math_content = if is_display { content.trim() } else { &content };
                    nodes.push(Node::new(kind).prop("math:source", math_content));
                    i = end + 1;
                    continue;
                }
            }

            // Function calls inline: #func(...)
            if c == '#' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                // Find end of function call
                let remaining: String = chars[i..].iter().collect();
                if remaining.starts_with("#link(") {
                    if let Some(paren_end) = find_paren_end(&remaining) {
                        let func_text = &remaining[..paren_end + 1];
                        // Check for bracket content
                        let after_paren = &remaining[paren_end + 1..];
                        let (full_text, skip) = if after_paren.starts_with('[') {
                            if let Some(bracket_end) = find_bracket_end(after_paren) {
                                (
                                    format!("{}{}", func_text, &after_paren[..bracket_end + 1]),
                                    paren_end + bracket_end + 2,
                                )
                            } else {
                                (func_text.to_string(), paren_end + 1)
                            }
                        } else {
                            (func_text.to_string(), paren_end + 1)
                        };

                        if let Some(url) = extract_string_arg(&full_text, "#link(") {
                            let text = extract_bracket_content(&full_text).unwrap_or(&url);
                            nodes.push(
                                Node::new(node::LINK)
                                    .prop(prop::URL, url.clone())
                                    .children(vec![
                                        Node::new(node::TEXT).prop(prop::CONTENT, text),
                                    ]),
                            );
                            i += skip;
                            continue;
                        }
                    }
                }
            }

            current.push(c);
            i += 1;
        }

        if !current.is_empty() {
            nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current));
        }

        nodes
    }

    fn find_matching(&self, chars: &[char], start: usize, delim: char) -> Option<(String, usize)> {
        let mut i = start;
        let mut content = String::new();

        while i < chars.len() {
            if chars[i] == delim {
                return Some((content, i));
            }
            content.push(chars[i]);
            i += 1;
        }

        None
    }
}

/// Extract a string argument from a function call like `#func("value")`
fn extract_string_arg(s: &str, prefix: &str) -> Option<String> {
    let after = s.strip_prefix(prefix)?;
    let quote_start = after.find('"')?;
    let rest = &after[quote_start + 1..];
    let quote_end = rest.find('"')?;
    Some(rest[..quote_end].to_string())
}

/// Extract content from brackets like `[content]`
fn extract_bracket_content(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end > start {
        Some(&s[start + 1..end])
    } else {
        None
    }
}

/// Find the matching closing parenthesis
fn find_paren_end(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the matching closing bracket
fn find_bracket_end(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.chars().enumerate() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_heading() {
        let doc = parse_str("= Title");
        let heading = &doc.content.children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
        assert_eq!(heading.props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_levels() {
        let doc = parse_str("= Level 1\n== Level 2\n=== Level 3");
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
        assert_eq!(doc.content.children[1].props.get_int(prop::LEVEL), Some(2));
        assert_eq!(doc.content.children[2].props.get_int(prop::LEVEL), Some(3));
    }

    #[test]
    fn test_parse_paragraph() {
        let doc = parse_str("Hello world!");
        let para = &doc.content.children[0];
        assert_eq!(para.kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str("This is *bold* text.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("This is _italic_ text.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_code() {
        let doc = parse_str("Use `code` here.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse_str("```rust\nfn main() {}\n```");
        let code = &doc.content.children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
        assert_eq!(code.props.get_str(prop::LANGUAGE), Some("rust"));
    }

    #[test]
    fn test_parse_list() {
        let doc = parse_str("- Item 1\n- Item 2");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_ordered_list() {
        let doc = parse_str("+ First\n+ Second");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    fn test_parse_image() {
        let doc = parse_str("#image(\"photo.png\")");
        let img = &doc.content.children[0];
        assert_eq!(img.kind.as_str(), node::IMAGE);
        assert_eq!(img.props.get_str(prop::URL), Some("photo.png"));
    }
}
