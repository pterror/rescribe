//! Edge case tests for rescribe.
//!
//! Tests Unicode handling, structural extremes, and other edge cases.

use rescribe_read_html as html;
use rescribe_read_markdown as markdown;
use rescribe_std::{node, prop};
use rescribe_write_html as html_write;
use rescribe_write_markdown as md_write;

/// Helper to extract all text content from a document.
fn extract_text(node: &rescribe_std::Node) -> String {
    let mut text = String::new();

    // Text nodes and code blocks store content in the CONTENT property
    if matches!(
        node.kind.as_str(),
        node::TEXT | node::CODE_BLOCK | node::CODE
    ) && let Some(content) = node.props.get_str(prop::CONTENT)
    {
        text.push_str(content);
    }

    for child in &node.children {
        text.push_str(&extract_text(child));
    }
    text
}

mod unicode {
    use super::*;

    #[test]
    fn test_emoji_basic() {
        let input = "Hello 👋 World";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("👋"));
    }

    #[test]
    fn test_emoji_skin_tone() {
        let input = "Wave 👋🏽 back";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("👋🏽"));
    }

    #[test]
    fn test_emoji_zwj_sequence() {
        // Family emoji (composed with zero-width joiners)
        let input = "Family: 👨‍👩‍👧‍👦";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("👨‍👩‍👧‍👦"));
    }

    #[test]
    fn test_emoji_flag() {
        let input = "Flag: 🇺🇸";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("🇺🇸"));
    }

    #[test]
    fn test_cjk_chinese() {
        let input = "中文测试 Chinese test";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("中文测试"));
    }

    #[test]
    fn test_cjk_japanese() {
        let input = "日本語テスト Japanese test";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("日本語テスト"));
    }

    #[test]
    fn test_cjk_korean() {
        let input = "한국어 테스트 Korean test";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("한국어 테스트"));
    }

    #[test]
    fn test_rtl_arabic() {
        let input = "مرحبا Hello";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("مرحبا"));
    }

    #[test]
    fn test_rtl_hebrew() {
        let input = "שלום Hello";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("שלום"));
    }

    #[test]
    fn test_combining_characters() {
        // e with combining acute accent (vs precomposed é)
        let input = "cafe\u{0301}"; // café with combining accent
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("cafe\u{0301}") || text.contains("café"));
    }

    #[test]
    fn test_zero_width_spaces() {
        let input = "zero\u{200B}width\u{200B}space";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        // Zero-width spaces should be preserved or normalized
        assert!(text.contains("zero") && text.contains("width") && text.contains("space"));
    }

    #[test]
    fn test_non_breaking_space() {
        let input = "non\u{00A0}breaking";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("non") && text.contains("breaking"));
    }

    #[test]
    fn test_math_symbols() {
        let input = "Sum: ∑, Integral: ∫, Infinity: ∞, Pi: π";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("∑"));
        assert!(text.contains("∫"));
        assert!(text.contains("∞"));
        assert!(text.contains("π"));
    }

    #[test]
    fn test_greek_letters() {
        let input = "α β γ δ ε Α Β Γ Δ Ε";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("α"));
        assert!(text.contains("Ε"));
    }

    #[test]
    fn test_em_en_dashes() {
        let input = "en–dash and em—dash";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("–")); // en dash
        assert!(text.contains("—")); // em dash
    }

    #[test]
    fn test_smart_quotes() {
        // Use Unicode escapes for smart quotes to avoid confusing Rust's parser
        let input = "\u{201C}smart\u{201D} \u{2018}quotes\u{2019}";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("\u{201C}") || text.contains("\""));
    }

    #[test]
    fn test_unicode_roundtrip_markdown() {
        let input = "# 中文标题\n\n日本語の段落。\n\nΕλληνικά κείμενο.";
        let doc = markdown::parse(input).unwrap().value;
        let output = md_write::emit(&doc).unwrap().value;
        let output_str = String::from_utf8(output).unwrap();

        // Re-parse
        let doc2 = markdown::parse(&output_str).unwrap().value;
        let text2 = extract_text(&doc2.content);

        assert!(text2.contains("中文标题"));
        assert!(text2.contains("日本語の段落"));
        assert!(text2.contains("Ελληνικά"));
    }

    #[test]
    fn test_unicode_roundtrip_html() {
        let input = "<h1>中文标题</h1><p>日本語の段落。</p>";
        let doc = html::parse(input).unwrap().value;
        let output = html_write::emit(&doc).unwrap().value;
        let output_str = String::from_utf8(output).unwrap();

        // Re-parse
        let doc2 = html::parse(&output_str).unwrap().value;
        let text2 = extract_text(&doc2.content);

        assert!(text2.contains("中文标题"));
        assert!(text2.contains("日本語の段落"));
    }
}

mod escaping {
    use super::*;

