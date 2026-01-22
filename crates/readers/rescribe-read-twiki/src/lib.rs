//! TWiki reader for rescribe.
//!
//! Parses TWiki markup into rescribe's document IR.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse TWiki markup into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse TWiki markup with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut result = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Headings: ---+ ---++ ---+++ etc
        if line.starts_with("---+") {
            let level = line.chars().skip(3).take_while(|&c| c == '+').count();
            let text = line.trim_start_matches('-').trim_start_matches('+').trim();
            result.push(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level.min(6) as i64)
                    .children(parse_inline(text)),
            );
            i += 1;
            continue;
        }

        // Horizontal rule: ---
        if line.trim() == "---" {
            result.push(Node::new(node::HORIZONTAL_RULE));
            i += 1;
            continue;
        }

        // Verbatim block
        if line.trim().starts_with("<verbatim>") {
            let (code_node, end) = parse_verbatim(&lines, i);
            result.push(code_node);
            i = end;
            continue;
        }

        // Table: |cell|cell|
        if line.trim().starts_with('|') && line.trim().ends_with('|') {
            let (table_node, end) = parse_table(&lines, i);
            result.push(table_node);
            i = end;
            continue;
        }

        // Lists (indented with spaces)
        if line.starts_with("   ")
            && (line.trim().starts_with('*') || line.trim().starts_with("1."))
        {
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
        if line.trim().is_empty()
            || line.starts_with("---")
            || line.starts_with("   ")
            || line.trim().starts_with('|')
            || line.trim().starts_with("<verbatim>")
        {
            break;
        }
        para_lines.push(line.trim());
        i += 1;
    }

    (para_lines, i)
}

fn parse_verbatim(lines: &[&str], start: usize) -> (Node, usize) {
    let mut code_lines = Vec::new();
    let mut i = start;

    // Handle single-line verbatim
    let first_line = lines[start].trim();
    if first_line.contains("</verbatim>") {
        let content = first_line
            .strip_prefix("<verbatim>")
            .unwrap_or(first_line)
            .strip_suffix("</verbatim>")
            .unwrap_or(first_line);
        return (
            Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.to_string()),
            start + 1,
        );
    }

    // Skip opening tag
    let after_tag = first_line
        .strip_prefix("<verbatim>")
        .unwrap_or("")
        .to_string();
    if !after_tag.is_empty() {
        code_lines.push(after_tag);
    }
    i += 1;

    while i < lines.len() {
        let line = lines[i];
        if line.contains("</verbatim>") {
            let before = line.split("</verbatim>").next().unwrap_or("");
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

fn parse_table(lines: &[&str], start: usize) -> (Node, usize) {
    let mut rows = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i].trim();
        if !line.starts_with('|') || !line.ends_with('|') {
            break;
        }

        let inner = &line[1..line.len() - 1];
        let cells: Vec<Node> = inner
            .split('|')
            .map(|cell| {
                let cell = cell.trim();
                // Header cells start with *
                if cell.starts_with('*') && cell.ends_with('*') {
                    Node::new(node::TABLE_HEADER).children(parse_inline(&cell[1..cell.len() - 1]))
                } else {
                    Node::new(node::TABLE_CELL).children(parse_inline(cell))
                }
            })
            .collect();

        rows.push(Node::new(node::TABLE_ROW).children(cells));
        i += 1;
    }

    (Node::new(node::TABLE).children(rows), i)
}

