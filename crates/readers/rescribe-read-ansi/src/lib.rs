//! ANSI escape sequence reader for rescribe.
//!
//! Parses text with ANSI escape codes into rescribe's document IR.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse ANSI-formatted text into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse ANSI-formatted text with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut result = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Empty line
        if line.is_empty() || strip_ansi(line).is_empty() {
            i += 1;
            continue;
        }

        // Collect paragraph lines
        let (para_lines, end) = collect_paragraph(&lines, i);
        if !para_lines.is_empty() {
            let text = para_lines.join(" ");
            result.push(Node::new(node::PARAGRAPH).children(parse_inline(&text)));
        }
        i = end;
    }

    let document = Document {
        content: Node::new(node::DOCUMENT).children(result),
        resources: Default::default(),
        metadata: Default::default(),
        source: None,
    };

    Ok(ConversionResult::ok(document))
}

fn collect_paragraph<'a>(lines: &[&'a str], start: usize) -> (Vec<&'a str>, usize) {
    let mut para_lines = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        if line.is_empty() || strip_ansi(line).is_empty() {
            break;
        }
        para_lines.push(line);
        i += 1;
    }

    (para_lines, i)
}

/// Strip ANSI escape sequences from text
fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // Skip until 'm' or end of sequence
            i += 2;
            while i < chars.len() && !chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i < chars.len() {
                i += 1; // Skip the terminating letter
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// ANSI SGR (Select Graphic Rendition) codes
#[derive(Default, Clone)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut current = String::new();
    let mut style = Style::default();

    while i < chars.len() {
        // Check for ANSI escape sequence
        if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // Flush current text
            if !current.is_empty() {
                nodes.push(create_styled_node(&current, &style));
                current.clear();
            }

            // Parse escape sequence
            i += 2; // Skip ESC [
            let mut params = String::new();
            while i < chars.len() && !chars[i].is_ascii_alphabetic() {
                params.push(chars[i]);
                i += 1;
            }

            if i < chars.len() {
                let cmd = chars[i];
                i += 1;

                if cmd == 'm' {
                    // SGR command
                    for code in params.split(';') {
                        match code.trim() {
                            "0" | "" => style = Style::default(), // Reset
                            "1" => style.bold = true,
                            "3" => style.italic = true,
                            "4" => style.underline = true,
                            "9" => style.strikethrough = true,
                            "22" => style.bold = false,
                            "23" => style.italic = false,
                            "24" => style.underline = false,
                            "29" => style.strikethrough = false,
                            _ => {} // Ignore colors and other codes
                        }
                    }
                }
            }
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    // Flush remaining text
    if !current.is_empty() {
        nodes.push(create_styled_node(&current, &style));
    }

    nodes
}

fn create_styled_node(text: &str, style: &Style) -> Node {
    let text_node = Node::new(node::TEXT).prop(prop::CONTENT, text.to_string());

    // Apply styles from innermost to outermost
    let mut node = text_node;

    if style.strikethrough {
        node = Node::new(node::STRIKEOUT).child(node);
    }
    if style.underline {
        node = Node::new(node::UNDERLINE).child(node);
    }
    if style.italic {
        node = Node::new(node::EMPHASIS).child(node);
    }
    if style.bold {
        node = Node::new(node::STRONG).child(node);
    }

    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let result = parse("Hello world").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_bold() {
        let result = parse("\x1b[1mBold text\x1b[0m").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_italic() {
        let result = parse("\x1b[3mItalic text\x1b[0m").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_underline() {
        let result = parse("\x1b[4mUnderlined\x1b[0m").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[1mBold\x1b[0m"), "Bold");
        assert_eq!(strip_ansi("\x1b[31mRed\x1b[0m"), "Red");
        assert_eq!(strip_ansi("Plain text"), "Plain text");
    }

    #[test]
    fn test_combined_styles() {
        let result = parse("\x1b[1;3mBold and italic\x1b[0m").unwrap();
        assert!(!result.value.content.children.is_empty());
    }
}
