//! Creole wiki markup reader for rescribe.
//!
//! Parses Creole wiki markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Creole markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Creole markup with custom options.
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
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines().collect(),
            pos: 0,
            _marker: std::marker::PhantomData,
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

            // Nowiki block {{{ ... }}}
            if line.trim_start().starts_with("{{{") {
                nodes.push(self.parse_nowiki_block());
                continue;
            }

            // Heading = to ======
            if let Some(node) = self.try_parse_heading(line) {
                nodes.push(node);
                self.pos += 1;
                continue;
            }

            // Horizontal rule ----
            if line.trim().starts_with("----") {
                nodes.push(Node::new(node::HORIZONTAL_RULE));
                self.pos += 1;
                continue;
            }

            // Table
            if line.trim_start().starts_with('|') {
                nodes.push(self.parse_table());
                continue;
            }

            // List - but not bold **text**
            let trimmed = line.trim_start();
            if (trimmed.starts_with('*') && !trimmed.starts_with("**"))
                || (trimmed.starts_with('#') && !trimmed.starts_with("##"))
            {
                nodes.push(self.parse_list());
                continue;
            }

            // Paragraph
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn try_parse_heading(&self, line: &str) -> Option<Node> {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|&c| c == '=').count();

        if level > 0 && level <= 6 {
            let rest = trimmed[level..].trim();
            // Remove trailing = if present
            let content = rest.trim_end_matches('=').trim();
            let inline_nodes = Self::parse_inline(content);

            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level as i64)
                    .children(inline_nodes),
            );
        }
        None
    }

    fn parse_nowiki_block(&mut self) -> Node {
        let first_line = self.lines[self.pos];
        let content_start = first_line.find("{{{").unwrap() + 3;
        let mut content = String::new();

        // Check if it ends on the same line
        if let Some(end_pos) = first_line[content_start..].find("}}}") {
            content.push_str(&first_line[content_start..content_start + end_pos]);
            self.pos += 1;
        } else {
            // Multi-line nowiki
            if content_start < first_line.len() {
                content.push_str(&first_line[content_start..]);
                content.push('\n');
            }
            self.pos += 1;

            while self.pos < self.lines.len() {
                let line = self.lines[self.pos];
                if let Some(end_pos) = line.find("}}}") {
                    content.push_str(&line[..end_pos]);
                    self.pos += 1;
                    break;
                } else {
                    content.push_str(line);
                    content.push('\n');
                    self.pos += 1;
                }
            }
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
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

        // Split by | but skip empty first/last
        let parts: Vec<&str> = trimmed.split('|').collect();

        for part in parts {
            if part.is_empty() {
                continue;
            }

            let is_header = part.starts_with('=');
            let cell_content = if is_header {
                part[1..].trim()
            } else {
                part.trim()
            };

            let inline_nodes = Self::parse_inline(cell_content);
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
        let first_char = first_line.trim_start().chars().next().unwrap();
        let ordered = first_char == '#';

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
                // Check for other list type
                let other_marker = if ordered { '*' } else { '#' };
                let other_count = trimmed.chars().take_while(|&c| c == other_marker).count();
                if other_count == 0 {
                    break;
                }
                // Different list type at same level - break
                if other_count == level {
                    break;
                }
            }

            if marker_count < level {
                break;
            }

            if marker_count == level {
                let content = trimmed[marker_count..].trim();
                let inline_nodes = Self::parse_inline(content);
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
                    } else if next_other_count > 0 {
                        item_children.push(self.parse_list_at_level(next_other_count, !ordered));
                    }
                }

                items.push(Node::new(node::LIST_ITEM).children(item_children));
            } else if marker_count > level {
                // Nested list - handled by item above
                break;
            }
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
            if trimmed.starts_with('=')
                || trimmed.starts_with("----")
                || trimmed.starts_with('|')
                || (trimmed.starts_with('*') && !trimmed.starts_with("**"))
                || (trimmed.starts_with('#') && !trimmed.starts_with("##"))
                || trimmed.starts_with("{{{")
            {
                break;
            }

            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line.trim());
            self.pos += 1;
        }

        let inline_nodes = Self::parse_inline(&text);
        Node::new(node::PARAGRAPH).children(inline_nodes)
    }

    fn parse_inline(text: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Line break \\
            if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '\\' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                nodes.push(Node::new(node::LINE_BREAK));
                i += 2;
                continue;
            }

            // Inline nowiki {{{...}}}
            if i + 2 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' && chars[i + 2] == '{'
            {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }

                i += 3;
                let mut code = String::new();
                while i + 2 < chars.len() {
                    if chars[i] == '}' && chars[i + 1] == '}' && chars[i + 2] == '}' {
                        i += 3;
                        break;
                    }
                    code.push(chars[i]);
                    i += 1;
                }
                nodes.push(Node::new(node::CODE).prop(prop::CONTENT, code));
                continue;
            }

            // Bold **...**
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }

                i += 2;
                let mut bold_text = String::new();
                while i + 1 < chars.len() {
                    if chars[i] == '*' && chars[i + 1] == '*' {
                        i += 2;
                        break;
                    }
                    bold_text.push(chars[i]);
                    i += 1;
                }
                let inner = Self::parse_inline(&bold_text);
                nodes.push(Node::new(node::STRONG).children(inner));
                continue;
            }

            // Italic //...//
            if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
                // Make sure we're not in a URL (preceded by :)
                let preceded_by_colon = i > 0 && chars[i - 1] == ':';
                if !preceded_by_colon {
                    if !current.is_empty() {
                        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                        current.clear();
                    }

                    i += 2;
                    let mut italic_text = String::new();
                    while i + 1 < chars.len() {
                        if chars[i] == '/' && chars[i + 1] == '/' {
                            i += 2;
                            break;
                        }
                        italic_text.push(chars[i]);
                        i += 1;
                    }
                    let inner = Self::parse_inline(&italic_text);
                    nodes.push(Node::new(node::EMPHASIS).children(inner));
                    continue;
                }
            }

            // Link [[url|text]] or [[url]]
            if i + 1 < chars.len() && chars[i] == '[' && chars[i + 1] == '[' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }

                i += 2;
                let mut link_content = String::new();
                while i + 1 < chars.len() {
                    if chars[i] == ']' && chars[i + 1] == ']' {
                        i += 2;
                        break;
                    }
                    link_content.push(chars[i]);
                    i += 1;
                }

                let (url, link_text) = if let Some(pipe_pos) = link_content.find('|') {
                    (
                        &link_content[..pipe_pos],
                        link_content[pipe_pos + 1..].to_string(),
                    )
                } else {
                    (link_content.as_str(), link_content.clone())
                };

                let text_node = Node::new(node::TEXT).prop(prop::CONTENT, link_text);
                nodes.push(
                    Node::new(node::LINK)
                        .prop(prop::URL, url.to_string())
                        .children(vec![text_node]),
                );
                continue;
            }

            // Image {{url|alt}} or {{url}}
            if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
                // Not inline nowiki (checked above)
                if i + 2 < chars.len() && chars[i + 2] != '{' {
                    if !current.is_empty() {
                        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                        current.clear();
                    }

                    i += 2;
                    let mut img_content = String::new();
                    while i + 1 < chars.len() {
                        if chars[i] == '}' && chars[i + 1] == '}' {
                            i += 2;
                            break;
                        }
                        img_content.push(chars[i]);
                        i += 1;
                    }

                    let (url, alt) = if let Some(pipe_pos) = img_content.find('|') {
                        (
                            &img_content[..pipe_pos],
                            Some(img_content[pipe_pos + 1..].to_string()),
                        )
                    } else {
                        (img_content.as_str(), None)
                    };

                    let mut img = Node::new(node::IMAGE).prop(prop::URL, url.to_string());
                    if let Some(alt_text) = alt {
                        img = img.prop(prop::ALT, alt_text);
                    }
                    nodes.push(img);
                    continue;
                }
            }

            current.push(chars[i]);
            i += 1;
        }

        if !current.is_empty() {
            nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current));
        }

        nodes
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
        let doc = parse_str("**bold**\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("//italic//\n");
        let para = &doc.content.children[0];
        assert_eq!(para.children.len(), 1);
        assert_eq!(para.children[0].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("[[https://example.com|Example]]\n");
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
    fn test_parse_nowiki() {
        let doc = parse_str("{{{code}}}\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }
}
