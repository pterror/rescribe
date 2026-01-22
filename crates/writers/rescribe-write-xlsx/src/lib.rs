//! XLSX (Excel) writer for rescribe.
//!
//! Emits documents as Excel spreadsheets using the ooxml-sml crate.
//! Tables in the document become sheets in the workbook.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_write_xlsx::emit;
//!
//! let xlsx_bytes = emit(&doc)?;
//! std::fs::write("output.xlsx", xlsx_bytes.value)?;
//! ```

use ooxml_sml::WorkbookBuilder;
use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node};
use rescribe_std::{node, prop};
use std::io::Cursor;

/// Emit a document as XLSX.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as XLSX with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();
    ctx.convert_document(doc)?;

    let warnings = std::mem::take(&mut ctx.warnings);
    let bytes = ctx.finish()?;
    Ok(ConversionResult::with_warnings(bytes, warnings))
}

struct EmitContext {
    workbook: WorkbookBuilder,
    warnings: Vec<FidelityWarning>,
    sheet_count: usize,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            workbook: WorkbookBuilder::new(),
            warnings: Vec::new(),
            sheet_count: 0,
        }
    }

    fn convert_document(&mut self, doc: &Document) -> Result<(), EmitError> {
        self.convert_nodes(&doc.content.children)
    }

    fn convert_nodes(&mut self, nodes: &[Node]) -> Result<(), EmitError> {
        let mut current_sheet_name: Option<String> = None;
        let mut pending_table: Option<&Node> = None;

        for node in nodes {
            match node.kind.as_str() {
                "document" => {
                    self.convert_nodes(&node.children)?;
                }
                "heading" => {
                    // Flush any pending table first
                    if let Some(table) = pending_table.take() {
                        let name = current_sheet_name.take().unwrap_or_else(|| {
                            self.sheet_count += 1;
                            format!("Sheet{}", self.sheet_count)
                        });
                        self.convert_table(table, &name)?;
                    }
                    // Extract heading text as next sheet name
                    current_sheet_name = Some(extract_text(node));
                }
                "table" => {
                    // If we have a pending sheet name, use it; otherwise generate one
                    let name = current_sheet_name.take().unwrap_or_else(|| {
                        self.sheet_count += 1;
                        format!("Sheet{}", self.sheet_count)
                    });
                    self.convert_table(node, &name)?;
                }
                "definition_list" => {
                    // Definition lists from bibliography formats - convert to a sheet
                    self.sheet_count += 1;
                    let name = format!("Sheet{}", self.sheet_count);
                    self.convert_definition_list(node, &name)?;
                }
                _ => {
                    // Recurse into other containers
                    if !node.children.is_empty() {
                        self.convert_nodes(&node.children)?;
                    }
                }
            }
        }

        // Flush any remaining pending table
        if let Some(table) = pending_table {
            let name = current_sheet_name.unwrap_or_else(|| {
                self.sheet_count += 1;
                format!("Sheet{}", self.sheet_count)
            });
            self.convert_table(table, &name)?;
        }

        // If no sheets were added, create an empty sheet
        if self.workbook.sheet_count() == 0 {
            self.workbook.add_sheet("Sheet1");
        }

        Ok(())
    }

    fn convert_table(&mut self, table: &Node, name: &str) -> Result<(), EmitError> {
        let sheet = self.workbook.add_sheet(name);

        for (row_idx, row_node) in table.children.iter().enumerate() {
            if row_node.kind.as_str() != node::TABLE_ROW {
                continue;
            }

            for (col_idx, cell_node) in row_node.children.iter().enumerate() {
                let cell_text = extract_text(cell_node);
                if !cell_text.is_empty() {
                    let col_letter = column_to_letter(col_idx as u32 + 1);
                    let cell_ref = format!("{}{}", col_letter, row_idx + 1);

                    // Try to parse as number
                    if let Ok(num) = cell_text.parse::<f64>() {
                        sheet.set_cell(&cell_ref, num);
                    } else if cell_text.eq_ignore_ascii_case("true") {
                        sheet.set_cell(&cell_ref, true);
                    } else if cell_text.eq_ignore_ascii_case("false") {
                        sheet.set_cell(&cell_ref, false);
                    } else {
                        sheet.set_cell(&cell_ref, cell_text);
                    }
                }
            }
        }

        Ok(())
    }

    fn convert_definition_list(&mut self, list: &Node, name: &str) -> Result<(), EmitError> {
        let sheet = self.workbook.add_sheet(name);

        // Header row
        sheet.set_cell("A1", "Key");
        sheet.set_cell("B1", "Value");

        let mut row = 2u32;
        for entry in &list.children {
            // Look for term and description
            let mut term_text = String::new();
            let mut desc_text = String::new();

            for child in &entry.children {
                match child.kind.as_str() {
                    "definition_term" => {
                        term_text = extract_text(child);
                    }
                    "definition_desc" => {
                        desc_text = extract_text(child);
                    }
                    _ => {}
                }
            }

            if !term_text.is_empty() || !desc_text.is_empty() {
                sheet.set_cell(&format!("A{}", row), term_text);
                sheet.set_cell(&format!("B{}", row), desc_text);
                row += 1;
            }
        }

        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, EmitError> {
        let mut cursor = Cursor::new(Vec::new());
        self.workbook.write(&mut cursor).map_err(|e| {
            EmitError::Io(std::io::Error::other(format!(
                "Failed to write XLSX: {}",
                e
            )))
        })?;
        Ok(cursor.into_inner())
    }
}

