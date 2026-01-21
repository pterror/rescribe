//! Muse markup reader for rescribe.
//!
//! Parses Emacs Muse markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Muse markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Muse markup with custom options.
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

            // Example block <example>...</example>
            if line.trim_start().starts_with("<example>") {
                nodes.push(self.parse_example_block());
                continue;
            }

            // Verse block <verse>...</verse>
            if line.trim_start().starts_with("<verse>") {
                nodes.push(self.parse_verse_block());
                continue;
            }

            // Quote block <quote>...</quote>
            if line.trim_start().starts_with("<quote>") {
                nodes.push(self.parse_quote_block());
                continue;
            }

            // Heading * to *****
            if let Some(node) = self.try_parse_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Horizontal rule (4+ dashes)
            if line.trim().starts_with("----") {
                nodes.push(Node::new(node::HORIZONTAL_RULE));
                self.pos += 1;
                continue;
            }

            // Unordered list (space before -)
            if line.starts_with(" - ") || line.starts_with("  - ") {
                nodes.push(self.parse_unordered_list());
                continue;
            }

            // Ordered list (space before number)
            if self.is_ordered_list_item(line) {
                nodes.push(self.parse_ordered_list());
                continue;
            }

            // Definition list (term ::)
            if line.contains(" :: ") {
                nodes.push(self.parse_definition_list());
                continue;
            }

            // Indented code block
            if line.starts_with("  ") && !line.trim().is_empty() {
                nodes.push(self.parse_indented_code());
                continue;
            }

            // Regular paragraph
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn try_parse_heading(&self, line: &str) -> Option<Node> {
        // Muse headings: * to *****
        let level = line.chars().take_while(|&c| c == '*').count();

        if level > 0 && level <= 5 && line.len() > level && line.chars().nth(level) == Some(' ') {
            let content = line[level + 1..].trim();
            let inline_nodes = parse_inline(content);

            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level as i64)
                    .children(inline_nodes),
            );
        }
        None
    }

    fn parse_example_block(&mut self) -> Node {
        let mut content = String::new();
        let first_line = self.lines[self.pos];

        // Get content after <example> on same line
        if let Some(pos) = first_line.find("<example>") {
            let after = &first_line[pos + 9..];
            if let Some(end) = after.find("</example>") {
                content.push_str(&after[..end]);
                self.pos += 1;
                return Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
            }
            if !after.trim().is_empty() {
                content.push_str(after.trim());
                content.push('\n');
            }
        }
        self.pos += 1;

        // Multi-line
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if line.contains("</example>") {
                if let Some(pos) = line.find("</example>") {
                    content.push_str(&line[..pos]);
                }
                self.pos += 1;
                break;
            }
            content.push_str(line);
            content.push('\n');
            self.pos += 1;
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end().to_string())
    }

    fn parse_verse_block(&mut self) -> Node {
        let mut content = String::new();
        let first_line = self.lines[self.pos];

        if let Some(pos) = first_line.find("<verse>") {
            let after = &first_line[pos + 7..];
            if let Some(end) = after.find("</verse>") {
                content.push_str(&after[..end]);
                self.pos += 1;
                let inline = parse_inline(&content);
                return Node::new(node::BLOCKQUOTE)
                    .children(vec![Node::new(node::PARAGRAPH).children(inline)]);
            }
            if !after.trim().is_empty() {
                content.push_str(after.trim());
                content.push('\n');
            }
        }
        self.pos += 1;

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if line.contains("</verse>") {
                if let Some(pos) = line.find("</verse>") {
                    content.push_str(&line[..pos]);
                }
                self.pos += 1;
                break;
            }
            content.push_str(line);
            content.push('\n');
            self.pos += 1;
        }

        let inline = parse_inline(content.trim_end());
        Node::new(node::BLOCKQUOTE).children(vec![Node::new(node::PARAGRAPH).children(inline)])
    }

    fn parse_quote_block(&mut self) -> Node {
        let mut content = String::new();
        let first_line = self.lines[self.pos];

        if let Some(pos) = first_line.find("<quote>") {
            let after = &first_line[pos + 7..];
            if let Some(end) = after.find("</quote>") {
                content.push_str(&after[..end]);
                self.pos += 1;
                let inline = parse_inline(&content);
                return Node::new(node::BLOCKQUOTE)
                    .children(vec![Node::new(node::PARAGRAPH).children(inline)]);
            }
            if !after.trim().is_empty() {
                content.push_str(after.trim());
                content.push('\n');
            }
        }
        self.pos += 1;

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if line.contains("</quote>") {
                if let Some(pos) = line.find("</quote>") {
                    content.push_str(&line[..pos]);
                }
                self.pos += 1;
                break;
            }
            content.push_str(line);
            content.push('\n');
            self.pos += 1;
        }

        let inline = parse_inline(content.trim_end());
        Node::new(node::BLOCKQUOTE).children(vec![Node::new(node::PARAGRAPH).children(inline)])
    }

    fn is_ordered_list_item(&self, line: &str) -> bool {
        if line.starts_with(' ') {
            let trimmed = line.trim_start();
            if let Some(dot_pos) = trimmed.find(". ") {
                let num = &trimmed[..dot_pos];
                return num.chars().all(|c| c.is_ascii_digit());
            }
        }
        false
    }

    fn parse_unordered_list(&mut self) -> Node {
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if !line.starts_with(" - ") && !line.starts_with("  - ") {
                break;
            }

            let content = line.trim_start()[2..].trim();
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
            if let Some(dot_pos) = trimmed.find(". ") {
                let content = &trimmed[dot_pos + 2..];
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

    fn parse_definition_list(&mut self) -> Node {
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if !line.contains(" :: ") {
                break;
            }

            if let Some(sep_pos) = line.find(" :: ") {
                let term = &line[..sep_pos];
                let desc = &line[sep_pos + 4..];

                let term_node =
                    Node::new(node::DEFINITION_TERM).children(parse_inline(term.trim()));
                let desc_node = Node::new(node::DEFINITION_DESC).children(vec![
                    Node::new(node::PARAGRAPH).children(parse_inline(desc.trim())),
                ]);

                items.push(term_node);
                items.push(desc_node);
            }
            self.pos += 1;
        }

        Node::new(node::DEFINITION_LIST).children(items)
    }

    fn parse_indented_code(&mut self) -> Node {
        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if !line.starts_with("  ") && !line.trim().is_empty() {
                break;
            }

            if let Some(stripped) = line.strip_prefix("  ") {
                content.push_str(stripped);
                content.push('\n');
            }
            self.pos += 1;
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end().to_string())
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut text = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if line.trim().is_empty() {
                break;
            }

            // Check for block elements - but not **bold**
            let is_heading = line.chars().take_while(|&c| c == '*').count() > 0
                && line.chars().find(|&c| c != '*') == Some(' ');
            if is_heading
                || line.starts_with("----")
                || line.starts_with(" - ")
                || (line.starts_with("  ") && !line.trim().is_empty())
                || line.contains(" :: ")
                || line.trim_start().starts_with('<')
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
        // Inline code =...=
        if chars[i] == '='
            && i + 1 < chars.len()
            && let Some((end, content)) = find_closing(&chars, i + 1, '=')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end + 1;
            continue;
        }

        // Bold **...** (doubled asterisks)
        if chars[i] == '*'
            && i + 1 < chars.len()
            && chars[i + 1] == '*'
            && i + 2 < chars.len()
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

        // Emphasis *...*
        if chars[i] == '*'
            && i + 1 < chars.len()
            && chars[i + 1] != '*'
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && let Some((end, content)) = find_closing(&chars, i + 1, '*')
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

        // Link [[url][text]] or [[url]]
        if chars[i] == '['
            && i + 1 < chars.len()
            && chars[i + 1] == '['
            && let Some((end, url, link_text)) = parse_muse_link(&chars, i)
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

