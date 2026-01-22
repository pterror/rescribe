//! EndNote XML writer for rescribe.
//!
//! Emits documents as EndNote XML bibliography files.

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node};
use std::io::Cursor;

/// EndNote entry node type.
const ENDNOTE_ENTRY: &str = "endnote:entry";
const BIBTEX_ENTRY: &str = "bibtex:entry";
const RIS_ENTRY: &str = "ris:entry";
const CITATION_ENTRY: &str = "citation_entry";

/// Emit a document as EndNote XML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as EndNote XML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();
    ctx.write_document(doc)?;
    let warnings = std::mem::take(&mut ctx.warnings);
    Ok(ConversionResult::with_warnings(ctx.finish(), warnings))
}

struct EmitContext {
    writer: Writer<Cursor<Vec<u8>>>,
    warnings: Vec<FidelityWarning>,
    record_count: usize,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            writer: Writer::new(Cursor::new(Vec::new())),
            warnings: Vec::new(),
            record_count: 0,
        }
    }

    fn write_document(&mut self, doc: &Document) -> Result<(), EmitError> {
        // XML declaration
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Root element
        self.writer
            .write_event(Event::Start(BytesStart::new("xml")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Records element
        self.writer
            .write_event(Event::Start(BytesStart::new("records")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Write all entries
        self.write_nodes(&doc.content.children)?;

        // Close records
        self.writer
            .write_event(Event::End(BytesEnd::new("records")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Close root
        self.writer
            .write_event(Event::End(BytesEnd::new("xml")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn write_nodes(&mut self, nodes: &[Node]) -> Result<(), EmitError> {
        for node in nodes {
            self.write_node(node)?;
        }
        Ok(())
    }

    fn write_node(&mut self, node: &Node) -> Result<(), EmitError> {
        match node.kind.as_str() {
            "document" | "definition_list" => self.write_nodes(&node.children)?,
            ENDNOTE_ENTRY => self.write_endnote_entry(node)?,
            BIBTEX_ENTRY => self.write_bibtex_entry(node)?,
            RIS_ENTRY => self.write_ris_entry(node)?,
            CITATION_ENTRY => self.write_citation_entry(node)?,
            _ => {
                if is_bibtex_type(node.kind.as_str()) {
                    self.write_typed_entry(node)?;
                } else {
                    self.write_nodes(&node.children)?;
                }
            }
        }
        Ok(())
    }

    fn write_endnote_entry(&mut self, node: &Node) -> Result<(), EmitError> {
        self.record_count += 1;

        self.writer
            .write_event(Event::Start(BytesStart::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Record number
        self.write_element("rec-number", &self.record_count.to_string())?;

        // Reference type
        if let Some(ref_type) = node.props.get_str("endnote:type") {
            self.write_element("ref-type", ref_type)?;
        }

        // Write all endnote: prefixed properties
        for (key, value) in node.props.iter() {
            if let Some(field) = key.strip_prefix("endnote:")
                && field != "type"
                && field != "key"
                && let rescribe_core::PropValue::String(s) = value
            {
                self.write_element(field, s)?;
            }
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn write_bibtex_entry(&mut self, node: &Node) -> Result<(), EmitError> {
        self.record_count += 1;

        self.writer
            .write_event(Event::Start(BytesStart::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        self.write_element("rec-number", &self.record_count.to_string())?;

        // Convert bibtex type to EndNote type
        if let Some(bibtex_type) = node.props.get_str("bibtex:type") {
            let endnote_type = bibtex_to_endnote_type(bibtex_type);
            self.write_element("ref-type", endnote_type)?;
        }

        // Map bibtex fields
        self.write_bibtex_fields(node)?;

        self.writer
            .write_event(Event::End(BytesEnd::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn write_bibtex_fields(&mut self, node: &Node) -> Result<(), EmitError> {
        // Titles
        if let Some(title) = node.props.get_str("bibtex:title") {
            self.write_nested_element("titles", "title", title)?;
        }

        // Authors
        if let Some(author) = node.props.get_str("bibtex:author") {
            self.write_authors(author)?;
        }

        // Year
        if let Some(year) = node.props.get_str("bibtex:year") {
            self.write_nested_element("dates", "year", year)?;
        }

        // Journal -> secondary-title
        if let Some(journal) = node.props.get_str("bibtex:journal") {
            self.write_nested_element("periodical", "full-title", journal)?;
        }

        // Volume
        if let Some(volume) = node.props.get_str("bibtex:volume") {
            self.write_element("volume", volume)?;
        }

        // Number
        if let Some(number) = node.props.get_str("bibtex:number") {
            self.write_element("number", number)?;
        }

        // Pages
        if let Some(pages) = node.props.get_str("bibtex:pages") {
            self.write_element("pages", pages)?;
        }

        // Publisher
        if let Some(publisher) = node.props.get_str("bibtex:publisher") {
            self.write_element("publisher", publisher)?;
        }

        // DOI
        if let Some(doi) = node.props.get_str("bibtex:doi") {
            self.write_element("electronic-resource-num", doi)?;
        }

        // URL
        if let Some(url) = node.props.get_str("bibtex:url") {
            self.write_nested_element("urls", "web-urls", url)?;
        }

        // Abstract
        if let Some(abs) = node.props.get_str("bibtex:abstract") {
            self.write_element("abstract", abs)?;
        }

        Ok(())
    }

    fn write_ris_entry(&mut self, node: &Node) -> Result<(), EmitError> {
        self.record_count += 1;

        self.writer
            .write_event(Event::Start(BytesStart::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        self.write_element("rec-number", &self.record_count.to_string())?;

        // Convert RIS type to EndNote type
        if let Some(ris_type) = node.props.get_str("ris:type") {
            let endnote_type = ris_to_endnote_type(ris_type);
            self.write_element("ref-type", endnote_type)?;
        }

        // Map RIS fields
        for (key, value) in node.props.iter() {
            if let Some(tag) = key.strip_prefix("ris:")
                && tag != "type"
                && tag != "key"
                && let rescribe_core::PropValue::String(s) = value
                && let Some(endnote_field) = ris_tag_to_endnote(tag)
            {
                self.write_element(endnote_field, s)?;
            }
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn write_citation_entry(&mut self, node: &Node) -> Result<(), EmitError> {
        self.record_count += 1;

        self.writer
            .write_event(Event::Start(BytesStart::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        self.write_element("rec-number", &self.record_count.to_string())?;

        // Convert CSL type to EndNote type
        if let Some(csl_type) = node.props.get_str("type") {
            let endnote_type = csl_to_endnote_type(csl_type);
            self.write_element("ref-type", endnote_type)?;
        }

        // Map CSL fields
        if let Some(title) = node.props.get_str("title") {
            self.write_nested_element("titles", "title", title)?;
        }
        if let Some(author) = node.props.get_str("author") {
            self.write_authors(author)?;
        }
        if let Some(issued) = node.props.get_str("issued") {
            self.write_nested_element("dates", "year", issued)?;
        }
        if let Some(container) = node.props.get_str("container-title") {
            self.write_nested_element("periodical", "full-title", container)?;
        }
        if let Some(volume) = node.props.get_str("volume") {
            self.write_element("volume", volume)?;
        }
        if let Some(page) = node.props.get_str("page") {
            self.write_element("pages", page)?;
        }
        if let Some(doi) = node.props.get_str("DOI") {
            self.write_element("electronic-resource-num", doi)?;
        }
        if let Some(url) = node.props.get_str("URL") {
            self.write_nested_element("urls", "web-urls", url)?;
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn write_typed_entry(&mut self, node: &Node) -> Result<(), EmitError> {
        self.record_count += 1;

        self.writer
            .write_event(Event::Start(BytesStart::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        self.write_element("rec-number", &self.record_count.to_string())?;

        let endnote_type = bibtex_to_endnote_type(node.kind.as_str());
        self.write_element("ref-type", endnote_type)?;

        // Standard fields
        if let Some(title) = node.props.get_str("title") {
            self.write_nested_element("titles", "title", title)?;
        }
        if let Some(author) = node.props.get_str("author") {
            self.write_authors(author)?;
        }
        if let Some(year) = node.props.get_str("year") {
            self.write_nested_element("dates", "year", year)?;
        }
        if let Some(journal) = node.props.get_str("journal") {
            self.write_nested_element("periodical", "full-title", journal)?;
        }
        if let Some(volume) = node.props.get_str("volume") {
            self.write_element("volume", volume)?;
        }
        if let Some(number) = node.props.get_str("number") {
            self.write_element("number", number)?;
        }
        if let Some(pages) = node.props.get_str("pages") {
            self.write_element("pages", pages)?;
        }
        if let Some(publisher) = node.props.get_str("publisher") {
            self.write_element("publisher", publisher)?;
        }
        if let Some(doi) = node.props.get_str("doi") {
            self.write_element("electronic-resource-num", doi)?;
        }
        if let Some(url) = node.props.get_str("url") {
            self.write_nested_element("urls", "web-urls", url)?;
        }
        if let Some(abs) = node.props.get_str("abstract") {
            self.write_element("abstract", abs)?;
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("record")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn write_element(&mut self, name: &str, value: &str) -> Result<(), EmitError> {
        self.writer
            .write_event(Event::Start(BytesStart::new(name)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        self.writer
            .write_event(Event::Text(BytesText::new(value)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        self.writer
            .write_event(Event::End(BytesEnd::new(name)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        Ok(())
    }

    fn write_nested_element(
        &mut self,
        parent: &str,
        child: &str,
        value: &str,
    ) -> Result<(), EmitError> {
        self.writer
            .write_event(Event::Start(BytesStart::new(parent)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        self.write_element(child, value)?;
        self.writer
            .write_event(Event::End(BytesEnd::new(parent)))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        Ok(())
    }

    fn write_authors(&mut self, authors: &str) -> Result<(), EmitError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("contributors")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("authors")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        // Split by " and " or ";"
        let author_list: Vec<&str> = if authors.contains(" and ") {
            authors.split(" and ").collect()
        } else {
            authors.split(';').collect()
        };

        for author in author_list {
            self.write_element("author", author.trim())?;
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("authors")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("contributors")))
            .map_err(|e| EmitError::Io(std::io::Error::other(format!("XML error: {}", e))))?;

        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.writer.into_inner().into_inner()
    }
}

fn is_bibtex_type(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "article"
            | "book"
            | "booklet"
            | "conference"
            | "inbook"
            | "incollection"
            | "inproceedings"
            | "manual"
            | "mastersthesis"
            | "misc"
            | "phdthesis"
            | "proceedings"
            | "techreport"
            | "unpublished"
            | "online"
            | "software"
            | "dataset"
    )
}

fn bibtex_to_endnote_type(bibtex: &str) -> &'static str {
    match bibtex.to_lowercase().as_str() {
        "article" => "Journal Article",
        "book" => "Book",
        "incollection" | "inbook" => "Book Section",
        "inproceedings" | "conference" => "Conference Paper",
        "phdthesis" => "Thesis",
        "mastersthesis" => "Thesis",
        "techreport" => "Report",
        "online" => "Web Page",
        "software" => "Computer Program",
        "dataset" => "Dataset",
        "unpublished" => "Manuscript",
        "booklet" => "Book",
        "proceedings" => "Conference Proceedings",
        "manual" => "Book",
        _ => "Generic",
    }
}

fn ris_to_endnote_type(ris: &str) -> &'static str {
    match ris {
        "JOUR" => "Journal Article",
        "BOOK" => "Book",
        "CHAP" | "SECT" => "Book Section",
        "CONF" | "CPAPER" => "Conference Paper",
        "THES" => "Thesis",
        "RPRT" => "Report",
        "ELEC" | "WEB" => "Web Page",
        "COMP" => "Computer Program",
        "DATA" => "Dataset",
        "MGZN" | "NEWS" => "Magazine Article",
        "UNPB" => "Manuscript",
        _ => "Generic",
    }
}

fn csl_to_endnote_type(csl: &str) -> &'static str {
    match csl {
        "article-journal" | "article-magazine" | "article-newspaper" => "Journal Article",
        "book" => "Book",
        "chapter" => "Book Section",
        "paper-conference" => "Conference Paper",
        "thesis" => "Thesis",
        "report" => "Report",
        "webpage" | "post-weblog" => "Web Page",
        "software" => "Computer Program",
        "dataset" => "Dataset",
        _ => "Generic",
    }
}

fn ris_tag_to_endnote(tag: &str) -> Option<&'static str> {
    match tag.to_uppercase().as_str() {
        "TI" | "T1" => Some("title"),
        "AU" | "A1" => Some("author"),
        "PY" | "Y1" => Some("year"),
        "JO" | "JF" | "T2" => Some("secondary-title"),
        "VL" => Some("volume"),
        "IS" => Some("number"),
        "SP" | "EP" => Some("pages"),
        "PB" => Some("publisher"),
        "DO" => Some("electronic-resource-num"),
        "UR" => Some("url"),
        "AB" => Some("abstract"),
        "KW" => Some("keyword"),
        "SN" => Some("isbn"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::NodeKind;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_empty_document() {
        let doc = Document::new();
        let output = emit_str(&doc);
        assert!(output.contains("<?xml"));
        assert!(output.contains("<records"));
        assert!(output.contains("</records>"));
    }

    #[test]
    fn test_emit_typed_entry() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("author", "Smith, John")
            .prop("title", "A Great Paper")
            .prop("journal", "Nature")
            .prop("year", "2020");

        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(entry));

        let output = emit_str(&doc);
        assert!(output.contains("<record>"));
        assert!(output.contains("<ref-type>Journal Article</ref-type>"));
        assert!(output.contains("<author>Smith, John</author>"));
        assert!(output.contains("<title>A Great Paper</title>"));
        assert!(output.contains("<year>2020</year>"));
    }

    #[test]
    fn test_emit_multiple_authors() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("author", "Smith, John and Doe, Jane")
            .prop("title", "Collaborative Work");

        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(entry));

        let output = emit_str(&doc);
        assert!(output.contains("<author>Smith, John</author>"));
        assert!(output.contains("<author>Doe, Jane</author>"));
    }
}
