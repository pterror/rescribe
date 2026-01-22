//! TikiWiki reader for rescribe.
//!
//! Parses TikiWiki markup into rescribe's document IR.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse TikiWiki markup into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse TikiWiki markup with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut result = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Headings: ! !! !!! etc
        if let Some(rest) = line.strip_prefix('!') {
            let level = 1 + rest.chars().take_while(|&c| c == '!').count();
            let text = rest.trim_start_matches('!').trim();
            result.push(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level.min(6) as i64)
                    .children(parse_inline(text)),
            );
            i += 1;
            continue;
        }

        // Horizontal rule
        if line.trim() == "---" {
            result.push(Node::new(node::HORIZONTAL_RULE));
            i += 1;
            continue;
        }

        // Code block: {CODE()}...{CODE}
        if line.trim().starts_with("{CODE") {
            let (code_node, end) = parse_code_block(&lines, i);
            result.push(code_node);
            i = end;
            continue;
        }

        // Table: ||cell|cell||
        if line.trim().starts_with("||") {
            let (table_node, end) = parse_table(&lines, i);
            result.push(table_node);
            i = end;
            continue;
        }

        // Lists
        if line.starts_with('*') || line.starts_with('#') {
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
            || line.starts_with('!')
            || line.starts_with('*')
            || line.starts_with('#')
            || line.trim().starts_with("||")
            || line.trim().starts_with("{CODE")
            || line.trim() == "---"
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

    // Extract language if present: {CODE(lang=python)}
    let lang = if let Some(paren_start) = first_line.find('(') {
        if let Some(paren_end) = first_line.find(')') {
            let params = &first_line[paren_start + 1..paren_end];
            params
                .split(',')
                .find_map(|p| {
                    p.strip_prefix("lang=")
                        .or_else(|| p.strip_prefix("language="))
                })
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    let mut code_lines = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i];
        if line.trim() == "{CODE}" || line.trim().starts_with("{CODE}") {
            let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code_lines.join("\n"));
            if let Some(l) = lang {
                node = node.prop(prop::LANGUAGE, l);
            }
            return (node, i + 1);
        }
        code_lines.push(line);
        i += 1;
    }

    let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code_lines.join("\n"));
    if let Some(l) = lang {
        node = node.prop(prop::LANGUAGE, l);
    }
    (node, i)
}

fn parse_table(lines: &[&str], start: usize) -> (Node, usize) {
    let mut rows = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i].trim();
        if !line.starts_with("||") {
            break;
        }

        // Parse row: ||cell|cell||
        let inner = line.trim_start_matches("||").trim_end_matches("||");
        let cells: Vec<Node> = inner
            .split('|')
            .map(|cell| Node::new(node::TABLE_CELL).children(parse_inline(cell.trim())))
            .collect();

        rows.push(Node::new(node::TABLE_ROW).children(cells));
        i += 1;
    }

    (Node::new(node::TABLE).children(rows), i)
}

fn parse_list(lines: &[&str], start: usize) -> (Node, usize) {
    let first_char = lines[start].chars().next().unwrap_or(' ');
    let ordered = first_char == '#';
    let mut items = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let marker = if ordered { '#' } else { '*' };

        if !line.starts_with(marker) {
            break;
        }

        let text = line.trim_start_matches(marker).trim();
        items.push(
            Node::new(node::LIST_ITEM)
                .child(Node::new(node::PARAGRAPH).children(parse_inline(text))),
        );
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
        // Bold: __text__
        if i + 1 < chars.len()
            && chars[i] == '_'
            && chars[i + 1] == '_'
            && let Some((content, end)) = find_delimited(&chars, i + 2, "__")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::STRONG).children(parse_inline(&content)));
            i = end;
            continue;
        }

        // Italic: ''text''
        if i + 1 < chars.len()
            && chars[i] == '\''
            && chars[i + 1] == '\''
            && let Some((content, end)) = find_delimited(&chars, i + 2, "''")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::EMPHASIS).children(parse_inline(&content)));
            i = end;
            continue;
        }

        // Underline: ===text===
        if i + 2 < chars.len()
            && chars[i] == '='
            && chars[i + 1] == '='
            && chars[i + 2] == '='
            && let Some((content, end)) = find_delimited(&chars, i + 3, "===")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::UNDERLINE).children(parse_inline(&content)));
            i = end;
            continue;
        }

        // Strikethrough: --text--
        if i + 1 < chars.len()
            && chars[i] == '-'
            && chars[i + 1] == '-'
            && let Some((content, end)) = find_delimited(&chars, i + 2, "--")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::STRIKEOUT).children(parse_inline(&content)));
            i = end;
            continue;
        }

        // Inline code: -+text+-
        if i + 1 < chars.len()
            && chars[i] == '-'
            && chars[i + 1] == '+'
            && let Some((content, end)) = find_delimited(&chars, i + 2, "+-")
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
            i = end;
            continue;
        }

        // Link: [url|label] or [url]
        if chars[i] == '['
            && let Some((content, end)) = find_bracket_content(&chars, i + 1, '[', ']')
        {
            if !current.is_empty() {
                nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                current.clear();
            }
            let parts: Vec<&str> = content.splitn(2, '|').collect();
            let url = parts[0].trim();
            let label = if parts.len() > 1 {
                parts[1].trim()
            } else {
                url
            };
            nodes.push(
                Node::new(node::LINK)
                    .prop(prop::URL, url.to_string())
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, label.to_string())),
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

fn find_bracket_content(
    chars: &[char],
    start: usize,
    _open: char,
    close: char,
) -> Option<(String, usize)> {
    let mut i = start;
    while i < chars.len() {
        if chars[i] == close {
            let content: String = chars[start..i].iter().collect();
            return Some((content, i + 1));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let result = parse("!Heading 1\n!!Heading 2").unwrap();
        assert_eq!(result.value.content.children.len(), 2);
    }

    #[test]
    fn test_parse_bold() {
        let result = parse("This is __bold__ text").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_italic() {
        let result = parse("This is ''italic'' text").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_link() {
        let result = parse("[http://example.com|Example]").unwrap();
        assert!(!result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_list() {
        let result = parse("*Item 1\n*Item 2").unwrap();
        assert_eq!(result.value.content.children.len(), 1);
        assert_eq!(result.value.content.children[0].kind.as_str(), node::LIST);
    }

    #[test]
    fn test_parse_table() {
        let result = parse("||A|B||\n||C|D||").unwrap();
        assert_eq!(result.value.content.children.len(), 1);
        assert_eq!(result.value.content.children[0].kind.as_str(), node::TABLE);
    }
}
