//! txt2tags (t2t) reader for rescribe.
//!
//! Parses txt2tags markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse txt2tags markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse txt2tags markup with custom options.
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

            // Skip empty lines
            if line.trim().is_empty() {
                self.pos += 1;
                continue;
            }

            // Comment (% at start of line)
            if line.starts_with('%') {
                self.pos += 1;
                continue;
            }

            // Verbatim block (```)
            if line.trim() == "```" {
                nodes.push(self.parse_verbatim_block());
                continue;
            }

            // Raw block (""")
            if line.trim() == "\"\"\"" {
                nodes.push(self.parse_raw_block());
                continue;
            }

            // Heading = Title = or + Title +
            if let Some(node) = self.try_parse_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Horizontal rule (20+ dashes, equals, or underscores)
            if is_horizontal_rule(line) {
                nodes.push(Node::new(node::HORIZONTAL_RULE));
                self.pos += 1;
                continue;
            }

            // Quote (lines starting with TAB)
            if line.starts_with('\t') {
                nodes.push(self.parse_quote());
                continue;
            }

            // Unordered list (- item)
            if line.trim_start().starts_with("- ") {
                nodes.push(self.parse_list(false));
                continue;
            }

            // Ordered list (+ item)
            if line.trim_start().starts_with("+ ") {
                nodes.push(self.parse_list(true));
                continue;
            }

            // Table (| cell |)
            if line.trim_start().starts_with('|') {
                nodes.push(self.parse_table());
                continue;
            }

            // Regular paragraph
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn try_parse_heading(&self, line: &str) -> Option<Node> {
        let trimmed = line.trim();

        // Check for = or + delimited headings
        for (marker, numbered) in [('=', false), ('+', true)] {
            let level = trimmed.chars().take_while(|&c| c == marker).count();
            if level > 0 && level <= 5 {
                let end_marker_count = trimmed.chars().rev().take_while(|&c| c == marker).count();
                if end_marker_count >= level {
                    // Extract content between markers
                    let content_start = level;
                    let content_end = trimmed.len() - end_marker_count;
                    if content_start < content_end {
                        let content = trimmed[content_start..content_end].trim();
                        // Check for label [label-name]
                        let (text, _label) = if let Some(bracket_pos) = content.rfind('[') {
                            if content.ends_with(']') {
                                (
                                    content[..bracket_pos].trim(),
                                    Some(&content[bracket_pos + 1..content.len() - 1]),
                                )
                            } else {
                                (content, None)
                            }
                        } else {
                            (content, None)
                        };

                        let inline_nodes = parse_inline(text);
                        let mut heading = Node::new(node::HEADING)
                            .prop(prop::LEVEL, level as i64)
                            .children(inline_nodes);

                        if numbered {
                            heading = heading.prop("numbered", true);
                        }

                        return Some(heading);
                    }
                }
            }
        }
        None
    }

    fn parse_verbatim_block(&mut self) -> Node {
        let mut content = String::new();
        self.pos += 1; // Skip opening ```

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if line.trim() == "```" {
                self.pos += 1;
                break;
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(line);
            self.pos += 1;
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
    }

    fn parse_raw_block(&mut self) -> Node {
        let mut content = String::new();
        self.pos += 1; // Skip opening """

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if line.trim() == "\"\"\"" {
                self.pos += 1;
                break;
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(line);
            self.pos += 1;
        }

        Node::new(node::RAW_BLOCK).prop(prop::CONTENT, content)
    }

    fn parse_quote(&mut self) -> Node {
        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if !line.starts_with('\t') {
                break;
            }
            if !content.is_empty() {
                content.push(' ');
            }
            content.push_str(line[1..].trim());
            self.pos += 1;
        }

        let inline_nodes = parse_inline(&content);
        Node::new(node::BLOCKQUOTE)
            .children(vec![Node::new(node::PARAGRAPH).children(inline_nodes)])
    }

    fn parse_list(&mut self, ordered: bool) -> Node {
        let mut items = Vec::new();
        let marker = if ordered { "+ " } else { "- " };

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            if !trimmed.starts_with(marker) {
                break;
            }

            let content = &trimmed[2..];
            let inline_nodes = parse_inline(content);
            let para = Node::new(node::PARAGRAPH).children(inline_nodes);
            items.push(Node::new(node::LIST_ITEM).children(vec![para]));
            self.pos += 1;
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items)
    }

    fn parse_table(&mut self) -> Node {
        let mut rows = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim();

            if !trimmed.starts_with('|') {
                break;
            }

            let is_header = trimmed.starts_with("||");
            let row_content = if is_header {
                &trimmed[2..]
            } else {
                &trimmed[1..]
            };

            let mut cells = Vec::new();
            for cell_text in row_content.split('|') {
                let cell_text = cell_text.trim();
                if cell_text.is_empty() && cells.is_empty() {
                    continue; // Skip empty leading cell
                }
                if cell_text.is_empty() {
                    continue; // Skip empty cells
                }
                let inline_nodes = parse_inline(cell_text);
                let cell_node = if is_header {
                    Node::new(node::TABLE_HEADER).children(inline_nodes)
                } else {
                    Node::new(node::TABLE_CELL).children(inline_nodes)
                };
                cells.push(cell_node);
            }

            if !cells.is_empty() {
                rows.push(Node::new(node::TABLE_ROW).children(cells));
            }
            self.pos += 1;
        }

        Node::new(node::TABLE).children(rows)
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut text = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            // End paragraph at empty line or block element
            if line.trim().is_empty()
                || line.starts_with('%')
                || line.trim() == "```"
                || line.trim() == "\"\"\""
                || self.try_parse_heading(line).is_some()
                || is_horizontal_rule(line)
                || line.starts_with('\t')
                || line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("+ ")
                || line.trim_start().starts_with('|')
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

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() >= 20 {
        let first_char = trimmed.chars().next().unwrap_or(' ');
        if first_char == '-' || first_char == '=' || first_char == '_' {
            return trimmed.chars().all(|c| c == first_char);
        }
    }
    false
}

fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Bold **text**
        if chars[i] == '*'
            && i + 1 < chars.len()
            && chars[i + 1] == '*'
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '*')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            nodes.push(Node::new(node::STRONG).children(inner));
            i = end + 2;
            continue;
        }

        // Italic //text//
        if chars[i] == '/'
            && i + 1 < chars.len()
            && chars[i + 1] == '/'
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '/')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            nodes.push(Node::new(node::EMPHASIS).children(inner));
            i = end + 2;
            continue;
        }

        // Underline __text__
        if chars[i] == '_'
            && i + 1 < chars.len()
            && chars[i + 1] == '_'
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '_')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            nodes.push(Node::new(node::UNDERLINE).children(inner));
            i = end + 2;
            continue;
        }

        // Strikethrough --text--
        if chars[i] == '-'
            && i + 1 < chars.len()
            && chars[i + 1] == '-'
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '-')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            nodes.push(Node::new(node::STRIKEOUT).children(inner));
            i = end + 2;
            continue;
        }

        // Monospace ``text``
        if chars[i] == '`'
            && i + 1 < chars.len()
            && chars[i + 1] == '`'
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '`')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end + 2;
            continue;
        }

        // Link [label url] or image [filename.ext]
        if chars[i] == '['
            && let Some((end, label, url)) = parse_link_or_image(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            if is_image_url(&url) {
                nodes.push(Node::new(node::IMAGE).prop(prop::URL, url));
            } else {
                let text_node = Node::new(node::TEXT).prop(prop::CONTENT, label);
                nodes.push(
                    Node::new(node::LINK)
                        .prop(prop::URL, url)
                        .children(vec![text_node]),
                );
            }
            i = end;
            continue;
        }

        // Auto-detect URLs
        if (chars[i] == 'h' || chars[i] == 'H')
            && let Some((end, url)) = try_parse_url(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let text_node = Node::new(node::TEXT).prop(prop::CONTENT, url.clone());
            nodes.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .children(vec![text_node]),
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

fn find_double_closing(chars: &[char], start: usize, marker: char) -> Option<(usize, String)> {
    let mut i = start;
    let mut content = String::new();

    while i + 1 < chars.len() {
        if chars[i] == marker && chars[i + 1] == marker {
            return Some((i, content));
        }
        content.push(chars[i]);
        i += 1;
    }
    None
}

fn parse_link_or_image(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    // [label url] or [filename.ext]
    if start >= chars.len() || chars[start] != '[' {
        return None;
    }

    let mut i = start + 1;
    let mut content = String::new();

    while i < chars.len() && chars[i] != ']' {
        content.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() {
        return None;
    }

    let content = content.trim();

    // Check if it's [label url] format
    if let Some(space_pos) = content.rfind(' ') {
        let label = content[..space_pos].trim();
        let url = content[space_pos + 1..].trim();
        // Ensure URL looks like a URL
        if url.contains('.') || url.starts_with('#') || url.starts_with("http") {
            return Some((i + 1, label.to_string(), url.to_string()));
        }
    }

    // Single item - could be URL or image
    if content.contains('.') {
        return Some((i + 1, content.to_string(), content.to_string()));
    }

    None
}

fn is_image_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".svg")
        || lower.ends_with(".webp")
}

fn try_parse_url(chars: &[char], start: usize) -> Option<(usize, String)> {
    let rest: String = chars[start..].iter().collect();
    if rest.starts_with("http://")
        || rest.starts_with("https://")
        || rest.starts_with("HTTP://")
        || rest.starts_with("HTTPS://")
    {
        let mut end = start;
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        let url: String = chars[start..end].iter().collect();
        return Some((end, url));
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
        let doc = parse_str("= Title =\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_level2() {
        let doc = parse_str("== Subtitle ==\n");
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_numbered_heading() {
        let doc = parse_str("+ Numbered +\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(
            doc.content.children[0].props.get_bool("numbered"),
            Some(true)
        );
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
        let doc = parse_str("//italic//\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_underline() {
        let doc = parse_str("__underline__\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::UNDERLINE);
    }

    #[test]
    fn test_parse_strikethrough() {
        let doc = parse_str("--strike--\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::STRIKEOUT);
    }

    #[test]
    fn test_parse_monospace() {
        let doc = parse_str("``code``\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_unordered_list() {
        let doc = parse_str("- item1\n- item2\n");
        assert_eq!(doc.content.children.len(), 1);
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_ordered_list() {
        let doc = parse_str("+ first\n+ second\n");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    fn test_parse_verbatim_block() {
        let doc = parse_str("```\ncode here\n```\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(
            doc.content.children[0].props.get_str(prop::CONTENT),
            Some("code here")
        );
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("[click here http://example.com]\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("http://example.com"));
    }

    #[test]
    fn test_parse_quote() {
        let doc = parse_str("\tquoted text\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::BLOCKQUOTE);
    }

    #[test]
    fn test_skip_comments() {
        let doc = parse_str("% comment\ntext\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }
}
