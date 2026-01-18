//! Pandoc JSON writer for rescribe.
//!
//! Emits rescribe's document IR as Pandoc's JSON AST format.
//! This enables interoperability with Pandoc's extensive format support.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning};
use rescribe_std::{Node, node, prop};
use serde_json::{Map, Value, json};

/// Pandoc API version we emit.
const PANDOC_API_VERSION: [i64; 2] = [1, 23];

/// Emit a document as Pandoc JSON.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Pandoc JSON with custom options.
pub fn emit_with_options(
    doc: &Document,
    options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut emitter = Emitter::new();

    let blocks = emitter.emit_blocks(&doc.content.children);
    let meta = emitter.emit_meta(&doc.metadata);

    let output = json!({
        "pandoc-api-version": PANDOC_API_VERSION,
        "meta": meta,
        "blocks": blocks
    });

    let json_str = if options.pretty {
        serde_json::to_string_pretty(&output)
    } else {
        serde_json::to_string(&output)
    }
    .map_err(|e| EmitError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    Ok(ConversionResult::with_warnings(
        json_str.into_bytes(),
        emitter.warnings,
    ))
}

/// Emitter state.
struct Emitter {
    warnings: Vec<FidelityWarning>,
}

impl Emitter {
    fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    fn emit_meta(&self, props: &rescribe_core::Properties) -> Value {
        let mut meta = Map::new();
        for (key, value) in props.iter() {
            let meta_value = match value {
                rescribe_core::PropValue::String(s) => {
                    // Emit as MetaInlines for compatibility
                    let inlines = self.text_to_inlines(s);
                    json!({"t": "MetaInlines", "c": inlines})
                }
                rescribe_core::PropValue::Int(i) => {
                    json!({"t": "MetaString", "c": i.to_string()})
                }
                rescribe_core::PropValue::Float(f) => {
                    json!({"t": "MetaString", "c": f.to_string()})
                }
                rescribe_core::PropValue::Bool(b) => {
                    json!({"t": "MetaBool", "c": b})
                }
                rescribe_core::PropValue::List(items) => {
                    let list: Vec<Value> = items
                        .iter()
                        .filter_map(|item| {
                            // Only handle string items in lists
                            if let rescribe_core::PropValue::String(s) = item {
                                let inlines = self.text_to_inlines(s);
                                Some(json!({"t": "MetaInlines", "c": inlines}))
                            } else {
                                None
                            }
                        })
                        .collect();
                    json!({"t": "MetaList", "c": list})
                }
                rescribe_core::PropValue::Map(_) => continue, // Skip nested maps for now
            };
            meta.insert(key.clone(), meta_value);
        }
        Value::Object(meta)
    }

    fn text_to_inlines(&self, text: &str) -> Vec<Value> {
        let mut inlines = Vec::new();
        for (i, word) in text.split_whitespace().enumerate() {
            if i > 0 {
                inlines.push(json!({"t": "Space"}));
            }
            inlines.push(json!({"t": "Str", "c": word}));
        }
        inlines
    }

    fn emit_blocks(&mut self, nodes: &[Node]) -> Vec<Value> {
        nodes.iter().filter_map(|n| self.emit_block(n)).collect()
    }

    fn emit_block(&mut self, node: &Node) -> Option<Value> {
        match node.kind.as_str() {
            node::PARAGRAPH => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Para", "c": inlines}))
            }
            node::HEADING => {
                let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
                let id = node.props.get_str(prop::ID).unwrap_or("").to_string();
                let inlines = self.emit_inlines(&node.children);
                Some(json!({
                    "t": "Header",
                    "c": [level, [id, [], []], inlines]
                }))
            }
            node::CODE_BLOCK => {
                let content = node.props.get_str(prop::CONTENT).unwrap_or("");
                let lang = node.props.get_str(prop::LANGUAGE).unwrap_or("");
                let classes = if lang.is_empty() { vec![] } else { vec![lang] };
                Some(json!({
                    "t": "CodeBlock",
                    "c": [["", classes, []], content]
                }))
            }
            node::BLOCKQUOTE => {
                let blocks = self.emit_blocks(&node.children);
                Some(json!({"t": "BlockQuote", "c": blocks}))
            }
            node::LIST => {
                let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
                let items: Vec<Value> = node
                    .children
                    .iter()
                    .map(|item| Value::Array(self.emit_blocks(&item.children)))
                    .collect();

                if ordered {
                    let start = node.props.get_int(prop::START).unwrap_or(1);
                    Some(json!({
                        "t": "OrderedList",
                        "c": [[start, {"t": "Decimal"}, {"t": "Period"}], items]
                    }))
                } else {
                    Some(json!({"t": "BulletList", "c": items}))
                }
            }
            node::LIST_ITEM => {
                // List items are handled by LIST
                None
            }
            node::HORIZONTAL_RULE => Some(json!({"t": "HorizontalRule"})),
            node::TABLE => self.emit_table(node),
            node::DIV => {
                let id = node.props.get_str(prop::ID).unwrap_or("").to_string();
                let classes_str = node.props.get_str(prop::CLASSES).unwrap_or("");
                let classes: Vec<&str> = if classes_str.is_empty() {
                    vec![]
                } else {
                    classes_str.split_whitespace().collect()
                };
                let blocks = self.emit_blocks(&node.children);
                Some(json!({
                    "t": "Div",
                    "c": [[id, classes, []], blocks]
                }))
            }
            node::RAW_BLOCK => {
                let format = node.props.get_str(prop::FORMAT).unwrap_or("html");
                let content = node.props.get_str(prop::CONTENT).unwrap_or("");
                Some(json!({
                    "t": "RawBlock",
                    "c": [format, content]
                }))
            }
            node::DEFINITION_LIST => {
                let items: Vec<Value> = self.emit_definition_items(&node.children);
                Some(json!({"t": "DefinitionList", "c": items}))
            }
            node::DOCUMENT => {
                // Document is implicit in Pandoc JSON
                None
            }
            "math_display" => {
                let source = node.props.get_str("math:source").unwrap_or("");
                // Wrap in Para for block-level display
                Some(json!({
                    "t": "Para",
                    "c": [{"t": "Math", "c": [{"t": "DisplayMath"}, source]}]
                }))
            }
            _ => None,
        }
    }

    fn emit_definition_items(&mut self, children: &[Node]) -> Vec<Value> {
        let mut items = Vec::new();
        let mut current_term: Option<Vec<Value>> = None;
        let mut current_defs: Vec<Vec<Value>> = Vec::new();

        for child in children {
            match child.kind.as_str() {
                node::DEFINITION_TERM => {
                    // Push previous item if exists
                    if let Some(term) = current_term.take() {
                        items.push(json!([term, current_defs]));
                        current_defs = Vec::new();
                    }
                    current_term = Some(self.emit_inlines(&child.children));
                }
                node::DEFINITION_DESC => {
                    let blocks = self.emit_blocks(&child.children);
                    current_defs.push(blocks);
                }
                _ => {}
            }
        }

        // Push last item
        if let Some(term) = current_term {
            items.push(json!([term, current_defs]));
        }

        items
    }

    fn emit_table(&mut self, node: &Node) -> Option<Value> {
        // Collect rows
        let mut header_rows: Vec<Value> = Vec::new();
        let mut body_rows: Vec<Value> = Vec::new();

        for child in &node.children {
            match child.kind.as_str() {
                node::TABLE_HEAD => {
                    for row in &child.children {
                        if let Some(r) = self.emit_table_row(row) {
                            header_rows.push(r);
                        }
                    }
                }
                node::TABLE_ROW => {
                    // Check if this is a header row (first row with TABLE_HEADER cells)
                    let is_header = child
                        .children
                        .first()
                        .is_some_and(|c| c.kind.as_str() == node::TABLE_HEADER);

                    if let Some(r) = self.emit_table_row(child) {
                        if is_header && body_rows.is_empty() {
                            header_rows.push(r);
                        } else {
                            body_rows.push(r);
                        }
                    }
                }
                node::TABLE_BODY => {
                    for row in &child.children {
                        if let Some(r) = self.emit_table_row(row) {
                            body_rows.push(r);
                        }
                    }
                }
                _ => {}
            }
        }

        // Determine column count
        let col_count = header_rows
            .first()
            .or(body_rows.first())
            .and_then(|r| r.get("c"))
            .and_then(|c| c.get(1))
            .and_then(|cells| cells.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        if col_count == 0 {
            return None;
        }

        // Build ColSpecs
        let colspecs: Vec<Value> = (0..col_count)
            .map(|_| json!([{"t": "AlignDefault"}, {"t": "ColWidthDefault"}]))
            .collect();

        // Build TableHead
        let table_head = json!([["", [], []], header_rows]);

        // Build TableBody
        let table_body = json!([[["", [], []], 0, [], body_rows]]);

        // Build TableFoot (empty)
        let table_foot = json!([["", [], []], []]);

        Some(json!({
            "t": "Table",
            "c": [
                ["", [], []],  // Attr
                [null, []],    // Caption
                colspecs,
                table_head,
                [table_body],
                table_foot
            ]
        }))
    }

    fn emit_table_row(&mut self, row: &Node) -> Option<Value> {
        let cells: Vec<Value> = row
            .children
            .iter()
            .filter_map(|cell| self.emit_table_cell(cell))
            .collect();

        Some(json!({
            "t": "Row",
            "c": [["", [], []], cells]
        }))
    }

    fn emit_table_cell(&mut self, cell: &Node) -> Option<Value> {
        let blocks = self.emit_blocks(&cell.children);
        // If cell contains just inlines (common case), wrap in Plain
        let blocks = if blocks.is_empty() {
            let inlines = self.emit_inlines(&cell.children);
            if inlines.is_empty() {
                vec![]
            } else {
                vec![json!({"t": "Plain", "c": inlines})]
            }
        } else {
            blocks
        };

        Some(json!({
            "t": "Cell",
            "c": [
                ["", [], []],           // Attr
                {"t": "AlignDefault"},  // Alignment
                1,                      // RowSpan
                1,                      // ColSpan
                blocks
            ]
        }))
    }

    fn emit_inlines(&mut self, nodes: &[Node]) -> Vec<Value> {
        nodes.iter().filter_map(|n| self.emit_inline(n)).collect()
    }

    fn emit_inline(&mut self, node: &Node) -> Option<Value> {
        match node.kind.as_str() {
            node::TEXT => {
                let content = node.props.get_str(prop::CONTENT)?;
                // Split into words with spaces
                let mut inlines = Vec::new();
                for (i, word) in content.split(' ').enumerate() {
                    if i > 0 {
                        inlines.push(json!({"t": "Space"}));
                    }
                    if !word.is_empty() {
                        inlines.push(json!({"t": "Str", "c": word}));
                    }
                }
                if inlines.len() == 1 {
                    Some(inlines.into_iter().next().unwrap())
                } else {
                    // Return multiple - caller needs to flatten
                    // For simplicity, just return first or merge
                    Some(json!({"t": "Str", "c": content}))
                }
            }
            node::SOFT_BREAK => Some(json!({"t": "SoftBreak"})),
            node::LINE_BREAK => Some(json!({"t": "LineBreak"})),
            node::EMPHASIS => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Emph", "c": inlines}))
            }
            node::STRONG => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Strong", "c": inlines}))
            }
            node::STRIKEOUT => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Strikeout", "c": inlines}))
            }
            node::UNDERLINE => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Underline", "c": inlines}))
            }
            node::SUPERSCRIPT => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Superscript", "c": inlines}))
            }
            node::SUBSCRIPT => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Subscript", "c": inlines}))
            }
            node::SMALL_CAPS => {
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "SmallCaps", "c": inlines}))
            }
            node::QUOTED => {
                let qt = node.props.get_str(prop::QUOTE_TYPE).unwrap_or("double");
                let quote_type = if qt == "single" {
                    json!({"t": "SingleQuote"})
                } else {
                    json!({"t": "DoubleQuote"})
                };
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Quoted", "c": [quote_type, inlines]}))
            }
            node::CODE => {
                let content = node.props.get_str(prop::CONTENT).unwrap_or("");
                Some(json!({"t": "Code", "c": [["", [], []], content]}))
            }
            node::LINK => {
                let url = node.props.get_str(prop::URL).unwrap_or("");
                let title = node.props.get_str(prop::TITLE).unwrap_or("");
                let inlines = self.emit_inlines(&node.children);
                Some(json!({
                    "t": "Link",
                    "c": [["", [], []], inlines, [url, title]]
                }))
            }
            node::IMAGE => {
                let url = node.props.get_str(prop::URL).unwrap_or("");
                let title = node.props.get_str(prop::TITLE).unwrap_or("");
                let alt = node.props.get_str(prop::ALT).unwrap_or("");
                let alt_inlines = self.text_to_inlines(alt);
                Some(json!({
                    "t": "Image",
                    "c": [["", [], []], alt_inlines, [url, title]]
                }))
            }
            "math_inline" => {
                let source = node.props.get_str("math:source").unwrap_or("");
                Some(json!({"t": "Math", "c": [{"t": "InlineMath"}, source]}))
            }
            node::RAW_INLINE => {
                let format = node.props.get_str(prop::FORMAT).unwrap_or("html");
                let content = node.props.get_str(prop::CONTENT).unwrap_or("");
                Some(json!({"t": "RawInline", "c": [format, content]}))
            }
            node::FOOTNOTE_DEF => {
                let blocks = self.emit_blocks(&node.children);
                Some(json!({"t": "Note", "c": blocks}))
            }
            node::SPAN => {
                let id = node.props.get_str(prop::ID).unwrap_or("").to_string();
                let classes_str = node.props.get_str(prop::CLASSES).unwrap_or("");
                let classes: Vec<&str> = if classes_str.is_empty() {
                    vec![]
                } else {
                    classes_str.split_whitespace().collect()
                };
                let inlines = self.emit_inlines(&node.children);
                Some(json!({
                    "t": "Span",
                    "c": [[id, classes, []], inlines]
                }))
            }
            node::CITE => {
                // Simple cite - just output the inlines with empty citations
                let inlines = self.emit_inlines(&node.children);
                Some(json!({"t": "Cite", "c": [[], inlines]}))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::doc;

    fn emit_json(document: &Document) -> Value {
        let result = emit(document).unwrap();
        let json_str = String::from_utf8(result.value).unwrap();
        serde_json::from_str(&json_str).unwrap()
    }

    #[test]
    fn test_emit_paragraph() {
        let document = doc(|d| d.para(|i| i.text("Hello world")));
        let json = emit_json(&document);

        assert_eq!(json["pandoc-api-version"], json!([1, 23]));
        assert_eq!(json["blocks"][0]["t"], "Para");
    }

    #[test]
    fn test_emit_heading() {
        let document = doc(|d| d.heading(2, |i| i.text("Title")));
        let json = emit_json(&document);

        assert_eq!(json["blocks"][0]["t"], "Header");
        assert_eq!(json["blocks"][0]["c"][0], 2);
    }

    #[test]
    fn test_emit_code_block() {
        // code_block_lang takes (code, lang)
        let document = doc(|d| d.code_block_lang("fn main() {}", "rust"));
        let json = emit_json(&document);

        assert_eq!(json["blocks"][0]["t"], "CodeBlock");
        // CodeBlock format: [[id, classes, attrs], content]
        let code_block = &json["blocks"][0]["c"];
        assert_eq!(code_block[0][1][0], "rust"); // language in classes
        assert_eq!(code_block[1], "fn main() {}"); // content
    }

    #[test]
    fn test_emit_list() {
        let document =
            doc(|d| d.bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2"))));
        let json = emit_json(&document);

        assert_eq!(json["blocks"][0]["t"], "BulletList");
        assert_eq!(json["blocks"][0]["c"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_emit_emphasis() {
        let document = doc(|d| {
            d.para(|i| {
                i.em(|i| i.text("italic"))
                    .text(" and ")
                    .strong(|i| i.text("bold"))
            })
        });
        let json = emit_json(&document);

        let inlines = json["blocks"][0]["c"].as_array().unwrap();
        assert!(inlines.iter().any(|i| i["t"] == "Emph"));
        assert!(inlines.iter().any(|i| i["t"] == "Strong"));
    }

    #[test]
    fn test_emit_link() {
        let document = doc(|d| d.para(|i| i.link("https://example.com", |i| i.text("Example"))));
        let json = emit_json(&document);

        let link = &json["blocks"][0]["c"][0];
        assert_eq!(link["t"], "Link");
        assert_eq!(link["c"][2][0], "https://example.com");
    }

    #[test]
    fn test_roundtrip() {
        use rescribe_read_pandoc_json::parse;

        let document = doc(|d| {
            d.heading(1, |i| i.text("Title"))
                .para(|i| i.text("Hello ").em(|i| i.text("world")))
                .bullet_list(|l| l.item(|i| i.text("Item 1")).item(|i| i.text("Item 2")))
        });

        let json_bytes = emit(&document).unwrap().value;
        let json_str = String::from_utf8(json_bytes).unwrap();

        let parsed = parse(&json_str).unwrap().value;

        // Verify structure preserved
        assert_eq!(parsed.content.children.len(), 3);
        assert_eq!(parsed.content.children[0].kind.as_str(), node::HEADING);
        assert_eq!(parsed.content.children[1].kind.as_str(), node::PARAGRAPH);
        assert_eq!(parsed.content.children[2].kind.as_str(), node::LIST);
    }
}
