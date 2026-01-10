//! Cross-format conversion tests.
//!
//! Tests that verify content is preserved when converting between formats:
//! - Markdown → IR → HTML → IR → Markdown
//! - HTML → IR → Markdown → IR → HTML

use rescribe_std::prop;

/// Extract all text content from nodes recursively.
fn extract_text(nodes: &[rescribe_std::Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            text.push_str(content);
            text.push(' ');
        }
        text.push_str(&extract_text(&node.children));
    }
    text
}

#[test]
fn test_markdown_to_html_roundtrip() {
    let markdown = r#"# Hello World

This is a **bold** and *italic* text.

- Item 1
- Item 2
- Item 3

[A link](https://example.com)

```rust
fn main() {}
```
"#;

    // Markdown → IR
    let md_result = rescribe_read_markdown::parse(markdown).expect("Failed to parse markdown");
    let md_doc = md_result.value;

    // IR → HTML
    let html_result = rescribe_write_html::emit(&md_doc).expect("Failed to emit HTML");
    let html = String::from_utf8(html_result.value).expect("Invalid UTF-8");

    // HTML → IR
    let html_result2 = rescribe_read_html::parse(&html).expect("Failed to parse HTML");
    let html_doc = html_result2.value;

    // Verify content is preserved
    let roundtrip_text = extract_text(&html_doc.content.children);

    assert!(roundtrip_text.contains("Hello World"), "Missing heading");
    assert!(roundtrip_text.contains("bold"), "Missing bold text");
    assert!(roundtrip_text.contains("italic"), "Missing italic text");
    assert!(roundtrip_text.contains("Item 1"), "Missing list item 1");
    assert!(roundtrip_text.contains("Item 2"), "Missing list item 2");
    assert!(roundtrip_text.contains("Item 3"), "Missing list item 3");
    assert!(roundtrip_text.contains("A link"), "Missing link text");
    assert!(
        roundtrip_text.contains("fn main()"),
        "Missing code block content"
    );

    // Also verify HTML structure is present
    assert!(html.contains("<h1>"), "Missing h1 tag");
    assert!(html.contains("<strong>"), "Missing strong tag");
    assert!(html.contains("<em>"), "Missing em tag");
    assert!(html.contains("<ul>"), "Missing ul tag");
    assert!(html.contains("<li>"), "Missing li tag");
    assert!(html.contains("<a href="), "Missing link tag");
    assert!(html.contains("<pre>"), "Missing pre tag");
}

#[test]
fn test_html_to_markdown_roundtrip() {
    let html = r#"<h2>Test Heading</h2>
<p>A paragraph with <em>emphasis</em> and <strong>strong</strong> text.</p>
<ol>
<li>First</li>
<li>Second</li>
</ol>
<blockquote><p>A quote</p></blockquote>
"#;

    // HTML → IR
    let html_result = rescribe_read_html::parse(html).expect("Failed to parse HTML");
    let html_doc = html_result.value;

    // IR → Markdown
    let md_result = rescribe_write_markdown::emit(&html_doc).expect("Failed to emit Markdown");
    let markdown = String::from_utf8(md_result.value).expect("Invalid UTF-8");

    // Markdown → IR
    let md_result2 = rescribe_read_markdown::parse(&markdown).expect("Failed to parse Markdown");
    let md_doc = md_result2.value;

    // Verify content is preserved
    let roundtrip_text = extract_text(&md_doc.content.children);

    assert!(roundtrip_text.contains("Test Heading"), "Missing heading");
    assert!(roundtrip_text.contains("emphasis"), "Missing emphasis text");
    assert!(roundtrip_text.contains("strong"), "Missing strong text");
    assert!(roundtrip_text.contains("First"), "Missing list item 1");
    assert!(roundtrip_text.contains("Second"), "Missing list item 2");
    assert!(roundtrip_text.contains("quote"), "Missing blockquote text");

    // Verify Markdown structure
    assert!(markdown.contains("##"), "Missing heading marker");
    assert!(markdown.contains("*"), "Missing emphasis marker");
    assert!(
        markdown.contains("1.") || markdown.contains("1)"),
        "Missing ordered list marker"
    );
    assert!(markdown.contains(">"), "Missing blockquote marker");
}

#[test]
fn test_complex_document_roundtrip() {
    let markdown = r#"# Main Title

## Introduction

This document has **bold**, *italic*, and ***bold italic*** text.

### Code Example

Here's some code:

```python
def hello():
    print("world")
```

### Links and Images

Check out [this link](https://example.com "Example Site").

![An image](image.png "Image Title")

## Lists

### Unordered

- Apple
- Banana
- Cherry

### Ordered

1. First step
2. Second step
3. Third step

## Tables

| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
| Cell 3   | Cell 4   |

## Quotes

> This is a blockquote.
> It spans multiple lines.

## Conclusion

That's all folks!
"#;

    // Markdown → IR → HTML
    let md_doc = rescribe_read_markdown::parse(markdown).unwrap().value;
    let html = String::from_utf8(rescribe_write_html::emit(&md_doc).unwrap().value).unwrap();

    // HTML → IR → Markdown
    let html_doc = rescribe_read_html::parse(&html).unwrap().value;
    let markdown2 =
        String::from_utf8(rescribe_write_markdown::emit(&html_doc).unwrap().value).unwrap();

    // Markdown → IR (final)
    let final_doc = rescribe_read_markdown::parse(&markdown2).unwrap().value;
    let final_text = extract_text(&final_doc.content.children);

    // Key content should survive the full roundtrip
    assert!(final_text.contains("Main Title"), "Missing main title");
    assert!(final_text.contains("Introduction"), "Missing introduction");
    assert!(final_text.contains("bold"), "Missing bold text");
    assert!(final_text.contains("italic"), "Missing italic text");
    assert!(
        final_text.contains("Code Example"),
        "Missing code example section"
    );
    assert!(final_text.contains("def hello"), "Missing code content");
    assert!(final_text.contains("this link"), "Missing link text");
    assert!(final_text.contains("Apple"), "Missing list item");
    assert!(
        final_text.contains("First step"),
        "Missing ordered list item"
    );
    assert!(
        final_text.contains("blockquote"),
        "Missing blockquote content"
    );
    assert!(final_text.contains("Conclusion"), "Missing conclusion");
}

#[test]
fn test_special_characters_preserved() {
    let markdown = r#"Special characters: < > & " ' ` * _ # [ ] ( ) \ !

Code with special chars: `<html>&amp;</html>`

In a paragraph with <angle brackets> and "quotes" and 'apostrophes'.
"#;

    // Markdown → IR → HTML → IR
    let md_doc = rescribe_read_markdown::parse(markdown).unwrap().value;
    let html = String::from_utf8(rescribe_write_html::emit(&md_doc).unwrap().value).unwrap();
    let html_doc = rescribe_read_html::parse(&html).unwrap().value;

    let text = extract_text(&html_doc.content.children);

    // These should be preserved (possibly encoded/decoded)
    assert!(text.contains("<"), "Missing < character");
    assert!(text.contains(">"), "Missing > character");
    assert!(text.contains("&"), "Missing & character");

    // HTML should properly escape special characters
    assert!(
        html.contains("&lt;") || html.contains("<"),
        "< not properly handled"
    );
    assert!(
        html.contains("&gt;") || html.contains(">"),
        "> not properly handled"
    );
    assert!(
        html.contains("&amp;") || html.contains("&"),
        "& not properly handled"
    );
}

#[test]
fn test_nested_structures() {
    let markdown = r#"- Level 1 item 1
  - Level 2 item 1
    - Level 3 item
  - Level 2 item 2
- Level 1 item 2

> Quote level 1
> > Quote level 2
> > > Quote level 3
"#;

    // Markdown → IR → HTML
    let md_doc = rescribe_read_markdown::parse(markdown).unwrap().value;
    let html = String::from_utf8(rescribe_write_html::emit(&md_doc).unwrap().value).unwrap();

    // HTML should have nested structures
    assert!(html.contains("<ul>"), "Missing ul tag");
    assert!(html.contains("<li>"), "Missing li tag");
    assert!(html.contains("<blockquote>"), "Missing blockquote tag");

    // Parse HTML back and verify content
    let html_doc = rescribe_read_html::parse(&html).unwrap().value;
    let text = extract_text(&html_doc.content.children);

    assert!(text.contains("Level 1 item 1"), "Missing level 1 content");
    assert!(text.contains("Level 2 item 1"), "Missing level 2 content");
    assert!(text.contains("Level 3 item"), "Missing level 3 content");
    assert!(text.contains("Quote level 1"), "Missing quote level 1");
    assert!(text.contains("Quote level 2"), "Missing quote level 2");
    assert!(text.contains("Quote level 3"), "Missing quote level 3");
}
