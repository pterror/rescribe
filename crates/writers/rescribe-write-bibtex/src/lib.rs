//! BibTeX writer for rescribe.
//!
//! Emits documents as BibTeX source. This writer expects documents containing
//! bibliographic entries with specific properties.

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::prop;

/// BibTeX entry types.
const BIBTEX_ENTRY: &str = "bibtex:entry";

/// Emit a document as BibTeX.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as BibTeX with custom options.
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

/// Emit context for tracking state during emission.
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

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }
}

/// Emit a sequence of nodes.
fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_node(node, ctx);
    }
}

/// Emit a single node.
fn emit_node(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        "document" => emit_nodes(&node.children, ctx),

        BIBTEX_ENTRY => emit_entry(node, ctx),

        // For backwards compatibility, also support generic citation entries
        "citation_entry" => emit_citation_entry(node, ctx),

        _ => {
            // Check if it might be a bibtex-like entry type
            if is_bibtex_type(node.kind.as_str()) {
                emit_typed_entry(node, ctx);
            } else {
                ctx.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                    format!("Unknown node type for BibTeX: {}", node.kind.as_str()),
                ));
                emit_nodes(&node.children, ctx);
            }
        }
    }
}

/// Check if a string is a known BibTeX entry type.
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

/// Emit a BibTeX entry.
fn emit_entry(node: &Node, ctx: &mut EmitContext) {
    let entry_type = node
        .props
        .get_str("bibtex:type")
        .unwrap_or("misc")
        .to_lowercase();
    let cite_key = node.props.get_str("bibtex:key").unwrap_or("unknown");

    ctx.write("@");
    ctx.write(&entry_type);
    ctx.write("{");
    ctx.write(cite_key);
    ctx.write(",\n");

    // Emit all bibtex: prefixed properties as fields
    emit_bibtex_fields(node, ctx);

    ctx.write("}\n\n");
}

/// Emit a typed entry (where the node kind is the entry type).
fn emit_typed_entry(node: &Node, ctx: &mut EmitContext) {
    let entry_type = node.kind.as_str().to_lowercase();
    let cite_key = node
        .props
        .get_str("key")
        .or(node.props.get_str(prop::ID))
        .unwrap_or("unknown");

    ctx.write("@");
    ctx.write(&entry_type);
    ctx.write("{");
    ctx.write(cite_key);
    ctx.write(",\n");

    emit_standard_fields(node, ctx);

    ctx.write("}\n\n");
}

/// Emit a citation entry with CSL-like properties.
fn emit_citation_entry(node: &Node, ctx: &mut EmitContext) {
    // Map CSL type to BibTeX type
    let csl_type = node.props.get_str("type").unwrap_or("misc");
    let entry_type = csl_to_bibtex_type(csl_type);
    let cite_key = node.props.get_str(prop::ID).unwrap_or("unknown");

    ctx.write("@");
    ctx.write(entry_type);
    ctx.write("{");
    ctx.write(cite_key);
    ctx.write(",\n");

    emit_csl_fields(node, ctx);

    ctx.write("}\n\n");
}

/// Map CSL types to BibTeX types.
fn csl_to_bibtex_type(csl_type: &str) -> &'static str {
    match csl_type {
        "article-journal" | "article-magazine" | "article-newspaper" => "article",
        "book" => "book",
        "chapter" => "incollection",
        "paper-conference" => "inproceedings",
        "thesis" => "phdthesis",
        "report" => "techreport",
        "webpage" | "post-weblog" => "online",
        "software" => "software",
        "dataset" => "dataset",
        _ => "misc",
    }
}

/// Emit fields from bibtex: prefixed properties.
fn emit_bibtex_fields(node: &Node, ctx: &mut EmitContext) {
    let mut fields: Vec<(&str, String)> = Vec::new();

    // Collect all bibtex: prefixed properties
    for (key, value) in node.props.iter() {
        if let Some(field_name) = key.strip_prefix("bibtex:")
            && field_name != "type"
            && field_name != "key"
            && let rescribe_core::PropValue::String(s) = value
        {
            fields.push((field_name, s.clone()));
        }
    }

    // Sort fields for consistent output
    fields.sort_by(|a, b| a.0.cmp(b.0));

    for (name, value) in fields {
        emit_field(name, &value, ctx);
    }
}

/// Emit standard BibTeX fields from properties.
fn emit_standard_fields(node: &Node, ctx: &mut EmitContext) {
    // Standard BibTeX fields
    let field_mappings = [
        ("author", "author"),
        ("title", "title"),
        ("journal", "journal"),
        ("booktitle", "booktitle"),
        ("year", "year"),
        ("volume", "volume"),
        ("number", "number"),
        ("pages", "pages"),
        ("publisher", "publisher"),
        ("address", "address"),
        ("edition", "edition"),
        ("editor", "editor"),
        ("series", "series"),
        ("month", "month"),
        ("note", "note"),
        ("doi", "doi"),
        ("url", "url"),
        ("isbn", "isbn"),
        ("issn", "issn"),
        ("abstract", "abstract"),
        ("keywords", "keywords"),
        ("institution", "institution"),
        ("school", "school"),
        ("howpublished", "howpublished"),
        ("organization", "organization"),
        ("chapter", "chapter"),
    ];

    for (prop_name, field_name) in field_mappings {
        if let Some(value) = node.props.get_str(prop_name) {
            emit_field(field_name, value, ctx);
        }
    }
}

