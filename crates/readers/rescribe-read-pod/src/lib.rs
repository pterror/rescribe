//! POD (Plain Old Documentation) reader for rescribe.
//!
//! Parses Perl POD markup into the rescribe document model.

use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse POD markup.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse POD markup with custom options.
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
    in_pod: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines().collect(),
            pos: 0,
            in_pod: false,
        }
    }

    fn parse(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            // Check for =pod or =head* to enter POD mode
            if line.starts_with("=pod") || line.starts_with("=head") {
                self.in_pod = true;
            }

            // Check for =cut to exit POD mode
            if line.starts_with("=cut") {
                self.in_pod = false;
                self.pos += 1;
                continue;
            }

            // If not in POD mode and we haven't seen a command, skip
            if !self.in_pod && !line.starts_with('=') {
                self.pos += 1;
                continue;
            }

            // Process POD content
            if line.starts_with('=') {
                if let Some(node) = self.parse_command(line) {
                    nodes.push(node);
                }
                self.pos += 1;
                continue;
            }

            // Verbatim paragraph (starts with whitespace)
            if line.starts_with(' ') || line.starts_with('\t') {
                nodes.push(self.parse_verbatim());
                continue;
            }

            // Skip blank lines
            if line.trim().is_empty() {
                self.pos += 1;
                continue;
            }

            // Ordinary paragraph
            nodes.push(self.parse_paragraph());
        }

        nodes
    }

    fn parse_command(&mut self, line: &str) -> Option<Node> {
        // =head1 through =head6
        if let Some(rest) = line.strip_prefix("=head")
            && let Some(level_char) = rest.chars().next()
            && let Some(level) = level_char.to_digit(10)
            && (1..=6).contains(&level)
        {
            let title = rest.get(1..)?.trim();
            let inline_nodes = parse_inline(title);
            return Some(
                Node::new(node::HEADING)
                    .prop(prop::LEVEL, level as i64)
                    .children(inline_nodes),
            );
        }

        // =over / =item / =back
        if line.starts_with("=over") {
            return Some(self.parse_list());
        }

        // =pod is just a marker, skip
        if line.starts_with("=pod") {
            return None;
        }

        // =begin / =end / =for - format-specific content
        if line.starts_with("=begin") || line.starts_with("=end") || line.starts_with("=for") {
            // Skip format-specific content for now
            return None;
        }

        // =encoding - metadata
        if line.starts_with("=encoding") {
            return None;
        }

        None
    }

    fn parse_list(&mut self) -> Node {
        let mut items = Vec::new();
        self.pos += 1; // Skip =over

        // Determine if it's ordered (numbered items) or not
        let mut is_ordered = false;

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if line.starts_with("=back") {
                self.pos += 1;
                break;
            }

            if line.starts_with("=item") {
                let item_content = line.strip_prefix("=item").unwrap_or("").trim();

                // Check if it's a numbered item
                if item_content.starts_with(|c: char| c.is_ascii_digit()) {
                    is_ordered = true;
                }

                // Collect item content from following paragraphs
                self.pos += 1;
                let mut item_nodes = Vec::new();

                // Parse the item marker itself if it has text after the number/bullet
                let marker_text = if let Some(stripped) = item_content.strip_prefix('*') {
                    stripped.trim()
                } else if let Some(dot_pos) = item_content.find('.') {
                    item_content.get(dot_pos + 1..).unwrap_or("").trim()
                } else {
                    item_content
                };

                if !marker_text.is_empty() {
                    let inline_nodes = parse_inline(marker_text);
                    item_nodes.push(Node::new(node::PARAGRAPH).children(inline_nodes));
                }

                // Collect following paragraphs until next =item or =back
                while self.pos < self.lines.len() {
                    let inner_line = self.lines[self.pos];

                    if inner_line.starts_with("=item") || inner_line.starts_with("=back") {
                        break;
                    }

                    if inner_line.trim().is_empty() {
                        self.pos += 1;
                        continue;
                    }

                    if inner_line.starts_with(' ') || inner_line.starts_with('\t') {
                        item_nodes.push(self.parse_verbatim());
                    } else {
                        item_nodes.push(self.parse_paragraph());
                    }
                }

                items.push(Node::new(node::LIST_ITEM).children(item_nodes));
            } else {
                self.pos += 1;
            }
        }

        Node::new(node::LIST)
            .prop(prop::ORDERED, is_ordered)
            .children(items)
    }

    fn parse_verbatim(&mut self) -> Node {
        let mut content = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            // Verbatim ends when we hit a non-indented line
            if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }

            // Remove one level of indentation (typically 4 spaces or 1 tab)
            let stripped = if let Some(rest) = line.strip_prefix("    ") {
                rest
            } else if let Some(rest) = line.strip_prefix('\t') {
                rest
            } else {
                line
            };

            content.push_str(stripped);
            content.push('\n');
            self.pos += 1;
        }

        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content.trim_end().to_string())
    }

    fn parse_paragraph(&mut self) -> Node {
        let mut text = String::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            // Paragraph ends on blank line, command, or verbatim start
            if line.trim().is_empty()
                || line.starts_with('=')
                || line.starts_with(' ')
                || line.starts_with('\t')
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

/// Parse POD formatting codes into inline nodes.
fn parse_inline(text: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Check for formatting codes: X<...>
        if i + 1 < chars.len() && chars[i + 1] == '<' {
            let code = chars[i];
            if matches!(
                code,
                'B' | 'I' | 'C' | 'L' | 'F' | 'S' | 'U' | 'E' | 'X' | 'Z'
            ) {
                // Check for double angle brackets C<< ... >>
                let (content, end_pos) = if i + 2 < chars.len() && chars[i + 2] == '<' {
                    // Double angle brackets with space padding - start from first <
                    find_double_bracket_content(&chars, i + 1)
                } else {
                    // Single angle bracket
                    find_single_bracket_content(&chars, i + 2)
                };

                if let Some((content, end)) = content.zip(end_pos) {
                    // For escape codes (E, X, Z, S), don't flush buffer - just accumulate
                    let is_escape = matches!(code, 'E' | 'X' | 'Z' | 'S');

                    if !is_escape && !current.is_empty() {
                        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                        current.clear();
                    }

                    match code {
                        'B' => {
                            let inner = parse_inline(&content);
                            nodes.push(Node::new(node::STRONG).children(inner));
                        }
                        'I' | 'F' => {
                            // F<> (filename) is typically rendered as italic
                            let inner = parse_inline(&content);
                            nodes.push(Node::new(node::EMPHASIS).children(inner));
                        }
                        'U' => {
                            let inner = parse_inline(&content);
                            nodes.push(Node::new(node::UNDERLINE).children(inner));
                        }
                        'C' => {
                            nodes.push(Node::new(node::CODE).prop(prop::CONTENT, content));
                        }
                        'L' => {
                            // Link: L<name> or L<text|url> or L<text|name/"section">
                            let (url, label) = if let Some(pipe_pos) = content.find('|') {
                                (
                                    content[pipe_pos + 1..].to_string(),
                                    content[..pipe_pos].to_string(),
                                )
                            } else {
                                (content.clone(), content)
                            };
                            let text_node = Node::new(node::TEXT).prop(prop::CONTENT, label);
                            nodes.push(
                                Node::new(node::LINK)
                                    .prop(prop::URL, url)
                                    .children(vec![text_node]),
                            );
                        }
                        'S' => {
                            // Non-breaking spaces - just accumulate the text
                            current.push_str(&content);
                        }
                        'E' => {
                            // Escape: E<lt>, E<gt>, E<amp>, E<sol>, E<verbar>, etc.
                            let escaped = match content.as_str() {
                                "lt" => "<",
                                "gt" => ">",
                                "amp" => "&",
                                "sol" => "/",
                                "verbar" => "|",
                                "quot" => "\"",
                                "apos" => "'",
                                _ => {
                                    // Try numeric (E<0x201E> or E<8222>)
                                    if let Some(hex) = content.strip_prefix("0x")
                                        && let Ok(code) = u32::from_str_radix(hex, 16)
                                        && let Some(c) = char::from_u32(code)
                                    {
                                        current.push(c);
                                        i = end;
                                        continue;
                                    } else if let Ok(code) = content.parse::<u32>()
                                        && let Some(c) = char::from_u32(code)
                                    {
                                        current.push(c);
                                        i = end;
                                        continue;
                                    }
                                    ""
                                }
                            };
                            if !escaped.is_empty() {
                                current.push_str(escaped);
                            }
                            i = end;
                            continue;
                        }
                        'X' | 'Z' => {
                            // X<> is an index entry (invisible)
                            // Z<> is a null element (invisible)
                            i = end;
                            continue;
                        }
                        _ => {}
                    }

                    i = end;
                    continue;
                }
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

fn find_single_bracket_content(chars: &[char], start: usize) -> (Option<String>, Option<usize>) {
    let mut i = start;
    let mut content = String::new();
    let mut depth = 1;

    while i < chars.len() {
        if chars[i] == '<' {
            depth += 1;
            content.push(chars[i]);
        } else if chars[i] == '>' {
            depth -= 1;
            if depth == 0 {
                return (Some(content), Some(i + 1));
            }
            content.push(chars[i]);
        } else {
            content.push(chars[i]);
        }
        i += 1;
    }

    (None, None)
}

fn find_double_bracket_content(chars: &[char], start: usize) -> (Option<String>, Option<usize>) {
    // Count opening brackets
    let mut bracket_count = 0;
    let mut i = start;
    while i < chars.len() && chars[i] == '<' {
        bracket_count += 1;
        i += 1;
    }

    // Skip leading space
    if i < chars.len() && chars[i] == ' ' {
        i += 1;
    }

    // Find closing brackets (same count followed by optional space)
    let mut content = String::new();
    while i < chars.len() {
        // Check for closing sequence: space + N closing brackets
        if chars[i] == ' ' && i + bracket_count < chars.len() {
            let mut all_closing = true;
            for j in 0..bracket_count {
                if chars[i + 1 + j] != '>' {
                    all_closing = false;
                    break;
                }
            }
            if all_closing {
                return (Some(content), Some(i + 1 + bracket_count));
            }
        }

        // Also check for direct closing (without space) for simpler cases
        if chars[i] == '>' {
            let mut closing_count = 0;
            let mut j = i;
            while j < chars.len() && chars[j] == '>' {
                closing_count += 1;
                j += 1;
            }
            if closing_count >= bracket_count {
                return (
                    Some(content.trim_end().to_string()),
                    Some(i + bracket_count),
                );
            }
        }

        content.push(chars[i]);
        i += 1;
    }

    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_heading() {
        let doc = parse_str("=head1 NAME\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(1));
    }

    #[test]
    fn test_parse_heading_level2() {
        let doc = parse_str("=head2 DESCRIPTION\n");
        assert_eq!(doc.content.children[0].props.get_int(prop::LEVEL), Some(2));
    }

    #[test]
    fn test_parse_paragraph() {
        let doc = parse_str("=pod\n\nThis is a paragraph.\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str("=pod\n\nThis is B<bold> text.\n");
        let para = &doc.content.children[0];
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::STRONG)
        );
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str("=pod\n\nThis is I<italic> text.\n");
        let para = &doc.content.children[0];
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
    }

    #[test]
    fn test_parse_code() {
        let doc = parse_str("=pod\n\nUse C<my $var> here.\n");
        let para = &doc.content.children[0];
        assert!(para.children.iter().any(|n| n.kind.as_str() == node::CODE));
    }

    #[test]
    fn test_parse_link() {
        let doc = parse_str("=pod\n\nSee L<perlpod> for details.\n");
        let para = &doc.content.children[0];
        assert!(para.children.iter().any(|n| n.kind.as_str() == node::LINK));
    }

    #[test]
    fn test_parse_verbatim() {
        let doc = parse_str("=pod\n\n    print \"Hello\";\n");
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }

    #[test]
    fn test_parse_list() {
        let doc = parse_str("=over\n\n=item * First\n\n=item * Second\n\n=back\n");
        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_escape() {
        let doc = parse_str("=pod\n\nE<lt>tag E<gt>\n");
        let para = &doc.content.children[0];
        let text = para.children[0].props.get_str(prop::CONTENT).unwrap_or("");
        assert!(text.contains('<'));
        assert!(text.contains('>'));
    }

    #[test]
    fn test_parse_double_brackets() {
        let doc = parse_str("=pod\n\nC<< $a <=> $b >>\n");
        let para = &doc.content.children[0];
        let code = para.children.iter().find(|n| n.kind.as_str() == node::CODE);
        assert!(code.is_some());
        let content = code.unwrap().props.get_str(prop::CONTENT).unwrap_or("");
        assert!(content.contains("<=>"));
    }
}
