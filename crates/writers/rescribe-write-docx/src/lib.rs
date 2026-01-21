//! DOCX (Word) writer for rescribe.
//!
//! Emits rescribe documents as Word documents (.docx) using the ooxml-wml crate.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_write_docx::emit;
//!
//! let doc = // ... create a rescribe Document
//! let bytes = emit(&doc)?.value;
//! std::fs::write("output.docx", bytes)?;
//! ```

use ooxml_wml::DocumentBuilder;
use rescribe_core::{
    ConversionResult, Document, EmitError, FidelityWarning, Node, Severity, WarningKind,
};
use rescribe_std::{node, prop};

/// Emit a rescribe Document as DOCX bytes.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut warnings = Vec::new();
    let mut builder = DocumentBuilder::new();

    // Convert document content
    convert_node(&mut builder, &doc.content, &mut warnings)?;

    // TODO: Add metadata support

    // Serialize to bytes
    let mut bytes = Vec::new();
    builder
        .write(&mut std::io::Cursor::new(&mut bytes))
        .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

    Ok(ConversionResult {
        value: bytes,
        warnings,
    })
}

fn convert_node(
    builder: &mut DocumentBuilder,
    node: &Node,
    warnings: &mut Vec<FidelityWarning>,
) -> Result<(), EmitError> {
    match node.kind.as_str() {
        node::DOCUMENT => {
            for child in &node.children {
                convert_node(builder, child, warnings)?;
            }
        }
        node::PARAGRAPH => {
            let text = extract_text(node);
            builder.add_paragraph(&text);
        }
        node::HEADING => {
            let text = extract_text(node);
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as u8;
            // Create a paragraph with heading style
            let para = builder.body_mut().add_paragraph();
            para.set_properties(ooxml_wml::ParagraphProperties {
                style: Some(format!("Heading{}", level)),
                ..Default::default()
            });
            para.add_run().set_text(&text);
        }
        node::LIST => {
            // TODO: Implement list support
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::Simplified("lists".to_string()),
                "Lists converted to plain paragraphs".to_string(),
            ));
            for child in &node.children {
                convert_node(builder, child, warnings)?;
            }
        }
        node::LIST_ITEM => {
            let text = extract_text(node);
            builder.add_paragraph(&format!("- {}", text));
        }
        node::TABLE => {
            // TODO: Implement table support using builder.body_mut().add_table()
            warnings.push(FidelityWarning::new(
                Severity::Minor,
                WarningKind::Simplified("tables".to_string()),
                "Tables converted to plain text".to_string(),
            ));
            for row in &node.children {
                let row_text: Vec<String> = row.children.iter().map(extract_text).collect();
                builder.add_paragraph(&row_text.join("\t"));
            }
        }
        node::CODE_BLOCK => {
            let content = node.props.get_str(prop::CONTENT).unwrap_or("");
            builder.add_paragraph(content);
            // TODO: Apply code style
        }
        node::BLOCKQUOTE => {
            // TODO: Implement blockquote styling
            for child in &node.children {
                convert_node(builder, child, warnings)?;
            }
        }
        _ => {
            // For other node types, extract text and add as paragraph
            let text = extract_text(node);
            if !text.is_empty() {
                builder.add_paragraph(&text);
            }
        }
    }
    Ok(())
}

fn extract_text(node: &Node) -> String {
    let mut text = String::new();

    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        text.push_str(content);
    }

    for child in &node.children {
        text.push_str(&extract_text(child));
    }

    text
}

#[cfg(test)]
mod tests {
    // Tests would go here
}