/// Extract all text content from a node recursively.
fn extract_text(node: &Node) -> String {
    let mut text = String::new();

    if let Some(content) = node.props.get_str(prop::CONTENT) {
        text.push_str(content);
    }

    for child in &node.children {
        let child_text = extract_text(child);
        if !child_text.is_empty() {
            if !text.is_empty() && !text.ends_with(' ') {
                text.push(' ');
            }
            text.push_str(&child_text);
        }
    }

    text.trim().to_string()
}

/// Convert a 1-based column number to Excel column letters (A, B, ..., Z, AA, AB, ...).
fn column_to_letter(mut col: u32) -> String {
    let mut result = String::new();
    while col > 0 {
        col -= 1;
        let c = (b'A' + (col % 26) as u8) as char;
        result.insert(0, c);
        col /= 26;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::NodeKind;

    #[test]
    fn test_column_to_letter() {
        assert_eq!(column_to_letter(1), "A");
        assert_eq!(column_to_letter(2), "B");
        assert_eq!(column_to_letter(26), "Z");
        assert_eq!(column_to_letter(27), "AA");
        assert_eq!(column_to_letter(28), "AB");
        assert_eq!(column_to_letter(52), "AZ");
        assert_eq!(column_to_letter(53), "BA");
    }

    #[test]
    fn test_emit_empty_document() {
        let doc = Document::new();
        let result = emit(&doc).unwrap();
        // Should produce valid XLSX (ZIP with XML)
        assert!(!result.value.is_empty());
        // XLSX files start with ZIP magic
        assert_eq!(&result.value[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    }

    #[test]
    fn test_emit_table() {
        let table = Node::new(NodeKind::from(node::TABLE)).children(vec![
            Node::new(NodeKind::from(node::TABLE_ROW)).children(vec![
                Node::new(NodeKind::from(node::TABLE_HEADER)).child(
                    Node::new(NodeKind::from(node::PARAGRAPH))
                        .child(Node::new(NodeKind::from(node::TEXT)).prop(prop::CONTENT, "Name")),
                ),
                Node::new(NodeKind::from(node::TABLE_HEADER)).child(
                    Node::new(NodeKind::from(node::PARAGRAPH))
                        .child(Node::new(NodeKind::from(node::TEXT)).prop(prop::CONTENT, "Age")),
                ),
            ]),
            Node::new(NodeKind::from(node::TABLE_ROW)).children(vec![
                Node::new(NodeKind::from(node::TABLE_CELL)).child(
                    Node::new(NodeKind::from(node::PARAGRAPH))
                        .child(Node::new(NodeKind::from(node::TEXT)).prop(prop::CONTENT, "Alice")),
                ),
                Node::new(NodeKind::from(node::TABLE_CELL)).child(
                    Node::new(NodeKind::from(node::PARAGRAPH))
                        .child(Node::new(NodeKind::from(node::TEXT)).prop(prop::CONTENT, "30")),
                ),
            ]),
        ]);

        let doc =
            Document::new().with_content(Node::new(NodeKind::from(node::DOCUMENT)).child(table));

        let result = emit(&doc).unwrap();
        assert!(!result.value.is_empty());
        // XLSX files start with ZIP magic
        assert_eq!(&result.value[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    }
}
