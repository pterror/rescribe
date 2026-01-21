//! XLSX (Excel) reader for rescribe.
//!
//! Parses Excel spreadsheets into rescribe's document IR using the ooxml-sml crate.
//! Each sheet becomes a section with a heading and table.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_read_xlsx::parse_file;
//!
//! let result = parse_file("spreadsheet.xlsx")?;
//! let doc = result.value;
//! // Process the document...
//! ```

use ooxml_sml::{CellValue, Workbook};
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, Node, ParseError, Properties, Severity,
    SourceInfo, WarningKind,
};
use rescribe_std::{node, prop};
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

/// Parse an XLSX file from a path.
pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<ConversionResult<Document>, ParseError> {
    let file = File::open(path).map_err(|e| {
        ParseError::Io(std::io::Error::other(format!("Failed to open XLSX: {}", e)))
    })?;
    parse(BufReader::new(file))
}

/// Parse XLSX from a reader that implements Read + Seek.
pub fn parse<R: Read + Seek>(reader: R) -> Result<ConversionResult<Document>, ParseError> {
    let mut workbook = Workbook::from_reader(reader)
        .map_err(|e| ParseError::Invalid(format!("Failed to parse XLSX: {}", e)))?;

    let mut converter = Converter::new();
    let children = converter.convert_workbook(&mut workbook)?;

    let metadata = extract_metadata(&workbook);

    let document = Document {
        content: Node::new(node::DOCUMENT).children(children),
        resources: Default::default(),
        metadata,
        source: Some(SourceInfo {
            format: "xlsx".to_string(),
            metadata: Properties::new(),
        }),
    };

    Ok(ConversionResult::with_warnings(
        document,
        converter.warnings,
    ))
}

/// Parse XLSX from bytes.
pub fn parse_bytes(bytes: &[u8]) -> Result<ConversionResult<Document>, ParseError> {
    let cursor = std::io::Cursor::new(bytes);
    parse(cursor)
}

struct Converter {
    warnings: Vec<FidelityWarning>,
}

impl Converter {
    fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(FidelityWarning::new(
            Severity::Minor,
            WarningKind::FeatureLost("xlsx".to_string()),
            message,
        ));
    }

    fn convert_workbook<R: Read + Seek>(
        &mut self,
        workbook: &mut Workbook<R>,
    ) -> Result<Vec<Node>, ParseError> {
        let mut children = Vec::new();
        let sheet_names = workbook
            .sheet_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        for (i, name) in sheet_names.iter().enumerate() {
            let sheet = workbook.sheet(i).map_err(|e| {
                ParseError::Invalid(format!("Failed to load sheet '{}': {}", name, e))
            })?;

            // Add heading for sheet name (if multiple sheets)
            if sheet_names.len() > 1 {
                let heading = Node::new(node::HEADING)
                    .prop(prop::LEVEL, 2i64)
                    .child(Node::new(node::TEXT).prop(prop::CONTENT, sheet.name().to_string()));
                children.push(heading);
            }

            // Convert sheet to table
            if let Some(table) = self.convert_sheet(&sheet)? {
                children.push(table);
            }
        }

        Ok(children)
    }

    fn convert_sheet(&mut self, sheet: &ooxml_sml::Sheet) -> Result<Option<Node>, ParseError> {
        let rows = sheet.rows();

        if rows.is_empty() {
            return Ok(None);
        }

        // Determine dimensions
        let (min_row, min_col, max_row, max_col) = match sheet.dimensions() {
            Some(dims) => dims,
            None => return Ok(None),
        };

        let mut table_rows = Vec::new();
        let mut first_row = true;

        for row_num in min_row..=max_row {
            let mut cells = Vec::new();

            for col_num in min_col..=max_col {
                let cell_value = if let Some(row) = sheet.row(row_num) {
                    if let Some(cell) = row.cell_at_column(col_num) {
                        self.convert_cell_value(cell.value())
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Use table_header for first row, table_cell for others
                let cell_kind = if first_row {
                    node::TABLE_HEADER
                } else {
                    node::TABLE_CELL
                };

                let text_node = Node::new(node::TEXT).prop(prop::CONTENT, cell_value);
                let para = Node::new(node::PARAGRAPH).child(text_node);
                cells.push(Node::new(cell_kind).child(para));
            }

            table_rows.push(Node::new(node::TABLE_ROW).children(cells));
            first_row = false;
        }

        Ok(Some(Node::new(node::TABLE).children(table_rows)))
    }

    fn convert_cell_value(&mut self, value: &CellValue) -> String {
        match value {
            CellValue::Empty => String::new(),
            CellValue::String(s) => s.clone(),
            CellValue::Number(n) => {
                // Format numbers nicely (avoid trailing .0 for integers)
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    (*n as i64).to_string()
                } else {
                    n.to_string()
                }
            }
            CellValue::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            CellValue::Error(e) => {
                self.warn(format!("Cell contains error: {}", e));
                e.clone()
            }
        }
    }
}

fn extract_metadata<R: Read + Seek>(_workbook: &Workbook<R>) -> Properties {
    let mut metadata = Properties::new();
    // TODO: Extract properties from XLSX if ooxml-sml exposes them
    metadata.set("format", "xlsx");
    metadata
}

#[cfg(test)]
mod tests {
    // Tests would require actual XLSX files
    // Integration tests can be added with test fixtures
}
