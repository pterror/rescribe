//! Markua (Leanpub) reader for rescribe.
//!
//! Parses Markua markup (Markdown for books) into the rescribe document model.
//! Markua is CommonMark with extensions like asides, blurbs, and special blocks.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Markua markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Markua markup with custom options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    let nodes = parser.parse();

    let root = Node::new(node::DOCUMENT).children(nodes);
    let doc = Document::new().with_content(root);

    Ok(ConversionResult::ok(doc))
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines().collect(),
            pos: 0,
        }
    }

    fn parse(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if line.trim().is_empty() {
                self.pos += 1;
                continue;
            }

            // ATX headings: # Title
            if let Some(node) = self.try_parse_atx_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Scene break: * * * or - - - or *** or ---
            if self.is_scene_break(line) {
                nodes.push(Node::new(node::HORIZONTAL_RULE));
                self.pos += 1;
                continue;
            }

            // Fenced code block
            if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                nodes.push(self.parse_fenced_code_block());
                continue;
            }

            // Markua special blocks: A>, B>, W>, T>, E>, D>, Q>, I>
            if let Some(block_type) = Self::get_special_block_type(line) {
                nodes.push(self.parse_special_block(block_type));
                continue;
            }

            // Blockquote: > text
            if line.trim_start().starts_with("> ") || line.trim_start() == ">" {
                nodes.push(self.parse_blockquote());
                continue;
            }

            // Unordered list: - or * or +
            let trimmed = line.trim_start();
            if (trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ "))
                && !self.is_scene_break(line)
            {
                nodes.push(self.parse_list(false));
                continue;
            }

            // Ordered list: 1. or 1)
            if self.is_ordered_list_item(line) {
                nodes.push(self.parse_list(true));
                continue;
            }

            // Regular paragraph
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn try_parse_atx_heading(&self, line: &str) -> Option<Node> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            return None;
        }

        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if level == 0 || level > 6 {
            return None;
        }

        let rest = &trimmed[level..];
        // Heading must be followed by space or be empty
        if !rest.is_empty() && !rest.starts_with(' ') {
            return None;
        }

        // Remove trailing # if present
        let title = rest.trim().trim_end_matches('#').trim();
        let inline_nodes = parse_inline(title);

        Some(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level as i64)
                .children(inline_nodes),
        )
    }

    fn is_scene_break(&self, line: &str) -> bool {
        let trimmed = line.trim();
        // * * * or - - - or *** or --- (at least 3 characters)
        if trimmed.len() < 3 {
            return false;
        }

        let chars: Vec<char> = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        if chars.len() < 3 {
            return false;
        }

        let first = chars[0];
        (first == '*' || first == '-' || first == '_') && chars.iter().all(|&c| c == first)
    }

    fn get_special_block_type(line: &str) -> Option<&'static str> {
        let trimmed = line.trim_start();
        let prefixes = [
            ("A> ", "aside"),
            ("B> ", "blurb"),
            ("W> ", "warning"),
            ("T> ", "tip"),
            ("E> ", "error"),
            ("D> ", "discussion"),
            ("Q> ", "question"),
            ("I> ", "information"),
        ];

        for (prefix, block_type) in prefixes {
            if trimmed.starts_with(prefix) {
                return Some(block_type);
            }
        }
        None
    }

    fn parse_special_block(&mut self, block_type: &str) -> Node {
        let prefix = match block_type {
            "aside" => "A> ",
            "blurb" => "B> ",
            "warning" => "W> ",
            "tip" => "T> ",
            "error" => "E> ",
            "discussion" => "D> ",
            "question" => "Q> ",
            "information" => "I> ",
            _ => return Node::new(node::PARAGRAPH),
        };

        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix(prefix) {
                if !content.is_empty() {
                    content.push(' ');
                }
                content.push_str(rest);
                self.pos += 1;
            } else if trimmed.is_empty() {
                self.pos += 1;
                break;
            } else {
                break;
            }
        }

        let inline_nodes = parse_inline(&content);
        let para = Node::new(node::PARAGRAPH).children(inline_nodes);

        Node::new(node::DIV)
            .prop("class", block_type)
            .children(vec![para])
    }

    fn parse_fenced_code_block(&mut self) -> Node {
        let first_line = self.lines[self.pos].trim_start();
        let fence_char = first_line.chars().next().unwrap_or('`');
        let fence_len = first_line.chars().take_while(|&c| c == fence_char).count();

        // Extract info string (language)
        let info_string = first_line[fence_len..].trim();
        let language = if info_string.is_empty() {
            None
        } else {
            Some(info_string.split_whitespace().next().unwrap_or(""))
        };

        self.pos += 1;
        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            // Check for closing fence
            if trimmed.starts_with(fence_char)
                && trimmed.chars().take_while(|&c| c == fence_char).count() >= fence_len
            {
                self.pos += 1;
                break;
            }

            content.push_str(line);
            content.push('\n');
            self.pos += 1;
        }

        let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end());
        if let Some(lang) = language {
            node = node.prop(prop::LANGUAGE, lang);
        }
        node
    }

    fn parse_blockquote(&mut self) -> Node {
        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix("> ") {
                if !content.is_empty() {
                    content.push(' ');
                }
                content.push_str(rest);
                self.pos += 1;
            } else if trimmed == ">" {
                // Empty blockquote line
                self.pos += 1;
            } else if trimmed.is_empty() {
                self.pos += 1;
                break;
            } else {
                break;
            }
        }

        let inline_nodes = parse_inline(&content);
        let para = Node::new(node::PARAGRAPH).children(inline_nodes);
        Node::new(node::BLOCKQUOTE).children(vec![para])
    }

    fn is_ordered_list_item(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        // Check for digit(s) followed by . or ) and space
        let mut chars = trimmed.chars();
        let mut has_digit = false;

        while let Some(c) = chars.next() {
            if c.is_ascii_digit() {
                has_digit = true;
            } else if has_digit && (c == '.' || c == ')') {
                // Check if followed by space or end
                match chars.next() {
                    Some(' ') | None => return true,
                    _ => return false,
                }
            } else {
                return false;
            }
        }
        false
    }

    fn parse_list(&mut self, ordered: bool) -> Node {
        let mut items = Vec::new();
        let base_indent = self.lines[self.pos].len() - self.lines[self.pos].trim_start().len();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim_start();

            if trimmed.is_empty() {
                self.pos += 1;
                continue;
            }

            if indent < base_indent {
                break;
            }

            let is_bullet =
                trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ");
            let is_numbered = self.is_ordered_list_item(line);

            if !is_bullet && !is_numbered {
                break;
            }

            let content = if is_bullet {
                &trimmed[2..]
            } else {
                // Find position after number and delimiter
                let marker_end = trimmed.find(". ").or_else(|| trimmed.find(") "));
                if let Some(pos) = marker_end {
                    &trimmed[pos + 2..]
                } else {
                    break;
                }
            };

            let inline_nodes = parse_inline(content);
            let para = Node::new(node::PARAGRAPH).children(inline_nodes);
            items.push(Node::new(node::LIST_ITEM).children(vec![para]));
            self.pos += 1;
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items)
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut text = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if line.trim().is_empty() {
                break;
            }

            // Check for block elements
            let trimmed = line.trim_start();
            if self.try_parse_atx_heading(line).is_some()
                || self.is_scene_break(line)
                || trimmed.starts_with("```")
                || trimmed.starts_with("~~~")
                || trimmed.starts_with("> ")
                || Self::get_special_block_type(line).is_some()
                || trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || self.is_ordered_list_item(line)
            {
                break;
            }

            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line.trim());
            self.pos += 1;
        }

        let inline_nodes = parse_inline(&text);
        Node::new(node::PARAGRAPH).children(inline_nodes)
    }
}

