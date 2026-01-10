//! Integration tests against Pandoc's HTML test fixtures.
//!
//! These tests read Pandoc's test HTML files and verify our parser handles them correctly.
//! The fixture files are not copied into the repo (GPL-2.0 licensed) - they're referenced
//! from the local Pandoc repository.

use rescribe_read_html::parse;
use rescribe_std::{node, prop};
use std::fs;
use std::path::Path;

const PANDOC_TEST_DIR: &str = "/home/me/git/pandoc/test";

fn pandoc_file(name: &str) -> Option<String> {
    let path = Path::new(PANDOC_TEST_DIR).join(name);
    if path.exists() {
        fs::read_to_string(&path).ok()
    } else {
        None
    }
}

/// Count nodes of a specific kind in a document tree.
fn count_nodes_of_kind(nodes: &[rescribe_std::Node], kind: &str) -> usize {
    let mut count = 0;
    for node in nodes {
        if node.kind.as_str() == kind {
            count += 1;
        }
        count += count_nodes_of_kind(&node.children, kind);
    }
    count
}

/// Extract all text content from nodes.
fn extract_all_text(nodes: &[rescribe_std::Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        if let Some(content) = node.props.get_str(prop::CONTENT) {
            text.push_str(content);
        }
        text.push_str(&extract_all_text(&node.children));
    }
    text
}

#[test]
fn test_parse_html_reader_fixture() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let doc = result.value;
    let children = &doc.content.children;

    // Check that we parsed various elements
    let heading_count = count_nodes_of_kind(children, node::HEADING);
    assert!(
        heading_count > 10,
        "Expected many headings, got {}",
        heading_count
    );

    let paragraph_count = count_nodes_of_kind(children, node::PARAGRAPH);
    assert!(
        paragraph_count > 50,
        "Expected many paragraphs, got {}",
        paragraph_count
    );

    let list_count = count_nodes_of_kind(children, node::LIST);
    assert!(list_count > 10, "Expected many lists, got {}", list_count);

    let table_count = count_nodes_of_kind(children, node::TABLE);
    assert!(table_count > 5, "Expected many tables, got {}", table_count);

    let code_block_count = count_nodes_of_kind(children, node::CODE_BLOCK);
    assert!(
        code_block_count > 3,
        "Expected code blocks, got {}",
        code_block_count
    );

    let blockquote_count = count_nodes_of_kind(children, node::BLOCKQUOTE);
    assert!(
        blockquote_count > 5,
        "Expected blockquotes, got {}",
        blockquote_count
    );

    // Check that key content is present
    let all_text = extract_all_text(children);
    assert!(all_text.contains("Pandoc Test Suite"), "Missing title");
    assert!(all_text.contains("Headers"), "Missing headers section");
    assert!(
        all_text.contains("Block Quotes"),
        "Missing block quotes section"
    );
    assert!(
        all_text.contains("Code Blocks"),
        "Missing code blocks section"
    );
    assert!(all_text.contains("Lists"), "Missing lists section");
    assert!(
        all_text.contains("Inline Markup"),
        "Missing inline markup section"
    );
    assert!(all_text.contains("Tables"), "Missing tables section");
}

#[test]
fn test_parse_headings_levels() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let children = &result.value.content.children;

    // Collect all heading levels
    let mut levels_found = [false; 6];
    fn check_levels(nodes: &[rescribe_std::Node], levels: &mut [bool; 6]) {
        for node in nodes {
            if node.kind.as_str() == node::HEADING
                && let Some(level) = node.props.get_int(prop::LEVEL)
                && (1..=6).contains(&level)
            {
                levels[level as usize - 1] = true;
            }
            check_levels(&node.children, levels);
        }
    }
    check_levels(children, &mut levels_found);

    // The fixture has h1 through h5
    assert!(levels_found[0], "Missing h1");
    assert!(levels_found[1], "Missing h2");
    assert!(levels_found[2], "Missing h3");
    assert!(levels_found[3], "Missing h4");
    assert!(levels_found[4], "Missing h5");
}

#[test]
fn test_parse_links() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let children = &result.value.content.children;

    // Find links
    fn find_links(nodes: &[rescribe_std::Node]) -> Vec<(Option<String>, Option<String>)> {
        let mut links = Vec::new();
        for node in nodes {
            if node.kind.as_str() == node::LINK {
                links.push((
                    node.props.get_str(prop::URL).map(|s| s.to_string()),
                    node.props.get_str(prop::TITLE).map(|s| s.to_string()),
                ));
            }
            links.extend(find_links(&node.children));
        }
        links
    }

    let links = find_links(children);
    assert!(!links.is_empty(), "No links found");

    // Check for specific links from the fixture
    let has_url_link = links.iter().any(|(url, _)| url.as_deref() == Some("/url"));
    assert!(has_url_link, "Missing /url link");

    let has_example_link = links.iter().any(|(url, _)| {
        url.as_ref()
            .map(|u| u.contains("example.com"))
            .unwrap_or(false)
    });
    assert!(has_example_link, "Missing example.com link");
}

