//! Pandoc JSON reader for rescribe.
//!
//! Parses Pandoc's JSON AST format into rescribe's document IR.
//! This enables interoperability with Pandoc's extensive format support.

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Properties, Severity,
    WarningKind,
};
use rescribe_std::{Node, node, prop};
use serde::Deserialize;
use serde_json::Value;

/// Parse Pandoc JSON into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse Pandoc JSON with custom options.
pub fn parse_with_options(
    input: &str,
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let pandoc: PandocDocument =
        serde_json::from_str(input).map_err(|e| ParseError::Invalid(e.to_string()))?;

    let mut converter = Converter::new();
    let children = converter.convert_blocks(&pandoc.blocks);
    let metadata = converter.convert_meta(&pandoc.meta);

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root).with_metadata(metadata);

    Ok(ConversionResult::with_warnings(doc, converter.warnings))
}

/// Pandoc document structure.
#[derive(Debug, Deserialize)]
struct PandocDocument {
    #[serde(rename = "pandoc-api-version")]
    #[allow(dead_code)]
    api_version: Vec<i64>,
    meta: Value,
    blocks: Vec<Value>,
}

/// Converter state.
struct Converter {
    warnings: Vec<FidelityWarning>,
}

impl Converter {
    fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    fn warn(&mut self, kind: WarningKind, msg: impl Into<String>) {
        self.warnings
            .push(FidelityWarning::new(Severity::Minor, kind, msg));
    }

    fn convert_meta(&mut self, meta: &Value) -> Properties {
        let mut props = Properties::new();
        if let Value::Object(map) = meta {
            for (key, value) in map {
                if let Some(v) = self.extract_meta_value(value) {
                    props.set(key.clone(), v);
                }
            }
        }
        props
    }

