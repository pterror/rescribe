//! RIS (Research Information Systems) writer for rescribe.
//!
//! Emits documents as RIS format for bibliographic data.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node};

/// RIS entry node type.
const RIS_ENTRY: &str = "ris:entry";
const BIBTEX_ENTRY: &str = "bibtex:entry";
const CITATION_ENTRY: &str = "citation_entry";

/// Emit a document as RIS.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as RIS with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();
    emit_nodes(&doc.content.children, &mut ctx);
    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
        }
    }

    fn write_tag(&mut self, tag: &str, value: &str) {
        self.output.push_str(tag);
        self.output.push_str("  - ");
        self.output.push_str(value);
        self.output.push('\n');
    }
}

fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_node(node, ctx);
    }
}

fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        "document" | "definition_list" => emit_nodes(&node.children, ctx),

        RIS_ENTRY => emit_ris_entry(node, ctx),
        BIBTEX_ENTRY => emit_bibtex_entry(node, ctx),
        CITATION_ENTRY => emit_citation_entry(node, ctx),

        _ => {
            // Check for known entry types
            if is_bibtex_type(node.kind.as_str()) {
                emit_typed_entry(node, ctx);
            } else {
                emit_nodes(&node.children, ctx);
            }
        }
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

fn emit_ris_entry(node: &Node, ctx: &mut EmitContext) {
    let ris_type = node.props.get_str("ris:type").unwrap_or("GEN");
    ctx.write_tag("TY", ris_type);

    // Emit all ris: prefixed properties
    for (key, value) in node.props.iter() {
        if let Some(tag) = key.strip_prefix("ris:")
            && tag != "type"
            && tag != "key"
            && let rescribe_core::PropValue::String(s) = value
        {
            ctx.write_tag(&tag.to_uppercase(), s);
        }
    }

    ctx.write_tag("ER", "");
    ctx.output.push('\n');
}

fn emit_bibtex_entry(node: &Node, ctx: &mut EmitContext) {
    let bibtex_type = node.props.get_str("bibtex:type").unwrap_or("misc");
    let ris_type = bibtex_type_to_ris(bibtex_type);

    ctx.write_tag("TY", ris_type);

    // Map bibtex fields to RIS tags
    emit_bibtex_fields(node, ctx);

    ctx.write_tag("ER", "");
    ctx.output.push('\n');
}

fn emit_bibtex_fields(node: &Node, ctx: &mut EmitContext) {
    for (key, value) in node.props.iter() {
        if let Some(field) = key.strip_prefix("bibtex:")
            && let rescribe_core::PropValue::String(s) = value
            && let Some(ris_tag) = bibtex_field_to_ris(field)
        {
            ctx.write_tag(ris_tag, s);
        }
    }
}

fn emit_citation_entry(node: &Node, ctx: &mut EmitContext) {
    let csl_type = node.props.get_str("type").unwrap_or("misc");
    let ris_type = csl_type_to_ris(csl_type);

    ctx.write_tag("TY", ris_type);

    // Map CSL fields to RIS
    if let Some(title) = node.props.get_str("title") {
        ctx.write_tag("TI", title);
    }
    if let Some(author) = node.props.get_str("author") {
        // Authors might be semicolon-separated
        for a in author.split(';') {
            ctx.write_tag("AU", a.trim());
        }
    }
    if let Some(container) = node.props.get_str("container-title") {
        ctx.write_tag("JO", container);
    }
    if let Some(issued) = node.props.get_str("issued") {
        ctx.write_tag("PY", issued);
    }
    if let Some(volume) = node.props.get_str("volume") {
        ctx.write_tag("VL", volume);
    }
    if let Some(page) = node.props.get_str("page") {
        // Pages might be in format "start-end"
        let parts: Vec<&str> = page.split('-').collect();
        if !parts.is_empty() {
            ctx.write_tag("SP", parts[0]);
        }
        if parts.len() > 1 {
            ctx.write_tag("EP", parts[1]);
        }
    }
    if let Some(doi) = node.props.get_str("DOI") {
        ctx.write_tag("DO", doi);
    }
    if let Some(url) = node.props.get_str("URL") {
        ctx.write_tag("UR", url);
    }
    if let Some(abs) = node.props.get_str("abstract") {
        ctx.write_tag("AB", abs);
    }

    ctx.write_tag("ER", "");
    ctx.output.push('\n');
}

