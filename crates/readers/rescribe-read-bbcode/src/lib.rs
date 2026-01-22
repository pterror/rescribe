//! BBCode reader for rescribe.
//!
//! Parses BBCode forum markup into rescribe's document IR.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse BBCode markup into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse BBCode markup with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut result = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Code block: [code]...[/code]
        if line.trim().to_lowercase().starts_with("[code]") {
            let (code_node, end) = parse_code_block(&lines, i);
            result.push(code_node);
            i = end;
            continue;
        }

        // Quote: [quote]...[/quote]
        if line.trim().to_lowercase().starts_with("[quote") {
            let (quote_node, end) = parse_quote(&lines, i);
            result.push(quote_node);
            i = end;
            continue;
        }

        // List: [list]...[/list]
        if line.trim().to_lowercase().starts_with("[list") {
            let (list_node, end) = parse_list(&lines, i);
            result.push(list_node);
            i = end;
            continue;
        }

        // Empty line
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // Regular paragraph
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
        let lower = line.trim().to_lowercase();
        if line.trim().is_empty()
            || lower.starts_with("[code")
            || lower.starts_with("[quote")
            || lower.starts_with("[list")
        {
            break;
        }
        para_lines.push(line.trim());
        i += 1;
    }

    (para_lines, i)
}

fn parse_code_block(lines: &[&str], start: usize) -> (Node, usize) {
    let first_line = lines[start].trim();
    let mut code_lines = Vec::new();
    let mut i = start;

    // Single line code block
    if first_line.to_lowercase().contains("[/code]") {
        let content = first_line
            .strip_prefix("[code]")
            .or_else(|| first_line.strip_prefix("[CODE]"))
            .unwrap_or(first_line)
            .strip_suffix("[/code]")
            .or_else(|| first_line.strip_suffix("[/CODE]"))
            .unwrap_or(first_line);
        return (
            Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.to_string()),
            start + 1,
        );
    }

    // Multi-line
    let after_tag = first_line
        .strip_prefix("[code]")
        .or_else(|| first_line.strip_prefix("[CODE]"))
        .unwrap_or("");
    if !after_tag.is_empty() {
        code_lines.push(after_tag.to_string());
    }
    i += 1;

    while i < lines.len() {
        let line = lines[i];
        let lower = line.to_lowercase();
        if lower.contains("[/code]") {
            let before = line.split("[/code]").next().unwrap_or("");
            let before = before.split("[/CODE]").next().unwrap_or(before);
            if !before.is_empty() {
                code_lines.push(before.to_string());
            }
            return (
                Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code_lines.join("\n")),
                i + 1,
            );
        }
        code_lines.push(line.to_string());
        i += 1;
    }

    (
        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code_lines.join("\n")),
        i,
    )
}

fn parse_quote(lines: &[&str], start: usize) -> (Node, usize) {
    let mut quote_lines = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i];
        if line.to_lowercase().contains("[/quote]") {
            let text = quote_lines.join(" ");
            return (
                Node::new(node::BLOCKQUOTE)
                    .child(Node::new(node::PARAGRAPH).children(parse_inline(&text))),
                i + 1,
            );
        }
        if !line.trim().is_empty() {
            quote_lines.push(line.trim());
        }
        i += 1;
    }

    let text = quote_lines.join(" ");
    (
        Node::new(node::BLOCKQUOTE).child(Node::new(node::PARAGRAPH).children(parse_inline(&text))),
        i,
    )
}

fn parse_list(lines: &[&str], start: usize) -> (Node, usize) {
    let first_line = lines[start].trim().to_lowercase();
    let ordered = first_line.contains("[list=1]") || first_line.contains("[list=a]");
    let mut items = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i];
        let lower = line.to_lowercase();
        if lower.contains("[/list]") {
            return (
                Node::new(node::LIST)
                    .prop(prop::ORDERED, ordered)
                    .children(items),
                i + 1,
            );
        }
        if lower.trim().starts_with("[*]") {
            let text = line
                .trim()
                .strip_prefix("[*]")
                .or_else(|| line.trim().strip_prefix("[*]"))
                .unwrap_or(line)
                .trim();
            items.push(
                Node::new(node::LIST_ITEM)
                    .child(Node::new(node::PARAGRAPH).children(parse_inline(text))),
            );
        }
        i += 1;
    }

    (
        Node::new(node::LIST)
            .prop(prop::ORDERED, ordered)
            .children(items),
        i,
    )
}

fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '['
            && let Some((tag, content, end)) = parse_bbcode_tag(&chars, i)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }

            let node = match tag.to_lowercase().as_str() {
                "b" => Some(Node::new(node::STRONG).children(parse_inline(&content))),
                "i" => Some(Node::new(node::EMPHASIS).children(parse_inline(&content))),
                "u" => Some(Node::new(node::UNDERLINE).children(parse_inline(&content))),
                "s" | "strike" => Some(Node::new(node::STRIKEOUT).children(parse_inline(&content))),
                "code" => Some(Node::new(node::CODE).prop(prop::CONTENT, content)),
                _ if tag.to_lowercase().starts_with("url=") => {
                    let url = tag[4..].to_string();
                    Some(
                        Node::new(node::LINK)
                            .prop(prop::URL, url)
                            .children(parse_inline(&content)),
                    )
                }
                "url" => Some(
                    Node::new(node::LINK)
                        .prop(prop::URL, content.clone())
                        .child(Node::new(node::TEXT).prop(prop::CONTENT, content)),
                ),
                _ if tag.to_lowercase().starts_with("img") => {
                    Some(Node::new(node::IMAGE).prop(prop::URL, content))
                }
                _ if tag.to_lowercase().starts_with("color=") => {
                    Some(Node::new(node::SPAN).children(parse_inline(&content)))
                }
                _ if tag.to_lowercase().starts_with("size=") => {
                    Some(Node::new(node::SPAN).children(parse_inline(&content)))
                }
                _ => None,
            };

            if let Some(n) = node {
                nodes.push(n);
                i = end;
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

fn parse_bbcode_tag(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if chars[start] != '[' {
        return None;
    }

    // Find tag name
    let mut tag = String::new();
    let mut i = start + 1;
    while i < chars.len() && chars[i] != ']' {
        tag.push(chars[i]);
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }
    i += 1; // Skip ]

    // Find closing tag
    let close_tag = format!("[/{}]", tag.split('=').next().unwrap_or(&tag));
    let close_lower = close_tag.to_lowercase();

    let content_start = i;

    while i < chars.len() {
        // Check for closing tag
        let remaining: String = chars[i..].iter().collect();
        if remaining.to_lowercase().starts_with(&close_lower) {
            let content: String = chars[content_start..i].iter().collect();
            return Some((tag, content, i + close_tag.len()));
        }
        i += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bold() {
        let result = parse("This is [b]bold[/b] text").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_italic() {
        let result = parse("This is [i]italic[/i] text").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_link() {
        let result = parse("[url=http://example.com]Example[/url]").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_list() {
        let result = parse("[list]\n[*]Item 1\n[*]Item 2\n[/list]").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_code() {
        let result = parse("[code]print('hello')[/code]").unwrap();
        assert!(!result.value.content.children.is_empty());
    }
}
