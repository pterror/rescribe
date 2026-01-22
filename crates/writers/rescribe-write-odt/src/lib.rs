//! ODT (OpenDocument Text) writer for rescribe.
//!
//! Generates ODF/ODT documents from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};
use std::io::{Cursor, Write};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Emit a document to ODT.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to ODT with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // mimetype must be first and uncompressed
        zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(b"application/vnd.oasis.opendocument.text")
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // META-INF/manifest.xml
        zip.start_file("META-INF/manifest.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_manifest().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // meta.xml
        zip.start_file("meta.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_meta(doc).as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // styles.xml
        zip.start_file("styles.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_styles().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // content.xml
        zip.start_file("content.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_content(doc).as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        zip.finish()
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
    }

    Ok(ConversionResult::ok(buffer.into_inner()))
}

fn generate_manifest() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>
</manifest:manifest>
"#.to_string()
}

fn generate_meta(doc: &Document) -> String {
    let title = doc.metadata.get_str("title").unwrap_or("Untitled");
    let author = doc.metadata.get_str("author").unwrap_or("");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                      xmlns:dc="http://purl.org/dc/elements/1.1/"
                      xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0"
                      office:version="1.2">
  <office:meta>
    <dc:title>{}</dc:title>
    <dc:creator>{}</dc:creator>
    <meta:generator>rescribe</meta:generator>
  </office:meta>
</office:document-meta>
"#,
        escape_xml(title),
        escape_xml(author)
    )
}

fn generate_styles() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                        xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
                        xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
                        xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
                        office:version="1.2">
  <office:styles>
    <style:style style:name="Bold" style:family="text">
      <style:text-properties fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Italic" style:family="text">
      <style:text-properties fo:font-style="italic"/>
    </style:style>
    <style:style style:name="Code" style:family="text">
      <style:text-properties style:font-name="Courier New"/>
    </style:style>
    <style:style style:name="Underline" style:family="text">
      <style:text-properties style:text-underline-style="solid"/>
    </style:style>
    <style:style style:name="Strikethrough" style:family="text">
      <style:text-properties style:text-line-through-style="solid"/>
    </style:style>
  </office:styles>
  <office:automatic-styles>
    <style:style style:name="Heading1" style:family="paragraph">
      <style:text-properties fo:font-size="24pt" fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Heading2" style:family="paragraph">
      <style:text-properties fo:font-size="18pt" fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Heading3" style:family="paragraph">
      <style:text-properties fo:font-size="14pt" fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Preformatted" style:family="paragraph">
      <style:text-properties style:font-name="Courier New"/>
    </style:style>
    <style:style style:name="Quotation" style:family="paragraph">
      <style:paragraph-properties fo:margin-left="0.5in"/>
    </style:style>
  </office:automatic-styles>
</office:document-styles>
"#
    .to_string()
}

fn generate_content(doc: &Document) -> String {
    let mut content = String::new();

    content.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                         xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
                         xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
                         xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
                         xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
                         xmlns:xlink="http://www.w3.org/1999/xlink"
                         office:version="1.2">
  <office:automatic-styles>
    <style:style style:name="Heading1" style:family="paragraph">
      <style:text-properties fo:font-size="24pt" fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Heading2" style:family="paragraph">
      <style:text-properties fo:font-size="18pt" fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Heading3" style:family="paragraph">
      <style:text-properties fo:font-size="14pt" fo:font-weight="bold"/>
    </style:style>
    <style:style style:name="Preformatted" style:family="paragraph">
      <style:text-properties style:font-name="Courier New"/>
    </style:style>
  </office:automatic-styles>
  <office:body>
    <office:text>
"#,
    );

    emit_nodes(&doc.content.children, &mut content);

    content.push_str("    </office:text>\n  </office:body>\n</office:document-content>\n");

    content
}

fn emit_nodes(nodes: &[Node], output: &mut String) {
    for node in nodes {
        emit_node(node, output);
    }
}