/// Parse inline formatting.
fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Strong: **text** or __text__
        if i + 1 < chars.len()
            && ((chars[i] == '*' && chars[i + 1] == '*')
                || (chars[i] == '_' && chars[i + 1] == '_'))
        {
            let marker = chars[i];
            if let Some((end, content)) = find_double_marker(&chars, i + 2, marker) {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                let inner = parse_inline(&content);
                nodes.push(Node::new(node::STRONG).children(inner));
                i = end + 2;
                continue;
            }
        }

        // Emphasis: *text* or _text_
        if chars[i] == '*' || chars[i] == '_' {
            let marker = chars[i];
            if let Some((end, content)) = find_single_marker(&chars, i + 1, marker) {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                let inner = parse_inline(&content);
                nodes.push(Node::new(node::EMPHASIS).children(inner));
                i = end + 1;
                continue;
            }
        }

        // Inline code: `code`
        if chars[i] == '`'
            && let Some((end, content)) = find_backtick_content(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end;
            continue;
        }

        // Link: [text](url) or [text][ref]
        if chars[i] == '['
            && let Some((end, link_text, url)) = parse_link(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let text_nodes = parse_inline(&link_text);
            nodes.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .children(text_nodes),
            );
            i = end;
            continue;
        }

        // Image: ![alt](url)
        if chars[i] == '!'
            && i + 1 < chars.len()
            && chars[i + 1] == '['
            && let Some((end, alt, url)) = parse_link(&chars, i + 1)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(
                Node::new(node::IMAGE)
                    .prop(prop::URL, url)
                    .prop(prop::ALT, alt),
            );
            i = end;
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current));
    }

    nodes
}

