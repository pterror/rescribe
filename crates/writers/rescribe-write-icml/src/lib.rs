//! ICML (InCopy Markup Language) writer for rescribe.
//!
//! Generates Adobe InDesign/InCopy ICML markup from rescribe's document IR.

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, Node};
use rescribe_std::{node, prop};

/// Emit a document to ICML.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document to ICML with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut output = String::new();

    // ICML header
    output.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?aid style="50" type="snippet" readerVersion="6.0" featureSet="513" product="8.0(370)" ?>
<?aid SnsijsRef="0" type="Snippet" ?>
<Document DOMVersion="8.0" Self="d">
  <RootCharacterStyleGroup Self="u10">
    <CharacterStyle Self="CharacterStyle/$ID/[No character style]" Name="$ID/[No character style]" />
    <CharacterStyle Self="CharacterStyle/Bold" Name="Bold" FontStyle="Bold" />
    <CharacterStyle Self="CharacterStyle/Italic" Name="Italic" FontStyle="Italic" />
    <CharacterStyle Self="CharacterStyle/Code" Name="Code" AppliedFont="Courier" />
  </RootCharacterStyleGroup>
  <RootParagraphStyleGroup Self="u11">
    <ParagraphStyle Self="ParagraphStyle/$ID/[No paragraph style]" Name="$ID/[No paragraph style]" />
    <ParagraphStyle Self="ParagraphStyle/Heading1" Name="Heading1" PointSize="24" FontStyle="Bold" />
    <ParagraphStyle Self="ParagraphStyle/Heading2" Name="Heading2" PointSize="18" FontStyle="Bold" />
    <ParagraphStyle Self="ParagraphStyle/Heading3" Name="Heading3" PointSize="14" FontStyle="Bold" />
    <ParagraphStyle Self="ParagraphStyle/Body" Name="Body" />
    <ParagraphStyle Self="ParagraphStyle/Code" Name="Code" AppliedFont="Courier" />
    <ParagraphStyle Self="ParagraphStyle/BlockQuote" Name="BlockQuote" LeftIndent="36" />
  </RootParagraphStyleGroup>
  <Story Self="u12" TrackChanges="false" StoryTitle="" AppliedTOCStyle="n" AppliedNamedGrid="n">
    <StoryPreference OpticalMarginAlignment="false" OpticalMarginSize="12" />
