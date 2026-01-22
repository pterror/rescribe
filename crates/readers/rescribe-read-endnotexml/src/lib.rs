//! EndNote XML reader for rescribe.
//!
//! Parses EndNote XML bibliography files into rescribe's document IR.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_read_endnotexml::parse;
//!
//! let xml = r#"<?xml version="1.0"?>
//! <xml><records><record>...</record></records></xml>"#;
//! let result = parse(xml).unwrap();
//! ```

use quick_xml::Reader;
use quick_xml::events::Event;
use rescribe_core::{ConversionResult, Document, Node, ParseError, ParseOptions, Properties};
use rescribe_std::{node, prop};

/// Parse EndNote XML into a document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse EndNote XML with options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut buf = Vec::new();
    let mut current_entry: Option<EndNoteEntry> = None;
    let mut _current_element = String::new();
    let mut current_text = String::new();
    let mut in_authors = false;
    let mut in_keywords = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                _current_element = name.clone();
                current_text.clear();

                match name.as_str() {
                    "record" => {
                        current_entry = Some(EndNoteEntry::new());
                    }
                    "authors" | "contributors" => {
                        in_authors = true;
                    }
                    "keywords" => {
                        in_keywords = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if let Some(ref mut entry) = current_entry {
                    match name.as_str() {
                        "record" => {
                            if let Some(e) = current_entry.take() {
                                entries.push(e.into_node());
                            }
                        }
                        "authors" | "contributors" => {
                            in_authors = false;
                        }
                        "keywords" => {
                            in_keywords = false;
                        }
                        "author" if in_authors => {
                            if !current_text.is_empty() {
                                entry.authors.push(current_text.clone());
                            }
                        }
                        "keyword" if in_keywords => {
                            if !current_text.is_empty() {
                                entry.keywords.push(current_text.clone());
                            }
                        }
                        "ref-type" => {
                            entry.ref_type = current_text.clone();
                        }
                        "title" | "secondary-title" | "tertiary-title" => {
                            if entry.title.is_empty() && name == "title" {
                                entry.title = current_text.clone();
                            } else if name == "secondary-title" {
                                entry.journal = current_text.clone();
                            }
                        }
                        "year" => {
                            entry.year = current_text.clone();
                        }
                        "volume" => {
                            entry.volume = current_text.clone();
                        }
                        "number" => {
                            entry.number = current_text.clone();
                        }
                        "pages" => {
                            entry.pages = current_text.clone();
                        }
                        "publisher" => {
                            entry.publisher = current_text.clone();
                        }
                        "isbn" | "issn" => {
                            entry.isbn = current_text.clone();
                        }
                        "electronic-resource-num" => {
                            // This is typically the DOI
                            entry.doi = current_text.clone();
                        }
                        "url" | "web-urls" => {
                            if entry.url.is_empty() {
                                entry.url = current_text.clone();
                            }
                        }
                        "abstract" => {
                            entry.abstract_text = current_text.clone();
                        }
                        "label" | "rec-number" => {
                            if entry.key.is_empty() {
                                entry.key = current_text.clone();
                            }
                        }
                        _ => {}
                    }
                }
                current_text.clear();
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                current_text.push_str(&text);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParseError::Invalid(format!("XML error: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    let content = if entries.is_empty() {
        Node::new(node::DOCUMENT)
    } else {
        Node::new(node::DOCUMENT).child(Node::new(node::DEFINITION_LIST).children(entries))
    };

    Ok(ConversionResult::ok(Document {
        content,
        resources: Default::default(),
        metadata: Properties::new(),
        source: None,
    }))
}

/// EndNote entry being parsed.
struct EndNoteEntry {
    ref_type: String,
    key: String,
    title: String,
    authors: Vec<String>,
    journal: String,
    year: String,
    volume: String,
    number: String,
    pages: String,
    publisher: String,
    isbn: String,
    doi: String,
    url: String,
    abstract_text: String,
    keywords: Vec<String>,
}

impl EndNoteEntry {
    fn new() -> Self {
        Self {
            ref_type: String::new(),
            key: String::new(),
            title: String::new(),
            authors: Vec::new(),
            journal: String::new(),
            year: String::new(),
            volume: String::new(),
            number: String::new(),
            pages: String::new(),
            publisher: String::new(),
            isbn: String::new(),
            doi: String::new(),
            url: String::new(),
            abstract_text: String::new(),
            keywords: Vec::new(),
        }
    }

    fn into_node(self) -> Node {
        let cite_key = if self.key.is_empty() {
            self.generate_cite_key()
        } else {
            self.key.clone()
        };

        let bibtex_type = endnote_type_to_bibtex(&self.ref_type);

        // Create the term (citation key)
        let term = Node::new(node::DEFINITION_TERM)
            .child(Node::new(node::CODE).prop(prop::CONTENT, cite_key.clone()));

        // Build the description content
        let mut desc_children = Vec::new();

        // Entry type badge
        let type_text = format!("[{}] ", bibtex_type);
        desc_children.push(
            Node::new(node::SPAN)
                .prop("html:class", "endnote-type")
                .child(Node::new(node::TEXT).prop(prop::CONTENT, type_text)),
        );

        // Authors
        if !self.authors.is_empty() {
            let author_text = self.authors.join("; ");
            desc_children.push(
                Node::new(node::STRONG)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, author_text)),
            );
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }

        // Title
        if !self.title.is_empty() {
            desc_children.push(
                Node::new(node::EMPHASIS)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, self.title.clone())),
            );
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }

        // Journal
        if !self.journal.is_empty() {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, self.journal.clone()));
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
        }

        // Volume and number
        if !self.volume.is_empty() {
            let vol_text = if !self.number.is_empty() {
                format!("{}({}).", self.volume, self.number)
            } else {
                format!("{}.", self.volume)
            };
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, vol_text));
        }

        // Year
        if !self.year.is_empty() {
            desc_children
                .push(Node::new(node::TEXT).prop(prop::CONTENT, format!(" ({})", self.year)));
        }

        // Pages
        if !self.pages.is_empty() {
            desc_children
                .push(Node::new(node::TEXT).prop(prop::CONTENT, format!(", pp. {}", self.pages)));
        }

        // DOI
        if !self.doi.is_empty() {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
            desc_children.push(
                Node::new(node::LINK)
                    .prop(prop::URL, format!("https://doi.org/{}", self.doi))
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, format!("doi:{}", self.doi))),
            );
        }

        // URL
        if !self.url.is_empty() {
            desc_children.push(Node::new(node::TEXT).prop(prop::CONTENT, ". "));
            desc_children.push(
                Node::new(node::LINK)
                    .prop(prop::URL, self.url.clone())
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, self.url.clone())),
            );
        }

        let desc = Node::new(node::DEFINITION_DESC)
            .prop("endnote:type", self.ref_type)
            .prop("endnote:key", cite_key)
            .child(Node::new(node::PARAGRAPH).children(desc_children));

        Node::new("endnote:entry").children(vec![term, desc])
    }

    fn generate_cite_key(&self) -> String {
        let author_part = self
            .authors
            .first()
            .map(|a| {
                a.split(',')
                    .next()
                    .unwrap_or(a)
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(8)
                    .collect::<String>()
                    .to_lowercase()
            })
            .unwrap_or_else(|| "unknown".to_string());

        format!("{}{}", author_part, self.year)
    }
}

