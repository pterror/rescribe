//! VimWiki reader for rescribe.
//!
//! Parses VimWiki markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse VimWiki markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse VimWiki markup with custom options.
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

            // Heading = Title = to ====== Title ======
            if let Some(node) = self.try_parse_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Horizontal rule (4+ dashes)
            if line.trim().chars().all(|c| c == '-') && line.trim().len() >= 4 {
                nodes.push(Node::new(node::HORIZONTAL_RULE));
                self.pos += 1;
                continue;
            }

            // Preformatted block {{{ ... }}}
            if line.trim_start().starts_with("{{{") {
                nodes.push(self.parse_preformatted());
                continue;
            }

            // Unordered list (* or -)
            let trimmed = line.trim_start();
            if (trimmed.starts_with("* ") && !trimmed.starts_with("**"))
                || trimmed.starts_with("- ")
            {
                nodes.push(self.parse_list(false));
                continue;
            }

            // Ordered list (1. or a))
            if self.is_ordered_list_item(line) {
                nodes.push(self.parse_list(true));
                continue;
            }

            // Blockquote (lines starting with >)
            if trimmed.starts_with("> ") || trimmed == ">" {
                nodes.push(self.parse_blockquote());
                continue;
            }

            // Table
            if trimmed.starts_with('|') {
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

        // Count leading =
        let level = trimmed.chars().take_while(|&c| c == '=').count();
        if level == 0 || level > 6 {
            return None;
        }

        // Check for matching trailing =
        let trailing = trimmed.chars().rev().take_while(|&c| c == '=').count();
        if trailing < level {
            return None;
        }

        // Extract content
        let content = &trimmed[level..trimmed.len() - trailing].trim();
        if content.is_empty() {
            return None;
        }

        let inline_nodes = parse_inline(content);
        Some(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level as i64)
                .children(inline_nodes),
        )
    }

    fn parse_preformatted(&mut self) -> Node {
        let mut content = String::new();
        let first_line = self.lines[self.pos].trim_start();

        // Check for language after {{{
        let lang = if first_line.len() > 3 {
            let after = first_line[3..].trim();
            if !after.is_empty() {
                Some(after.to_string())
            } else {
                None
            }
        } else {
            None
        };

        self.pos += 1;

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if line.trim() == "}}}" {
                self.pos += 1;
                break;
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(line);
            self.pos += 1;
        }

        let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
        if let Some(l) = lang {
            node = node.prop(prop::LANGUAGE, l);
        }
        node
    }

    fn is_ordered_list_item(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        // Check for 1. or a) style
        if let Some(dot_pos) = trimmed.find(". ") {
            let prefix = &trimmed[..dot_pos];
            return prefix.chars().all(|c| c.is_ascii_digit());
        }
        if let Some(paren_pos) = trimmed.find(") ") {
            let prefix = &trimmed[..paren_pos];
            return prefix.len() == 1 && prefix.chars().all(|c| c.is_ascii_lowercase());
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

            // Check if still in list at same or deeper level
            if indent < base_indent {
                break;
            }

            let is_bullet = (trimmed.starts_with("* ") && !trimmed.starts_with("**"))
                || trimmed.starts_with("- ");
            let is_numbered = self.is_ordered_list_item(line);

            if !is_bullet && !is_numbered {
                break;
            }

            // Extract item content
            let content = if is_bullet {
                &trimmed[2..]
            } else if let Some(pos) = trimmed.find(". ") {
                &trimmed[pos + 2..]
            } else if let Some(pos) = trimmed.find(") ") {
                &trimmed[pos + 2..]
            } else {
                break;
            };

            // Check for checkbox [ ] or [X]
            let (checkbox_state, actual_content) = if let Some(rest) = content.strip_prefix("[ ] ")
            {
                (Some(false), rest)
            } else if let Some(rest) = content
                .strip_prefix("[X] ")
                .or_else(|| content.strip_prefix("[x] "))
            {
                (Some(true), rest)
            } else {
                (None, content)
            };

            let inline_nodes = parse_inline(actual_content);
            let para = Node::new(node::PARAGRAPH).children(inline_nodes);
            let mut item = Node::new(node::LIST_ITEM).children(vec![para]);

            if let Some(checked) = checkbox_state {
                item = item.prop("checked", checked);
            }

            items.push(item);
            self.pos += 1;
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items)
    }

    fn parse_blockquote(&mut self) -> Node {
        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            if !trimmed.starts_with('>') {
                break;
            }

            let text = if let Some(rest) = trimmed.strip_prefix("> ") {
                rest
            } else if trimmed == ">" {
                ""
            } else {
                break;
            };

            if !content.is_empty() {
                content.push(' ');
            }
            content.push_str(text);
            self.pos += 1;
        }

        let inline_nodes = parse_inline(&content);
        Node::new(node::BLOCKQUOTE)
            .children(vec![Node::new(node::PARAGRAPH).children(inline_nodes)])
    }

    fn parse_table(&mut self) -> Node {
        let mut rows = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim();

            if !trimmed.starts_with('|') {
                break;
            }

            // Check for separator row (|---|---|)
            if trimmed.contains("---") {
                self.pos += 1;
                continue;
            }

            let mut cells = Vec::new();
            let parts: Vec<&str> = trimmed.split('|').collect();

            for part in &parts[1..] {
                // Skip empty trailing part
                if part.trim().is_empty() && parts.last() == Some(part) {
                    continue;
                }
                let inline_nodes = parse_inline(part.trim());
                cells.push(Node::new(node::TABLE_CELL).children(inline_nodes));
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

            if line.trim().is_empty() {
                break;
            }

            // Check for block elements
            if self.try_parse_heading(line).is_some()
                || (line.trim().chars().all(|c| c == '-') && line.trim().len() >= 4)
                || line.trim_start().starts_with("{{{")
                || line.trim_start().starts_with("* ")
                || line.trim_start().starts_with("- ")
                || self.is_ordered_list_item(line)
                || line.trim_start().starts_with("> ")
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

fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Bold *text*
        if chars[i] == '*'
            && i + 1 < chars.len()
            && chars[i + 1] != '*'
            && chars[i + 1] != ' '
            && let Some((end, content)) = find_closing(&chars, i + 1, '*')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            nodes.push(Node::new(node::STRONG).children(inner));
            i = end + 1;
            continue;
        }

        // Italic _text_
        if chars[i] == '_'
            && i + 1 < chars.len()
            && chars[i + 1] != ' '
            && let Some((end, content)) = find_closing(&chars, i + 1, '_')
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

        // Strikethrough ~~text~~
        if chars[i] == '~'
            && i + 1 < chars.len()
            && chars[i + 1] == '~'
            && let Some((end, content)) = find_double_closing(&chars, i + 2, '~')
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

        // Code `text`
        if chars[i] == '`'
            && let Some((end, content)) = find_closing(&chars, i + 1, '`')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end + 1;
            continue;
        }

        // Wiki link [[link]] or [[link|description]]
        if chars[i] == '['
            && i + 1 < chars.len()
            && chars[i + 1] == '['
            && let Some((end, url, label)) = parse_wiki_link(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let text_node = Node::new(node::TEXT).prop(prop::CONTENT, label);
            nodes.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .children(vec![text_node]),
            );
            i = end;
            continue;
        }

        // Image {{image.png}} or {{image.png|alt}}
        if chars[i] == '{'
            && i + 1 < chars.len()
            && chars[i + 1] == '{'
            && let Some((end, url, alt)) = parse_image(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let mut img = Node::new(node::IMAGE).prop(prop::URL, url);
            if let Some(a) = alt {
                img = img.prop(prop::ALT, a);
            }
            nodes.push(img);
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
        if chars[i] == marker
            && (i + 1 >= chars.len() || chars[i + 1] == ' ' || !chars[i + 1].is_alphanumeric())
        {
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

fn parse_wiki_link(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    // [[link]] or [[link|description]]
    if start + 1 >= chars.len() || chars[start] != '[' || chars[start + 1] != '[' {
        return None;
    }

    let mut i = start + 2;
    let mut content = String::new();

    while i < chars.len() {
        if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
            let (url, label) = if let Some(pipe_pos) = content.find('|') {
                (
                    content[..pipe_pos].to_string(),
                    content[pipe_pos + 1..].to_string(),
                )
            } else {
                (content.clone(), content)
            };
            return Some((i + 2, url, label));
        }
        content.push(chars[i]);
        i += 1;
    }
    None
}

fn parse_image(chars: &[char], start: usize) -> Option<(usize, String, Option<String>)> {
    // {{image.png}} or {{image.png|alt}}
    if start + 1 >= chars.len() || chars[start] != '{' || chars[start + 1] != '{' {
        return None;
    }

    let mut i = start + 2;
    let mut content = String::new();

    while i < chars.len() {
        if chars[i] == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
            let (url, alt) = if let Some(pipe_pos) = content.find('|') {
                (
                    content[..pipe_pos].to_string(),
                    Some(content[pipe_pos + 1..].to_string()),
                )
            } else {
                (content, None)
            };
            return Some((i + 2, url, alt));
        }
        content.push(chars[i]);
        i += 1;
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
    fn test_parse_paragraph() {
        let doc = parse_str("Hello world\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str("*bold*\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("_italic_\n");
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
    fn test_parse_wiki_link() {
        let doc = parse_str("[[MyPage]]\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("MyPage"));
    }

    #[test]
    fn test_parse_wiki_link_with_description() {
        let doc = parse_str("[[MyPage|click here]]\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("MyPage"));
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
    fn test_parse_ordered_list() {
        let doc = parse_str("1. first\n2. second\n");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    fn test_parse_preformatted() {
        let doc = parse_str("{{{\ncode here\n}}}\n");
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }

    #[test]
    fn test_parse_checkbox() {
        let doc = parse_str("* [ ] unchecked\n* [X] checked\n");
        let list = &doc.content.children[0];
        assert_eq!(list.children[0].props.get_bool("checked"), Some(false));
        assert_eq!(list.children[1].props.get_bool("checked"), Some(true));
    }
}
