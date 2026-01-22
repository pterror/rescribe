//! PPTX (PowerPoint) reader for rescribe.
//!
//! Parses PPTX presentations into rescribe's document IR.
//! Each slide becomes a section headed by its title.

use quick_xml::Reader;
use quick_xml::events::Event;
use rescribe_core::{ConversionResult, Document, ParseError, ParseOptions, Properties};
use rescribe_std::{Node, node, prop};
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Parse PPTX input into a document.
pub fn parse(input: &[u8]) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse PPTX input into a document with options.
pub fn parse_with_options(
    input: &[u8],
    _options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let cursor = Cursor::new(input);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| ParseError::Invalid(format!("Invalid PPTX: {}", e)))?;

    let metadata = Properties::new();
    let mut doc = Node::new(node::DOCUMENT);

    // Get slide count from presentation.xml
    let slide_count = get_slide_count(&mut archive)?;

    // Parse each slide
    for i in 1..=slide_count {
        let slide_path = format!("ppt/slides/slide{}.xml", i);
        if let Ok(mut slide_file) = archive.by_name(&slide_path) {
            let mut slide_xml = String::new();
            slide_file
                .read_to_string(&mut slide_xml)
                .map_err(ParseError::Io)?;

            if let Some(slide_node) = parse_slide(&slide_xml, i)? {
                doc = doc.child(slide_node);
            }
        }
    }

    Ok(ConversionResult::ok(Document {
        content: doc,
        resources: Default::default(),
        metadata,
        source: None,
    }))
}

fn get_slide_count(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<usize, ParseError> {
    let mut count = 0;

    // Count slide files
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                count += 1;
            }
        }
    }

    Ok(count)
}

fn parse_slide(xml: &str, slide_num: usize) -> Result<Option<Node>, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut current_text = String::new();
    let mut in_text = false;
    let mut is_title = false;
    let mut title = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = name.local_name();

                match local.as_ref() {
                    b"t" => {
                        in_text = true;
                        current_text.clear();
                    }
                    b"ph" => {
                        // Check for title placeholder
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"type" {
                                let value = String::from_utf8_lossy(&attr.value);
                                if value == "title" || value == "ctrTitle" {
                                    is_title = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let local = name.local_name();

                match local.as_ref() {
                    b"t" => {
                        if in_text && !current_text.is_empty() {
                            if is_title && title.is_empty() {
                                title = current_text.clone();
                            } else {
                                texts.push(current_text.clone());
                            }
                        }
                        in_text = false;
                    }
                    b"sp" => {
                        is_title = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_text {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    current_text.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParseError::Invalid(format!("XML error: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // Build slide content
    if title.is_empty() && texts.is_empty() {
        return Ok(None);
    }

    let mut slide = Node::new(node::DIV).prop("slide", slide_num as i64);

    // Add title as heading
    if !title.is_empty() {
        let heading = Node::new(node::HEADING)
            .prop(prop::LEVEL, 1)
            .child(Node::new(node::TEXT).prop(prop::CONTENT, title));
        slide = slide.child(heading);
    }

    // Add content as paragraphs
    for text in texts {
        if !text.trim().is_empty() {
            let para =
                Node::new(node::PARAGRAPH).child(Node::new(node::TEXT).prop(prop::CONTENT, text));
            slide = slide.child(para);
        }
    }

    Ok(Some(slide))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn create_test_pptx() -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let options = SimpleFileOptions::default();

            // Content types
            zip.start_file("[Content_Types].xml", options).unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#,
            )
            .unwrap();

            // Slide 1
            zip.start_file("ppt/slides/slide1.xml", options).unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:nvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:txBody>
          <a:p><a:r><a:t>Test Title</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Content text</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#,
            )
            .unwrap();

            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn test_parse_basic() {
        let pptx = create_test_pptx();
        let result = parse(&pptx).unwrap();
        assert!(!result.value.content.children.is_empty());
    }
}
