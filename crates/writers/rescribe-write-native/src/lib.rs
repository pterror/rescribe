//! Native format writer for rescribe.
//!
//! Outputs a human-readable representation of the document AST for debugging.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node, PropValue};

/// Emit a document to native format.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to native format with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();

    output.push_str("Document {\n");

    // Metadata
    if !doc.metadata.is_empty() {
        output.push_str("  metadata: {\n");
        for (key, value) in doc.metadata.iter() {
            output.push_str(&format!("    {}: {}\n", key, format_value(value)));
        }
        output.push_str("  }\n");
    }

    // Content
    output.push_str("  content:\n");
    emit_node(&doc.content, &mut output, 2);

    // Resources
    if !doc.resources.is_empty() {
        output.push_str("  resources: [\n");
        for (id, resource) in &doc.resources {
            output.push_str(&format!(
                "    Resource {{ id: {:?}, mime: {:?}, size: {} }}\n",
                id,
                resource.mime_type,
                resource.data.len()
            ));
        }
        output.push_str("  ]\n");
    }

    output.push_str("}\n");

    Ok(ConversionResult::ok(output.into_bytes()))
}

fn emit_node(node: &Node, output: &mut String, indent: usize) {
    let indent_str = "  ".repeat(indent);

    output.push_str(&format!("{}{}(", indent_str, node.kind));

    // Props
    if !node.props.is_empty() {
        output.push_str(" {");
        let props: Vec<String> = node
            .props
            .iter()
            .map(|(k, v)| format!(" {}: {}", k, format_value(v)))
            .collect();
        output.push_str(&props.join(","));
        output.push_str(" }");
    }

    // Children
    if node.children.is_empty() {
        output.push_str(")\n");
    } else {
        output.push_str(") [\n");
        for child in &node.children {
            emit_node(child, output, indent + 1);
        }
        output.push_str(&format!("{}]\n", indent_str));
    }
}

fn format_value(value: &PropValue) -> String {
    match value {
        PropValue::String(s) => format!("{:?}", s),
        PropValue::Int(i) => format!("{}", i),
        PropValue::Float(f) => format!("{}", f),
        PropValue::Bool(b) => format!("{}", b),
        PropValue::List(items) => {
            let formatted: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", formatted.join(", "))
        }
        PropValue::Map(map) => {
            let formatted: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{{}}}", formatted.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::*;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_basic() {
        let doc = doc(|d| {
            d.heading(1, |h| h.text("Title"))
                .para(|p| p.text("Hello world"))
        });
        let output = emit_str(&doc);
        assert!(output.contains("Document {"));
        assert!(output.contains("heading("));
        assert!(output.contains("paragraph("));
        assert!(output.contains("text("));
    }

    #[test]
    fn test_emit_props() {
        let doc = doc(|d| d.heading(2, |h| h.text("Level 2")));
        let output = emit_str(&doc);
        assert!(output.contains("level: 2"));
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value(&PropValue::String("test".into())), "\"test\"");
        assert_eq!(format_value(&PropValue::Int(42)), "42");
        assert_eq!(format_value(&PropValue::Bool(true)), "true");
    }
}