/// Map EndNote reference types to BibTeX types.
fn endnote_type_to_bibtex(endnote_type: &str) -> &'static str {
    // EndNote uses numeric types, but also accepts names
    match endnote_type.to_lowercase().as_str() {
        "journal article" | "0" | "17" => "article",
        "book" | "6" => "book",
        "book section" | "5" => "incollection",
        "conference paper" | "conference proceedings" | "10" | "47" => "inproceedings",
        "thesis" | "32" => "phdthesis",
        "report" | "27" => "techreport",
        "web page" | "electronic source" | "12" | "16" => "online",
        "computer program" | "9" => "software",
        "dataset" | "59" => "dataset",
        "magazine article" | "19" => "article",
        "newspaper article" | "23" => "article",
        "manuscript" | "36" => "unpublished",
        "edited book" | "28" => "book",
        "patent" | "25" => "misc",
        _ => "misc",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xml>
  <records>
    <record>
      <ref-type>Journal Article</ref-type>
      <contributors>
        <authors>
          <author>Smith, John</author>
        </authors>
      </contributors>
      <titles>
        <title>A Great Paper</title>
        <secondary-title>Nature</secondary-title>
      </titles>
      <dates>
        <year>2020</year>
      </dates>
    </record>
  </records>
</xml>"#;

        let result = parse(xml).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_empty() {
        let xml = r#"<?xml version="1.0"?><xml><records></records></xml>"#;
        let result = parse(xml).unwrap();
        let doc = result.value;
        assert!(doc.content.children.is_empty());
    }

    #[test]
    fn test_parse_multiple_authors() {
        let xml = r#"<?xml version="1.0"?>
<xml>
  <records>
    <record>
      <ref-type>Journal Article</ref-type>
      <contributors>
        <authors>
          <author>Smith, John</author>
          <author>Doe, Jane</author>
        </authors>
      </contributors>
      <titles>
        <title>Collaborative Work</title>
      </titles>
      <dates>
        <year>2021</year>
      </dates>
    </record>
  </records>
</xml>"#;

        let result = parse(xml).unwrap();
        let doc = result.value;
        assert!(!doc.content.children.is_empty());
    }
}
