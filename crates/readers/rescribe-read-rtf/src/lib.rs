//! RTF (Rich Text Format) reader for rescribe.
//!
//! Parses RTF documents into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse an RTF document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse an RTF document with custom options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    let nodes = parser.parse()?;

    let root = Node::new(node::DOCUMENT).children(nodes);
    let doc = Document::new().with_content(root);

    Ok(ConversionResult::ok(doc))
}

#[derive(Default, Clone)]
struct TextState {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse(&mut self) -> Result<Vec<Node>, ParseError> {
        // Skip to document content (past {\rtf1...)
        self.skip_header();

        let mut state = TextState::default();
        let mut paragraphs = Vec::new();
        let mut current_para = Vec::new();
        let mut current_text = String::new();

        while self.pos < self.input.len() {
            let ch = self.current_char();

            match ch {
                '\\' => {
                    // Control word or symbol
                    self.pos += 1;
                    if self.pos >= self.input.len() {
                        break;
                    }

                    let next = self.current_char();
                    if next.is_alphabetic() {
                        // Control word
                        let (word, param) = self.read_control_word();
                        self.handle_control_word(
                            &word,
                            param,
                            &mut state,
                            &mut current_text,
                            &mut current_para,
                            &mut paragraphs,
                        );
                    } else if next == '\'' {
                        // Hex character
                        self.pos += 1;
                        if self.pos + 2 <= self.input.len() {
                            let hex = &self.input[self.pos..self.pos + 2];
                            if let Ok(code) = u8::from_str_radix(hex, 16) {
                                current_text.push(code as char);
                            }
                            self.pos += 2;
                        }
                    } else {
                        // Control symbol - special characters
                        match next {
                            '\\' => current_text.push('\\'),
                            '{' => current_text.push('{'),
                            '}' => current_text.push('}'),
                            '~' => current_text.push('\u{00A0}'), // non-breaking space
                            '-' => {}                             // optional hyphen, ignore
                            '_' => current_text.push('\u{2011}'), // non-breaking hyphen
                            '\n' | '\r' => {}                     // line break after control word
                            _ => {}
                        }
                        self.pos += 1;
                    }
                }
                '{' => {
                    // Start of group - push state
                    self.pos += 1;
                    // For simplicity, we don't handle nested groups deeply
                    // Skip special groups like \fonttbl, \colortbl, \stylesheet
                    self.skip_special_groups();
                }
                '}' => {
                    // End of group - pop state
                    self.pos += 1;
                }
                '\n' | '\r' => {
                    // Ignore line breaks in RTF (they're not significant)
                    self.pos += 1;
                }
                _ => {
                    current_text.push(ch);
                    self.pos += 1;
                }
            }
        }

        // Flush remaining text
        if !current_text.is_empty() {
            let text_node = self.make_text_node(&current_text, &state);
            current_para.push(text_node);
        }

        // Flush remaining paragraph
        if !current_para.is_empty() {
            paragraphs.push(Node::new(node::PARAGRAPH).children(current_para));
        }

        Ok(paragraphs)
    }

    fn current_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn skip_header(&mut self) {
        // Skip to after {\rtf1 and past the ANSI/encoding declaration
        if let Some(pos) = self.input.find("\\rtf") {
            self.pos = pos;
            // Skip past \rtfN
            while self.pos < self.input.len() {
                let ch = self.current_char();
                if ch == ' ' || ch == '\\' || ch == '{' {
                    break;
                }
                self.pos += 1;
            }
        }
    }

    fn skip_special_groups(&mut self) {
        // Check if this group starts with a special control word
        let start = self.pos;
        if self.pos < self.input.len() && self.current_char() == '\\' {
            let temp_pos = self.pos + 1;
            let rest = &self.input[temp_pos..];

            // List of groups to skip entirely
            let skip_groups = [
                "fonttbl",
                "colortbl",
                "stylesheet",
                "info",
                "pict",
                "object",
                "header",
                "footer",
                "headerl",
                "headerr",
                "footerl",
                "footerr",
                "*",
            ];

            for group in skip_groups {
                if rest.starts_with(group) {
                    // Skip to matching closing brace
                    let mut depth = 1;
                    self.pos = start;
                    while self.pos < self.input.len() && depth > 0 {
                        match self.current_char() {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            '\\' => {
                                self.pos += 1;
                                if self.pos < self.input.len() {
                                    self.pos += 1;
                                }
                                continue;
                            }
                            _ => {}
                        }
                        self.pos += 1;
                    }
                    return;
                }
            }
        }
    }

