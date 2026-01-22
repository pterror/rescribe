//! PPTX (PowerPoint) writer for rescribe.
//!
//! Generates PPTX presentations from rescribe's document IR.
//! Slides are created from level-1 headings.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};
use std::io::{Cursor, Write};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Emit a document to PPTX.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to PPTX with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let title = doc
        .metadata
        .get_str("title")
        .unwrap_or("Presentation")
        .to_string();

    // Collect slides (split on h1 headings)
    let slides = collect_slides(&doc.content.children);

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // [Content_Types].xml
        zip.start_file("[Content_Types].xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_content_types(slides.len()).as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // _rels/.rels
        zip.start_file("_rels/.rels", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_rels().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/presentation.xml
        zip.start_file("ppt/presentation.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_presentation(slides.len()).as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/_rels/presentation.xml.rels
        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_presentation_rels(slides.len()).as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/slideMasters/slideMaster1.xml
        zip.start_file("ppt/slideMasters/slideMaster1.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_slide_master().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/slideMasters/_rels/slideMaster1.xml.rels
        zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_slide_master_rels().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/slideLayouts/slideLayout1.xml
        zip.start_file("ppt/slideLayouts/slideLayout1.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_slide_layout().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/slideLayouts/_rels/slideLayout1.xml.rels
        zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_slide_layout_rels().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // ppt/theme/theme1.xml
        zip.start_file("ppt/theme/theme1.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_theme().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // Generate title slide
        zip.start_file("ppt/slides/slide1.xml", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_title_slide(&title, doc.metadata.get_str("author")).as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        zip.start_file("ppt/slides/_rels/slide1.xml.rels", options)
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        zip.write_all(generate_slide_rels().as_bytes())
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

        // Generate content slides
        for (i, slide) in slides.iter().enumerate() {
            let slide_num = i + 2; // Title slide is 1
            zip.start_file(format!("ppt/slides/slide{}.xml", slide_num), options)
                .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
            zip.write_all(generate_content_slide(slide).as_bytes())
                .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;

            zip.start_file(
                format!("ppt/slides/_rels/slide{}.xml.rels", slide_num),
                options,
            )
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
            zip.write_all(generate_slide_rels().as_bytes())
                .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
        }

        zip.finish()
            .map_err(|e| EmitError::Io(std::io::Error::other(e.to_string())))?;
    }

    Ok(ConversionResult::ok(buffer.into_inner()))
}

struct Slide<'a> {
    title: String,
    content: Vec<&'a Node>,
}

fn collect_slides(nodes: &[Node]) -> Vec<Slide<'_>> {
    let mut slides: Vec<Slide> = Vec::new();
    let mut current_title = String::new();
    let mut current_content: Vec<&Node> = Vec::new();

    for node in nodes {
        if node.kind.as_str() == node::HEADING {
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            if level == 1 {
                // Save previous slide if it has content
                if !current_content.is_empty() || !current_title.is_empty() {
                    slides.push(Slide {
                        title: current_title,
                        content: current_content,
                    });
                    current_content = Vec::new();
                }
                current_title = get_text_content(node);
            } else {
                current_content.push(node);
            }
        } else {
            current_content.push(node);
        }
    }

    // Save final slide
    if !current_content.is_empty() || !current_title.is_empty() {
        slides.push(Slide {
            title: current_title,
            content: current_content,
        });
    }

    slides
}

fn get_text_content(node: &Node) -> String {
    let mut text = String::new();
    collect_text(node, &mut text);
    text
}

fn collect_text(node: &Node, output: &mut String) {
    if node.kind.as_str() == node::TEXT
        && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        output.push_str(content);
    }
    for child in &node.children {
        collect_text(child, output);
    }
}

fn generate_content_types(slide_count: usize) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
"#.to_string();

    // Title slide + content slides
    for i in 1..=(slide_count + 1) {
        xml.push_str(&format!(
            "  <Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\n",
            i
        ));
    }

    xml.push_str("</Types>\n");
    xml
}

fn generate_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>
"#
    .to_string()
}

fn generate_presentation(slide_count: usize) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
"#
    .to_string();

    for i in 1..=(slide_count + 1) {
        xml.push_str(&format!(
            "    <p:sldId id=\"{}\" r:id=\"rId{}\"/>\n",
            255 + i,
            i + 2
        ));
    }

    xml.push_str(
        r#"  </p:sldIdLst>
  <p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>
"#,
    );
    xml
}

