//! Jupyter notebook (ipynb) reader for rescribe.
//!
//! Parses Jupyter notebooks into rescribe's document IR.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_read_ipynb::parse;
//!
//! let ipynb_content = r#"{"nbformat": 4, "cells": []}"#;
//! let result = parse(ipynb_content)?;
//! let doc = result.value;
//! ```

use base64::Engine;
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, Node, ParseError, Properties, Resource,
    ResourceId, ResourceMap, Severity, SourceInfo, WarningKind,
};
use rescribe_std::{node, prop};
use serde::Deserialize;

/// Jupyter notebook format structure
#[derive(Debug, Deserialize)]
struct Notebook {
    #[serde(default = "default_nbformat")]
    nbformat: u32,
    #[serde(default)]
    nbformat_minor: u32,
    #[serde(default)]
    metadata: NotebookMetadata,
    #[serde(default)]
    cells: Vec<Cell>,
}

fn default_nbformat() -> u32 {
    4
}

#[derive(Debug, Default, Deserialize)]
struct NotebookMetadata {
    #[serde(default)]
    kernelspec: Option<KernelSpec>,
    #[serde(default)]
    language_info: Option<LanguageInfo>,
}

#[derive(Debug, Deserialize)]
struct KernelSpec {
    #[serde(default)]
    name: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    language: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LanguageInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    file_extension: String,
}

#[derive(Debug, Deserialize)]
struct Cell {
    cell_type: String,
    #[serde(default)]
    source: CellSource,
    #[serde(default)]
    metadata: serde_json::Value,
    #[serde(default)]
    outputs: Vec<Output>,
    #[serde(default)]
    execution_count: Option<u32>,
}

/// Cell source can be a string or array of strings
#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum CellSource {
    #[default]
    Empty,
    String(String),
    Lines(Vec<String>),
}

