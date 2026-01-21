//! MediaWiki reader for rescribe.
//!
//! Parses MediaWiki markup into rescribe's document IR.
//!
//! # Example
//!
//! ```
//! use rescribe_read_mediawiki::parse;
//!
//! let result = parse("== Heading ==\n\nSome '''bold''' text.").unwrap();
//! let doc = result.value;
//! ```

use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse MediaWiki text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse();

    let document = Document {
        content: Node::new(node::DOCUMENT).children(parser.result),
        resources: Default::default(),
        metadata: Properties::new(),
        source: None,
    };

    Ok(ConversionResult::with_warnings(document, parser.warnings))
}

struct Parser<'a> {
    #[allow(dead_code)]
    input: &'a str,
    result: Vec<Node>,
    warnings: Vec<FidelityWarning>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            result: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn parse(&mut self) {
        let lines: Vec<&str> = self.input.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            if trimmed.is_empty() {
                i += 1;
                continue;
            }

            // Heading
            if trimmed.starts_with('=')
                && let Some(heading) = self.parse_heading(trimmed)
            {
                self.result.push(heading);
                i += 1;
                continue;
            }

            // List
            if trimmed.starts_with('*') || trimmed.starts_with('#') {
                let (list, consumed) = self.parse_list(&lines[i..]);
                self.result.push(list);
                i += consumed;
                continue;
            }

            // Horizontal rule
            if trimmed == "----" || trimmed.chars().all(|c| c == '-') && trimmed.len() >= 4 {
                self.result.push(Node::new(node::HORIZONTAL_RULE));
                i += 1;
                continue;
            }

            // Code block (indented with space)
            if line.starts_with(' ') {
                let (block, consumed) = self.parse_code_block(&lines[i..]);
                self.result.push(block);
                i += consumed;
                continue;
            }

            // Regular paragraph
            let (para, consumed) = self.parse_paragraph(&lines[i..]);
            self.result.push(para);
            i += consumed;
        }
    }

    fn parse_heading(&self, line: &str) -> Option<Node> {
        let trimmed = line.trim();

        // Count leading `=`
        let level = trimmed.chars().take_while(|&c| c == '=').count();
        if level == 0 || level > 6 {
            return None;
        }

        // Check for matching trailing `=`
        let content = trimmed.trim_start_matches('=').trim_end_matches('=').trim();

        let children = self.parse_inline(content);
        Some(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level as i64)
                .children(children),
        )
    }

    fn parse_list(&self, lines: &[&str]) -> (Node, usize) {
        let mut items: Vec<Node> = Vec::new();
        let mut consumed = 0;
        let first_char = lines[0].trim().chars().next().unwrap_or('*');
        let ordered = first_char == '#';

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }

            // Check if this is a list item with the same marker
            let marker = if ordered { '#' } else { '*' };
            if !trimmed.starts_with(marker) {
                break;
            }

            // Count depth
            let depth = trimmed.chars().take_while(|&c| c == marker).count();

            if depth == 1 {
                // Top-level item
                let content = trimmed.trim_start_matches(marker).trim();
                let children = self.parse_inline(content);
                let item =
                    Node::new(node::LIST_ITEM).child(Node::new(node::PARAGRAPH).children(children));
                items.push(item);
            } else {
                // Nested list - for simplicity, treat as flat item with markers stripped
                let content = trimmed.trim_start_matches(marker).trim();
                let children = self.parse_inline(content);
                let item =
                    Node::new(node::LIST_ITEM).child(Node::new(node::PARAGRAPH).children(children));
                items.push(item);
            }

            consumed += 1;
        }

        let list = Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items);

        (list, consumed.max(1))
    }

    fn parse_code_block(&self, lines: &[&str]) -> (Node, usize) {
        let mut content = String::new();
        let mut consumed = 0;

        for line in lines {
            if !line.starts_with(' ') && !line.is_empty() {
                break;
            }
            if !content.is_empty() {
                content.push('\n');
            }
            // Remove one leading space
            content.push_str(line.strip_prefix(' ').unwrap_or(line));
            consumed += 1;
        }

        let block = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content);
        (block, consumed.max(1))
    }

    fn parse_paragraph(&self, lines: &[&str]) -> (Node, usize) {
        let mut text = String::new();
        let mut consumed = 0;

        for line in lines {
            let trimmed = line.trim();

            // Stop at empty lines, headings, lists, rules
            if trimmed.is_empty()
                || trimmed.starts_with('=')
                || trimmed.starts_with('*')
                || trimmed.starts_with('#')
                || (trimmed.chars().all(|c| c == '-') && trimmed.len() >= 4)
            {
                break;
            }

            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(trimmed);
            consumed += 1;
        }

        let children = self.parse_inline(&text);
        let para = Node::new(node::PARAGRAPH).children(children);
        (para, consumed.max(1))
    }

    #[allow(clippy::only_used_in_recursion)]
    fn parse_inline(&self, text: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current_text = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Bold: '''text'''
            if i + 2 < chars.len()
                && chars[i] == '\''
                && chars[i + 1] == '\''
                && chars[i + 2] == '\''
            {
                if !current_text.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current_text.clone()));
                    current_text.clear();
                }

                // Find closing '''
                let start = i + 3;
                let mut end = start;
                while end + 2 < chars.len() {
                    if chars[end] == '\'' && chars[end + 1] == '\'' && chars[end + 2] == '\'' {
                        break;
                    }
                    end += 1;
                }

                if end + 2 < chars.len() {
                    let inner: String = chars[start..end].iter().collect();
                    let inner_nodes = self.parse_inline(&inner);
                    nodes.push(Node::new(node::STRONG).children(inner_nodes));
                    i = end + 3;
                    continue;
                }
            }

            // Italic: ''text''
            if i + 1 < chars.len() && chars[i] == '\'' && chars[i + 1] == '\'' {
                // Make sure it's not bold
                if i + 2 < chars.len() && chars[i + 2] == '\'' {
                    // This is bold, handled above
                } else {
                    if !current_text.is_empty() {
                        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current_text.clone()));
                        current_text.clear();
                    }

                    // Find closing ''
                    let start = i + 2;
                    let mut end = start;
                    while end + 1 < chars.len() {
                        if chars[end] == '\'' && chars[end + 1] == '\'' {
                            // Make sure it's not '''
                            if end + 2 < chars.len() && chars[end + 2] == '\'' {
                                end += 1;
                                continue;
                            }
                            break;
                        }
                        end += 1;
                    }

                    if end + 1 < chars.len() {
                        let inner: String = chars[start..end].iter().collect();
                        let inner_nodes = self.parse_inline(&inner);
                        nodes.push(Node::new(node::EMPHASIS).children(inner_nodes));
                        i = end + 2;
                        continue;
                    }
                }
            }

            // Internal link: [[Title]] or [[Title|text]]
            if i + 1 < chars.len() && chars[i] == '[' && chars[i + 1] == '[' {
                if !current_text.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current_text.clone()));
                    current_text.clear();
                }

                // Find closing ]]
                let start = i + 2;
                let mut end = start;
                while end + 1 < chars.len() {
                    if chars[end] == ']' && chars[end + 1] == ']' {
                        break;
                    }
                    end += 1;
                }

                if end + 1 < chars.len() {
                    let inner: String = chars[start..end].iter().collect();
                    let (url, text) = if let Some(pipe_pos) = inner.find('|') {
                        let url = &inner[..pipe_pos];
                        let text = &inner[pipe_pos + 1..];
                        (url.to_string(), text.to_string())
                    } else {
                        (inner.clone(), inner)
                    };

                    nodes.push(
                        Node::new(node::LINK)
                            .prop(prop::URL, url)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, text)),
                    );
                    i = end + 2;
                    continue;
                }
            }

            // External link: [url text]
            if chars[i] == '[' && (i + 1 >= chars.len() || chars[i + 1] != '[') {
                if !current_text.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current_text.clone()));
                    current_text.clear();
                }

                // Find closing ]
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end] != ']' {
                    end += 1;
                }

                if end < chars.len() {
                    let inner: String = chars[start..end].iter().collect();
                    let parts: Vec<&str> = inner.splitn(2, ' ').collect();
                    let url = parts[0].to_string();
                    let text = if parts.len() > 1 {
                        parts[1].to_string()
                    } else {
                        url.clone()
                    };

                    nodes.push(
                        Node::new(node::LINK)
                            .prop(prop::URL, url)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, text)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Regular character
            current_text.push(chars[i]);
            i += 1;
        }

        if !current_text.is_empty() {
            nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current_text));
        }

        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let result = parse("== Heading ==").unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 1);
        let heading = &doc.content.children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
        assert_eq!(heading.props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_bold() {
        let result = parse("'''bold'''").unwrap();
        let doc = result.value;
        let para = &doc.content.children[0];
        let strong = &para.children[0];
        assert_eq!(strong.kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let result = parse("''italic''").unwrap();
        let doc = result.value;
        let para = &doc.content.children[0];
        let em = &para.children[0];
        assert_eq!(em.kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_list() {
        let result = parse("* Item 1\n* Item 2").unwrap();
        let doc = result.value;
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_link() {
        let result = parse("[[Title|Link text]]").unwrap();
        let doc = result.value;
        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("Title"));
    }
}
