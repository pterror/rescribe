//! Jira markup reader for rescribe.
//!
//! Parses Jira/Confluence wiki markup into rescribe documents.

#![allow(clippy::collapsible_if)]

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Jira markup source into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Jira markup with custom options.
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

        // Heading: h1. to h6.
        if let Some(rest) = line.strip_prefix("h1. ") {
            self.advance();
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 1i64)
                    .children(self.parse_inline(rest)),
            );
        }
        if let Some(rest) = line.strip_prefix("h2. ") {
            self.advance();
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 2i64)
                    .children(self.parse_inline(rest)),
            );
        }
        if let Some(rest) = line.strip_prefix("h3. ") {
            self.advance();
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 3i64)
                    .children(self.parse_inline(rest)),
            );
        }
        if let Some(rest) = line.strip_prefix("h4. ") {
            self.advance();
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 4i64)
                    .children(self.parse_inline(rest)),
            );
        }
        if let Some(rest) = line.strip_prefix("h5. ") {
            self.advance();
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 5i64)
                    .children(self.parse_inline(rest)),
            );
        }
        if let Some(rest) = line.strip_prefix("h6. ") {
            self.advance();
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, 6i64)
                    .children(self.parse_inline(rest)),
            );
        }

        // Code block: {code} or {code:lang}
        if line.starts_with("{code") {
            return Some(self.parse_code_block());
        }

        // Quote block: {quote}
        if line.trim() == "{quote}" {
            return Some(self.parse_quote_block());
        }

        // Panel: {panel}
        if line.starts_with("{panel") {
            return Some(self.parse_panel_block());
        }

        // Lists: * or #
        if line.starts_with('*') || line.starts_with('#') {
            return Some(self.parse_list());
        }

        // Table: starts with |
        if line.starts_with('|') {
            return Some(self.parse_table());
        }

        // Horizontal rule: ----
        if line.trim() == "----" {
            self.advance();
            return Some(Node::new(node::HORIZONTAL_RULE));
        }

        // Default: paragraph
        Some(self.parse_paragraph())
    }

    fn parse_code_block(&mut self) -> Node {
        let line = self.current_line().unwrap();
        self.advance();

        // Extract language from {code:lang}
        let lang = if let Some(rest) = line.strip_prefix("{code:") {
            rest.strip_suffix('}').map(|s| s.to_string())
        } else {
            None
        };

        let mut content = String::new();
        while let Some(line) = self.current_line() {
            if line.trim() == "{code}" {
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

    fn parse_quote_block(&mut self) -> Node {
        self.advance(); // Skip {quote}
        let mut children = Vec::new();

        while let Some(line) = self.current_line() {
            if line.trim() == "{quote}" {
                self.advance();
                break;
            }
            if line.trim().is_empty() {
                self.advance();
                continue;
            }
            children.push(Node::new(node::PARAGRAPH).children(self.parse_inline(line)));
            self.advance();
        }

        Node::new(node::BLOCKQUOTE).children(children)
    }

    fn parse_panel_block(&mut self) -> Node {
        self.advance(); // Skip {panel...}
        let mut children = Vec::new();

        while let Some(line) = self.current_line() {
            if line.trim() == "{panel}" {
                self.advance();
                break;
            }
            if line.trim().is_empty() {
                self.advance();
                continue;
            }
            children.push(Node::new(node::PARAGRAPH).children(self.parse_inline(line)));
            self.advance();
        }

        Node::new(node::DIV)
            .prop("jira:type", "panel")
            .children(children)
    }

    fn parse_list(&mut self) -> Node {
        let first_line = self.current_line().unwrap();
        let ordered = first_line.starts_with('#');
        let mut items = Vec::new();

        while let Some(line) = self.current_line() {
            let marker = if ordered { '#' } else { '*' };
            if !line.starts_with(marker) {
                break;
            }

            // Count depth
            let depth = line.chars().take_while(|&c| c == marker).count();
            let content = line[depth..].trim_start();

            // For now, just handle single-level lists
            if depth == 1 {
                let item = Node::new(node::LIST_ITEM).children(vec![
                    Node::new(node::PARAGRAPH).children(self.parse_inline(content)),
                ]);
                items.push(item);
            }
            self.advance();
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items)
    }

    fn parse_table(&mut self) -> Node {
        let mut rows = Vec::new();
        let mut first_row = true;

        while let Some(line) = self.current_line() {
            if !line.starts_with('|') {
                break;
            }

            // Check if header row (starts with ||)
            let is_header = line.starts_with("||");
            let cells: Vec<Node> = if is_header {
                line.split("||")
                    .filter(|s| !s.is_empty())
                    .map(|cell| {
                        Node::new(node::TABLE_HEADER).children(self.parse_inline(cell.trim()))
                    })
                    .collect()
            } else {
                line.split('|')
                    .filter(|s| !s.is_empty())
                    .map(|cell| {
                        Node::new(node::TABLE_CELL).children(self.parse_inline(cell.trim()))
                    })
                    .collect()
            };

            let row = Node::new(node::TABLE_ROW).children(cells);

            if first_row && is_header {
                rows.push(Node::new(node::TABLE_HEAD).child(row));
            } else {
                if first_row {
                    // Start tbody
                }
                rows.push(row);
            }

            first_row = false;
            self.advance();
        }

        Node::new(node::TABLE).children(rows)
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut lines = Vec::new();

        while let Some(line) = self.current_line() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            // Stop at block-level elements
            if trimmed.starts_with("h1. ")
                || trimmed.starts_with("h2. ")
                || trimmed.starts_with("h3. ")
                || trimmed.starts_with("h4. ")
                || trimmed.starts_with("h5. ")
                || trimmed.starts_with("h6. ")
                || trimmed.starts_with("{code")
                || trimmed == "{quote}"
                || trimmed.starts_with("{panel")
                || trimmed.starts_with('*')
                || trimmed.starts_with('#')
                || trimmed.starts_with('|')
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
            // Bold: *text*
            if chars[i] == '*' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_delim(&chars, i + 1, '*') {
                    nodes.push(
                        Node::new(node::STRONG)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Italic: _text_
            if chars[i] == '_' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_delim(&chars, i + 1, '_') {
                    nodes.push(
                        Node::new(node::EMPHASIS)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Strikethrough: -text-
            if chars[i] == '-' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_delim(&chars, i + 1, '-') {
                    nodes.push(
                        Node::new(node::STRIKEOUT)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Underline: +text+
            if chars[i] == '+' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_delim(&chars, i + 1, '+') {
                    nodes.push(
                        Node::new(node::UNDERLINE)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Superscript: ^text^
            if chars[i] == '^' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_delim(&chars, i + 1, '^') {
                    nodes.push(
                        Node::new(node::SUPERSCRIPT)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Subscript: ~text~
            if chars[i] == '~' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_delim(&chars, i + 1, '~') {
                    nodes.push(
                        Node::new(node::SUBSCRIPT)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                    );
                    i = end + 1;
                    continue;
                }
            }

            // Monospace: {{text}}
            if i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((content, end)) = self.find_double_brace(&chars, i + 2) {
                    nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
                    i = end + 2;
                    continue;
                }
            }

            // Link: [text|url] or [url]
            if chars[i] == '[' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((link_content, end)) = self.find_bracket(&chars, i + 1) {
                    let (text, url) = if let Some(pipe_pos) = link_content.find('|') {
                        (&link_content[..pipe_pos], &link_content[pipe_pos + 1..])
                    } else {
                        (link_content.as_str(), link_content.as_str())
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

            // Image: !url! or !url|alt!
            if chars[i] == '!' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }
                if let Some((img_content, end)) = self.find_delim(&chars, i + 1, '!') {
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
                    i = end + 1;
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

    fn find_delim(&self, chars: &[char], start: usize, delim: char) -> Option<(String, usize)> {
        let mut content = String::new();
        let mut i = start;

        while i < chars.len() {
            if chars[i] == delim {
                return Some((content, i));
            }
            content.push(chars[i]);
            i += 1;
        }

        None
    }

    fn find_double_brace(&self, chars: &[char], start: usize) -> Option<(String, usize)> {
        let mut content = String::new();
        let mut i = start;

        while i + 1 < chars.len() {
            if chars[i] == '}' && chars[i + 1] == '}' {
                return Some((content, i));
            }
            content.push(chars[i]);
            i += 1;
        }

        None
    }

    fn find_bracket(&self, chars: &[char], start: usize) -> Option<(String, usize)> {
        let mut content = String::new();
        let mut i = start;

        while i < chars.len() {
            if chars[i] == ']' {
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
        let doc = parse_str("h1. Title");
        let heading = &doc.content.children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
        assert_eq!(heading.props.get_int(prop::LEVEL), Some(1));
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
        let doc = parse_str("Use {{code}} here.");
        let para = &doc.content.children[0];
        assert_eq!(para.children[1].kind.as_str(), node::CODE);
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("Click [here|https://example.com].");
        let para = &doc.content.children[0];
        let link = &para.children[1];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_list() {
        let doc = parse_str("* Item 1\n* Item 2");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse_str("{code:java}\npublic class Test {}\n{code}");
        let code = &doc.content.children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
        assert_eq!(code.props.get_str(prop::LANGUAGE), Some("java"));
    }
}
