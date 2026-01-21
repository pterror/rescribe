//! Textile markup reader for rescribe.
//!
//! Parses Textile markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Textile markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Textile markup with custom options.
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

            // Block code bc. or bc..
            if line.starts_with("bc.") {
                nodes.push(self.parse_code_block());
                continue;
            }

            // Blockquote bq.
            if line.starts_with("bq.") {
                nodes.push(self.parse_blockquote());
                continue;
            }

            // Pre block pre.
            if line.starts_with("pre.") {
                nodes.push(self.parse_pre_block());
                continue;
            }

            // Heading h1. to h6.
            if let Some(node) = self.try_parse_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Table
            if line.trim_start().starts_with('|') {
                nodes.push(self.parse_table());
                continue;
            }

            // List
            if line.trim_start().starts_with("* ")
                || line.trim_start().starts_with("# ")
                || line.trim_start().starts_with("** ")
                || line.trim_start().starts_with("## ")
            {
                nodes.push(self.parse_list());
                continue;
            }

            // Regular paragraph p. or just text
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn try_parse_heading(&self, line: &str) -> Option<Node> {
        for level in 1..=6 {
            let prefix = format!("h{}.", level);
            if line.starts_with(&prefix) {
                let content = line[prefix.len()..].trim();
                let inline_nodes = parse_inline(content);
                return Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, level as i64)
                        .children(inline_nodes),
                );
            }
        }
        None
    }

    fn parse_code_block(&mut self) -> Node {
        let first_line = self.lines[self.pos];
        let extended = first_line.starts_with("bc..");

        let content_start = if extended { 4 } else { 3 };
        let mut content = String::new();

        // Get content from first line
        let first_content = first_line[content_start..].trim();
        if !first_content.is_empty() {
            content.push_str(first_content);
            content.push('\n');
        }
        self.pos += 1;

        if extended {
            // Extended block continues until blank line
            while self.pos < self.lines.len() {
                let line = self.lines[self.pos];
                if line.trim().is_empty() {
                    break;
                }
                content.push_str(line);
                content.push('\n');
                self.pos += 1;
            }
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end().to_string())
    }

    fn parse_pre_block(&mut self) -> Node {
        let first_line = self.lines[self.pos];
        let extended = first_line.starts_with("pre..");

        let content_start = if extended { 5 } else { 4 };
        let mut content = String::new();

        let first_content = first_line[content_start..].trim();
        if !first_content.is_empty() {
            content.push_str(first_content);
            content.push('\n');
        }
        self.pos += 1;

        if extended {
            while self.pos < self.lines.len() {
                let line = self.lines[self.pos];
                if line.trim().is_empty() {
                    break;
                }
                content.push_str(line);
                content.push('\n');
                self.pos += 1;
            }
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end().to_string())
    }

    fn parse_blockquote(&mut self) -> Node {
        let first_line = self.lines[self.pos];
        let extended = first_line.starts_with("bq..");

        let content_start = if extended { 4 } else { 3 };
        let mut text = String::new();

        let first_content = first_line[content_start..].trim();
        if !first_content.is_empty() {
            text.push_str(first_content);
        }
        self.pos += 1;

        if extended {
            while self.pos < self.lines.len() {
                let line = self.lines[self.pos];
                if line.trim().is_empty() {
                    break;
                }
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(line.trim());
                self.pos += 1;
            }
        }

        let inline_nodes = parse_inline(&text);
        let para = Node::new(node::PARAGRAPH).children(inline_nodes);
        Node::new(node::BLOCKQUOTE).children(vec![para])
    }

    fn parse_table(&mut self) -> Node {
        let mut rows = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if !line.trim_start().starts_with('|') {
                break;
            }

            let row = self.parse_table_row(line);
            rows.push(row);
            self.pos += 1;
        }

        Node::new(node::TABLE).children(rows)
    }

    fn parse_table_row(&self, line: &str) -> Node {
        let mut cells = Vec::new();
        let trimmed = line.trim();

        // Remove leading/trailing |
        let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
        let parts: Vec<&str> = inner.split('|').collect();

        for part in parts {
            let part = part.trim();
            let is_header = part.starts_with("_.");
            let cell_content = if is_header { part[2..].trim() } else { part };

            let inline_nodes = parse_inline(cell_content);
            let cell_kind = if is_header {
                node::TABLE_HEADER
            } else {
                node::TABLE_CELL
            };

            cells.push(Node::new(cell_kind).children(inline_nodes));
        }

        Node::new(node::TABLE_ROW).children(cells)
    }

    fn parse_list(&mut self) -> Node {
        let first_line = self.lines[self.pos];
        let trimmed = first_line.trim_start();
        let ordered = trimmed.starts_with('#');

        self.parse_list_at_level(1, ordered)
    }

    fn parse_list_at_level(&mut self, level: usize, ordered: bool) -> Node {
        let marker = if ordered { '#' } else { '*' };
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            let trimmed = line.trim_start();

            // Count markers
            let marker_count = trimmed.chars().take_while(|&c| c == marker).count();

            if marker_count == 0 {
                // Check for other list type or end
                let other_marker = if ordered { '*' } else { '#' };
                let other_count = trimmed.chars().take_while(|&c| c == other_marker).count();
                if other_count == 0 {
                    break;
                }
                if other_count <= level {
                    break;
                }
            }

            if marker_count < level {
                break;
            }

            if marker_count == level
                && trimmed.len() > marker_count
                && trimmed.chars().nth(marker_count) == Some(' ')
            {
                let content = trimmed[marker_count + 1..].trim();
                let inline_nodes = parse_inline(content);
                let para = Node::new(node::PARAGRAPH).children(inline_nodes);
                let mut item_children = vec![para];

                self.pos += 1;

                // Check for nested list
                if self.pos < self.lines.len() {
                    let next_line = self.lines[self.pos];
                    let next_trimmed = next_line.trim_start();
                    let next_marker_count =
                        next_trimmed.chars().take_while(|&c| c == marker).count();
                    let other_marker = if ordered { '*' } else { '#' };
                    let next_other_count = next_trimmed
                        .chars()
                        .take_while(|&c| c == other_marker)
                        .count();

                    if next_marker_count > level {
                        item_children.push(self.parse_list_at_level(next_marker_count, ordered));
                    } else if next_other_count > level {
                        item_children.push(self.parse_list_at_level(next_other_count, !ordered));
                    }
                }

                items.push(Node::new(node::LIST_ITEM).children(item_children));
            } else if marker_count > level {
                break;
            } else {
                self.pos += 1;
            }
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items)
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut text = String::new();
        let first_line = self.lines[self.pos];

        // Check for p. prefix
        let first_content = first_line
            .strip_prefix("p.")
            .map(|s| s.trim())
            .unwrap_or_else(|| first_line.trim());

        text.push_str(first_content);
        self.pos += 1;

        // Continue until empty line or block element
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if line.trim().is_empty() {
                break;
            }

            // Check for block elements
            if line.starts_with("h1.")
                || line.starts_with("h2.")
                || line.starts_with("h3.")
                || line.starts_with("h4.")
                || line.starts_with("h5.")
                || line.starts_with("h6.")
                || line.starts_with("bc.")
                || line.starts_with("bq.")
                || line.starts_with("pre.")
                || line.starts_with("p.")
                || line.trim_start().starts_with('|')
                || line.trim_start().starts_with("* ")
                || line.trim_start().starts_with("# ")
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
        if chars[i] == '@' {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }

            i += 1;
            let mut code = String::new();
            while i < chars.len() && chars[i] != '@' {
                code.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1; // skip closing @
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, code));
            continue;
        }

        // Try to parse formatting markers
        if let Some((new_i, node)) = try_parse_formatting(&chars, i, &mut current, &mut nodes) {
            i = new_i;
            nodes.push(node);
            continue;
        }

        // Link "text":url
        if chars[i] == '"'
            && let Some((link_end, link_text, url)) = parse_textile_link(&chars, i)
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
            i = link_end;
            continue;
        }

        // Image !url!
        if chars[i] == '!'
            && let Some((img_end, url, alt)) = parse_textile_image(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let mut img = Node::new(node::IMAGE).prop(prop::URL, url);
            if let Some(alt_text) = alt {
                img = img.prop(prop::ALT, alt_text);
            }
            nodes.push(img);
            i = img_end;
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

fn try_parse_formatting(
    chars: &[char],
    i: usize,
    current: &mut String,
    nodes: &mut Vec<Node>,
) -> Option<(usize, Node)> {
    // Define formatting markers: (marker, doubled_marker, node_kind, check_prev)
    let markers: &[(char, char, &str, bool)] = &[
        ('*', '*', node::STRONG, true),
        ('_', '_', node::EMPHASIS, true),
        ('-', '-', node::STRIKEOUT, true),
        ('+', '+', node::UNDERLINE, true),
        ('^', ' ', node::SUPERSCRIPT, false), // ^ has no doubled version
        ('~', ' ', node::SUBSCRIPT, false),   // ~ has no doubled version
    ];

    for &(marker, doubled, kind, check_prev) in markers {
        if chars[i] != marker {
            continue;
        }

        // Check previous char if needed
        if check_prev && i > 0 && chars[i - 1].is_alphanumeric() {
            continue;
        }

        // Check next char exists and is valid
        if i + 1 >= chars.len() || chars[i + 1] == ' ' {
            continue;
        }

        // Skip if doubled marker
        if doubled != ' ' && chars[i + 1] == doubled {
            continue;
        }

        if let Some((end, content)) = find_closing_marker(chars, i + 1, marker) {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let inner = parse_inline(&content);
            return Some((end + 1, Node::new(kind).children(inner)));
        }
    }

    None
}

fn find_closing_marker(chars: &[char], start: usize, marker: char) -> Option<(usize, String)> {
    let mut i = start;
    let mut content = String::new();

    while i < chars.len() {
        if chars[i] == marker && (i + 1 >= chars.len() || !chars[i + 1].is_alphanumeric()) {
            return Some((i, content));
        }
        content.push(chars[i]);
        i += 1;
    }
    None
}

fn parse_textile_link(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    // "text":url
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

    // Must be followed by :
    if i >= chars.len() || chars[i] != ':' {
        return None;
    }
    i += 1; // skip :

    // Collect URL until whitespace or end
    let mut url = String::new();
    while i < chars.len() && !chars[i].is_whitespace() {
        url.push(chars[i]);
        i += 1;
    }

    if url.is_empty() {
        return None;
    }

    Some((i, link_text, url))
}

fn parse_textile_image(chars: &[char], start: usize) -> Option<(usize, String, Option<String>)> {
    // !url! or !url(alt)!
    if chars[start] != '!' {
        return None;
    }

    let mut i = start + 1;
    let mut url = String::new();
    let mut alt = None;

    while i < chars.len() && chars[i] != '!' && chars[i] != '(' {
        url.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() {
        return None;
    }

    // Alt text in parentheses
    if chars[i] == '(' {
        i += 1;
        let mut alt_text = String::new();
        while i < chars.len() && chars[i] != ')' {
            alt_text.push(chars[i]);
            i += 1;
        }
        if i < chars.len() && chars[i] == ')' {
            alt = Some(alt_text);
            i += 1;
        }
    }

    // Must end with !
    if i >= chars.len() || chars[i] != '!' {
        return None;
    }
    i += 1;

    if url.is_empty() {
        return None;
    }

    Some((i, url, alt))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_heading() {
        let doc = parse_str("h1. Title\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_levels() {
        let doc = parse_str("h2. Level 2\nh3. Level 3\n");
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
        let doc = parse_str("*bold*\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("_italic_\n");
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
        let doc = parse_str("\"Example\":https://example.com\n");
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_list() {
        let doc = parse_str("* item1\n* item2\n");
        assert_eq!(doc.content.children.len(), 1);
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse_str("bc. code here\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }
}