impl CellSource {
    fn as_string(&self) -> String {
        match self {
            CellSource::Empty => String::new(),
            CellSource::String(s) => s.clone(),
            CellSource::Lines(lines) => lines.join(""),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Output {
    output_type: String,
    #[serde(default)]
    text: Option<CellSource>,
    #[serde(default)]
    data: Option<OutputData>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    ename: Option<String>,
    #[serde(default)]
    evalue: Option<String>,
    #[serde(default)]
    traceback: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct OutputData {
    #[serde(rename = "text/plain", default)]
    text_plain: Option<CellSource>,
    #[serde(rename = "text/html", default)]
    text_html: Option<CellSource>,
    #[serde(rename = "text/markdown", default)]
    text_markdown: Option<CellSource>,
    #[serde(rename = "image/png", default)]
    image_png: Option<String>,
    #[serde(rename = "image/jpeg", default)]
    image_jpeg: Option<String>,
    #[serde(rename = "image/svg+xml", default)]
    image_svg: Option<CellSource>,
    #[serde(rename = "application/json", default)]
    application_json: Option<serde_json::Value>,
}

/// Parse a Jupyter notebook from a JSON string.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    let notebook: Notebook = serde_json::from_str(input)
        .map_err(|e| ParseError::Invalid(format!("Failed to parse notebook JSON: {}", e)))?;

    let mut converter = Converter::new();
    let children = converter.convert_notebook(&notebook)?;

    let metadata = extract_metadata(&notebook);

    let document = Document {
        content: Node::new(node::DOCUMENT).children(children),
        resources: converter.resources,
        metadata,
        source: Some(SourceInfo {
            format: "ipynb".to_string(),
            metadata: Properties::new(),
        }),
    };

    Ok(ConversionResult::with_warnings(
        document,
        converter.warnings,
    ))
}

/// Parse notebook from bytes.
pub fn parse_bytes(input: &[u8]) -> Result<ConversionResult<Document>, ParseError> {
    let input_str = std::str::from_utf8(input)
        .map_err(|e| ParseError::Invalid(format!("Invalid UTF-8 in notebook: {}", e)))?;
    parse(input_str)
}

struct Converter {
    warnings: Vec<FidelityWarning>,
    resources: ResourceMap,
}

impl Converter {
    fn new() -> Self {
        Self {
            warnings: Vec::new(),
            resources: ResourceMap::new(),
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(FidelityWarning::new(
            Severity::Minor,
            WarningKind::FeatureLost("ipynb".to_string()),
            message,
        ));
    }

    fn add_resource(&mut self, data: Vec<u8>, content_type: &str) -> ResourceId {
        let id = ResourceId::new();
        let resource = Resource::new(content_type.to_string(), data);
        self.resources.insert(id.clone(), resource);
        id
    }

    fn convert_notebook(&mut self, notebook: &Notebook) -> Result<Vec<Node>, ParseError> {
        let mut children = Vec::new();

        // Get the notebook's language for code cells
        let language = notebook
            .metadata
            .language_info
            .as_ref()
            .map(|l| l.name.clone())
            .or_else(|| {
                notebook
                    .metadata
                    .kernelspec
                    .as_ref()
                    .map(|k| k.language.clone())
            })
            .unwrap_or_else(|| "python".to_string());

        for (i, cell) in notebook.cells.iter().enumerate() {
            let cell_nodes = self.convert_cell(cell, &language, i)?;
            children.extend(cell_nodes);
        }

        Ok(children)
    }

    fn convert_cell(
        &mut self,
        cell: &Cell,
        language: &str,
        cell_index: usize,
    ) -> Result<Vec<Node>, ParseError> {
        match cell.cell_type.as_str() {
            "markdown" => self.convert_markdown_cell(cell),
            "code" => self.convert_code_cell(cell, language, cell_index),
            "raw" => self.convert_raw_cell(cell),
            other => {
                self.warn(format!("Unknown cell type: {}", other));
                Ok(vec![])
            }
        }
    }

    fn convert_markdown_cell(&mut self, cell: &Cell) -> Result<Vec<Node>, ParseError> {
        let source = cell.source.as_string();

        // Parse markdown content using the markdown reader
        let result = rescribe_read_markdown::parse(&source)
            .map_err(|e| ParseError::Invalid(format!("Failed to parse markdown cell: {}", e)))?;

        // Add any warnings from markdown parsing
        self.warnings.extend(result.warnings);

        // Return the children of the document (not the document node itself)
        Ok(result.value.content.children)
    }

    fn convert_code_cell(
        &mut self,
        cell: &Cell,
        language: &str,
        _cell_index: usize,
    ) -> Result<Vec<Node>, ParseError> {
        let mut nodes = Vec::new();

        let source = cell.source.as_string();

        // Create code block for the source
        let mut code_block = Node::new(node::CODE_BLOCK)
            .prop(prop::CONTENT, source)
            .prop(prop::LANGUAGE, language.to_string());

        // Add execution count if present
        if let Some(count) = cell.execution_count {
            code_block = code_block.prop("ipynb:execution_count", count as i64);
        }

        nodes.push(code_block);

        // Convert outputs
        for output in &cell.outputs {
            if let Some(output_nodes) = self.convert_output(output)? {
                nodes.extend(output_nodes);
            }
        }

        Ok(nodes)
    }

    fn convert_raw_cell(&mut self, cell: &Cell) -> Result<Vec<Node>, ParseError> {
        let source = cell.source.as_string();

        // Determine format from metadata if available
        let format = cell
            .metadata
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        Ok(vec![
            Node::new(node::RAW_BLOCK)
                .prop(prop::CONTENT, source)
                .prop(prop::FORMAT, format.to_string()),
        ])
    }

    fn convert_output(&mut self, output: &Output) -> Result<Option<Vec<Node>>, ParseError> {
        match output.output_type.as_str() {
            "stream" => {
                // stdout/stderr output
                let text = output
                    .text
                    .as_ref()
                    .map(|t| t.as_string())
                    .unwrap_or_default();
                let stream_name = output.name.as_deref().unwrap_or("stdout");

                Ok(Some(vec![
                    Node::new(node::CODE_BLOCK)
                        .prop(prop::CONTENT, text)
                        .prop("ipynb:output_type", "stream")
                        .prop("ipynb:stream_name", stream_name.to_string()),
                ]))
            }
            "execute_result" | "display_data" => self.convert_display_output(output),
            "error" => {
                // Error output
                let mut content = String::new();
                if let Some(ename) = &output.ename {
                    content.push_str(ename);
                    content.push_str(": ");
                }
                if let Some(evalue) = &output.evalue {
                    content.push_str(evalue);
                }
                if let Some(traceback) = &output.traceback {
                    content.push('\n');
                    content.push_str(&traceback.join("\n"));
                }

                Ok(Some(vec![
                    Node::new(node::CODE_BLOCK)
                        .prop(prop::CONTENT, strip_ansi(&content))
                        .prop("ipynb:output_type", "error"),
                ]))
            }
            other => {
                self.warn(format!("Unknown output type: {}", other));
                Ok(None)
            }
        }
    }

    fn convert_display_output(&mut self, output: &Output) -> Result<Option<Vec<Node>>, ParseError> {
        let data = match &output.data {
            Some(d) => d,
            None => return Ok(None),
        };

        // Priority order: images > HTML > markdown > plain text
        // Check for images first
        if let Some(png_b64) = &data.image_png {
            let png_data = base64::engine::general_purpose::STANDARD
                .decode(png_b64.trim())
                .map_err(|e| ParseError::Invalid(format!("Invalid PNG base64: {}", e)))?;

            let resource_id = self.add_resource(png_data, "image/png");
            return Ok(Some(vec![
                Node::new(node::IMAGE)
                    .prop(prop::URL, format!("resource:{}", resource_id.as_str()))
                    .prop(prop::ALT, "Output image"),
            ]));
        }

        if let Some(jpeg_b64) = &data.image_jpeg {
            let jpeg_data = base64::engine::general_purpose::STANDARD
                .decode(jpeg_b64.trim())
                .map_err(|e| ParseError::Invalid(format!("Invalid JPEG base64: {}", e)))?;

            let resource_id = self.add_resource(jpeg_data, "image/jpeg");
            return Ok(Some(vec![
                Node::new(node::IMAGE)
                    .prop(prop::URL, format!("resource:{}", resource_id.as_str()))
                    .prop(prop::ALT, "Output image"),
            ]));
        }

        if let Some(svg) = &data.image_svg {
            return Ok(Some(vec![
                Node::new(node::RAW_BLOCK)
                    .prop(prop::CONTENT, svg.as_string())
                    .prop(prop::FORMAT, "svg"),
            ]));
        }

        // HTML output
        if let Some(html) = &data.text_html {
            return Ok(Some(vec![
                Node::new(node::RAW_BLOCK)
                    .prop(prop::CONTENT, html.as_string())
                    .prop(prop::FORMAT, "html"),
            ]));
        }

        // Markdown output
        if let Some(md) = &data.text_markdown {
            let result = rescribe_read_markdown::parse(&md.as_string()).map_err(|e| {
                ParseError::Invalid(format!("Failed to parse markdown output: {}", e))
            })?;
            self.warnings.extend(result.warnings);
            return Ok(Some(result.value.content.children));
        }

        // Plain text output
        if let Some(text) = &data.text_plain {
            return Ok(Some(vec![
                Node::new(node::CODE_BLOCK)
                    .prop(prop::CONTENT, text.as_string())
                    .prop("ipynb:output_type", "text"),
            ]));
        }

        Ok(None)
    }
}

fn extract_metadata(notebook: &Notebook) -> Properties {
    let mut metadata = Properties::new();

    metadata.set("nbformat", notebook.nbformat as i64);
    metadata.set("nbformat_minor", notebook.nbformat_minor as i64);

    if let Some(kernel) = &notebook.metadata.kernelspec {
        if !kernel.name.is_empty() {
            metadata.set("kernel_name", kernel.name.clone());
        }
        if !kernel.display_name.is_empty() {
            metadata.set("kernel_display_name", kernel.display_name.clone());
        }
        if !kernel.language.is_empty() {
            metadata.set("language", kernel.language.clone());
        }
    }

    if let Some(lang) = &notebook.metadata.language_info {
        if !lang.name.is_empty() {
            metadata.set("language", lang.name.clone());
        }
        if !lang.version.is_empty() {
            metadata.set("language_version", lang.version.clone());
        }
    }

    metadata
}

/// Strip ANSI escape codes from text (used in error tracebacks)
fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip escape sequence
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_notebook() {
        let input = r#"{"nbformat": 4, "nbformat_minor": 5, "metadata": {}, "cells": []}"#;
        let result = parse(input).unwrap();
        assert!(result.value.content.children.is_empty());
    }

    #[test]
    fn test_parse_markdown_cell() {
        let input = r##"{
            "nbformat": 4,
            "cells": [{
                "cell_type": "markdown",
                "source": "# Hello",
                "metadata": {}
            }]
        }"##;
        let result = parse(input).unwrap();
        let children = &result.value.content.children;
        assert!(!children.is_empty());
        assert_eq!(children[0].kind.as_str(), node::HEADING);
    }

    #[test]
    fn test_parse_code_cell() {
        let input = r#"{
            "nbformat": 4,
            "metadata": {"language_info": {"name": "python"}},
            "cells": [{
                "cell_type": "code",
                "source": "print('hello')",
                "metadata": {},
                "outputs": [],
                "execution_count": 1
            }]
        }"#;
        let result = parse(input).unwrap();
        let children = &result.value.content.children;
        assert!(!children.is_empty());
        assert_eq!(children[0].kind.as_str(), node::CODE_BLOCK);
        assert_eq!(children[0].props.get_str(prop::LANGUAGE), Some("python"));
        assert_eq!(children[0].props.get_int("ipynb:execution_count"), Some(1));
    }

    #[test]
    fn test_parse_cell_source_array() {
        let input = r##"{
            "nbformat": 4,
            "cells": [{
                "cell_type": "markdown",
                "source": ["# Hello", "World"],
                "metadata": {}
            }]
        }"##;
        let result = parse(input).unwrap();
        let children = &result.value.content.children;
        assert!(!children.is_empty());
    }

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[31mRed\x1b[0m text";
        let result = strip_ansi(input);
        assert_eq!(result, "Red text");
    }
}
