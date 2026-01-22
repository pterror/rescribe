//! Native format reader for rescribe.
//!
//! Parses the human-readable AST format back into a Document.
//! This is a simplified parser - it handles the basic structure
//! but may not parse all edge cases.

use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions};
use rescribe_std::Node;

/// Parse native format input into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse native format input into a document with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = NativeParser::new(input);
    let doc = parser.parse_document()?;
    Ok(ConversionResult::ok(doc))
}

struct NativeParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> NativeParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_document(&mut self) -> Result<Document, ParseError> {
        self.skip_whitespace();
        self.expect_str("Document")?;
        self.skip_whitespace();
        self.expect_char('{')?;

        let mut content = Node::new("document");

        while !self.is_at_end() {
            self.skip_whitespace();

            if self.peek() == Some('}') {
                self.advance();
                break;
            }

            // Look for content: or metadata: or resources:
            if self.check_str("content:") {
                self.expect_str("content:")?;
                self.skip_whitespace();
                content = self.parse_node()?;
            } else if self.check_str("metadata:") {
                // Skip metadata for now
                self.skip_until('}');
                self.advance();
            } else if self.check_str("resources:") {
                // Skip resources for now
                self.skip_until(']');
                self.advance();
            } else {
                // Try to parse a node directly
                content = self.parse_node()?;
            }
        }

        Ok(Document {
            content,
            resources: Default::default(),
            metadata: Default::default(),
            source: None,
        })
    }

    fn parse_node(&mut self) -> Result<Node, ParseError> {
        self.skip_whitespace();

        // Parse node kind
        let kind = self.parse_identifier()?;
        self.skip_whitespace();
        self.expect_char('(')?;

        let mut node = Node::new(kind.as_str());

        // Parse optional props
        self.skip_whitespace();
        if self.peek() == Some('{') {
            self.advance();
            while self.peek() != Some('}') && !self.is_at_end() {
                self.skip_whitespace();
                if self.peek() == Some('}') {
                    break;
                }

                let key = self.parse_identifier()?;
                self.skip_whitespace();
                self.expect_char(':')?;
                self.skip_whitespace();
                let value = self.parse_value()?;

                node = node.prop(&key, value);

                self.skip_whitespace();
                if self.peek() == Some(',') {
                    self.advance();
                }
            }
            self.expect_char('}')?;
        }

        self.skip_whitespace();
        self.expect_char(')')?;

        // Parse optional children
        self.skip_whitespace();
        if self.peek() == Some('[') {
            self.advance();
            while self.peek() != Some(']') && !self.is_at_end() {
                self.skip_whitespace();
                if self.peek() == Some(']') {
                    break;
                }
                let child = self.parse_node()?;
                node = node.child(child);
                self.skip_whitespace();
            }
            self.expect_char(']')?;
        }

        Ok(node)
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(ParseError::Invalid("expected identifier".into()));
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_value(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace();

        if self.peek() == Some('"') {
            // String value
            self.advance();
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c == '"' && !self.input[self.pos.saturating_sub(1)..self.pos].ends_with('\\') {
                    break;
                }
                self.advance();
            }
            let value = self.input[start..self.pos].replace("\\\"", "\"");
            self.expect_char('"')?;
            Ok(value)
        } else {
            // Number or boolean
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_alphanumeric() || c == '.' || c == '-' {
                    self.advance();
                } else {
                    break;
                }
            }
            Ok(self.input[start..self.pos].to_string())
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_until(&mut self, target: char) {
        while let Some(c) = self.peek() {
            if c == target {
                break;
            }
            self.advance();
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn expect_char(&mut self, expected: char) -> Result<(), ParseError> {
        if self.peek() == Some(expected) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::Invalid(format!(
                "expected '{}', got {:?}",
                expected,
                self.peek()
            )))
        }
    }

    fn expect_str(&mut self, expected: &str) -> Result<(), ParseError> {
        if self.input[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            Ok(())
        } else {
            Err(ParseError::Invalid(format!("expected '{}'", expected)))
        }
    }

    fn check_str(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let input = r#"Document {
  content:
  document() [
    paragraph() [
      text( { content: "Hello" })
    ]
  ]
}"#;
        let result = parse(input).unwrap();
        assert_eq!(result.value.content.kind.as_str(), "document");
        assert_eq!(result.value.content.children.len(), 1);
    }

    #[test]
    fn test_parse_node() {
        let mut parser = NativeParser::new("heading( { level: 1 })");
        let node = parser.parse_node().unwrap();
        assert_eq!(node.kind.as_str(), "heading");
    }
}