fn parse_list(lines: &[&str], start: usize) -> (Node, usize) {
    let first_line = lines[start].trim();
    let ordered = first_line.starts_with("1.");
    let mut items = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with("   ") {
            break;
        }

        let trimmed = line.trim();
        let text = if ordered {
            trimmed.strip_prefix("1.").unwrap_or(trimmed).trim()
        } else {
            trimmed.strip_prefix('*').unwrap_or(trimmed).trim()
        };

        if trimmed.starts_with('*') || trimmed.starts_with("1.") {
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
        // Bold italic: __text__
        if i + 1 < chars.len()
            && chars[i] == '_'
            && chars[i + 1] == '_'
            && let Some((content, end)) = find_delimited(&chars, i + 2, "__")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(
                Node::new(node::STRONG)
                    .child(Node::new(node::EMPHASIS).children(parse_inline(&content))),
            );
            i = end;
            continue;
        }

        // Bold: *text*
        if chars[i] == '*'
            && let Some((content, end)) = find_delimited(&chars, i + 1, "*")
            && !content.is_empty()
            && !content.starts_with(' ')
            && !content.ends_with(' ')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::STRONG).children(parse_inline(&content)));
            i = end;
            continue;
        }

        // Italic: _text_
        if chars[i] == '_'
            && let Some((content, end)) = find_delimited(&chars, i + 1, "_")
            && !content.is_empty()
            && !content.starts_with(' ')
            && !content.ends_with(' ')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::EMPHASIS).children(parse_inline(&content)));
            i = end;
            continue;
        }

        // Bold fixed: ==text==
        if i + 1 < chars.len()
            && chars[i] == '='
            && chars[i + 1] == '='
            && let Some((content, end)) = find_delimited(&chars, i + 2, "==")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(
                Node::new(node::STRONG).child(Node::new(node::CODE).prop(prop::CONTENT, content)),
            );
            i = end;
            continue;
        }

        // Fixed: =text=
        if chars[i] == '='
            && let Some((content, end)) = find_delimited(&chars, i + 1, "=")
            && !content.is_empty()
            && !content.starts_with(' ')
            && !content.ends_with(' ')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end;
            continue;
        }

        // Link: [[url][label]] or [[url]]
        if i + 1 < chars.len()
            && chars[i] == '['
            && chars[i + 1] == '['
            && let Some((url, label, end)) = parse_twiki_link(&chars, i + 2)
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, label)),
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

fn find_delimited(chars: &[char], start: usize, delim: &str) -> Option<(String, usize)> {
    let delim_chars: Vec<char> = delim.chars().collect();
    let mut i = start;

    while i + delim_chars.len() <= chars.len() {
        let mut matches = true;
        for (j, dc) in delim_chars.iter().enumerate() {
            if chars[i + j] != *dc {
                matches = false;
                break;
            }
        }
        if matches {
            let content: String = chars[start..i].iter().collect();
            return Some((content, i + delim_chars.len()));
        }
        i += 1;
    }
    None
}

fn parse_twiki_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let mut url = String::new();
    let mut i = start;

    // Get URL part
    while i < chars.len() {
        if chars[i] == ']' {
            break;
        }
        url.push(chars[i]);
        i += 1;
    }

    if i >= chars.len() {
        return None;
    }

    // Check for [label] part
    if i + 1 < chars.len() && chars[i] == ']' && chars[i + 1] == '[' {
        let mut label = String::new();
        i += 2;
        while i < chars.len() && chars[i] != ']' {
            label.push(chars[i]);
            i += 1;
        }
        if i + 1 < chars.len() && chars[i] == ']' && chars[i + 1] == ']' {
            return Some((url, label, i + 2));
        }
    }

    // Just [[url]]
    if i + 1 < chars.len() && chars[i] == ']' && chars[i + 1] == ']' {
        let label = url.clone();
        return Some((url, label, i + 2));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let result = parse("---+ Heading 1\n---++ Heading 2").unwrap();
        assert_eq!(result.value.content.children.len(), 2);
    }

    #[test]
    fn test_parse_bold() {
        let result = parse("This is *bold* text").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_italic() {
        let result = parse("This is _italic_ text").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_code() {
        let result = parse("Use =code= here").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_link() {
        let result = parse("Visit [[http://example.com][Example]]").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_table() {
        let result = parse("| A | B |\n| C | D |").unwrap();
        assert_eq!(result.value.content.children.len(), 1);
        assert_eq!(result.value.content.children[0].kind.as_str(), node::TABLE);
    }
}