"#);

    emit_nodes(&doc.content.children, &mut output);

    output.push_str("  </Story>\n</Document>\n");

    Ok(ConversionResult::ok(output.into_bytes()))
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
            output.push_str(&format!(
                "    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/{}\">\n",
                style
            ));
            emit_inline_nodes(&node.children, output);
            output.push_str("      <Br />\n    </ParagraphStyleRange>\n");
        }

        node::PARAGRAPH => {
            output.push_str(
                "    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/Body\">\n",
            );
            emit_inline_nodes(&node.children, output);
            output.push_str("      <Br />\n    </ParagraphStyleRange>\n");
        }

        node::CODE_BLOCK => {
            output.push_str(
                "    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/Code\">\n",
            );
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                for line in content.lines() {
                    output.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");
                    output.push_str(&format!(
                        "        <Content>{}</Content>\n",
                        escape_xml(line)
                    ));
                    output.push_str("      </CharacterStyleRange>\n      <Br />\n");
                }
            }
            output.push_str("    </ParagraphStyleRange>\n");
        }

        node::BLOCKQUOTE => {
            output.push_str(
                "    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/BlockQuote\">\n",
            );
            for child in &node.children {
                if child.kind.as_str() == node::PARAGRAPH {
                    emit_inline_nodes(&child.children, output);
                } else {
                    emit_node(child, output);
                }
            }
            output.push_str("      <Br />\n    </ParagraphStyleRange>\n");
        }

        node::LIST => {
            let ordered = node.props.get_bool(prop::ORDERED).unwrap_or(false);
            let mut item_num = 1;

            for child in &node.children {
                if child.kind.as_str() == node::LIST_ITEM {
                    output.push_str(
                        "    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/Body\">\n",
                    );
                    output.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");

                    let bullet = if ordered {
                        let num = item_num;
                        item_num += 1;
                        format!("{}. ", num)
                    } else {
                        "• ".to_string()
                    };
                    output.push_str(&format!("        <Content>{}</Content>\n", bullet));
                    output.push_str("      </CharacterStyleRange>\n");

                    for item_child in &child.children {
                        if item_child.kind.as_str() == node::PARAGRAPH {
                            emit_inline_nodes(&item_child.children, output);
                        }
                    }
                    output.push_str("      <Br />\n    </ParagraphStyleRange>\n");
                }
            }
        }

        node::HORIZONTAL_RULE => {
            output.push_str(
                "    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/Body\">\n",
            );
            output.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");
            output.push_str("        <Content>―――――――――</Content>\n");
            output.push_str(
                "      </CharacterStyleRange>\n      <Br />\n    </ParagraphStyleRange>\n",
            );
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
                output.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");
                output.push_str(&format!(
                    "        <Content>{}</Content>\n",
                    escape_xml(content)
                ));
                output.push_str("      </CharacterStyleRange>\n");
            }
        }

        node::STRONG => {
            output.push_str(
                "      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/Bold\">\n",
            );
            for child in &node.children {
                if child.kind.as_str() == node::TEXT
                    && let Some(content) = child.props.get_str(prop::CONTENT)
                {
                    output.push_str(&format!(
                        "        <Content>{}</Content>\n",
                        escape_xml(content)
                    ));
                }
            }
            output.push_str("      </CharacterStyleRange>\n");
        }

        node::EMPHASIS => {
            output.push_str(
                "      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/Italic\">\n",
            );
            for child in &node.children {
                if child.kind.as_str() == node::TEXT
                    && let Some(content) = child.props.get_str(prop::CONTENT)
                {
                    output.push_str(&format!(
                        "        <Content>{}</Content>\n",
                        escape_xml(content)
                    ));
                }
            }
            output.push_str("      </CharacterStyleRange>\n");
        }

        node::CODE => {
            output.push_str(
                "      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/Code\">\n",
            );
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                output.push_str(&format!(
                    "        <Content>{}</Content>\n",
                    escape_xml(content)
                ));
            }
            output.push_str("      </CharacterStyleRange>\n");
        }

        node::LINK => {
            // ICML supports hyperlinks through HyperlinkTextDestination
            // For simplicity, just output the text with the URL after
            output.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");
            let mut link_text = String::new();
            collect_text(&node.children, &mut link_text);
            output.push_str(&format!(
                "        <Content>{}</Content>\n",
                escape_xml(&link_text)
            ));
            if let Some(url) = node.props.get_str(prop::URL) {
                output.push_str(&format!(
                    "        <Content> ({})</Content>\n",
                    escape_xml(url)
                ));
            }
            output.push_str("      </CharacterStyleRange>\n");
        }

        node::LINE_BREAK => {
            output.push_str("      <Br />\n");
        }

        node::SOFT_BREAK => {
            output.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");
            output.push_str("        <Content> </Content>\n");
            output.push_str("      </CharacterStyleRange>\n");
        }

        _ => {
            for child in &node.children {
                emit_inline_node(child, output);
            }
        }
    }
}

fn collect_text(nodes: &[Node], output: &mut String) {
    for node in nodes {
        if node.kind.as_str() == node::TEXT
            && let Some(content) = node.props.get_str(prop::CONTENT)
        {
            output.push_str(content);
        }
        collect_text(&node.children, output);
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
        assert!(output.contains("ParagraphStyle/Heading1"));
        assert!(output.contains("Title"));
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn test_emit_formatting() {
        let doc = doc(|d| d.para(|p| p.strong(|s| s.text("bold"))));
        let output = emit_str(&doc);
        assert!(output.contains("CharacterStyle/Bold"));
        assert!(output.contains("bold"));
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }
}
