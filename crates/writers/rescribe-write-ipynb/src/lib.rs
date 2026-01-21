//! Jupyter notebook (ipynb) writer for rescribe.
//!
//! Emits rescribe's document IR as Jupyter notebook JSON.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_write_ipynb::emit;
//!
//! let doc = Document::new();
//! let result = emit(&doc)?;
//! let json = String::from_utf8(result.value)?;
//! ```

use rescribe_core::{
    ConversionResult, Document, EmitError, FidelityWarning, Node, Severity, WarningKind,
};
use rescribe_std::{node, prop};
use serde::Serialize;

/// Emit a document as a Jupyter notebook.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new(doc);
    let notebook = ctx.build_notebook()?;

    let json = serde_json::to_string_pretty(&notebook).map_err(|e| {
        EmitError::Io(std::io::Error::other(format!(
            "Failed to serialize notebook: {}",
            e
        )))
    })?;

    Ok(ConversionResult::with_warnings(
        json.into_bytes(),
        ctx.warnings,
    ))
}

/// Jupyter notebook structure for serialization
#[derive(Debug, Serialize)]
struct Notebook {
    nbformat: u32,
    nbformat_minor: u32,
    metadata: NotebookMetadata,
    cells: Vec<Cell>,
}

#[derive(Debug, Serialize)]
struct NotebookMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    kernelspec: Option<KernelSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_info: Option<LanguageInfo>,
}

#[derive(Debug, Serialize)]
struct KernelSpec {
    name: String,
    display_name: String,
    language: String,
}

#[derive(Debug, Serialize)]
struct LanguageInfo {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Debug, Serialize)]
struct Cell {
    cell_type: String,
    source: Vec<String>,
    metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<Output>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_count: Option<u32>,
}

#[derive(Debug, Serialize)]
struct Output {
    output_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_count: Option<u32>,
}

struct EmitContext<'a> {
    doc: &'a Document,
    warnings: Vec<FidelityWarning>,
}

impl<'a> EmitContext<'a> {
    fn new(doc: &'a Document) -> Self {
        Self {
            doc,
            warnings: Vec::new(),
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(FidelityWarning::new(
            Severity::Minor,
            WarningKind::FeatureLost("ipynb".to_string()),
            message,
        ));
    }

    fn build_notebook(&mut self) -> Result<Notebook, EmitError> {
        // Get metadata from document
        let nbformat = self
            .doc
            .metadata
            .get_int("nbformat")
            .map(|v| v as u32)
            .unwrap_or(4);
        let nbformat_minor = self
            .doc
            .metadata
            .get_int("nbformat_minor")
            .map(|v| v as u32)
            .unwrap_or(5);

        // Extract language info
        let language = self
            .doc
            .metadata
            .get_str("language")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "python".to_string());

        let language_info = Some(LanguageInfo {
            name: language.clone(),
            version: self
                .doc
                .metadata
                .get_str("language_version")
                .map(|s| s.to_string()),
        });

        let kernelspec = Some(KernelSpec {
            name: self
                .doc
                .metadata
                .get_str("kernel_name")
                .map(|s| s.to_string())
                .unwrap_or_else(|| language.clone()),
            display_name: self
                .doc
                .metadata
                .get_str("kernel_display_name")
                .map(|s| s.to_string())
                .unwrap_or_else(|| language.clone()),
            language: language.clone(),
        });

        // Convert document nodes to cells
        let cells = self.convert_nodes_to_cells(&self.doc.content.children, &language)?;

        Ok(Notebook {
            nbformat,
            nbformat_minor,
            metadata: NotebookMetadata {
                kernelspec,
                language_info,
            },
            cells,
        })
    }

    fn convert_nodes_to_cells(
        &mut self,
        nodes: &[Node],
        language: &str,
    ) -> Result<Vec<Cell>, EmitError> {
        let mut cells = Vec::new();
        let mut markdown_nodes: Vec<&Node> = Vec::new();

        for node in nodes {
            let kind = node.kind.as_str();

            // Check if this is a code block
            if kind == node::CODE_BLOCK {
                // Flush any accumulated markdown content
                if !markdown_nodes.is_empty() {
                    cells.push(self.create_markdown_cell(&markdown_nodes)?);
                    markdown_nodes.clear();
                }

                // Create code cell
                cells.push(self.create_code_cell(node, language)?);
            } else if kind == node::RAW_BLOCK {
                // Check if this is notebook output
                let is_output = node.props.get_str("ipynb:output_type").is_some();

                if is_output {
                    // Skip outputs - they're associated with code cells
                    self.warn("Output block found outside code cell context");
                } else {
                    // Flush markdown
                    if !markdown_nodes.is_empty() {
                        cells.push(self.create_markdown_cell(&markdown_nodes)?);
                        markdown_nodes.clear();
                    }

                    // Create raw cell
                    cells.push(self.create_raw_cell(node)?);
                }
            } else {
                // Accumulate as markdown
                markdown_nodes.push(node);
            }
        }

        // Flush remaining markdown content
        if !markdown_nodes.is_empty() {
            cells.push(self.create_markdown_cell(&markdown_nodes)?);
        }

        Ok(cells)
    }

