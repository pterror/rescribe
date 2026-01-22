//! BibLaTeX writer for rescribe.
//!
//! Emits documents as BibLaTeX source with BibLaTeX-specific fields
//! (date, journaltitle, subtitle, etc.).

use rescribe_core::{
    ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node, Severity,
    WarningKind,
};
use rescribe_std::prop;

/// BibLaTeX entry node type.
const BIBLATEX_ENTRY: &str = "biblatex:entry";
const BIBTEX_ENTRY: &str = "bibtex:entry";

/// Emit a document as BibLaTeX.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as BibLaTeX with options.
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

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
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
        BIBLATEX_ENTRY => emit_biblatex_entry(node, ctx),
        BIBTEX_ENTRY => emit_bibtex_entry(node, ctx),
        "citation_entry" => emit_citation_entry(node, ctx),
        _ => {
            if is_biblatex_type(node.kind.as_str()) {
                emit_typed_entry(node, ctx);
            } else {
                ctx.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(node.kind.as_str().to_string()),
                    format!("Unknown node type for BibLaTeX: {}", node.kind.as_str()),
                ));
                emit_nodes(&node.children, ctx);
            }
        }
    }
}

fn is_biblatex_type(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "article"
            | "book"
            | "mvbook"
            | "inbook"
            | "bookinbook"
            | "suppbook"
            | "booklet"
            | "collection"
            | "mvcollection"
            | "incollection"
            | "suppcollection"
            | "manual"
            | "misc"
            | "online"
            | "patent"
            | "periodical"
            | "suppperiodical"
            | "proceedings"
            | "mvproceedings"
            | "inproceedings"
            | "reference"
            | "mvreference"
            | "inreference"
            | "report"
            | "set"
            | "software"
            | "thesis"
            | "unpublished"
            | "xdata"
            | "dataset"
    )
}

fn emit_biblatex_entry(node: &Node, ctx: &mut EmitContext) {
    let entry_type = node
        .props
        .get_str("biblatex:type")
        .unwrap_or("misc")
        .to_lowercase();
    let cite_key = node.props.get_str("biblatex:key").unwrap_or("unknown");

    ctx.write("@");
    ctx.write(&entry_type);
    ctx.write("{");
    ctx.write(cite_key);
    ctx.write(",\n");

    emit_biblatex_fields(node, ctx);

    ctx.write("}\n\n");
}

fn emit_bibtex_entry(node: &Node, ctx: &mut EmitContext) {
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

    // Convert BibTeX fields to BibLaTeX style
    emit_bibtex_to_biblatex_fields(node, ctx);

    ctx.write("}\n\n");
}

fn emit_citation_entry(node: &Node, ctx: &mut EmitContext) {
    let csl_type = node.props.get_str("type").unwrap_or("misc");
    let entry_type = csl_to_biblatex_type(csl_type);
    let cite_key = node.props.get_str(prop::ID).unwrap_or("unknown");

    ctx.write("@");
    ctx.write(entry_type);
    ctx.write("{");
    ctx.write(cite_key);
    ctx.write(",\n");

    emit_csl_fields(node, ctx);

    ctx.write("}\n\n");
}

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

fn csl_to_biblatex_type(csl: &str) -> &'static str {
    match csl {
        "article-journal" | "article-magazine" | "article-newspaper" => "article",
        "book" => "book",
        "chapter" => "incollection",
        "paper-conference" => "inproceedings",
        "thesis" => "thesis",
        "report" => "report",
        "webpage" | "post-weblog" => "online",
        "software" => "software",
        "dataset" => "dataset",
        "patent" => "patent",
        _ => "misc",
    }
}

fn emit_biblatex_fields(node: &Node, ctx: &mut EmitContext) {
    let mut fields: Vec<(&str, String)> = Vec::new();

    for (key, value) in node.props.iter() {
        if let Some(field_name) = key.strip_prefix("biblatex:")
            && field_name != "type"
            && field_name != "key"
            && let rescribe_core::PropValue::String(s) = value
        {
            fields.push((field_name, s.clone()));
        }
    }

    fields.sort_by(|a, b| a.0.cmp(b.0));

    for (name, value) in fields {
        emit_field(name, &value, ctx);
    }
}

fn emit_bibtex_to_biblatex_fields(node: &Node, ctx: &mut EmitContext) {
    // BibLaTeX field mappings from BibTeX
    let field_mappings = [
        ("bibtex:journal", "journaltitle"),
        ("bibtex:year", "date"),
        ("bibtex:address", "location"),
    ];

    for (bibtex_field, biblatex_field) in field_mappings {
        if let Some(value) = node.props.get_str(bibtex_field) {
            emit_field(biblatex_field, value, ctx);
        }
    }

    // Direct mappings (same field name in both)
    for (key, value) in node.props.iter() {
        if let Some(field_name) = key.strip_prefix("bibtex:")
            && field_name != "type"
            && field_name != "key"
            && field_name != "journal"
            && field_name != "year"
            && field_name != "address"
            && let rescribe_core::PropValue::String(s) = value
        {
            emit_field(field_name, s, ctx);
        }
    }
}