fn emit_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::DOCUMENT => emit_nodes(&node.children, output),

        node::HEADING => {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            let style = match level {
                1 => "Heading1",
                2 => "Heading2",
                _ => "Heading3",
            };
            output.push_str(&format!("      <text:p text:style-name=\"{}\">\n", style));
            emit_inline_nodes(&node.children, output);
            output.push_str("      </text:p>\n");
        }

        node::PARAGRAPH => {
            output.push_str("      <text:p>\n");
            emit_inline_nodes(&node.children, output);
            output.push_str("      </text:p>\n");
        }

        node::CODE_BLOCK => {
            output.push_str("      <text:p text:style-name=\"Preformatted\">\n");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                for (i, line) in content.lines().enumerate() {
                    if i > 0 {
                        output.push_str("<text:line-break/>");
                    }
                    output.push_str(&escape_xml(line));
                }
            }
            output.push_str("\n      </text:p>\n");
        }

        node::BLOCKQUOTE => {
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    output.push_str("      <text:p text:style-name=\"Quotation\">\n");
                    emit_inline_nodes(&child.children, output);
                    output.push_str("      </text:p>\n");
                } else {
                    emit_node(child, output);
                }
            }
        }

        node::LIST => {
            output.push_str("      <text:list>\n");
            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str("        <text:list-item>\n");
                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            output.push_str("          <text:p>");
                            emit_inline_nodes(&item_child.children, output);
                            output.push_str("</text:p>\n");
                        } else {
                            emit_node(item_child, output);
                        }
                    }
                    output.push_str("        </text:list-item>\n");
                }
            }
            output.push_str("      </text:list>\n");
        }

        node::TABLE => {
            output.push_str("      <table:table>\n");
            for row in &node.children {
                if row.kind.as_str() == node::TABLE_ROW {
                    output.push_str("        <table:table-row>\n");
                    for cell in &row.children {
                        output.push_str("          <table:table-cell>\n");
                        output.push_str("            <text:p>");
                        emit_inline_nodes(&cell.children, output);
                        output.push_str("</text:p>\n");
                        output.push_str("          </table:table-cell>\n");
                    }
                    output.push_str("        </table:table-row>\n");
                }
            }
            output.push_str("      </table:table>\n");
        }

        node::HORIZONTAL_RULE => {
            output.push_str("      <text:p>―――――――――――――――――――</text:p>\n");
        }

        node::DIV | node::SPAN | node::FIGURE => {
            emit_nodes(&node.children, output);
        }

        _ => emit_nodes(&node.children, output),
    }
}

fn emit_inline_nodes(nodes: &[Node], output: &mut String) {
    for node in nodes {
        emit_inline_node(node, output);
    }
}

fn emit_inline_node(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_xml(content));
            }
        }

        node::STRONG => {
            output.push_str("<text:span text:style-name=\"Bold\">");
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::EMPHASIS => {
            output.push_str("<text:span text:style-name=\"Italic\">");
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::UNDERLINE => {
            output.push_str("<text:span text:style-name=\"Underline\">");
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::STRIKEOUT => {
            output.push_str("<text:span text:style-name=\"Strikethrough\">");
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::CODE => {
            output.push_str("<text:span text:style-name=\"Code\">");
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&escape_xml(content));
            }
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::LINK => {
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!(
                    "<text:a xlink:type=\"simple\" xlink:href=\"{}\">",
                    escape_xml(url)
                ));
            }
            emit_inline_nodes(&node.children, output);
            if node.props.get_str(prop::URL).is_some() {
                output.push_str("</text:a>");
            }
        }

        node::IMAGE => {
            // ODT images require embedding in the ZIP; for now output placeholder
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!("[Image: {}]", escape_xml(url)));
            }
        }

        node::SUBSCRIPT => {
            output.push_str("<text:span text:style-name=\"Subscript\">");
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::SUPERSCRIPT => {
            output.push_str("<text:span text:style-name=\"Superscript\">");
            emit_inline_nodes(&node.children, output);
            output.push_str("</text:span>");
        }

        node::LINE_BREAK => output.push_str("<text:line-break/>"),
        node::SOFT_BREAK => output.push(' '),

        _ => emit_inline_nodes(&node.children, output),
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_std::builder::*;

    #[test]
    fn test_emit_basic() {
        let document = doc(|d| {
            d.heading(1, |h| h.text("Title"))
                .para(|p| p.text("Hello world"))
        });
        let result = emit(&document).unwrap();
        assert!(!result.value.is_empty());

        // Check it's a valid ZIP starting with PK
        assert_eq!(&result.value[0..2], b"PK");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }
}