fn parse_muse_link(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    // [[url][text]] or [[url]]
    if start + 1 >= chars.len() || chars[start] != '[' || chars[start + 1] != '[' {
        return None;
    }

    let mut i = start + 2;
    let mut url = String::new();

    // Collect URL until ] or [
    while i < chars.len() && chars[i] != ']' && chars[i] != '[' {
        url.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() {
        return None;
    }

    // Check for link text
    if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == '[' {
        i += 2;
        let mut text = String::new();
        while i < chars.len() && chars[i] != ']' {
            text.push(chars[i]);
            i += 1;
        }
        if i + 1 < chars.len() && chars[i] == ']' && chars[i + 1] == ']' {
            return Some((i + 2, url, text));
        }
    } else if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
        // No link text, use URL
        return Some((i + 2, url.clone(), url));
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
        let doc = parse_str("* Title\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_levels() {
        let doc = parse_str("** Level 2\n*** Level 3\n");
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
        let doc = parse_str("**bold**\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_emphasis() {
        let doc = parse_str("text with *emphasis*\n");
        let para = &doc.content.children[0];
        // Should have text, emphasis, potentially more
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
    }

    #[test]
    fn test_parse_code() {
        let doc = parse_str("=code=\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("[[https://example.com][Example]]\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_unordered_list() {
        let doc = parse_str(" - item1\n - item2\n");
        assert_eq!(doc.content.children.len(), 1);
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_example_block() {
        let doc = parse_str("<example>\ncode here\n</example>\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }
}