    #[test]
    fn test_markdown_special_chars_in_text() {
        // These should be treated as literal text, not formatting
        let input = "Price: $100, 50% off, C# language";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("$100"));
        assert!(text.contains("50%"));
        assert!(text.contains("C#"));
    }

    #[test]
    fn test_html_entities_decoded() {
        let input = "<p>&lt;tag&gt; &amp; &quot;quoted&quot;</p>";
        let doc = html::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("<tag>"));
        assert!(text.contains("&"));
        assert!(text.contains("\"quoted\""));
    }

    #[test]
    fn test_html_special_chars_escaped_on_emit() {
        let doc =
            rescribe_std::builder::doc(|d| d.para(|i| i.text("<script>alert('xss')</script>")));

        let output = html_write::emit(&doc).unwrap().value;
        let output_str = String::from_utf8(output).unwrap();

        // Should be escaped, not raw
        assert!(!output_str.contains("<script>"));
        assert!(output_str.contains("&lt;script&gt;") || output_str.contains("&lt;"));
    }

    #[test]
    fn test_backslash_escapes_markdown() {
        let input = r"\*not emphasis\* \[not link\]";
        let doc = markdown::parse(input).unwrap().value;

        // Should not create emphasis or link nodes
        let text = extract_text(&doc.content);
        assert!(text.contains("*not emphasis*") || text.contains("not emphasis"));
    }

    #[test]
    fn test_url_special_chars() {
        let input = "[link](https://example.com/path?a=1&b=2#anchor)";
        let doc = markdown::parse(input).unwrap().value;

        // Find the link node
        fn find_link(node: &rescribe_std::Node) -> Option<String> {
            if node.kind.as_str() == node::LINK {
                return node.props.get_str(prop::URL).map(|s| s.to_string());
            }
            for child in &node.children {
                if let Some(url) = find_link(child) {
                    return Some(url);
                }
            }
            None
        }

        let url = find_link(&doc.content).expect("Should have link");
        assert!(url.contains("?a=1&b=2#anchor"));
    }
}

mod structure {
    use super::*;

    #[test]
    fn test_deeply_nested_lists() {
        let mut input = String::new();
        for i in 0..10 {
            input.push_str(&"  ".repeat(i));
            input.push_str("- item\n");
        }

        let doc = markdown::parse(&input).unwrap().value;
        // Should parse without panic
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_nested_blockquotes() {
        let input = "> level 1\n>> level 2\n>>> level 3\n>>>> level 4";
        let doc = markdown::parse(input).unwrap().value;
        assert!(!doc.content.children.is_empty());
    }

    #[test]
    fn test_list_in_blockquote() {
        let input = "> - item 1\n> - item 2\n>   - nested";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("item 1"));
        assert!(text.contains("nested"));
    }

    #[test]
    fn test_code_in_list() {
        let input = "- item with `code`\n- item with\n  ```\n  code block\n  ```";
        let doc = markdown::parse(input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("code"));
    }

    #[test]
    fn test_empty_document() {
        let doc = markdown::parse("").unwrap().value;
        assert!(doc.content.children.is_empty() || extract_text(&doc.content).trim().is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let doc = markdown::parse("   \n\n   \t\n").unwrap().value;
        assert!(extract_text(&doc.content).trim().is_empty());
    }

    #[test]
    fn test_single_character() {
        let doc = markdown::parse("a").unwrap().value;
        assert_eq!(extract_text(&doc.content).trim(), "a");
    }

    #[test]
    fn test_very_long_line() {
        let long_line = "a".repeat(10000);
        let doc = markdown::parse(&long_line).unwrap().value;
        let text = extract_text(&doc.content);
        assert_eq!(text.trim().len(), 10000);
    }

    #[test]
    fn test_many_paragraphs() {
        let input = (0..100)
            .map(|i| format!("Paragraph {}\n", i))
            .collect::<String>();
        let doc = markdown::parse(&input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("Paragraph 0"));
        assert!(text.contains("Paragraph 99"));
    }

    #[test]
    fn test_consecutive_headings() {
        let input = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        let doc = markdown::parse(input).unwrap().value;

        let heading_count = doc
            .content
            .children
            .iter()
            .filter(|n| n.kind.as_str() == node::HEADING)
            .count();

        assert_eq!(heading_count, 6);
    }

    #[test]
    fn test_html_unclosed_tags() {
        // html5ever should handle this gracefully
        let input = "<p>unclosed paragraph<p>another<div>nested";
        let doc = html::parse(input).unwrap().value;
        // Should not panic, and should extract some text
        let text = extract_text(&doc.content);
        assert!(text.contains("unclosed"));
    }

    #[test]
    fn test_html_deeply_nested() {
        let mut input = String::new();
        for _ in 0..50 {
            input.push_str("<div>");
        }
        input.push_str("content");
        for _ in 0..50 {
            input.push_str("</div>");
        }

        let doc = html::parse(&input).unwrap().value;
        let text = extract_text(&doc.content);
        assert!(text.contains("content"));
    }
}

mod malformed {
    use super::*;

    #[test]
    fn test_unmatched_emphasis() {
        // Should not panic, treat as literal
        let input = "**unmatched bold";
        let result = markdown::parse(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_broken_link() {
        let input = "[broken link(no closing bracket";
        let result = markdown::parse(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_broken_image() {
        let input = "![broken image";
        let result = markdown::parse(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unclosed_code_fence() {
        let input = "```\ncode without closing fence";
        let result = markdown::parse(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_html_script_tag() {
        let input = "<script>alert('xss')</script><p>text</p>";
        let doc = html::parse(input).unwrap().value;
        // Script content might be ignored or preserved, but shouldn't crash
        let text = extract_text(&doc.content);
        assert!(text.contains("text"));
    }

    #[test]
    fn test_html_invalid_nesting() {
        let input = "<p><div>invalid nesting</div></p>";
        let result = html::parse(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_null_bytes() {
        // Embedded null bytes
        let input = "hello\0world";
        let result = markdown::parse(input);
        // May fail or succeed, but shouldn't panic
        let _ = result;
    }
}
