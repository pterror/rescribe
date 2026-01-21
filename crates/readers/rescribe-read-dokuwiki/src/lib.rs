//! DokuWiki reader for rescribe.
//!
//! Parses DokuWiki markup into rescribe documents.

#![allow(clippy::collapsible_if)]

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse DokuWiki source into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse DokuWiki source with custom options.
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

        // Heading: ====== H1 ====== (6 =), ===== H2 ===== (5 =), etc.
        if trimmed.starts_with('=') && trimmed.ends_with('=') {
            return Some(self.parse_heading());
        }

        // Code block: <code> or <code lang>
        if trimmed.starts_with("<code") {
            return Some(self.parse_code_block());
        }

        // File block: <file>
        if trimmed.starts_with("<file") {
            return Some(self.parse_code_block());
        }

        // List: starts with spaces and * or -
        if line.starts_with("  ")
            && (line.trim_start().starts_with('*') || line.trim_start().starts_with('-'))
        {
            return Some(self.parse_list());
        }

        // Blockquote: > text
        if trimmed.starts_with('>') {
            return Some(self.parse_blockquote());
        }

        // Horizontal rule: ----
        if trimmed == "----" {
            self.advance();
            return Some(Node::new(node::HORIZONTAL_RULE));
        }

        // Default: paragraph
        Some(self.parse_paragraph())
    }

    fn parse_heading(&mut self) -> Node {
        let line = self.current_line().unwrap();
        self.advance();

        let trimmed = line.trim();

        // Count leading = signs
        let leading = trimmed.chars().take_while(|c| *c == '=').count();
        // DokuWiki uses 6 = for H1, 5 for H2, etc.
        let level = (7 - leading.min(6)) as i64;

        // Extract content between = signs
        let content = trimmed.trim_start_matches('=').trim_end_matches('=').trim();

        Node::new(node::HEADING)
            .prop(prop::LEVEL, level)
            .children(self.parse_inline(content))
    }

    fn parse_code_block(&mut self) -> Node {
        let line = self.current_line().unwrap();
        self.advance();

        // Extract language from <code lang> or <file lang>
        let lang = if let Some(start) = line.find('<') {
            let after = &line[start..];
            if let Some(end) = after.find('>') {
                let tag = &after[1..end];
                let parts: Vec<&str> = tag.split_whitespace().collect();
                if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let end_tag = if line.contains("<file") {
            "</file>"
        } else {
            "</code>"
        };

        let mut content = String::new();
        while let Some(line) = self.current_line() {
            if line.contains(end_tag) {
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
            node = node.prop(prop::LANGUAGE, lang);
        }
        node
    }

    fn parse_list(&mut self) -> Node {
        let mut items = Vec::new();
        let first_char = self
            .current_line()
            .and_then(|l| l.trim_start().chars().next());
        let ordered = first_char == Some('-');

        while let Some(line) = self.current_line() {
            if !line.starts_with("  ") {
                break;
            }
            let trimmed = line.trim_start();
            if !trimmed.starts_with('*') && !trimmed.starts_with('-') {
                break;
            }

            // Get content after marker
            let content = trimmed[1..].trim_start();
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

    fn parse_blockquote(&mut self) -> Node {
        let mut lines = Vec::new();

        while let Some(line) = self.current_line() {
            let trimmed = line.trim();
            if !trimmed.starts_with('>') {
                break;
            }
            let content = trimmed[1..].trim_start();
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
            if (trimmed.starts_with('=') && trimmed.ends_with('='))
                || trimmed.starts_with("<code")
                || trimmed.starts_with("<file")
                || line.starts_with("  ")
                || trimmed.starts_with('>')
                || trimmed == "----"
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
            // Bold: **text**
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_double_delim(&chars, i + 2, '*') {
                    nodes.push(
                        Node::new(node::STRONG)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, content)]),
                    );
                    i = end + 2;
                    continue;
                }
            }

            // Italic: //text//
            if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_double_delim(&chars, i + 2, '/') {
                    nodes.push(
                        Node::new(node::EMPHASIS)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, content)]),
                    );
                    i = end + 2;
                    continue;
                }
            }

            // Underline: __text__
            if i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_double_delim(&chars, i + 2, '_') {
                    nodes.push(
                        Node::new(node::UNDERLINE)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, content)]),
                    );
                    i = end + 2;
                    continue;
                }
            }

            // Monospace: ''text''
            if i + 1 < chars.len() && chars[i] == '\'' && chars[i + 1] == '\'' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_double_delim(&chars, i + 2, '\'') {
                    nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
                    i = end + 2;
                    continue;
                }
            }

            // Link: [[url|text]]
            if i + 1 < chars.len() && chars[i] == '[' && chars[i + 1] == '[' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((link_content, end)) = self.find_double_bracket(&chars, i + 2) {
                    let (url, link_text) = if let Some(pipe_pos) = link_content.find('|') {
                        (&link_content[..pipe_pos], &link_content[pipe_pos + 1..])
                    } else {
                        (link_content.as_str(), link_content.as_str())
                    };
                    nodes.push(
                        Node::new(node::LINK)
                            .prop(prop::URL, url)
                            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, link_text)]),
                    );
                    i = end + 2;
                    continue;
                }
            }

            // Image: {{url|alt}}
            if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((img_content, end)) = self.find_double_brace(&chars, i + 2) {
                    let (url, alt) = if let Some(pipe_pos) = img_content.find('|') {
                        (&img_content[..pipe_pos], Some(&img_content[pipe_pos + 1..]))
                    } else {
                        (img_content.as_str(), None)
                    };
                    let mut img = Node::new(node::IMAGE).prop(prop::URL, url);
                    if let Some(alt) = alt {
                        img = img.prop(prop::ALT, alt);
                    }
                    nodes.push(img);
                    i = end + 2;
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

    fn find_double_delim(
        &self,
        chars: &[char],
        start: usize,
        delim: char,
    ) -> Option<(String, usize)> {
        let mut i = start;
        let mut content = String::new();

        while i + 1 < chars.len() {
            if chars[i] == delim && chars[i + 1] == delim {
                return Some((content, i));
            }
            content.push(chars[i]);
            i += 1;
        }

        None
    }

    fn find_double_bracket(&self, chars: &[char], start: usize) -> Option<(String, usize)> {
        let mut i = start;
        let mut content = String::new();

        while i + 1 < chars.len() {
            if chars[i] == ']' && chars[i + 1] == ']' {
                return Some((content, i));
            }
            content.push(chars[i]);
            i += 1;
        }

        None
    }

    fn find_double_brace(&self, chars: &[char], start: usize) -> Option<(String, usize)> {
        let mut i = start;
        let mut content = String::new();

        while i + 1 < chars.len() {
            if chars[i] == '}' && chars[i + 1] == '}' {
                return Some((content, i));
            }
            content.push(chars[i]);
            i += 1;
        }

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
        let doc = parse_str("====== Title ======");
        let heading = &doc.content.children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
        assert_eq!(heading.props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_levels() {
        let doc = parse_str("====== H1 ======\n===== H2 =====\n==== H3 ====");
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
        let doc = parse_str("This is **bold** text.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("This is //italic// text.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_code() {
        let doc = parse_str("Use ''code'' here.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("Click [[https://example.com|here]].");
        let para = &doc.content.children[0];
        let link = &para.children[1];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_list() {
        let doc = parse_str("  * Item 1\n  * Item 2");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse_str("<code rust>\nfn main() {}\n</code>");
        let code = &doc.content.children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
        assert_eq!(code.props.get_str(prop::LANGUAGE), Some("rust"));
    }
}