/// Emit CSL fields mapped to BibTeX fields.
fn emit_csl_fields(node: &Node, ctx: &mut EmitContext) {
    // CSL to BibTeX field mappings
    if let Some(title) = node.props.get_str("title") {
        emit_field("title", title, ctx);
    }

    // Handle authors (could be array or string)
    if let Some(author) = node.props.get_str("author") {
        emit_field("author", author, ctx);
    }

    // Container title maps to journal/booktitle depending on type
    if let Some(container) = node.props.get_str("container-title") {
        let csl_type = node.props.get_str("type").unwrap_or("");
        if csl_type == "article-journal" {
            emit_field("journal", container, ctx);
        } else {
            emit_field("booktitle", container, ctx);
        }
    }

    // Date handling
    if let Some(year) = node.props.get_str("issued") {
        // Try to extract just the year
        let year_str = year.split('-').next().unwrap_or(year);
        emit_field("year", year_str, ctx);
    }

    // Other direct mappings
    let direct_mappings = [
        ("volume", "volume"),
        ("issue", "number"),
        ("page", "pages"),
        ("publisher", "publisher"),
        ("publisher-place", "address"),
        ("DOI", "doi"),
        ("URL", "url"),
        ("ISBN", "isbn"),
        ("ISSN", "issn"),
        ("abstract", "abstract"),
        ("note", "note"),
    ];

    for (csl_name, bibtex_name) in direct_mappings {
        if let Some(value) = node.props.get_str(csl_name) {
            emit_field(bibtex_name, value, ctx);
        }
    }
}

/// Emit a single BibTeX field.
fn emit_field(name: &str, value: &str, ctx: &mut EmitContext) {
    ctx.write("  ");
    ctx.write(name);
    ctx.write(" = {");
    ctx.write(&escape_bibtex(value));
    ctx.write("},\n");
}

/// Escape special BibTeX characters.
fn escape_bibtex(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // These characters have special meaning in BibTeX
            '#' | '$' | '%' | '&' | '_' => {
                result.push('\\');
                result.push(c);
            }
            // Preserve braces as they're used for grouping
            '{' | '}' => result.push(c),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::{Document, NodeKind};

    fn emit_str(doc: &Document) -> String {
        let result = emit(doc).unwrap();
        String::from_utf8(result.value).unwrap()
    }

    fn make_entry(entry_type: &str, key: &str, fields: Vec<(&str, &str)>) -> Node {
        let mut node = Node::new(NodeKind::from(BIBTEX_ENTRY))
            .prop("bibtex:type", entry_type)
            .prop("bibtex:key", key);
        for (name, value) in fields {
            node = node.prop(format!("bibtex:{}", name), value);
        }
        node
    }

    #[test]
    fn test_emit_article() {
        let entry = make_entry(
            "article",
            "smith2024",
            vec![
                ("author", "John Smith"),
                ("title", "A Great Paper"),
                ("journal", "Nature"),
                ("year", "2024"),
            ],
        );

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("@article{smith2024,"));
        assert!(output.contains("author = {John Smith},"));
        assert!(output.contains("title = {A Great Paper},"));
        assert!(output.contains("journal = {Nature},"));
        assert!(output.contains("year = {2024},"));
    }

    #[test]
    fn test_emit_book() {
        let entry = make_entry(
            "book",
            "knuth1997",
            vec![
                ("author", "Donald Knuth"),
                ("title", "The Art of Computer Programming"),
                ("publisher", "Addison-Wesley"),
                ("year", "1997"),
            ],
        );

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("@book{knuth1997,"));
        assert!(output.contains("author = {Donald Knuth},"));
    }

    #[test]
    fn test_escape_special_chars() {
        let entry = make_entry(
            "misc",
            "test",
            vec![("title", "100% Pure & Simple: A $10 Solution")],
        );

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("100\\% Pure \\& Simple: A \\$10 Solution"));
    }

    #[test]
    fn test_emit_typed_entry() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("key", "test2024")
            .prop("author", "Test Author")
            .prop("title", "Test Title")
            .prop("year", "2024");
        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry]));
        let output = emit_str(&doc);

        assert!(output.contains("@article{test2024,"));
        assert!(output.contains("author = {Test Author},"));
    }

    #[test]
    fn test_emit_multiple_entries() {
        let entry1 = make_entry("article", "first", vec![("title", "First")]);
        let entry2 = make_entry("book", "second", vec![("title", "Second")]);

        let doc = Document::new()
            .with_content(Node::new(NodeKind::from("document")).children(vec![entry1, entry2]));
        let output = emit_str(&doc);

        assert!(output.contains("@article{first,"));
        assert!(output.contains("@book{second,"));
    }
}