fn find_double_marker(chars: &[char], start: usize, marker: char) -> Option<(usize, String)> {
    let mut i = start;
    let mut content = String::new();

    while i + 1 < chars.len() {
        if chars[i] == marker && chars[i + 1] == marker {
            if !content.is_empty() {
                return Some((i, content));
            }
            return None;
        }
        content.push(chars[i]);
        i += 1;
    }
    None
}

fn find_single_marker(chars: &[char], start: usize, marker: char) -> Option<(usize, String)> {
    let mut i = start;
    let mut content = String::new();

    while i < chars.len() {
        if chars[i] == marker {
            // Don't match if this would form a double marker
            if i + 1 < chars.len() && chars[i + 1] == marker {
                content.push(chars[i]);
                i += 1;
                continue;
            }
            if !content.is_empty() {
                return Some((i, content));
            }
            return None;
        }
        content.push(chars[i]);
        i += 1;
    }
    None
}

fn find_backtick_content(chars: &[char], start: usize) -> Option<(usize, String)> {
    // Count opening backticks
    let mut backtick_count = 0;
    let mut i = start;
    while i < chars.len() && chars[i] == '`' {
        backtick_count += 1;
        i += 1;
    }

    // Find matching closing backticks
    let mut content = String::new();
    while i < chars.len() {
        if chars[i] == '`' {
            let mut closing_count = 0;
            let _close_start = i;
            while i < chars.len() && chars[i] == '`' {
                closing_count += 1;
                i += 1;
            }
            if closing_count == backtick_count {
                return Some((i, content.trim().to_string()));
            }
            // Not matching, add to content
            for _ in 0..closing_count {
                content.push('`');
            }
        } else {
            content.push(chars[i]);
            i += 1;
        }
    }
    None
}

fn parse_link(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    if chars[start] != '[' {
        return None;
    }

    // Find closing ]
    let mut i = start + 1;
    let mut link_text = String::new();

    while i < chars.len() && chars[i] != ']' {
        link_text.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() {
        return None;
    }

    i += 1; // Skip ]

    if i >= chars.len() {
        return None;
    }

    // Check for (url)
    if chars[i] == '(' {
        i += 1;
        let mut url = String::new();
        while i < chars.len() && chars[i] != ')' {
            url.push(chars[i]);
            i += 1;
        }
        if i < chars.len() {
            return Some((i + 1, link_text, url));
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
        let doc = parse_str("# Title\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_level2() {
        let doc = parse_str("## Subtitle\n");
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_paragraph() {
        let doc = parse_str("Hello world\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str("**bold**\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("*italic*\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_code() {
        let doc = parse_str("`code`\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("[click here](https://example.com)\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_aside() {
        let doc = parse_str("A> This is an aside.\n");
        let div = &doc.content.children[0];
        assert_eq!(div.kind.as_str(), node::DIV);
        assert_eq!(div.props.get_str("class"), Some("aside"));
    }

    #[test]
    fn test_parse_warning() {
        let doc = parse_str("W> This is a warning.\n");
        let div = &doc.content.children[0];
        assert_eq!(div.props.get_str("class"), Some("warning"));
    }

    #[test]
    fn test_parse_tip() {
        let doc = parse_str("T> This is a tip.\n");
        let div = &doc.content.children[0];
        assert_eq!(div.props.get_str("class"), Some("tip"));
    }

    #[test]
    fn test_parse_blockquote() {
        let doc = parse_str("> Quoted text\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::BLOCKQUOTE);
    }

    #[test]
    fn test_parse_unordered_list() {
        let doc = parse_str("- item1\n- item2\n");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_ordered_list() {
        let doc = parse_str("1. first\n2. second\n");
        let list = &doc.content.children[0];
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse_str("```\ncode here\n```\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }

    #[test]
    fn test_parse_code_block_with_language() {
        let doc = parse_str("```ruby\nputs 'hello'\n```\n");
        let code_block = &doc.content.children[0];
        assert_eq!(code_block.props.get_str(prop::LANGUAGE), Some("ruby"));
    }

    #[test]
    fn test_parse_scene_break() {
        let doc = parse_str("* * *\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::HORIZONTAL_RULE);
    }

    #[test]
    fn test_parse_image() {
        let doc = parse_str("![Alt text](image.png)\n");
        let para = &doc.content.children[0];
        let img = &para.children[0];
        assert_eq!(img.kind.as_str(), node::IMAGE);
        assert_eq!(img.props.get_str(prop::URL), Some("image.png"));
        assert_eq!(img.props.get_str(prop::ALT), Some("Alt text"));
    }
}