    fn create_markdown_cell(&mut self, nodes: &[&Node]) -> Result<Cell, EmitError> {
        // Convert nodes to markdown using the markdown writer
        let temp_doc = Document::new()
            .with_content(Node::new(node::DOCUMENT).children(nodes.iter().map(|n| (*n).clone())));

        let result = rescribe_write_markdown::emit(&temp_doc).map_err(|e| {
            EmitError::Io(std::io::Error::other(format!(
                "Failed to emit markdown: {}",
                e
            )))
        })?;

        self.warnings.extend(result.warnings);

        let markdown = String::from_utf8(result.value).map_err(|e| {
            EmitError::Io(std::io::Error::other(format!(
                "Invalid UTF-8 in markdown: {}",
                e
            )))
        })?;

        // Split into lines, preserving line endings
        let source = split_source(&markdown);

        Ok(Cell {
            cell_type: "markdown".to_string(),
            source,
            metadata: serde_json::json!({}),
            outputs: None,
            execution_count: None,
        })
    }

    fn create_code_cell(&mut self, node: &Node, default_language: &str) -> Result<Cell, EmitError> {
        let content = node.props.get_str(prop::CONTENT).unwrap_or("");
        let _cell_language = node
            .props
            .get_str(prop::LANGUAGE)
            .unwrap_or(default_language);

        // Get execution count if present
        let execution_count = node
            .props
            .get_int("ipynb:execution_count")
            .map(|v| v as u32);

        let source = split_source(content);

        Ok(Cell {
            cell_type: "code".to_string(),
            source,
            metadata: serde_json::json!({}),
            outputs: Some(Vec::new()), // Code cells always have outputs array
            execution_count,
        })
    }

    fn create_raw_cell(&mut self, node: &Node) -> Result<Cell, EmitError> {
        let content = node.props.get_str(prop::CONTENT).unwrap_or("");
        let format = node.props.get_str(prop::FORMAT);

        let mut metadata = serde_json::Map::new();
        if let Some(fmt) = format {
            metadata.insert(
                "format".to_string(),
                serde_json::Value::String(fmt.to_string()),
            );
        }

        let source = split_source(content);

        Ok(Cell {
            cell_type: "raw".to_string(),
            source,
            metadata: serde_json::Value::Object(metadata),
            outputs: None,
            execution_count: None,
        })
    }
}

/// Split source content into lines, preserving line endings (as Jupyter does)
fn split_source(content: &str) -> Vec<String> {
    if content.is_empty() {
        return vec![];
    }

    let mut lines = Vec::new();
    let mut start = 0;

    for (i, c) in content.char_indices() {
        if c == '\n' {
            lines.push(content[start..=i].to_string());
            start = i + 1;
        }
    }

    // Add remaining content (without newline at end)
    if start < content.len() {
        lines.push(content[start..].to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::doc;

    #[test]
    fn test_emit_empty_document() {
        let document = doc(|d| d);
        let result = emit(&document).unwrap();
        let json = String::from_utf8(result.value).unwrap();

        let notebook: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(notebook["nbformat"], 4);
        assert!(notebook["cells"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_emit_markdown_cell() {
        let document = doc(|d| d.heading(1, |i| i.text("Hello")).para(|i| i.text("World")));
        let result = emit(&document).unwrap();
        let json = String::from_utf8(result.value).unwrap();

        let notebook: serde_json::Value = serde_json::from_str(&json).unwrap();
        let cells = notebook["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0]["cell_type"], "markdown");
    }

    #[test]
    fn test_emit_code_cell() {
        let document = doc(|d| d.code_block_lang("python", "print('hello')"));
        let result = emit(&document).unwrap();
        let json = String::from_utf8(result.value).unwrap();

        let notebook: serde_json::Value = serde_json::from_str(&json).unwrap();
        let cells = notebook["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0]["cell_type"], "code");
    }

    #[test]
    fn test_split_source() {
        let source = "line1\nline2\nline3";
        let lines = split_source(source);
        assert_eq!(lines, vec!["line1\n", "line2\n", "line3"]);
    }

    #[test]
    fn test_split_source_with_trailing_newline() {
        let source = "line1\nline2\n";
        let lines = split_source(source);
        assert_eq!(lines, vec!["line1\n", "line2\n"]);
    }
}