fn emit_csl_fields(node: &Node, ctx: &mut EmitContext) {
    if let Some(title) = node.props.get_str("title") {
        emit_field("title", title, ctx);
    }

    if let Some(author) = node.props.get_str("author") {
        emit_field("author", author, ctx);
    }

    // Container title maps to journaltitle in BibLaTeX
    if let Some(container) = node.props.get_str("container-title") {
        let csl_type = node.props.get_str("type").unwrap_or("");
        if csl_type == "article-journal" {
            emit_field("journaltitle", container, ctx);
        } else {
            emit_field("booktitle", container, ctx);
        }
    }

    // Date handling - BibLaTeX uses date field
    if let Some(issued) = node.props.get_str("issued") {
        emit_field("date", issued, ctx);
    }

    let direct_mappings = [
        ("volume", "volume"),
        ("issue", "number"),
        ("page", "pages"),
        ("publisher", "publisher"),
        ("publisher-place", "location"),
        ("DOI", "doi"),
        ("URL", "url"),
        ("ISBN", "isbn"),
        ("ISSN", "issn"),
        ("abstract", "abstract"),
        ("note", "note"),
    ];

    for (csl_name, biblatex_name) in direct_mappings {
        if let Some(value) = node.props.get_str(csl_name) {
            emit_field(biblatex_name, value, ctx);
        }
    }
}

fn emit_standard_fields(node: &Node, ctx: &mut EmitContext) {
    // BibLaTeX standard fields
    let field_mappings = [
        ("author", "author"),
        ("title", "title"),
        ("subtitle", "subtitle"),
        ("journaltitle", "journaltitle"),
        ("journal", "journaltitle"), // Map BibTeX journal to BibLaTeX journaltitle
        ("booktitle", "booktitle"),
        ("maintitle", "maintitle"),
        ("date", "date"),
        ("year", "date"), // Map year to date
        ("volume", "volume"),
        ("number", "number"),
        ("pages", "pages"),
        ("publisher", "publisher"),
        ("location", "location"),
        ("address", "location"), // Map BibTeX address to BibLaTeX location
        ("edition", "edition"),
        ("editor", "editor"),
        ("series", "series"),
        ("note", "note"),
        ("doi", "doi"),
        ("eprint", "eprint"),
        ("eprinttype", "eprinttype"),
        ("url", "url"),
        ("urldate", "urldate"),
        ("isbn", "isbn"),
        ("issn", "issn"),
        ("abstract", "abstract"),
        ("keywords", "keywords"),
        ("institution", "institution"),
    ];

    let mut emitted = std::collections::HashSet::new();

    for (prop_name, field_name) in field_mappings {
        if !emitted.contains(field_name)
            && let Some(value) = node.props.get_str(prop_name)
        {
            emit_field(field_name, value, ctx);
            emitted.insert(field_name);
        }
    }
}

fn emit_field(name: &str, value: &str, ctx: &mut EmitContext) {
    ctx.write("  ");
    ctx.write(name);
    ctx.write(" = {");
    ctx.write(&escape_biblatex(value));
    ctx.write("},\n");
}

fn escape_biblatex(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '#' | '$' | '%' | '&' | '_' => {
                result.push('\\');
                result.push(c);
            }
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
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_article() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("key", "smith2024")
            .prop("author", "John Smith")
            .prop("title", "A Great Paper")
            .prop("journaltitle", "Nature")
            .prop("date", "2024-05-15");

        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(entry));
        let output = emit_str(&doc);

        assert!(output.contains("@article{smith2024,"));
        assert!(output.contains("author = {John Smith},"));
        assert!(output.contains("journaltitle = {Nature},"));
        assert!(output.contains("date = {2024-05-15},"));
    }

    #[test]
    fn test_emit_online() {
        let entry = Node::new(NodeKind::from("online"))
            .prop("key", "website2024")
            .prop("author", "Jane Doe")
            .prop("title", "A Great Website")
            .prop("url", "https://example.com")
            .prop("urldate", "2024-01-15");

        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(entry));
        let output = emit_str(&doc);

        assert!(output.contains("@online{website2024,"));
        assert!(output.contains("url = {https://example.com},"));
    }

    #[test]
    fn test_emit_with_subtitle() {
        let entry = Node::new(NodeKind::from("book"))
            .prop("key", "knuth1984")
            .prop("author", "Donald E. Knuth")
            .prop("title", "The TeXbook")
            .prop("subtitle", "A Complete Guide to TeX")
            .prop("publisher", "Addison-Wesley")
            .prop("date", "1984");

        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(entry));
        let output = emit_str(&doc);

        assert!(output.contains("@book{knuth1984,"));
        assert!(output.contains("subtitle = {A Complete Guide to TeX},"));
    }

    #[test]
    fn test_year_to_date() {
        let entry = Node::new(NodeKind::from("article"))
            .prop("key", "test")
            .prop("year", "2024");

        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(entry));
        let output = emit_str(&doc);

        // BibLaTeX should use date field
        assert!(output.contains("date = {2024},"));
    }
}