#[test]
fn test_parse_images() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let children = &result.value.content.children;

    // Find images
    fn find_images(nodes: &[rescribe_std::Node]) -> Vec<(Option<String>, Option<String>)> {
        let mut images = Vec::new();
        for node in nodes {
            if node.kind.as_str() == node::IMAGE {
                images.push((
                    node.props.get_str(prop::URL).map(|s| s.to_string()),
                    node.props.get_str(prop::ALT).map(|s| s.to_string()),
                ));
            }
            images.extend(find_images(&node.children));
        }
        images
    }

    let images = find_images(children);
    assert!(!images.is_empty(), "No images found");

    // Check for specific images from the fixture
    let has_lalune = images
        .iter()
        .any(|(url, _)| url.as_deref() == Some("lalune.jpg"));
    assert!(has_lalune, "Missing lalune.jpg image");

    let has_movie = images
        .iter()
        .any(|(url, _)| url.as_deref() == Some("movie.jpg"));
    assert!(has_movie, "Missing movie.jpg image");
}

#[test]
fn test_parse_code_blocks() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let children = &result.value.content.children;

    // Find code blocks
    fn find_code_blocks(nodes: &[rescribe_std::Node]) -> Vec<String> {
        let mut blocks = Vec::new();
        for node in nodes {
            if node.kind.as_str() == node::CODE_BLOCK
                && let Some(content) = node.props.get_str(prop::CONTENT)
            {
                blocks.push(content.to_string());
            }
            blocks.extend(find_code_blocks(&node.children));
        }
        blocks
    }

    let blocks = find_code_blocks(children);
    assert!(!blocks.is_empty(), "No code blocks found");

    // Check for code block content
    let has_sub_status = blocks.iter().any(|b| b.contains("sub status"));
    assert!(has_sub_status, "Missing 'sub status' code block");

    let has_print_working = blocks.iter().any(|b| b.contains("print \"working\""));
    assert!(has_print_working, "Missing 'print working' code block");
}

#[test]
fn test_parse_tables_with_colspan() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let children = &result.value.content.children;

    // Find cells with colspan
    fn find_cells_with_colspan(nodes: &[rescribe_std::Node]) -> Vec<i64> {
        let mut colspans = Vec::new();
        for node in nodes {
            if (node.kind.as_str() == node::TABLE_CELL || node.kind.as_str() == node::TABLE_HEADER)
                && let Some(colspan) = node.props.get_int(prop::COLSPAN)
                && colspan > 1
            {
                colspans.push(colspan);
            }
            colspans.extend(find_cells_with_colspan(&node.children));
        }
        colspans
    }

    let colspans = find_cells_with_colspan(children);
    assert!(!colspans.is_empty(), "No colspan cells found");
    assert!(colspans.contains(&2), "Missing colspan=2");
    assert!(colspans.contains(&3), "Missing colspan=3");
}

#[test]
fn test_parse_definition_list() {
    let Some(html) = pandoc_file("html-reader.html") else {
        eprintln!("Skipping test: Pandoc test fixtures not available");
        return;
    };

    let result = parse(&html).expect("Failed to parse HTML");
    let children = &result.value.content.children;

    let dl_count = count_nodes_of_kind(children, node::DEFINITION_LIST);
    assert!(dl_count > 0, "Expected definition lists, got {}", dl_count);

    let dt_count = count_nodes_of_kind(children, node::DEFINITION_TERM);
    assert!(dt_count > 0, "Expected definition terms, got {}", dt_count);

    let dd_count = count_nodes_of_kind(children, node::DEFINITION_DESC);
    assert!(
        dd_count > 0,
        "Expected definition descriptions, got {}",
        dd_count
    );
}

#[test]
fn test_roundtrip_simple_html() {
    // Parse some HTML, emit it, verify key content preserved
    let html = r#"<p>Hello <strong>world</strong>!</p><h2>Title</h2><ul><li>Item 1</li><li>Item 2</li></ul>"#;

    let result = parse(html).expect("Failed to parse HTML");
    let doc = result.value;

    // Emit back to HTML
    let emitted = rescribe_write_html::emit(&doc).expect("Failed to emit HTML");
    let output = String::from_utf8(emitted.value).expect("Invalid UTF-8");

    // Verify content is preserved
    assert!(output.contains("Hello"), "Missing 'Hello'");
    assert!(output.contains("world"), "Missing 'world'");
    assert!(output.contains("Title"), "Missing 'Title'");
    assert!(output.contains("Item 1"), "Missing 'Item 1'");
    assert!(output.contains("Item 2"), "Missing 'Item 2'");

    // Verify structure is preserved
    assert!(output.contains("<p>"), "Missing <p> tag");
    assert!(output.contains("<strong>"), "Missing <strong> tag");
    assert!(output.contains("<h2>"), "Missing <h2> tag");
    assert!(output.contains("<ul>"), "Missing <ul> tag");
    assert!(output.contains("<li>"), "Missing <li> tag");
}
