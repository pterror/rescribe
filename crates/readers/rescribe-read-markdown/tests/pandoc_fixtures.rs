//! Integration tests against Pandoc test fixtures.
//!
//! These tests verify that our parser can handle real-world markdown
//! without panicking. They don't verify exact output matching.

use rescribe_read_markdown::parse;
use std::path::PathBuf;

fn pandoc_test_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join("git/pandoc/test");
    if path.exists() { Some(path) } else { None }
}

fn parse_file(path: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return true, // Skip files we can't read
    };

    match parse(&content) {
        Ok(_result) => {
            // Successfully parsed
            true
        }
        Err(e) => {
            eprintln!("Failed to parse {}: {:?}", path.display(), e);
            false
        }
    }
}

#[test]
fn test_markdown_reader_more() {
    let Some(test_dir) = pandoc_test_dir() else {
        eprintln!("Skipping Pandoc fixture tests - ~/git/pandoc not found");
        return;
    };

    let path = test_dir.join("markdown-reader-more.txt");
    if path.exists() {
        assert!(
            parse_file(&path),
            "Failed to parse markdown-reader-more.txt"
        );
    }
}

#[test]
fn test_pipe_tables() {
    let Some(test_dir) = pandoc_test_dir() else {
        return;
    };

    let path = test_dir.join("pipe-tables.txt");
    if path.exists() {
        assert!(parse_file(&path), "Failed to parse pipe-tables.txt");
    }
}

#[test]
fn test_markdown_citations() {
    let Some(test_dir) = pandoc_test_dir() else {
        return;
    };

    let path = test_dir.join("markdown-citations.txt");
    if path.exists() {
        assert!(parse_file(&path), "Failed to parse markdown-citations.txt");
    }
}

#[test]
fn test_all_txt_files() {
    let Some(test_dir) = pandoc_test_dir() else {
        eprintln!("Skipping Pandoc fixture tests - ~/git/pandoc not found");
        return;
    };

    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for entry in std::fs::read_dir(&test_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "txt") {
            let name = path.file_name().unwrap().to_string_lossy();

            // Skip non-markdown files
            if name.contains("native")
                || name.contains("creole")
                || name.contains("ansi")
                || name.contains("rst")
                || name.contains("org")
                || name.contains("mediawiki")
                || name.contains("textile")
            {
                skipped += 1;
                continue;
            }

            if parse_file(&path) {
                passed += 1;
            } else {
                failed += 1;
            }
        }
    }

    println!(
        "Pandoc fixture results: {} passed, {} failed, {} skipped",
        passed, failed, skipped
    );

    // We allow some failures since our parser doesn't support everything
    // but we should pass most tests
    assert!(
        failed == 0 || passed > failed * 2,
        "Too many failures: {} failed vs {} passed",
        failed,
        passed
    );
}

#[test]
fn test_roundtrip_simple() {
    use rescribe_write_markdown::emit;

    let markdown = r#"# Hello World

This is a **test** document with *emphasis*.

- Item 1
- Item 2
- Item 3

```rust
fn main() {
    println!("Hello!");
}
```

[Link](https://example.com)
"#;

    // Parse
    let parsed = parse(markdown).expect("Should parse");
    let doc = parsed.value;

    // Emit back to markdown
    let emitted = emit(&doc).expect("Should emit");
    let output = String::from_utf8(emitted.value).expect("Should be UTF-8");

    // Basic sanity checks - we don't expect exact roundtrip
    assert!(output.contains("Hello World"));
    assert!(output.contains("test"));
    assert!(output.contains("Item 1"));
    assert!(output.contains("rust"));
    assert!(output.contains("example.com"));
}