fn emit_typed_entry(node: &Node, ctx: &mut EmitContext) {
    let bibtex_type = node.kind.as_str().to_lowercase();
    let ris_type = bibtex_type_to_ris(&bibtex_type);

    ctx.write_tag("TY", ris_type);

    // Emit standard fields
    let field_mappings = [
        ("author", "AU"),
        ("title", "TI"),
        ("journal", "JO"),
        ("booktitle", "T2"),
        ("year", "PY"),
        ("volume", "VL"),
        ("number", "IS"),
        ("pages", "SP"), // Note: pages needs special handling
        ("publisher", "PB"),
        ("address", "CY"),
        ("doi", "DO"),
        ("url", "UR"),
        ("abstract", "AB"),
        ("keywords", "KW"),
        ("isbn", "SN"),
        ("issn", "SN"),
    ];

    for (prop_name, ris_tag) in field_mappings {
        if let Some(value) = node.props.get_str(prop_name) {
            if prop_name == "author" {
                // Authors might be "and"-separated
                for author in value.split(" and ") {
                    ctx.write_tag(ris_tag, author.trim());
                }
            } else if prop_name == "pages" {
                // Handle page ranges
                let parts: Vec<&str> = value.split('-').collect();
                if !parts.is_empty() {
                    ctx.write_tag("SP", parts[0].trim());
                }
                if parts.len() > 1 {
                    ctx.write_tag("EP", parts[1].trim());
                }
            } else {
                ctx.write_tag(ris_tag, value);
            }
        }
    }

    ctx.write_tag("ER", "");
    ctx.output.push('\n');
}

fn bibtex_type_to_ris(bibtex: &str) -> &'static str {
    match bibtex.to_lowercase().as_str() {
        "article" => "JOUR",
        "book" => "BOOK",
        "incollection" | "inbook" => "CHAP",
        "inproceedings" | "conference" => "CONF",
        "phdthesis" => "THES",
        "mastersthesis" => "THES",
        "techreport" => "RPRT",
        "online" => "ELEC",
        "software" => "COMP",
        "dataset" => "DATA",
        "unpublished" => "UNPB",
        "booklet" => "PAMP",
        "proceedings" => "CONF",
        "manual" => "BOOK",
        _ => "GEN",
    }
}

fn bibtex_field_to_ris(field: &str) -> Option<&'static str> {
    match field {
        "author" => Some("AU"),
        "title" => Some("TI"),
        "journal" => Some("JO"),
        "booktitle" => Some("T2"),
        "year" => Some("PY"),
        "volume" => Some("VL"),
        "number" => Some("IS"),
        "publisher" => Some("PB"),
        "address" => Some("CY"),
        "doi" => Some("DO"),
        "url" => Some("UR"),
        "abstract" => Some("AB"),
        "keywords" => Some("KW"),
        "isbn" | "issn" => Some("SN"),
        "edition" => Some("ET"),
        "note" => Some("N1"),
        "type" | "key" => None,
        _ => None,
    }
}

fn csl_type_to_ris(csl: &str) -> &'static str {
    match csl {
        "article-journal" | "article-magazine" | "article-newspaper" => "JOUR",
        "book" => "BOOK",
        "chapter" => "CHAP",
        "paper-conference" => "CONF",
        "thesis" => "THES",
        "report" => "RPRT",
        "webpage" | "post-weblog" => "ELEC",
        "software" => "COMP",
        "dataset" => "DATA",
        _ => "GEN",
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
    fn test_emit_typed_entry() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("author", "Smith, John")
            .prop("title", "A Great Paper")
            .prop("journal", "Nature")
            .prop("year", "2020");

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("TY  - JOUR"));
        assert!(output.contains("AU  - Smith, John"));
        assert!(output.contains("TI  - A Great Paper"));
        assert!(output.contains("JO  - Nature"));
        assert!(output.contains("PY  - 2020"));
        assert!(output.contains("ER  -"));
    }

    #[test]
    fn test_emit_pages() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("title", "Test")
            .prop("pages", "123-456");

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("SP  - 123"));
        assert!(output.contains("EP  - 456"));
    }

    #[test]
    fn test_emit_multiple_authors() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("title", "Test")
            .prop("author", "Smith, John and Doe, Jane");

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("AU  - Smith, John"));
        assert!(output.contains("AU  - Doe, Jane"));
    }
}
