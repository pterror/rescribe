//! Fountain screenplay format writer for rescribe.
//!
//! Generates Fountain screenplay markup from rescribe's document IR.
//!
//! # Fountain Elements
//!
//! - Scene headings (INT./EXT.)
//! - Action
//! - Character and dialogue
//! - Parentheticals
//! - Transitions
//! - Title page metadata

use rescribe_core::{ConversionResult, Document, EmitError, EmitOptions, FidelityWarning, Node};
use rescribe_std::{node, prop};

/// Emit a document as Fountain format.
pub fn emit(doc: &Document) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    emit_with_options(doc, &EmitOptions::default())
}

/// Emit a document as Fountain format with options.
pub fn emit_with_options(
    doc: &Document,
    _options: &EmitOptions,
) -> Result<ConversionResult<Vec<u8>>, EmitError> {
    let mut ctx = EmitContext::new();

    // Emit title page metadata
    emit_title_page(doc, &mut ctx);

    // Emit content
    emit_nodes(&doc.content.children, &mut ctx);

    Ok(ConversionResult::with_warnings(
        ctx.output.into_bytes(),
        ctx.warnings,
    ))
}

struct EmitContext {
    output: String,
    warnings: Vec<FidelityWarning>,
}

impl EmitContext {
    fn new() -> Self {
        Self {
            output: String::new(),
            warnings: Vec::new(),
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn ensure_blank_line(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            if self.output.ends_with('\n') {
                self.output.push('\n');
            } else {
                self.output.push_str("\n\n");
            }
        }
    }
}

fn emit_title_page(doc: &Document, ctx: &mut EmitContext) {
    let mut has_title_page = false;

    // Standard title page fields
    let fields = [
        "title",
        "credit",
        "author",
        "authors",
        "source",
        "draft_date",
        "contact",
        "copyright",
        "notes",
    ];

    for field in fields {
        let key = format!("fountain:{}", field);
        if let Some(value) = doc.metadata.get_str(&key) {
            let display_key = field
                .split('_')
                .map(|s| {
                    let mut c = s.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().chain(c).collect(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            ctx.write(&display_key);
            ctx.write(": ");
            ctx.writeln(value);
            has_title_page = true;
        }
    }

    if has_title_page {
        ctx.writeln("");
    }
}

fn emit_nodes(nodes: &[Node], ctx: &mut EmitContext) {
    for node in nodes {
        emit_node(node, ctx);
    }
}

fn emit_node(node: &Node, ctx: &mut EmitContext) {
    let fountain_type = node.props.get_str("fountain:type").unwrap_or("");

    match fountain_type {
        "scene_heading" => emit_scene_heading(node, ctx),
        "action" => emit_action(node, ctx),
        "dialogue_block" => emit_dialogue_block(node, ctx),
        "character" => emit_character(node, ctx),
        "dialogue" => emit_dialogue(node, ctx),
        "parenthetical" => emit_parenthetical(node, ctx),
        "transition" => emit_transition(node, ctx),
        "centered" => emit_centered(node, ctx),
        "lyric" => emit_lyric(node, ctx),
        "note" => emit_note(node, ctx),
        "synopsis" => emit_synopsis(node, ctx),
        "section" => emit_section(node, ctx),
        "page_break" => emit_page_break(ctx),
        _ => emit_generic(node, ctx),
    }
}

fn emit_scene_heading(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_action(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_dialogue_block(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();

    for child in &node.children {
        let child_type = child.props.get_str("fountain:type").unwrap_or("");
        match child_type {
            "character" => {
                let text = get_text_content(child);
                let dual = node.props.get_bool("fountain:dual").unwrap_or(false);
                if dual {
                    ctx.writeln(&format!("{} ^", text.to_uppercase()));
                } else {
                    ctx.writeln(&text.to_uppercase());
                }
            }
            "parenthetical" => {
                let text = get_text_content(child);
                ctx.writeln(&text);
            }
            "dialogue" => {
                let text = get_text_content(child);
                ctx.writeln(&text);
            }
            _ => emit_node(child, ctx),
        }
    }
}

fn emit_character(node: &Node, ctx: &mut EmitContext) {
    let text = get_text_content(node);
    ctx.writeln(&text.to_uppercase());
}

fn emit_dialogue(node: &Node, ctx: &mut EmitContext) {
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_parenthetical(node: &Node, ctx: &mut EmitContext) {
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_transition(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    let text = get_text_content(node);
    // If it doesn't look like a standard transition, force it with >
    if !text.to_uppercase().ends_with("TO:") {
        ctx.write(">");
    }
    ctx.writeln(&text.to_uppercase());
}

fn emit_centered(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    ctx.write(">");
    emit_text_content(node, ctx);
    ctx.writeln("<");
}

fn emit_lyric(node: &Node, ctx: &mut EmitContext) {
    ctx.write("~");
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_note(node: &Node, ctx: &mut EmitContext) {
    ctx.write("[[");
    emit_text_content(node, ctx);
    ctx.writeln("]]");
}

fn emit_synopsis(node: &Node, ctx: &mut EmitContext) {
    ctx.write("= ");
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_section(node: &Node, ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    let level = node.props.get_int(prop::LEVEL).unwrap_or(1) as usize;
    ctx.write(&"#".repeat(level));
    ctx.write(" ");
    emit_text_content(node, ctx);
    ctx.writeln("");
}

fn emit_page_break(ctx: &mut EmitContext) {
    ctx.ensure_blank_line();
    ctx.writeln("===");
}

fn emit_generic(node: &Node, ctx: &mut EmitContext) {
    match node.kind.as_str() {
        node::HEADING => {
            ctx.ensure_blank_line();
            let level = node.props.get_int(prop::LEVEL).unwrap_or(1);
            // Level 2 headings become scene headings
            if level == 2 {
                ctx.write(".");
            } else {
                ctx.write(&"#".repeat(level as usize));
                ctx.write(" ");
            }
            emit_text_content(node, ctx);
            ctx.writeln("");
        }
        node::PARAGRAPH => {
            ctx.ensure_blank_line();
            emit_text_content(node, ctx);
            ctx.writeln("");
        }
        node::HORIZONTAL_RULE => {
            emit_page_break(ctx);
        }
        node::DIV => {
            emit_nodes(&node.children, ctx);
        }
        node::TEXT => {
            if let Some(content) = node.props.get_str(prop::CONTENT) {
                ctx.write(content);
            }
        }
        _ => {
            emit_nodes(&node.children, ctx);
        }
    }
}

fn emit_text_content(node: &Node, ctx: &mut EmitContext) {
    for child in &node.children {
        match child.kind.as_str() {
            node::TEXT => {
                if let Some(content) = child.props.get_str(prop::CONTENT) {
                    ctx.write(content);
                }
            }
            node::STRONG => {
                ctx.write("**");
                emit_text_content(child, ctx);
                ctx.write("**");
            }
            node::EMPHASIS => {
                ctx.write("*");
                emit_text_content(child, ctx);
                ctx.write("*");
            }
            node::UNDERLINE => {
                ctx.write("_");
                emit_text_content(child, ctx);
                ctx.write("_");
            }
            _ => emit_text_content(child, ctx),
        }
    }
}

fn get_text_content(node: &Node) -> String {
    let mut result = String::new();
    collect_text(node, &mut result);
    result
}

fn collect_text(node: &Node, result: &mut String) {
    if let Some(content) = node.props.get_str(prop::CONTENT) {
        result.push_str(content);
    }
    for child in &node.children {
        collect_text(child, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rescribe_core::NodeKind;

    fn emit_str(doc: &Document) -> String {
        String::from_utf8(emit(doc).unwrap().value).unwrap()
    }

    #[test]
    fn test_emit_scene_heading() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("heading"))
                    .prop("fountain:type", "scene_heading")
                    .prop("level", 2i64)
                    .child(
                        Node::new(NodeKind::from("text")).prop("content", "INT. COFFEE SHOP - DAY"),
                    ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("INT. COFFEE SHOP - DAY"));
    }

    #[test]
    fn test_emit_dialogue() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("div"))
                    .prop("fountain:type", "dialogue_block")
                    .child(
                        Node::new(NodeKind::from("paragraph"))
                            .prop("fountain:type", "character")
                            .child(Node::new(NodeKind::from("text")).prop("content", "John")),
                    )
                    .child(
                        Node::new(NodeKind::from("paragraph"))
                            .prop("fountain:type", "dialogue")
                            .child(
                                Node::new(NodeKind::from("text"))
                                    .prop("content", "Hello, how are you?"),
                            ),
                    ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("JOHN"));
        assert!(output.contains("Hello, how are you?"));
    }

    #[test]
    fn test_emit_transition() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("paragraph"))
                    .prop("fountain:type", "transition")
                    .child(Node::new(NodeKind::from("text")).prop("content", "CUT TO:")),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("CUT TO:"));
    }

    #[test]
    fn test_emit_action() {
        let doc = Document::new().with_content(
            Node::new(NodeKind::from("document")).child(
                Node::new(NodeKind::from("paragraph"))
                    .prop("fountain:type", "action")
                    .child(
                        Node::new(NodeKind::from("text")).prop("content", "The door slowly opens."),
                    ),
            ),
        );

        let output = emit_str(&doc);
        assert!(output.contains("The door slowly opens."));
    }

    #[test]
    fn test_emit_page_break() {
        let doc = Document::new().with_content(Node::new(NodeKind::from("document")).child(
            Node::new(NodeKind::from("horizontal_rule")).prop("fountain:type", "page_break"),
        ));

        let output = emit_str(&doc);
        assert!(output.contains("==="));
    }
}