    fn extract_meta_value(&mut self, value: &Value) -> Option<String> {
        // Pandoc meta values are wrapped: {"t": "MetaInlines", "c": [...]}
        let t = value.get("t")?.as_str()?;
        let c = value.get("c")?;

        match t {
            "MetaString" => c.as_str().map(|s| s.to_string()),
            "MetaInlines" => {
                if let Value::Array(inlines) = c {
                    Some(self.inlines_to_text(inlines))
                } else {
                    None
                }
            }
            "MetaBool" => c.as_bool().map(|b| b.to_string()),
            "MetaList" => {
                if let Value::Array(items) = c {
                    let texts: Vec<String> = items
                        .iter()
                        .filter_map(|item| self.extract_meta_value(item))
                        .collect();
                    Some(texts.join(", "))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn inlines_to_text(&self, inlines: &[Value]) -> String {
        let mut text = String::new();
        for inline in inlines {
            if let Some(t) = inline.get("t").and_then(|v| v.as_str()) {
                match t {
                    "Str" => {
                        if let Some(s) = inline.get("c").and_then(|v| v.as_str()) {
                            text.push_str(s);
                        }
                    }
                    "Space" => text.push(' '),
                    "SoftBreak" => text.push(' '),
                    "LineBreak" => text.push('\n'),
                    _ => {}
                }
            }
        }
        text
    }

    fn convert_blocks(&mut self, blocks: &[Value]) -> Vec<Node> {
        blocks
            .iter()
            .filter_map(|b| self.convert_block(b))
            .collect()
    }

    fn convert_block(&mut self, block: &Value) -> Option<Node> {
        let t = block.get("t")?.as_str()?;
        let c = block.get("c");

        match t {
            "Para" => self.convert_para(c?),
            "Plain" => self.convert_plain(c?),
            "Header" => self.convert_header(c?),
            "CodeBlock" => self.convert_code_block(c?),
            "BlockQuote" => self.convert_blockquote(c?),
            "BulletList" => self.convert_bullet_list(c?),
            "OrderedList" => self.convert_ordered_list(c?),
            "DefinitionList" => self.convert_definition_list(c?),
            "HorizontalRule" => Some(Node::new(node::HORIZONTAL_RULE)),
            "Table" => self.convert_table(c?),
            "Div" => self.convert_div(c?),
            "RawBlock" => self.convert_raw_block(c?),
            "LineBlock" => self.convert_line_block(c?),
            "Null" => None,
            _ => {
                self.warn(
                    WarningKind::UnsupportedNode(format!("pandoc:{}", t)),
                    format!("Unknown Pandoc block type: {}", t),
                );
                None
            }
        }
    }

    fn convert_para(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(inlines) = content {
            let children = self.convert_inlines(inlines);
            Some(Node::new(node::PARAGRAPH).children(children))
        } else {
            None
        }
    }

    fn convert_plain(&mut self, content: &Value) -> Option<Node> {
        // Plain is like Para but without paragraph semantics
        self.convert_para(content)
    }

    fn convert_header(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(arr) = content {
            let level = arr.first()?.as_i64()?;
            // arr[1] is [id, classes, attrs] - we can extract id
            let id = arr
                .get(1)
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let inlines = arr.get(2)?.as_array()?;
            let children = self.convert_inlines(inlines);

            let mut heading = Node::new(node::HEADING)
                .prop(prop::LEVEL, level)
                .children(children);

            if let Some(id_str) = id {
                heading = heading.prop(prop::ID, id_str.to_string());
            }

            Some(heading)
        } else {
            None
        }
    }

    fn convert_code_block(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(arr) = content {
            // [[id, classes, attrs], code_string]
            let attrs = arr.first()?.as_array()?;
            let code = arr.get(1)?.as_str()?;

            let classes = attrs.get(1).and_then(|v| v.as_array());
            let language = classes
                .and_then(|c| c.first())
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());

            let mut node = Node::new(node::CODE_BLOCK).prop(prop::CONTENT, code.to_string());
            if let Some(lang) = language {
                node = node.prop(prop::LANGUAGE, lang.to_string());
            }

            Some(node)
        } else {
            None
        }
    }

    fn convert_blockquote(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(blocks) = content {
            let children = self.convert_blocks(blocks);
            Some(Node::new(node::BLOCKQUOTE).children(children))
        } else {
            None
        }
    }

    fn convert_bullet_list(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(items) = content {
            let children: Vec<Node> = items
                .iter()
                .filter_map(|item| {
                    if let Value::Array(blocks) = item {
                        let item_children = self.convert_blocks(blocks);
                        Some(Node::new(node::LIST_ITEM).children(item_children))
                    } else {
                        None
                    }
                })
                .collect();

            Some(
                Node::new(node::LIST)
                    .prop(prop::ORDERED, false)
                    .children(children),
            )
        } else {
            None
        }
    }

    fn convert_ordered_list(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(arr) = content {
            // [ListAttributes, [[Block]]]
            let list_attrs = arr.first()?.as_array()?;
            let start = list_attrs.first().and_then(|v| v.as_i64()).unwrap_or(1);
            let items = arr.get(1)?.as_array()?;

            let children: Vec<Node> = items
                .iter()
                .filter_map(|item| {
                    if let Value::Array(blocks) = item {
                        let item_children = self.convert_blocks(blocks);
                        Some(Node::new(node::LIST_ITEM).children(item_children))
                    } else {
                        None
                    }
                })
                .collect();

            let mut list = Node::new(node::LIST)
                .prop(prop::ORDERED, true)
                .children(children);

            if start != 1 {
                list = list.prop(prop::START, start);
            }

            Some(list)
        } else {
            None
        }
    }

    fn convert_definition_list(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(items) = content {
            let mut children = Vec::new();

            for item in items {
                if let Value::Array(pair) = item {
                    // [term_inlines, [definition_blocks...]]
                    if let Some(term_inlines) = pair.first().and_then(|v| v.as_array()) {
                        let term_children = self.convert_inlines(term_inlines);
                        children.push(Node::new(node::DEFINITION_TERM).children(term_children));
                    }
                    if let Some(defs) = pair.get(1).and_then(|v| v.as_array()) {
                        for def in defs {
                            if let Value::Array(blocks) = def {
                                let def_children = self.convert_blocks(blocks);
                                children
                                    .push(Node::new(node::DEFINITION_DESC).children(def_children));
                            }
                        }
                    }
                }
            }

            Some(Node::new(node::DEFINITION_LIST).children(children))
        } else {
            None
        }
    }

    fn convert_table(&mut self, content: &Value) -> Option<Node> {
        // Pandoc 1.22+ table format: [Attr, Caption, [ColSpec], TableHead, [TableBody], TableFoot]
        if let Value::Array(arr) = content {
            let mut rows = Vec::new();

            // TableHead is at index 3
            if let Some(head) = arr.get(3)
                && let Some(head_rows) = self.extract_table_rows(head, true)
            {
                rows.extend(head_rows);
            }

            // TableBody is at index 4
            if let Some(Value::Array(bodies)) = arr.get(4) {
                for body in bodies {
                    if let Some(body_rows) = self.extract_table_body_rows(body) {
                        rows.extend(body_rows);
                    }
                }
            }

            if rows.is_empty() {
                return None;
            }

            Some(Node::new(node::TABLE).children(rows))
        } else {
            None
        }
    }

    fn extract_table_rows(&mut self, head: &Value, is_header: bool) -> Option<Vec<Node>> {
        // TableHead/TableFoot: [Attr, [Row]]
        let arr = head.as_array()?;
        let rows = arr.get(1)?.as_array()?;

        let result: Vec<Node> = rows
            .iter()
            .filter_map(|row| self.convert_table_row(row, is_header))
            .collect();

        Some(result)
    }

    fn extract_table_body_rows(&mut self, body: &Value) -> Option<Vec<Node>> {
        // TableBody: [Attr, RowHeadColumns, [Row], [Row]]
        let arr = body.as_array()?;
        // Index 2 is intermediate head rows, index 3 is body rows
        let body_rows = arr.get(3)?.as_array()?;

        let result: Vec<Node> = body_rows
            .iter()
            .filter_map(|row| self.convert_table_row(row, false))
            .collect();

        Some(result)
    }

    fn convert_table_row(&mut self, row: &Value, is_header: bool) -> Option<Node> {
        // Row: [Attr, [Cell]]
        let arr = row.as_array()?;
        let cells = arr.get(1)?.as_array()?;

        let cell_nodes: Vec<Node> = cells
            .iter()
            .filter_map(|cell| self.convert_table_cell(cell, is_header))
            .collect();

        Some(Node::new(node::TABLE_ROW).children(cell_nodes))
    }

    fn convert_table_cell(&mut self, cell: &Value, is_header: bool) -> Option<Node> {
        // Cell: [Attr, Alignment, RowSpan, ColSpan, [Block]]
        let arr = cell.as_array()?;
        let blocks = arr.get(4)?.as_array()?;

        let children = self.convert_blocks(blocks);
        let kind = if is_header {
            node::TABLE_HEADER
        } else {
            node::TABLE_CELL
        };

        Some(Node::new(kind).children(children))
    }

    fn convert_div(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(arr) = content {
            // [Attr, [Block]]
            let attrs = arr.first()?.as_array()?;
            let blocks = arr.get(1)?.as_array()?;

            let id = attrs
                .first()
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let classes = attrs.get(1).and_then(|v| v.as_array());

            let children = self.convert_blocks(blocks);
            let mut div = Node::new(node::DIV).children(children);

            if let Some(id_str) = id {
                div = div.prop(prop::ID, id_str.to_string());
            }
            if let Some(class_arr) = classes {
                let class_strs: Vec<String> = class_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if !class_strs.is_empty() {
                    div = div.prop(prop::CLASSES, class_strs.join(" "));
                }
            }

            Some(div)
        } else {
            None
        }
    }

    fn convert_raw_block(&mut self, content: &Value) -> Option<Node> {
        if let Value::Array(arr) = content {
            let format = arr.first()?.as_str()?;
            let text = arr.get(1)?.as_str()?;

            Some(
                Node::new(node::RAW_BLOCK)
                    .prop(prop::FORMAT, format.to_string())
                    .prop(prop::CONTENT, text.to_string()),
            )
        } else {
            None
        }
    }

    fn convert_line_block(&mut self, content: &Value) -> Option<Node> {
        // LineBlock is a block of lines, each line is [Inline]
        if let Value::Array(lines) = content {
            let mut children = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                if let Value::Array(inlines) = line {
                    children.extend(self.convert_inlines(inlines));
                    if i < lines.len() - 1 {
                        children.push(Node::new(node::LINE_BREAK));
                    }
                }
            }
            Some(Node::new(node::PARAGRAPH).children(children))
        } else {
            None
        }
    }

    fn convert_inlines(&mut self, inlines: &[Value]) -> Vec<Node> {
        inlines
            .iter()
            .filter_map(|i| self.convert_inline(i))
            .collect()
    }

    fn convert_inline(&mut self, inline: &Value) -> Option<Node> {
        let t = inline.get("t")?.as_str()?;
        let c = inline.get("c");

        match t {
            "Str" => {
                let text = c?.as_str()?;
                Some(Node::new(node::TEXT).prop(prop::CONTENT, text.to_string()))
            }
            "Space" => Some(Node::new(node::TEXT).prop(prop::CONTENT, " ".to_string())),
            "SoftBreak" => Some(Node::new(node::SOFT_BREAK)),
            "LineBreak" => Some(Node::new(node::LINE_BREAK)),
            "Emph" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::EMPHASIS).children(children))
            }
            "Strong" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::STRONG).children(children))
            }
            "Strikeout" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::STRIKEOUT).children(children))
            }
            "Underline" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::UNDERLINE).children(children))
            }
            "Superscript" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::SUPERSCRIPT).children(children))
            }
            "Subscript" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::SUBSCRIPT).children(children))
            }
            "SmallCaps" => {
                let inlines = c?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::SMALL_CAPS).children(children))
            }
            "Quoted" => {
                let arr = c?.as_array()?;
                let quote_type = arr.first()?.get("t")?.as_str()?;
                let inlines = arr.get(1)?.as_array()?;
                let children = self.convert_inlines(inlines);
                let qt = if quote_type == "SingleQuote" {
                    "single"
                } else {
                    "double"
                };
                Some(
                    Node::new(node::QUOTED)
                        .prop(prop::QUOTE_TYPE, qt.to_string())
                        .children(children),
                )
            }
            "Code" => {
                let arr = c?.as_array()?;
                let code = arr.get(1)?.as_str()?;
                Some(Node::new(node::CODE).prop(prop::CONTENT, code.to_string()))
            }
            "Link" => {
                let arr = c?.as_array()?;
                // [Attr, [Inline], Target]
                let inlines = arr.get(1)?.as_array()?;
                let target = arr.get(2)?.as_array()?;
                let url = target.first()?.as_str()?;
                let title = target
                    .get(1)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty());

                let children = self.convert_inlines(inlines);
                let mut link = Node::new(node::LINK)
                    .prop(prop::URL, url.to_string())
                    .children(children);

                if let Some(t) = title {
                    link = link.prop(prop::TITLE, t.to_string());
                }

                Some(link)
            }
            "Image" => {
                let arr = c?.as_array()?;
                // [Attr, [Inline], Target]
                let inlines = arr.get(1)?.as_array()?;
                let target = arr.get(2)?.as_array()?;
                let url = target.first()?.as_str()?;
                let title = target
                    .get(1)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty());

                let alt = self.inlines_to_text(inlines);
                let mut img = Node::new(node::IMAGE).prop(prop::URL, url.to_string());

                if !alt.is_empty() {
                    img = img.prop(prop::ALT, alt);
                }
                if let Some(t) = title {
                    img = img.prop(prop::TITLE, t.to_string());
                }

                Some(img)
            }
            "Math" => {
                let arr = c?.as_array()?;
                let math_type = arr.first()?.get("t")?.as_str()?;
                let tex = arr.get(1)?.as_str()?;

                let kind = if math_type == "InlineMath" {
                    "math_inline"
                } else {
                    "math_display"
                };

                Some(Node::new(kind).prop("math:source", tex.to_string()))
            }
            "RawInline" => {
                let arr = c?.as_array()?;
                let format = arr.first()?.as_str()?;
                let text = arr.get(1)?.as_str()?;

                Some(
                    Node::new(node::RAW_INLINE)
                        .prop(prop::FORMAT, format.to_string())
                        .prop(prop::CONTENT, text.to_string()),
                )
            }
            "Note" => {
                // Footnote - contains blocks
                let blocks = c?.as_array()?;
                let children = self.convert_blocks(blocks);
                Some(Node::new(node::FOOTNOTE_DEF).children(children))
            }
            "Span" => {
                let arr = c?.as_array()?;
                let attrs = arr.first()?.as_array()?;
                let inlines = arr.get(1)?.as_array()?;

                let id = attrs
                    .first()
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty());
                let classes = attrs.get(1).and_then(|v| v.as_array());

                let children = self.convert_inlines(inlines);
                let mut span = Node::new(node::SPAN).children(children);

                if let Some(id_str) = id {
                    span = span.prop(prop::ID, id_str.to_string());
                }
                if let Some(class_arr) = classes {
                    let class_strs: Vec<String> = class_arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if !class_strs.is_empty() {
                        span = span.prop(prop::CLASSES, class_strs.join(" "));
                    }
                }

                Some(span)
            }
            "Cite" => {
                // Citation - just extract the text for now
                let arr = c?.as_array()?;
                let inlines = arr.get(1)?.as_array()?;
                let children = self.convert_inlines(inlines);
                Some(Node::new(node::CITE).children(children))
            }
            _ => {
                self.warn(
                    WarningKind::UnsupportedNode(format!("pandoc:{}", t)),
                    format!("Unknown Pandoc inline type: {}", t),
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_paragraph() {
        let json = r#"{
            "pandoc-api-version": [1, 23],
            "meta": {},
            "blocks": [
                {"t": "Para", "c": [{"t": "Str", "c": "Hello"}, {"t": "Space"}, {"t": "Str", "c": "world"}]}
            ]
        }"#;

        let result = parse(json).unwrap();
        let doc = result.value;

        assert_eq!(doc.content.children.len(), 1);
        assert_eq!(doc.content.children[0].kind.as_str(), node::PARAGRAPH);
    }

    #[test]
    fn test_parse_heading() {
        let json = r#"{
            "pandoc-api-version": [1, 23],
            "meta": {},
            "blocks": [
                {"t": "Header", "c": [2, ["my-id", [], []], [{"t": "Str", "c": "Title"}]]}
            ]
        }"#;

        let result = parse(json).unwrap();
        let doc = result.value;

        let heading = &doc.content.children[0];
        assert_eq!(heading.kind.as_str(), node::HEADING);
        assert_eq!(heading.props.get_int(prop::LEVEL), Some(2));
        assert_eq!(heading.props.get_str(prop::ID), Some("my-id"));
    }

    #[test]
    fn test_parse_emphasis() {
        let json = r#"{
            "pandoc-api-version": [1, 23],
            "meta": {},
            "blocks": [
                {"t": "Para", "c": [
                    {"t": "Emph", "c": [{"t": "Str", "c": "italic"}]},
                    {"t": "Space"},
                    {"t": "Strong", "c": [{"t": "Str", "c": "bold"}]}
                ]}
            ]
        }"#;

        let result = parse(json).unwrap();
        let doc = result.value;

        let para = &doc.content.children[0];
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::EMPHASIS)
        );
        assert!(
            para.children
                .iter()
                .any(|n| n.kind.as_str() == node::STRONG)
        );
    }

    #[test]
    fn test_parse_list() {
        let json = r#"{
            "pandoc-api-version": [1, 23],
            "meta": {},
            "blocks": [
                {"t": "BulletList", "c": [
                    [{"t": "Plain", "c": [{"t": "Str", "c": "Item 1"}]}],
                    [{"t": "Plain", "c": [{"t": "Str", "c": "Item 2"}]}]
                ]}
            ]
        }"#;

        let result = parse(json).unwrap();
        let doc = result.value;

        let list = &doc.content.children[0];
        assert_eq!(list.kind.as_str(), node::LIST);
        assert_eq!(list.props.get_bool(prop::ORDERED), Some(false));
        assert_eq!(list.children.len(), 2);
    }

    #[test]
    fn test_parse_link() {
        let json = r#"{
            "pandoc-api-version": [1, 23],
            "meta": {},
            "blocks": [
                {"t": "Para", "c": [
                    {"t": "Link", "c": [
                        ["", [], []],
                        [{"t": "Str", "c": "Example"}],
                        ["https://example.com", ""]
                    ]}
                ]}
            ]
        }"#;

        let result = parse(json).unwrap();
        let doc = result.value;

        let para = &doc.content.children[0];
        let link = &para.children[0];
        assert_eq!(link.kind.as_str(), node::LINK);
        assert_eq!(link.props.get_str(prop::URL), Some("https://example.com"));
    }

    #[test]
    fn test_parse_metadata() {
        let json = r#"{
            "pandoc-api-version": [1, 23],
            "meta": {
                "title": {"t": "MetaInlines", "c": [{"t": "Str", "c": "My"}, {"t": "Space"}, {"t": "Str", "c": "Title"}]},
                "author": {"t": "MetaString", "c": "John Doe"}
            },
            "blocks": []
        }"#;

        let result = parse(json).unwrap();
        let doc = result.value;

        assert_eq!(doc.metadata.get_str("title"), Some("My Title"));
        assert_eq!(doc.metadata.get_str("author"), Some("John Doe"));
    }
}