fn generate_presentation_rels(slide_count: usize) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
"#.to_string();

    for i in 1..=(slide_count + 1) {
        xml.push_str(&format!(
            "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>\n",
            i + 2, i
        ));
    }

    xml.push_str("</Relationships>\n");
    xml
}

fn generate_slide_master() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483649" r:id="rId1"/>
  </p:sldLayoutIdLst>
</p:sldMaster>
"#.to_string()
}

fn generate_slide_master_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>
"#.to_string()
}

fn generate_slide_layout() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             type="blank">
  <p:cSld name="Blank">
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
</p:sldLayout>
"#
    .to_string()
}

fn generate_slide_layout_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>
"#.to_string()
}

fn generate_theme() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5B9BD5"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Office">
      <a:fillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
      </a:fillStyleLst>
      <a:lnStyleLst>
        <a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
        <a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
        <a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
      </a:lnStyleLst>
      <a:effectStyleLst>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
      </a:effectStyleLst>
      <a:bgFillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
      </a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>
"#
    .to_string()
}

fn generate_slide_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>
"#.to_string()
}

fn generate_title_slide(title: &str, author: Option<&str>) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="1600200"/>
            <a:ext cx="8229600" cy="1143000"/>
          </a:xfrm>
          <a:prstGeom prst="rect"/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr anchor="ctr"/>
          <a:p>
            <a:pPr algn="ctr"/>
            <a:r>
              <a:rPr lang="en-US" sz="4400" b="1"/>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
      {}
    </p:spTree>
  </p:cSld>
</p:sld>
"#,
        escape_xml(title),
        author.map_or(String::new(), |a| format!(
            r#"<p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Subtitle"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="3200400"/>
            <a:ext cx="8229600" cy="571500"/>
          </a:xfrm>
          <a:prstGeom prst="rect"/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr anchor="ctr"/>
          <a:p>
            <a:pPr algn="ctr"/>
            <a:r>
              <a:rPr lang="en-US" sz="2400"/>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>"#,
            escape_xml(a)
        ))
    )
}

fn generate_content_slide(slide: &Slide) -> String {
    let mut content_text = String::new();
    for node in &slide.content {
        emit_node_text(node, &mut content_text);
        content_text.push('\n');
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="274638"/>
            <a:ext cx="8229600" cy="1143000"/>
          </a:xfrm>
          <a:prstGeom prst="rect"/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:p>
            <a:r>
              <a:rPr lang="en-US" sz="3200" b="1"/>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="1600200"/>
            <a:ext cx="8229600" cy="4525963"/>
          </a:xfrm>
          <a:prstGeom prst="rect"/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:p>
            <a:r>
              <a:rPr lang="en-US" sz="2000"/>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>
"#,
        escape_xml(&slide.title),
        escape_xml(content_text.trim())
    )
}

fn emit_node_text(node: &Node, output: &mut String) {
    match node.kind.as_str() {
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
            }
        }
        node::PARAGRAPH => {
            for child in &node.children {
                emit_node_text(child, output);
            }
            output.push('\n');
        }
        node::LIST => {
            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str("• ");
                    for item_child in &child.children {
                        emit_node_text(item_child, output);
                    }
                    output.push('\n');
                }
            }
        }
        node::CODE_BLOCK => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(content);
                output.push('\n');
            }
        }
        _ => {
            for child in &node.children {
                emit_node_text(child, output);
            }
        }
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
            d.heading(1, |h| h.text("Slide 1"))
                .para(|p| p.text("Content 1"))
                .heading(1, |h| h.text("Slide 2"))
                .para(|p| p.text("Content 2"))
        });
        let result = emit(&document).unwrap();
        assert!(!result.value.is_empty());

        // Check it's a valid ZIP starting with PK
        assert_eq!(&result.value[0..2], b"PK");
    }

    #[test]
    fn test_collect_slides() {
        let nodes = vec![
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 1)
                .child(Node::new(node::TEXT).prop(prop::CONTENT, "Slide 1")),
            Node::new(node::PARAGRAPH).child(Node::new(node::TEXT).prop(prop::CONTENT, "Content")),
            Node::new(node::HEADING)
                .prop(prop::LEVEL, 1)
                .child(Node::new(node::TEXT).prop(prop::CONTENT, "Slide 2")),
        ];
        let slides = collect_slides(&nodes);
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].title, "Slide 1");
        assert_eq!(slides[1].title, "Slide 2");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }
}
