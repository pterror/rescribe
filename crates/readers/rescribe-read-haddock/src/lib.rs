//! Haddock markup reader for rescribe.
//!
//! Parses Haddock documentation markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Haddock markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Haddock markup with custom options.
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

            // Code block (indented with > or @, but not inline @code@)
            if line.starts_with("> ") || (line.starts_with("@ ") && !line.contains("@@")) {
                nodes.push(self.parse_code_block());
                continue;
            }

            // Heading (= to ====)
            if let Some(node) = self.try_parse_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Definition list [term]
            if line.trim_start().starts_with('[')
                && let Some(node) = self.parse_definition_list()
            {
                nodes.push(node);
                continue;
            }

            // Unordered list *
            if line.trim_start().starts_with("* ") {
                nodes.push(self.parse_unordered_list());
                continue;
            }

            // Ordered list (1)
            if self.is_ordered_list_item(line) {
                nodes.push(self.parse_ordered_list());
                continue;
            }

            // Regular paragraph
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn try_parse_heading(&self, line: &str) -> Option<Node> {
        let trimmed = line.trim_start();

        // Count leading = signs
        let level = trimmed.chars().take_while(|&c| c == '=').count();

        if level > 0 && level <= 6 {
            let rest = trimmed[level..].trim();
            // Remove trailing = if present
            let content = rest.trim_end_matches('=').trim();
            let inline_nodes = parse_inline(content);

            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level as i64)
                    .children(inline_nodes),
            );
        }
        None
    }

    fn parse_code_block(&mut self) -> Node {
        let mut content = String::new();
        let marker = self.lines[self.pos].chars().next().unwrap();
        let marker_with_space = format!("{} ", marker);

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if !line.starts_with(&marker_with_space) && !line.trim().is_empty() {
                break;
            }

            if line.starts_with(&marker_with_space) {
                // Remove the marker and space
                let code_line = &line[2..];
                content.push_str(code_line);
                content.push('\n');
            }
            self.pos += 1;
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end().to_string())
    }

    fn is_ordered_list_item(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        if trimmed.starts_with('(')
            && let Some(close) = trimmed.find(')')
        {
            let num = &trimmed[1..close];
            return num.chars().all(|c| c.is_ascii_digit());
        }
        false
    }

    fn parse_unordered_list(&mut self) -> Node {
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            if !trimmed.starts_with("* ") {
                break;
            }

            let content = trimmed[2..].trim();
            let inline_nodes = parse_inline(content);
            let para = Node::new(node::PARAGRAPH).children(inline_nodes);
            items.push(Node::new(node::LIST_ITEM).children(vec![para]));
            self.pos += 1;
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, false)
            .children(items)
    }

    fn parse_ordered_list(&mut self) -> Node {
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if !self.is_ordered_list_item(line) {
                break;
            }

            let trimmed = line.trim_start();
            // Find the closing ) and get content after it
            if let Some(close) = trimmed.find(')') {
                let content = trimmed[close + 1..].trim();
                let inline_nodes = parse_inline(content);
                let para = Node::new(node::PARAGRAPH).children(inline_nodes);
                items.push(Node::new(node::LIST_ITEM).children(vec![para]));
            }
            self.pos += 1;
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, true)
            .children(items)
    }

    fn parse_definition_list(&mut self) -> Option<Node> {
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            if !trimmed.starts_with('[') {
                break;
            }

            // Find closing bracket
            if let Some(close) = trimmed.find(']') {
                let term = &trimmed[1..close];
                let desc = trimmed[close + 1..].trim();

                let term_node = Node::new(node::DEFINITION_TERM).children(parse_inline(term));
                let desc_node = Node::new(node::DEFINITION_DESC).children(vec![
                    Node::new(node::PARAGRAPH).children(parse_inline(desc)),
                ]);

                items.push(term_node);
                items.push(desc_node);
            }
            self.pos += 1;
        }

        if items.is_empty() {
            None
        } else {
            Some(Node::new(node::DEFINITION_LIST).children(items))
        }
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
            if trimmed.starts_with('=')
                || trimmed.starts_with("* ")
                || trimmed.starts_with('[')
                || trimmed.starts_with("> ")
                || (trimmed.starts_with("@ ") && !trimmed.contains("@@"))
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

fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Inline code @...@
        if chars[i] == '@'
            && i + 1 < chars.len()
            && let Some((end, content)) = find_closing(&chars, i + 1, '@')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end + 1;
            continue;
        }

        // Bold __...__
        if chars[i] == '_'
            && i + 1 < chars.len()
            && chars[i + 1] == '_'
            && i + 2 < chars.len()
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '_')
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

        // Italic /.../ (but not //)
        if chars[i] == '/'
            && i + 1 < chars.len()
            && chars[i + 1] != '/'
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && let Some((end, content)) = find_closing(&chars, i + 1, '/')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            nodes.push(Node::new(node::EMPHASIS).children(inner));
            i = end + 1;
            continue;
        }

        // Identifier reference '...'
        if chars[i] == '\''
            && i + 1 < chars.len()
            && let Some((end, content)) = find_closing(&chars, i + 1, '\'')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end + 1;
            continue;
        }

        // Link "text"<url> or raw URL <url>
        if chars[i] == '"'
            && let Some((end, link_text, url)) = parse_haddock_link(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let text_node = Node::new(node::TEXT).prop(prop::CONTENT, link_text);
            nodes.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .children(vec![text_node]),
            );
            i = end;
            continue;
        }

        // Raw URL <url>
        if chars[i] == '<'
            && let Some((end, url)) = parse_raw_url(&chars, i)
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

fn find_closing(chars: &[char], start: usize, marker: char) -> Option<(usize, String)> {
    let mut i = start;
    let mut content = String::new();

    while i < chars.len() {
        if chars[i] == marker {
            return Some((i, content));
        }
        content.push(chars[i]);
        i += 1;
    }
    None
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

fn parse_haddock_link(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    // "text"<url>
    if chars[start] != '"' {
        return None;
    }

    let mut i = start + 1;
    let mut link_text = String::new();

    // Find closing "
    while i < chars.len() && chars[i] != '"' {
        link_text.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() || chars[i] != '"' {
        return None;
    }
    i += 1; // skip "

    // Must be followed by <
    if i >= chars.len() || chars[i] != '<' {
        return None;
    }
    i += 1; // skip <

    // Collect URL until >
    let mut url = String::new();
    while i < chars.len() && chars[i] != '>' {
        url.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() || chars[i] != '>' {
        return None;
    }
    i += 1; // skip >

    Some((i, link_text, url))
}

fn parse_raw_url(chars: &[char], start: usize) -> Option<(usize, String)> {
    // <url>
    if chars[start] != '<' {
        return None;
    }

    let mut i = start + 1;
    let mut url = String::new();

    while i < chars.len() && chars[i] != '>' {
        url.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() || chars[i] != '>' {
        return None;
    }
    i += 1;

    // Basic URL validation
    if url.starts_with("http://") || url.starts_with("https://") || url.contains('@') {
        Some((i, url))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_heading() {
        let doc = parse_str("= Title\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_levels() {
        let doc = parse_str("== Level 2\n=== Level 3\n");
        assert_eq!(doc.content.children.len(), 2);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(2));
        assert_eq!(doc.content.children[1].props.get_int(prop::LEVEL), Some(3));
    }

    #[test]
    fn test_parse_paragraph() {
        let doc = parse_str("Hello world\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str("__bold__\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("/italic/\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_code() {
        let doc = parse_str("@code@\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("\"Example\"<https://example.com>\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_unordered_list() {
        let doc = parse_str("* item1\n* item2\n");
        assert_eq!(doc.content.children.len(), 1);
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse_str("> code here\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }
}
