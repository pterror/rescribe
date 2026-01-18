#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Parse the input
        if let Ok(result1) = rescribe_read_markdown::parse(s) {
            let doc1 = result1.value;

            // Emit back to markdown
            if let Ok(emitted) = rescribe_write_markdown::emit(&doc1) {
                let md = String::from_utf8_lossy(&emitted.value);

                // Parse again - should succeed if emit succeeded
                if let Ok(result2) = rescribe_read_markdown::parse(&md) {
                    let doc2 = result2.value;

                    // Text content should be preserved
                    let text1 = extract_text(&doc1.content);
                    let text2 = extract_text(&doc2.content);

                    // Normalize whitespace for comparison
                    let norm1: String = text1.split_whitespace().collect::<Vec<_>>().join(" ");
                    let norm2: String = text2.split_whitespace().collect::<Vec<_>>().join(" ");

                    assert_eq!(norm1, norm2, "Text content changed through roundtrip");
                }
            }
        }
    }
});

fn extract_text(node: &rescribe_std::Node) -> String {
    use rescribe_std::{node, prop};

    let mut text = String::new();

    if node.kind.as_str() == node::TEXT {
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            text.push_str(content);
        }
    }

    for child in &node.children {
        text.push_str(&extract_text(child));
    }

    text
}