    fn read_control_word(&mut self) -> (String, Option<i32>) {
        let mut word = String::new();

        // Read alphabetic characters
        while self.pos < self.input.len() {
            let ch = self.current_char();
            if ch.is_ascii_alphabetic() {
                word.push(ch);
                self.pos += 1;
            } else {
                break;
            }
        }

        // Read optional numeric parameter
        let mut param = None;
        let mut negative = false;

        if self.pos < self.input.len() && self.current_char() == '-' {
            negative = true;
            self.pos += 1;
        }

        if self.pos < self.input.len() && self.current_char().is_ascii_digit() {
            let mut num = String::new();
            while self.pos < self.input.len() && self.current_char().is_ascii_digit() {
                num.push(self.current_char());
                self.pos += 1;
            }
            if let Ok(n) = num.parse::<i32>() {
                param = Some(if negative { -n } else { n });
            }
        }

        // Consume delimiter space if present
        if self.pos < self.input.len() && self.current_char() == ' ' {
            self.pos += 1;
        }

        (word, param)
    }

    fn handle_control_word(
        &mut self,
        word: &str,
        param: Option<i32>,
        state: &mut TextState,
        current_text: &mut String,
        current_para: &mut Vec<Node>,
        paragraphs: &mut Vec<Node>,
    ) {
        match word {
            // Paragraph break
            "par" | "pard" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                if !current_para.is_empty() {
                    paragraphs
                        .push(Node::new(node::PARAGRAPH).children(std::mem::take(current_para)));
                }
                if word == "pard" {
                    // Reset paragraph formatting
                    *state = TextState::default();
                }
            }

            // Line break within paragraph
            "line" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                current_para.push(Node::new(node::LINE_BREAK));
            }

            // Character formatting - flush text before changing state
            "b" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                state.bold = param.unwrap_or(1) != 0;
            }
            "i" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                state.italic = param.unwrap_or(1) != 0;
            }
            "ul" | "uld" | "uldb" | "ulw" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                state.underline = param.unwrap_or(1) != 0;
            }
            "ulnone" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                state.underline = false;
            }
            "strike" => {
                if !current_text.is_empty() {
                    let text_node = self.make_text_node(current_text, state);
                    current_para.push(text_node);
                    current_text.clear();
                }
                state.strikethrough = param.unwrap_or(1) != 0;
            }

            // Tab
            "tab" => current_text.push('\t'),

            // Em dash, en dash
            "emdash" => current_text.push('\u{2014}'),
            "endash" => current_text.push('\u{2013}'),

            // Quotes
            "lquote" => current_text.push('\u{2018}'),
            "rquote" => current_text.push('\u{2019}'),
            "ldblquote" => current_text.push('\u{201C}'),
            "rdblquote" => current_text.push('\u{201D}'),

            // Bullet
            "bullet" => current_text.push('\u{2022}'),

            // Ignore most other control words
            _ => {}
        }
    }

    fn make_text_node(&self, text: &str, state: &TextState) -> Node {
        let mut node = Node::new(node::TEXT).prop(prop::CONTENT, text.to_string());

        // Apply formatting by wrapping in appropriate nodes
        if state.bold || state.italic || state.underline || state.strikethrough {
            if state.strikethrough {
                node = Node::new(node::STRIKEOUT).children(vec![node]);
            }
            if state.underline {
                node = Node::new(node::UNDERLINE).children(vec![node]);
            }
            if state.italic {
                node = Node::new(node::EMPHASIS).children(vec![node]);
            }
            if state.bold {
                node = Node::new(node::STRONG).children(vec![node]);
            }
        }

        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_simple_text() {
        let doc = parse_str(r"{\rtf1 Hello world\par}");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str(r"{\rtf1 \b bold text\b0  normal\par}");
        let para = &doc.content.children[0];
        // Should contain a strong node
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::STRONG)
        );
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str(r"{\rtf1 \i italic\i0\par}");
        let para = &doc.content.children[0];
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
    }

    #[test]
    fn test_parse_underline() {
        let doc = parse_str(r"{\rtf1 \ul underlined\ulnone\par}");
        let para = &doc.content.children[0];
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::UNDERLINE)
        );
    }

    #[test]
    fn test_parse_multiple_paragraphs() {
        let doc = parse_str(r"{\rtf1 First paragraph\par Second paragraph\par}");
        assert_eq!(doc.content.children.len(), 2);
    }

    #[test]
    fn test_parse_escaped_chars() {
        let doc = parse_str(r"{\rtf1 Open \{ and close \}\par}");
        let para = &doc.content.children[0];
        let text = get_all_text(para);
        assert!(text.contains("{"));
        assert!(text.contains("}"));
    }

    #[test]
    fn test_parse_special_chars() {
        let doc = parse_str(r"{\rtf1 Em\emdash dash\par}");
        let para = &doc.content.children[0];
        let text = get_all_text(para);
        assert!(text.contains('\u{2014}')); // em dash
    }

    fn get_all_text(node: &Node) -> String {
        let mut text = String::new();
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            text.push_str(content);
        }
        for child in &node.children {
            text.push_str(&get_all_text(child));
        }
        text
    }
}
