//! Texinfo reader for rescribe.
//!
//! Parses GNU Texinfo documentation format into rescribe's document IR.
//!
//! # Example
//!
//! ```
//! use rescribe_read_texinfo::parse;
//!
//! let texinfo = r#"@chapter Introduction
//! This is the introduction.
//!
//! @section Getting Started
//! Here is how to get started."#;
//!
//! let result = parse(texinfo).unwrap();
//! let doc = result.value;
//! ```

use rescribe_core::{ConversionResult, Document, FidelityWarning, Node, ParseError, ParseOptions};
use rescribe_std::{node, prop};

/// Parse Texinfo into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Texinfo with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse()?;

    let document = Document {
        content: Node::new(node::DOCUMENT).children(parser.result),
        resources: Default::default(),
        metadata: parser.metadata,
        source: None,
    };

    Ok(ConversionResult::with_warnings(document, parser.warnings))
}

struct Parser<'a> {
    #[allow(dead_code)]
    input: &'a str,
    result: Vec<Node>,
    metadata: rescribe_core::Properties,
    warnings: Vec<FidelityWarning>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            result: Vec::new(),
            metadata: rescribe_core::Properties::new(),
            warnings: Vec::new(),
        }
    }

    fn parse(&mut self) -> Result<(), ParseError> {
        let lines: Vec<&str> = self.input.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim_start();

            // Skip comments
            if line.starts_with("@c ") || line.starts_with("@comment ") || line == "@c" {
                i += 1;
                continue;
            }

            // Skip directives we don't process
            if line.starts_with("@set ")
                || line.starts_with("@clear ")
                || line.starts_with("@include ")
                || line.starts_with("@setfilename ")
                || line.starts_with("@settitle ")
                || line.starts_with("@copying")
                || line.starts_with("@end copying")
                || line.starts_with("@titlepage")
                || line.starts_with("@end titlepage")
                || line.starts_with("@contents")
                || line.starts_with("@shortcontents")
                || line.starts_with("@summarycontents")
                || line.starts_with("@top")
                || line.starts_with("@bye")
                || line.starts_with("@dircategory")
                || line.starts_with("@direntry")
                || line.starts_with("@end direntry")
                || line.starts_with("\\input ")
            {
                // Extract title from @settitle
                if let Some(title) = line.strip_prefix("@settitle ") {
                    self.metadata.set("title", title.trim().to_string());
                }
                i += 1;
                continue;
            }

            // Handle node definitions (skip the @node line itself)
            if line.starts_with("@node ") {
                i += 1;
                continue;
            }

            // Handle headings
            if let Some(rest) = line.strip_prefix("@chapter ") {
                self.result.push(self.make_heading(1, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@unnumbered ") {
                self.result.push(self.make_heading(1, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@appendix ") {
                self.result.push(self.make_heading(1, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@section ") {
                self.result.push(self.make_heading(2, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@unnumberedsec ") {
                self.result.push(self.make_heading(2, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@appendixsec ") {
                self.result.push(self.make_heading(2, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@subsection ") {
                self.result.push(self.make_heading(3, rest.trim()));
                i += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@subsubsection ") {
                self.result.push(self.make_heading(4, rest.trim()));
                i += 1;
                continue;
            }

            // Handle lists
            if line.starts_with("@itemize") || line.starts_with("@enumerate") {
                let ordered = line.starts_with("@enumerate");
                let (list_node, end_line) = self.parse_list(&lines, i, ordered);
                self.result.push(list_node);
                i = end_line;
                continue;
            }

            // Handle definition lists (@table)
            if line.starts_with("@table") {
                let (table_node, end_line) = self.parse_table(&lines, i);
                self.result.push(table_node);
                i = end_line;
                continue;
            }

            // Handle code blocks
            if line.starts_with("@example") || line.starts_with("@verbatim") {
                let end_marker = if line.starts_with("@example") {
                    "@end example"
                } else {
                    "@end verbatim"
                };
                let (code_node, end_line) = self.parse_code_block(&lines, i, end_marker);
                self.result.push(code_node);
                i = end_line;
                continue;
            }

            // Handle quotations
            if line.starts_with("@quotation") {
                let (quote_node, end_line) = self.parse_quotation(&lines, i);
                self.result.push(quote_node);
                i = end_line;
                continue;
            }

            // Empty lines
            if line.is_empty() {
                i += 1;
                continue;
            }

            // Regular paragraph
            let (para_lines, end_line) = self.collect_paragraph(&lines, i);
            if !para_lines.is_empty() {
                let para_text = para_lines.join(" ");
                let inline_nodes = self.parse_inline(&para_text);
                if !inline_nodes.is_empty() {
                    self.result
                        .push(Node::new(node::PARAGRAPH).children(inline_nodes));
                }
            }
            i = end_line;
        }

        Ok(())
    }

    fn make_heading(&self, level: i64, text: &str) -> Node {
        let inline_nodes = self.parse_inline(text);
        Node::new(node::HEADING)
            .prop(prop::LEVEL, level)
            .children(inline_nodes)
    }

    fn collect_paragraph<'b>(&self, lines: &[&'b str], start: usize) -> (Vec<&'b str>, usize) {
        let mut para_lines = Vec::new();
        let mut i = start;

        while i < lines.len() {
            let line = lines[i].trim();

            // Stop at empty line or command
            if line.is_empty()
                || line.starts_with('@')
                    && !line.starts_with("@code{")
                    && !line.starts_with("@emph{")
                    && !line.starts_with("@strong{")
                    && !line.starts_with("@uref{")
                    && !line.starts_with("@url{")
                    && !line.starts_with("@xref{")
                    && !line.starts_with("@pxref{")
                    && !line.starts_with("@ref{")
                    && !line.starts_with("@samp{")
                    && !line.starts_with("@var{")
                    && !line.starts_with("@file{")
                    && !line.starts_with("@dfn{")
                    && !line.starts_with("@kbd{")
                    && !line.starts_with("@key{")
                    && !line.starts_with("@acronym{")
                    && !line.starts_with("@email{")
            {
                break;
            }

            para_lines.push(line);
            i += 1;
        }

        (para_lines, i)
    }

    fn parse_list(&self, lines: &[&str], start: usize, ordered: bool) -> (Node, usize) {
        let mut items = Vec::new();
        let mut i = start + 1; // Skip @itemize/@enumerate line
        let mut current_item: Vec<String> = Vec::new();

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("@end itemize") || line.starts_with("@end enumerate") {
                // Flush current item
                if !current_item.is_empty() {
                    let text = current_item.join(" ");
                    let inline = self.parse_inline(&text);
                    items.push(Node::new(node::LIST_ITEM).children(inline));
                }
                return (
                    Node::new(node::LIST)
                        .prop(prop::ORDERED, ordered)
                        .children(items),
                    i + 1,
                );
            }

            if line.starts_with("@item") {
                // Flush previous item
                if !current_item.is_empty() {
                    let text = current_item.join(" ");
                    let inline = self.parse_inline(&text);
                    items.push(Node::new(node::LIST_ITEM).children(inline));
                    current_item.clear();
                }

                // Get content after @item
                let rest = line.strip_prefix("@item").unwrap().trim();
                if !rest.is_empty() {
                    current_item.push(rest.to_string());
                }
            } else if !line.is_empty() && !line.starts_with("@c ") {
                current_item.push(line.to_string());
            }

            i += 1;
        }

        // No end marker found - return what we have
        if !current_item.is_empty() {
            let text = current_item.join(" ");
            let inline = self.parse_inline(&text);
            items.push(Node::new(node::LIST_ITEM).children(inline));
        }

        (
            Node::new(node::LIST)
                .prop(prop::ORDERED, ordered)
                .children(items),
            i,
        )
    }

    fn parse_table(&self, lines: &[&str], start: usize) -> (Node, usize) {
        let mut items = Vec::new();
        let mut i = start + 1; // Skip @table line
        let mut current_term: Option<String> = None;
        let mut current_desc: Vec<String> = Vec::new();

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("@end table") {
                // Flush current entry
                if let Some(term) = current_term.take() {
                    items.push(Node::new(node::DEFINITION_TERM).children(self.parse_inline(&term)));
                    if !current_desc.is_empty() {
                        let desc_text = current_desc.join(" ");
                        items.push(Node::new(node::DEFINITION_DESC).child(
                            Node::new(node::PARAGRAPH).children(self.parse_inline(&desc_text)),
                        ));
                        current_desc.clear();
                    }
                }
                return (Node::new(node::DEFINITION_LIST).children(items), i + 1);
            }

            if line.starts_with("@item ") {
                // Flush previous entry
                if let Some(term) = current_term.take() {
                    items.push(Node::new(node::DEFINITION_TERM).children(self.parse_inline(&term)));
                    if !current_desc.is_empty() {
                        let desc_text = current_desc.join(" ");
                        items.push(Node::new(node::DEFINITION_DESC).child(
                            Node::new(node::PARAGRAPH).children(self.parse_inline(&desc_text)),
                        ));
                        current_desc.clear();
                    }
                }

                let rest = line.strip_prefix("@item ").unwrap().trim();
                current_term = Some(rest.to_string());
            } else if !line.is_empty() && !line.starts_with("@c ") && !line.starts_with("@itemx ") {
                current_desc.push(line.to_string());
            }

            i += 1;
        }

        (Node::new(node::DEFINITION_LIST).children(items), i)
    }

    fn parse_code_block(&self, lines: &[&str], start: usize, end_marker: &str) -> (Node, usize) {
        let mut code_lines = Vec::new();
        let mut i = start + 1; // Skip @example/@verbatim line

        while i < lines.len() {
            let line = lines[i];

            if line.trim() == end_marker {
                let content = code_lines.join("\n");
                return (
                    Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content),
                    i + 1,
                );
            }

            code_lines.push(line);
            i += 1;
        }

        let content = code_lines.join("\n");
        (Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content), i)
    }

    fn parse_quotation(&self, lines: &[&str], start: usize) -> (Node, usize) {
        let mut quote_lines = Vec::new();
        let mut i = start + 1; // Skip @quotation line

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("@end quotation") {
                let text = quote_lines.join(" ");
                let inline = self.parse_inline(&text);
                return (
                    Node::new(node::BLOCKQUOTE).child(Node::new(node::PARAGRAPH).children(inline)),
                    i + 1,
                );
            }

            if !line.is_empty() {
                quote_lines.push(line);
            }

            i += 1;
        }

        let text = quote_lines.join(" ");
        let inline = self.parse_inline(&text);
        (
            Node::new(node::BLOCKQUOTE).child(Node::new(node::PARAGRAPH).children(inline)),
            i,
        )
    }

    fn parse_inline(&self, text: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '@' && i + 1 < chars.len() {
                // Check for inline commands
                if let Some((node, end_pos)) = self.try_parse_inline_command(&chars, i) {
                    if !current.is_empty() {
                        nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                        current.clear();
                    }
                    nodes.push(node);
                    i = end_pos;
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

    fn try_parse_inline_command(&self, chars: &[char], start: usize) -> Option<(Node, usize)> {
        // Collect command name
        let mut cmd = String::new();
        let mut i = start + 1; // Skip @

        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            cmd.push(chars[i]);
            i += 1;
        }

        // Check if followed by {
        if i >= chars.len() || chars[i] != '{' {
            return None;
        }

        // Find matching }
        let content_start = i + 1;
        let mut depth = 1;
        i += 1;

        while i < chars.len() && depth > 0 {
            match chars[i] {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }

        let content: String = chars[content_start..i - 1].iter().collect();

        let node = match cmd.as_str() {
            "emph" | "i" => Node::new(node::EMPHASIS).children(self.parse_inline(&content)),

            "strong" | "b" => Node::new(node::STRONG).children(self.parse_inline(&content)),

            "code" | "samp" | "kbd" | "key" | "file" | "command" | "option" | "env" => {
                Node::new(node::CODE).prop(prop::CONTENT, content)
            }

            "var" | "dfn" => Node::new(node::EMPHASIS).children(self.parse_inline(&content)),

            "uref" | "url" => {
                // Format: @uref{url} or @uref{url, text}
                let parts: Vec<&str> = content.splitn(2, ',').collect();
                let url = parts[0].trim();
                let text = if parts.len() > 1 {
                    parts[1].trim()
                } else {
                    url
                };
                Node::new(node::LINK)
                    .prop(prop::URL, url.to_string())
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
            }

            "email" => {
                let parts: Vec<&str> = content.splitn(2, ',').collect();
                let email = parts[0].trim();
                let text = if parts.len() > 1 {
                    parts[1].trim()
                } else {
                    email
                };
                Node::new(node::LINK)
                    .prop(prop::URL, format!("mailto:{}", email))
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
            }

            "xref" | "pxref" | "ref" => {
                // Cross-reference - just use the node name as link
                let parts: Vec<&str> = content.splitn(2, ',').collect();
                let node_name = parts[0].trim();
                Node::new(node::LINK)
                    .prop(prop::URL, format!("#{}", node_name))
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, node_name.to_string()))
            }

            "acronym" | "abbr" => {
                let parts: Vec<&str> = content.splitn(2, ',').collect();
                Node::new(node::TEXT).prop(prop::CONTENT, parts[0].trim().to_string())
            }

            "sc" => {
                // Small caps - just use text
                Node::new(node::SPAN)
                    .prop("style:text-transform", "small-caps")
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, content))
            }

            "footnote" => Node::new(node::FOOTNOTE_DEF)
                .child(Node::new(node::PARAGRAPH).children(self.parse_inline(&content))),

            _ => {
                // Unknown command - return as text
                return None;
            }
        };

        Some((node, i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let input = r#"@chapter Introduction
This is the introduction paragraph.

@section Getting Started
Here is how to get started."#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_headings() {
        let input = r#"@chapter Chapter One
@section Section One
@subsection Subsection One
@subsubsection Sub-subsection"#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 4);
    }

    #[test]
    fn test_parse_emphasis() {
        let input = r#"This is @emph{emphasized} and @strong{bold} text."#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_code() {
        let input = r#"Use @code{printf} to print."#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_list() {
        let input = r#"@itemize
@item First item
@item Second item
@end itemize"#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(doc.content.children[0].kind.as_str(), node::LIST);
    }

    #[test]
    fn test_parse_enumerate() {
        let input = r#"@enumerate
@item First
@item Second
@end enumerate"#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        let list = &doc.content.children[0];
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(true));
    }

    #[test]
    fn test_parse_example() {
        let input = r#"@example
int main() {
    return 0;
}
@end example"#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(doc.content.children[0].kind.as_str(), node::CODE_BLOCK);
    }

    #[test]
    fn test_parse_url() {
        let input = r#"Visit @uref{https://example.com, Example Site}."#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_quotation() {
        let input = r#"@quotation
This is a quoted passage.
@end quotation"#;

        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(doc.content.children[0].kind.as_str(), node::BLOCKQUOTE);
    }

    #[test]
    fn test_skip_comments() {
        let input = r#"@c This is a comment
This is visible.
@comment Another comment
Still visible."#;

        let result = parse(input).unwrap();
        let doc = result.value;
        // Should have 2 paragraphs (the visible text), not 4
        assert!(!doc.content.children.is_empty());
    }
}
