//! Fountain screenplay format reader for rescribe.
//!
//! Parses Fountain screenplay markup into rescribe's document IR.
//!
//! # Fountain Elements
//!
//! - Scene headings (INT./EXT.)
//! - Action
//! - Character and dialogue
//! - Parentheticals
//! - Transitions
//! - Title page metadata

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions, Properties};
use rescribe_std::{Node, node, prop};

/// Parse Fountain input into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Fountain input into a document with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input);
    let (content, metadata) = parser.parse();

    let document = Document {
        content: Node::new(node::DOCUMENT).children(content),
        resources: Default::default(),
        metadata,
        source: None,
    };

    Ok(ConversionResult::ok(document))
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
    metadata: Properties,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines().collect(),
            pos: 0,
            metadata: Properties::new(),
        }
    }

    fn parse(&mut self) -> (Vec<Node>, Properties) {
        let mut nodes = Vec::new();

        // Parse title page if present
        self.parse_title_page();

        // Parse screenplay body
        while self.pos < self.lines.len() {
            if let Some(node) = self.parse_element() {
                nodes.push(node);
            }
        }

        (nodes, std::mem::take(&mut self.metadata))
    }

    fn parse_title_page(&mut self) {
        // Valid title page fields
        let valid_fields = [
            "title",
            "credit",
            "author",
            "authors",
            "source",
            "draft date",
            "contact",
            "copyright",
            "notes",
        ];

        // Title page consists of key: value pairs at the start
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            // Empty line ends title page
            if line.trim().is_empty() {
                self.pos += 1;
                break;
            }

            // Check for key: value pattern
            if let Some((key, value)) = line.split_once(':') {
                let key_lower = key.trim().to_lowercase();

                // Only accept known title page fields
                if !valid_fields.contains(&key_lower.as_str()) {
                    break;
                }

                let value = value.trim();

                // Multi-line values are indented
                let mut full_value = value.to_string();
                self.pos += 1;

                while self.pos < self.lines.len() {
                    let next_line = self.lines[self.pos];
                    if next_line.starts_with("   ") || next_line.starts_with('\t') {
                        full_value.push('\n');
                        full_value.push_str(next_line.trim());
                        self.pos += 1;
                    } else {
                        break;
                    }
                }

                self.metadata.set(
                    format!("fountain:{}", key_lower.replace(' ', "_")),
                    rescribe_core::PropValue::String(full_value),
                );
            } else {
                // Not a title page element
                break;
            }
        }
    }

    fn parse_element(&mut self) -> Option<Node> {
        if self.pos >= self.lines.len() {
            return None;
        }

        let line = self.lines[self.pos];

        // Skip empty lines
        if line.trim().is_empty() {
            self.pos += 1;
            return None;
        }

        // Page break: ===
        if line.trim() == "===" {
            self.pos += 1;
            return Some(Node::new(node::HORIZONTAL_RULE).prop("fountain:type", "page_break"));
        }

        // Section: # heading
        if line.starts_with('#') {
            return Some(self.parse_section());
        }

        // Synopsis: = text
        if line.starts_with('=') && !line.starts_with("===") {
            return Some(self.parse_synopsis());
        }

        // Note: [[text]]
        if line.contains("[[") {
            return Some(self.parse_note());
        }

        // Centered text: >text<
        if line.starts_with('>') && line.ends_with('<') {
            return Some(self.parse_centered());
        }

        // Transition: text ending in TO: or starting with >
        if self.is_transition(line) {
            return Some(self.parse_transition());
        }

        // Scene heading
        if self.is_scene_heading(line) {
            return Some(self.parse_scene_heading());
        }

        // Character (all caps, possibly with dialogue following)
        if self.is_character(line) {
            return Some(self.parse_character_and_dialogue());
        }

        // Lyric: ~text
        if line.starts_with('~') {
            return Some(self.parse_lyric());
        }

        // Default: action
        Some(self.parse_action())
    }

    fn is_scene_heading(&self, line: &str) -> bool {
        let line = line.trim();
        // Forced scene heading
        if line.starts_with('.') && line.len() > 1 {
            return true;
        }
        // Standard scene heading prefixes
        let upper = line.to_uppercase();
        upper.starts_with("INT ")
            || upper.starts_with("INT.")
            || upper.starts_with("EXT ")
            || upper.starts_with("EXT.")
            || upper.starts_with("INT/EXT")
            || upper.starts_with("I/E")
            || upper.starts_with("EST ")
            || upper.starts_with("EST.")
    }

    fn is_transition(&self, line: &str) -> bool {
        let line = line.trim();
        // Forced transition
        if line.starts_with('>') && !line.ends_with('<') {
            return true;
        }
        // Standard transitions end in TO:
        line.to_uppercase().ends_with("TO:") && line == line.to_uppercase()
    }

    fn is_character(&self, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() {
            return false;
        }
        // Forced character
        if line.starts_with('@') {
            return true;
        }
        // Must be all uppercase (allowing parentheticals like (V.O.))
        let name_part = if let Some(paren_pos) = line.find('(') {
            &line[..paren_pos]
        } else {
            line
        };
        let name_part = name_part.trim();
        !name_part.is_empty()
            && name_part
                .chars()
                .all(|c| c.is_uppercase() || c.is_whitespace() || c == '^')
            && name_part.chars().any(|c| c.is_alphabetic())
    }

    fn parse_scene_heading(&mut self) -> Node {
        let line = self.lines[self.pos].trim();
        self.pos += 1;

        // Remove forced marker if present
        let heading = line.strip_prefix('.').unwrap_or(line);

        Node::new(node::HEADING)
            .prop(prop::LEVEL, 2i64)
            .prop("fountain:type", "scene_heading")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, heading.to_string()))
    }

    fn parse_transition(&mut self) -> Node {
        let line = self.lines[self.pos].trim();
        self.pos += 1;

        // Remove forced marker if present
        let text = line.strip_prefix('>').map(|s| s.trim()).unwrap_or(line);

        Node::new(node::PARAGRAPH)
            .prop("fountain:type", "transition")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
    }

    fn parse_character_and_dialogue(&mut self) -> Node {
        let char_line = self.lines[self.pos].trim();
        self.pos += 1;

        // Remove forced marker if present
        let char_name = char_line.strip_prefix('@').unwrap_or(char_line);

        // Check for dual dialogue marker
        let dual = char_name.ends_with('^');
        let char_name = char_name.trim_end_matches('^').trim();

        let mut dialogue_node = Node::new(node::DIV)
            .prop("fountain:type", "dialogue_block")
            .child(
                Node::new(node::PARAGRAPH)
                    .prop("fountain:type", "character")
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, char_name.to_string())),
            );

        if dual {
            dialogue_node = dialogue_node.prop("fountain:dual", true);
        }

        // Parse dialogue and parentheticals
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos].trim();

            if line.is_empty() {
                break;
            }

            // Parenthetical
            if line.starts_with('(') && line.ends_with(')') {
                dialogue_node = dialogue_node.child(
                    Node::new(node::PARAGRAPH)
                        .prop("fountain:type", "parenthetical")
                        .child(Node::new(node::TEXT).prop(prop::CONTENT, line.to_string())),
                );
                self.pos += 1;
            }
            // Dialogue line
            else if !self.is_scene_heading(line)
                && !self.is_transition(line)
                && !self.is_character(line)
            {
                dialogue_node = dialogue_node.child(
                    Node::new(node::PARAGRAPH)
                        .prop("fountain:type", "dialogue")
                        .child(Node::new(node::TEXT).prop(prop::CONTENT, line.to_string())),
                );
                self.pos += 1;
            } else {
                break;
            }
        }

        dialogue_node
    }

    fn parse_action(&mut self) -> Node {
        let mut lines = Vec::new();

        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];

            if line.trim().is_empty() {
                self.pos += 1;
                break;
            }

            // Check if this starts a new element
            if self.is_scene_heading(line)
                || self.is_transition(line)
                || self.is_character(line)
                || line.starts_with('#')
                || line.starts_with('=')
                || line.starts_with('~')
                || line.contains("[[")
            {
                break;
            }

            // Handle forced action with !
            let text = line.strip_prefix('!').unwrap_or(line);

            lines.push(text.to_string());
            self.pos += 1;
        }

        let content = lines.join("\n");
        Node::new(node::PARAGRAPH)
            .prop("fountain:type", "action")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, content))
    }

    fn parse_section(&mut self) -> Node {
        let line = self.lines[self.pos];
        self.pos += 1;

        // Count # symbols for level
        let level = line.chars().take_while(|&c| c == '#').count() as i64;
        let text = line[level as usize..].trim();

        Node::new(node::HEADING)
            .prop(prop::LEVEL, level)
            .prop("fountain:type", "section")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
    }

    fn parse_synopsis(&mut self) -> Node {
        let line = self.lines[self.pos].trim();
        self.pos += 1;

        let text = line[1..].trim(); // Remove = prefix

        Node::new(node::PARAGRAPH)
            .prop("fountain:type", "synopsis")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
    }

    fn parse_note(&mut self) -> Node {
        let line = self.lines[self.pos];
        self.pos += 1;

        // Extract note content between [[ and ]]
        let start = line.find("[[").unwrap_or(0);
        let end = line.find("]]").unwrap_or(line.len());
        let note_text = &line[start + 2..end];

        Node::new(node::PARAGRAPH)
            .prop("fountain:type", "note")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, note_text.to_string()))
    }

    fn parse_centered(&mut self) -> Node {
        let line = self.lines[self.pos].trim();
        self.pos += 1;

        // Remove > and < markers
        let text = &line[1..line.len() - 1];

        Node::new(node::PARAGRAPH)
            .prop("fountain:type", "centered")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
    }

    fn parse_lyric(&mut self) -> Node {
        let line = self.lines[self.pos].trim();
        self.pos += 1;

        let text = &line[1..]; // Remove ~ prefix

        Node::new(node::PARAGRAPH)
            .prop("fountain:type", "lyric")
            .child(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title_page() {
        let input = "Title: My Screenplay\nAuthor: John Doe\n\nINT. HOUSE - DAY";
        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(doc.metadata.get_str("fountain:title").is_some());
    }

    #[test]
    fn test_parse_scene_heading() {
        let input = "INT. COFFEE SHOP - DAY";
        let result = parse(input).unwrap();
        let doc = result.value;
        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(
            doc.content.children[0].props.get_str("fountain:type"),
            Some("scene_heading")
        );
    }

    #[test]
    fn test_parse_dialogue() {
        let input = "JOHN\nHello, how are you?";
        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(
            doc.content.children[0].props.get_str("fountain:type"),
            Some("dialogue_block")
        );
    }

    #[test]
    fn test_parse_action() {
        let input = "The door slowly opens. A figure emerges from the shadows.";
        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(
            doc.content.children[0].props.get_str("fountain:type"),
            Some("action")
        );
    }

    #[test]
    fn test_parse_transition() {
        let input = "CUT TO:";
        let result = parse(input).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
        assert_eq!(
            doc.content.children[0].props.get_str("fountain:type"),
            Some("transition")
        );
    }
}
