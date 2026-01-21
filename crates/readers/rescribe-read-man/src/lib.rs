//! Man page (roff/troff) reader for rescribe.
//!
//! Parses Unix manual pages into rescribe's document IR.
//! Supports common man macros like .TH, .SH, .SS, .PP, .TP, .B, .I, etc.

use rescribe_core::{ConversionResult, Document, Node, ParseError, Properties};
use rescribe_std::{node, prop};

/// Parse man page source into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    let root = parser.parse_document();

    let mut metadata = Properties::new();
    if let Some(title) = parser.title.take() {
        metadata.set("title", title);
    }
    if let Some(section) = parser.section.take() {
        metadata.set("man:section", section);
    }

    let doc = Document {
        content: root,
        resources: Default::default(),
        metadata,
        source: None,
    };

    Ok(ConversionResult::ok(doc))
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
    title: Option<String>,
    section: Option<String>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let lines: Vec<&str> = input.lines().collect();
        Self {
            lines,
            pos: 0,
            title: None,
            section: None,
        }
    }

    fn parse_document(&mut self) -> Node {
        let mut children = Vec::new();

        while self.pos < self.lines.len() {
            if let Some(node) = self.parse_element() {
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

    fn parse_element(&mut self) -> Option<Node> {
        let line = self.current_line()?;

        // Skip empty lines and comments
        if line.is_empty() {
            self.advance();
            return None;
        }
        if line.starts_with(".\\\"") || line.starts_with("'\\\"") {
            self.advance();
            return None;
        }

        // Macro lines start with .
        if line.starts_with('.') {
            return self.parse_macro();
        }

        // Plain text paragraph
        Some(self.parse_text_block())
    }

    fn parse_macro(&mut self) -> Option<Node> {
        let line = self.current_line()?;
        self.advance();

        let (macro_name, args) = self.parse_macro_line(line);

        match macro_name.as_str() {
            // Title header
            "TH" => {
                if !args.is_empty() {
                    self.title = Some(args[0].clone());
                }
                if args.len() > 1 {
                    self.section = Some(args[1].clone());
                }
                // Create a heading with the title
                if let Some(title) = &self.title {
                    return Some(
                        Node::new(node::HEADING)
                            .prop(prop::LEVEL, 1i64)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, title.clone())),
                    );
                }
                None
            }

            // Section heading
            "SH" => {
                let text = args.join(" ");
                Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, 2i64)
                        .children(self.parse_inline_text(&text)),
                )
            }

            // Subsection heading
            "SS" => {
                let text = args.join(" ");
                Some(
                    Node::new(node::HEADING)
                        .prop(prop::LEVEL, 3i64)
                        .children(self.parse_inline_text(&text)),
                )
            }

            // Paragraph break
            "PP" | "P" | "LP" => {
                // Consume following text as paragraph
                self.parse_paragraph()
            }

            // Indented paragraph
            "IP" | "TP" => {
                // These create list-like structures
                // IP has an optional tag, TP has tag on next line
                let tag = if macro_name == "TP" {
                    // Tag is on the next line
                    self.current_line().map(|l| {
                        self.advance();
                        l.to_string()
                    })
                } else {
                    args.first().cloned()
                };

                // Content follows
                let content = self.collect_paragraph_text();
                let content_inline = self.parse_inline_text(&content);

                if let Some(tag) = tag {
                    // Create definition list item
                    Some(Node::new(node::DEFINITION_LIST).children(vec![
                            Node::new(node::DEFINITION_TERM)
                                .children(self.parse_inline_text(&tag)),
                            Node::new(node::DEFINITION_DESC).child(
                                Node::new(node::PARAGRAPH).children(content_inline),
                            ),
                        ]))
                } else {
                    Some(Node::new(node::PARAGRAPH).children(content_inline))
                }
            }

            // Relative indent start/end
            "RS" | "RE" => {
                // Skip these for now, they affect indentation
                None
            }

            // No-fill (preformatted)
            "nf" => Some(self.parse_preformatted()),

            // Bold text
            "B" => {
                let text = args.join(" ");
                Some(Node::new(node::PARAGRAPH).child(
                    Node::new(node::STRONG).child(Node::new(node::TEXT).prop(prop::CONTENT, text)),
                ))
            }

            // Italic text
            "I" => {
                let text = args.join(" ");
                Some(
                    Node::new(node::PARAGRAPH).child(
                        Node::new(node::EMPHASIS)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, text)),
                    ),
                )
            }

            // Bold-Roman alternation
            "BR" => self.parse_alternating(&args, true),

            // Italic-Roman alternation
            "IR" => self.parse_alternating(&args, false),

            // Roman-Bold alternation
            "RB" => self.parse_alternating(&args, true),

            // Roman-Italic alternation
            "RI" => self.parse_alternating(&args, false),

            // Bold-Italic alternation
            "BI" => {
                let mut children = Vec::new();
                let mut is_bold = true;
                for arg in &args {
                    if is_bold {
                        children.push(
                            Node::new(node::STRONG)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, arg.clone())),
                        );
                    } else {
                        children.push(
                            Node::new(node::EMPHASIS)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, arg.clone())),
                        );
                    }
                    is_bold = !is_bold;
                }
                Some(Node::new(node::PARAGRAPH).children(children))
            }

            // Italic-Bold alternation
            "IB" => {
                let mut children = Vec::new();
                let mut is_italic = true;
                for arg in &args {
                    if is_italic {
                        children.push(
                            Node::new(node::EMPHASIS)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, arg.clone())),
                        );
                    } else {
                        children.push(
                            Node::new(node::STRONG)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, arg.clone())),
                        );
                    }
                    is_italic = !is_italic;
                }
                Some(Node::new(node::PARAGRAPH).children(children))
            }

            // URL (groff extension)
            "URL" | "UR" => {
                let url = args.first().cloned().unwrap_or_default();
                let text = args.get(1).cloned().unwrap_or_else(|| url.clone());
                Some(
                    Node::new(node::PARAGRAPH).child(
                        Node::new(node::LINK)
                            .prop(prop::URL, url)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, text)),
                    ),
                )
            }

            // End URL
            "UE" => None,

            // Horizontal rule / break
            "sp" => Some(Node::new(node::HORIZONTAL_RULE)),

            // Other macros - ignore
            _ => None,
        }
    }

    fn parse_macro_line(&self, line: &str) -> (String, Vec<String>) {
        let line = &line[1..]; // Skip leading .
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quote = false;

        for c in line.chars() {
            match c {
                '"' => {
                    in_quote = !in_quote;
                }
                ' ' | '\t' if !in_quote => {
                    if !current.is_empty() {
                        parts.push(current);
                        current = String::new();
                    }
                }
                _ => {
                    current.push(c);
                }
            }
        }
        if !current.is_empty() {
            parts.push(current);
        }

        let macro_name = parts.first().cloned().unwrap_or_default();
        let args = parts.into_iter().skip(1).collect();

        (macro_name, args)
    }

    fn parse_paragraph(&mut self) -> Option<Node> {
        let text = self.collect_paragraph_text();
        if text.is_empty() {
            return None;
        }
        Some(Node::new(node::PARAGRAPH).children(self.parse_inline_text(&text)))
    }

    fn collect_paragraph_text(&mut self) -> String {
        let mut lines = Vec::new();

        while let Some(line) = self.current_line() {
            // Stop at macro lines or empty lines
            if line.is_empty() || line.starts_with('.') {
                break;
            }
            lines.push(line);
            self.advance();
        }

        lines.join(" ")
    }

    fn parse_text_block(&mut self) -> Node {
        let text = self.collect_paragraph_text();
        Node::new(node::PARAGRAPH).children(self.parse_inline_text(&text))
    }

    fn parse_preformatted(&mut self) -> Node {
        let mut lines = Vec::new();

        while let Some(line) = self.current_line() {
            if line == ".fi" {
                self.advance();
                break;
            }
            // Skip macro lines inside preformatted
            if !line.starts_with('.') {
                lines.push(line);
            }
            self.advance();
        }

        let content = lines.join("\n");
        Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content)
    }

    fn parse_alternating(&mut self, args: &[String], bold_first: bool) -> Option<Node> {
        let mut children = Vec::new();
        let mut use_style = bold_first;

        for arg in args {
            if use_style {
                children.push(
                    Node::new(if bold_first {
                        node::STRONG
                    } else {
                        node::EMPHASIS
                    })
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, arg.clone())),
                );
            } else {
                children.push(Node::new(node::TEXT).prop(prop::CONTENT, arg.clone()));
            }
            use_style = !use_style;
        }

        Some(Node::new(node::PARAGRAPH).children(children))
    }

    fn parse_inline_text(&self, text: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Font escape: \fX or \f(XX
            if i + 2 < chars.len() && chars[i] == '\\' && chars[i + 1] == 'f' {
                if !current.is_empty() {
                    nodes.push(Node::new(node::TEXT).prop(prop::CONTENT, current.clone()));
                    current.clear();
                }

                let font_char = chars[i + 2];
                i += 3;

                // Find the text until next font change or end
                let mut styled_text = String::new();
                while i < chars.len() {
                    if i + 2 < chars.len() && chars[i] == '\\' && chars[i + 1] == 'f' {
                        break;
                    }
                    styled_text.push(chars[i]);
                    i += 1;
                }

                if !styled_text.is_empty() {
                    let styled_node = match font_char {
                        'B' => Node::new(node::STRONG)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, styled_text)),
                        'I' => Node::new(node::EMPHASIS)
                            .child(Node::new(node::TEXT).prop(prop::CONTENT, styled_text)),
                        'R' | 'P' => Node::new(node::TEXT).prop(prop::CONTENT, styled_text),
                        _ => Node::new(node::TEXT).prop(prop::CONTENT, styled_text),
                    };
                    nodes.push(styled_node);
                }
                continue;
            }

            // Other escapes
            if i + 1 < chars.len() && chars[i] == '\\' {
                match chars[i + 1] {
                    '-' => current.push('-'),
                    '\\' => current.push('\\'),
                    'e' => current.push('\\'),
                    '&' => {} // Zero-width space, ignore
                    _ => {
                        current.push(chars[i]);
                        current.push(chars[i + 1]);
                    }
                }
                i += 2;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(input: &str) -> Document {
        parse(input).unwrap().value
    }

    #[test]
    fn test_parse_title() {
        let doc = parse_str(".TH TEST 1 \"2024-01-01\" \"Version 1.0\"");
        assert_eq!(doc.metadata.get_str("title"), Some("TEST"));
        assert_eq!(doc.metadata.get_str("man:section"), Some("1"));
    }

    #[test]
    fn test_parse_sections() {
        let doc = parse_str(".SH NAME\ntest \\- a test program\n.SH SYNOPSIS\ntest [options]");
        assert_eq!(doc.content.children.len(), 4); // 2 headings + 2 paragraphs
    }

    #[test]
    fn test_parse_bold() {
        let doc = parse_str(".B bold text");
        let para = &doc.content.children[0];
        assert_eq!(para.kind.as_str(), node::PARAGRAPH);
        assert_eq!(para.children[0].kind.as_str(), node::STRONG);
    }

    #[test]
    fn test_parse_italic() {
        let doc = parse_str(".I italic text");
        let para = &doc.content.children[0];
        assert_eq!(para.kind.as_str(), node::PARAGRAPH);
        assert_eq!(para.children[0].kind.as_str(), node::EMPHASIS);
    }

    #[test]
    fn test_parse_preformatted() {
        let doc = parse_str(".nf\ncode line 1\ncode line 2\n.fi");
        let code = &doc.content.children[0];
        assert_eq!(code.kind.as_str(), node::CODE_BLOCK);
    }

    #[test]
    fn test_parse_inline_font() {
        let doc = parse_str("This is \\fBbold\\fR text");
        let para = &doc.content.children[0];
        // Should have multiple children
        assert!(para.children.len() >= 2);
    }
}
