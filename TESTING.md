# Rescribe Testing Strategy

Comprehensive testing plan for the rescribe document conversion library.

## Current State

Basic unit tests exist for:
- Individual readers (markdown, HTML, LaTeX, org-mode, PDF, Pandoc JSON)
- Individual writers (markdown, HTML, LaTeX, org-mode, plaintext, Pandoc JSON)
- Transforms (ShiftHeadings, StripEmpty, MergeText, Pipeline)
- Cross-format roundtrips (markdown ↔ HTML)
- Source info preservation (style hints in markdown)

## Testing Categories

### 1. Roundtrip Tests

**Goal**: Verify that parse → emit → parse produces equivalent documents.

**Approach**:
- For each format with both reader and writer
- Parse input, emit to same format, parse again
- Compare AST structure (not byte-for-byte)
- With `preserve_source_info` + `use_source_info`: verify style hints preserved

**Priority formats**:
- Markdown (most style variations: ATX/setext headings, */_ emphasis, fence chars)
- HTML (structure should be identical)
- Org-mode

**Test cases needed**:
- [ ] All heading levels (1-6)
- [ ] All list types (ordered, unordered, nested, tight/loose)
- [ ] All inline formatting (emphasis, strong, code, links, images)
- [ ] Code blocks (fenced with ` vs ~, indented, with/without language)
- [ ] Tables (simple, with alignment, complex)
- [ ] Blockquotes (simple, nested, with other content)
- [ ] Mixed content (all elements in one document)

### 2. Cross-Format Conversion Matrix

**Goal**: Verify semantic preservation across format boundaries.

**Matrix** (R=reader, W=writer):

| From↓ To→ | MD | HTML | LaTeX | Org | Plain | Pandoc JSON |
|-----------|----|----- |-------|-----|-------|-------------|
| Markdown  | RW | W    | W     | W   | W     | W           |
| HTML      | W  | RW   | W     | W   | W     | W           |
| LaTeX     | W  | W    | RW    | W   | W     | W           |
| Org       | W  | W    | W     | RW  | W     | W           |
| PDF       | W  | W    | W     | W   | W     | W           |
| Pandoc JSON | W | W   | W     | W   | W     | RW          |

**Test approach**:
- Convert representative document through each path
- Verify text content preserved
- Track expected fidelity warnings per path
- Compare structure where formats support it

### 3. Edge Cases

#### 3.1 Unicode & Internationalization
- [ ] Emoji: 👋🏽 (skin tone modifiers), 👨‍👩‍👧‍👦 (ZWJ sequences)
- [ ] RTL text: Arabic, Hebrew mixed with LTR
- [ ] CJK characters: Chinese, Japanese, Korean
- [ ] Combining characters: é vs e + ́
- [ ] Special Unicode: zero-width spaces, non-breaking spaces, em/en dashes
- [ ] Math symbols: ∑, ∫, ∞, Greek letters

#### 3.2 Structural Edge Cases
- [ ] Deeply nested lists (10+ levels)
- [ ] Deeply nested blockquotes
- [ ] Lists inside blockquotes inside lists
- [ ] Empty nodes at various levels
- [ ] Single-child wrappers (div > div > p)
- [ ] Adjacent same-type nodes (consecutive headings)

#### 3.3 Size Extremes
- [ ] Empty document
- [ ] Single character
- [ ] Whitespace only
- [ ] Very long lines (10k+ chars)
- [ ] Very large documents (1MB+, 10k+ nodes)
- [ ] Very deep nesting (100+ levels)

#### 3.4 Escaping & Special Characters
- [ ] Markdown: `*_[]()#<>&\`` in text content
- [ ] HTML: `<>&"'` in attributes and content
- [ ] LaTeX: `\{}$%&#_^~` in text
- [ ] URLs with special chars: spaces, unicode, query strings

#### 3.5 Malformed Input
- [ ] Unclosed HTML tags
- [ ] Unmatched markdown emphasis
- [ ] Invalid UTF-8 sequences
- [ ] Binary data in text formats
- [ ] Truncated documents

### 4. Fuzz Testing

**Goal**: Find crashes, panics, and unexpected behavior with random inputs.

**Tools**:
- `cargo-fuzz` with libFuzzer
- `arbtest` for property-based testing
- `proptest` for generating valid documents

**Fuzz targets**:
```rust
// fuzz/fuzz_targets/markdown_reader.rs
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = rescribe_read_markdown::parse(s);
    }
});

// fuzz/fuzz_targets/html_reader.rs
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = rescribe_read_html::parse(s);
    }
});

// fuzz/fuzz_targets/pdf_reader.rs
fuzz_target!(|data: &[u8]| {
    let _ = rescribe_read_pdf::parse(data);
});
```

**Invariants to verify**:
- Parsers never panic on any input
- Emitters never panic on any valid Document
- `parse(emit(doc))` produces valid document if doc was valid

### 5. Fixture Tests

**Goal**: Test against real-world documents and known-good outputs.

**Sources**:
- Pandoc test suite: `~/git/pandoc/test/` (reference only, GPL)
- CommonMark spec examples
- GFM spec examples
- Real documents from documentation projects

**Approach**:
- Golden file testing: compare output against stored expected output
- Snapshot testing with `insta` crate
- Update snapshots intentionally when behavior changes

**Fixture categories**:
- [ ] CommonMark spec compliance
- [ ] GFM extensions (tables, task lists, strikethrough)
- [ ] Real README files from popular projects
- [ ] Academic papers (LaTeX)
- [ ] Documentation sites (HTML)

### 6. Compatibility Testing

**Goal**: Ensure interoperability with Pandoc ecosystem.

**Pandoc JSON bridge test**:
```bash
# Convert with Pandoc, read JSON, emit format, compare
pandoc input.md -t json | rescribe convert --from pandoc-json --to html
pandoc input.md -t html
# Compare outputs
```

**Test cases**:
- [ ] Simple documents match Pandoc output
- [ ] Complex documents have documented differences
- [ ] Pandoc JSON roundtrip preserves structure

### 7. Property-Based Testing

**Goal**: Verify invariants hold for all possible inputs.

**Properties to test**:

```rust
// Text content is preserved through conversion
proptest! {
    fn text_preserved(doc: ArbitraryDocument) {
        let emitted = emit(&doc);
        let reparsed = parse(&emitted);
        assert_eq!(extract_text(&doc), extract_text(&reparsed));
    }
}

// Structure is preserved through same-format roundtrip
proptest! {
    fn structure_preserved(doc: ArbitraryDocument) {
        let emitted = markdown::emit(&doc);
        let reparsed = markdown::parse(&emitted);
        assert_eq!(doc.structure(), reparsed.structure());
    }
}

// Fidelity warnings are accurate
proptest! {
    fn warnings_accurate(doc: ArbitraryDocument) {
        let result = latex::emit(&doc);
        for warning in &result.warnings {
            // Verify warned-about feature is actually lost
            assert!(feature_lost(&doc, &result.value, &warning));
        }
    }
}
```

**Document generators**:
- Random valid AST trees
- Random markdown/HTML strings (grammar-aware)
- Mutations of fixture documents

### 8. Performance Testing

**Goal**: Catch performance regressions, establish baselines.

**Benchmarks** (using `criterion`):
- Parse 1KB, 10KB, 100KB, 1MB documents per format
- Emit same sizes per format
- Full conversion pipeline benchmarks
- Memory usage for large documents

**Regression tracking**:
- CI runs benchmarks on each PR
- Compare against baseline, flag significant regressions
- Store historical data for trend analysis

## Implementation Priority

### Phase 1: Foundation
1. Set up fuzz testing infrastructure (`cargo-fuzz`)
2. Add Unicode edge case tests
3. Add structural edge case tests
4. Expand roundtrip tests for all formats

### Phase 2: Compatibility
1. Set up Pandoc comparison tests
2. Add CommonMark spec compliance tests
3. Create fixture test infrastructure with `insta`
4. Collect real-world test documents

### Phase 3: Advanced
1. Property-based testing with `proptest`
2. Performance benchmarks with `criterion`
3. CI integration for regression detection
4. Cross-format conversion matrix coverage

## Test Infrastructure

### Directory Structure
```
tests/
├── fixtures/           # Test input files
│   ├── markdown/
│   ├── html/
│   ├── latex/
│   └── expected/       # Golden files
├── edge_cases.rs       # Unicode, nesting, size tests
├── roundtrip.rs        # Same-format roundtrip tests
├── cross_format.rs     # Format conversion tests
├── compatibility.rs    # Pandoc comparison tests
└── common/
    └── mod.rs          # Shared test utilities

fuzz/
├── Cargo.toml
└── fuzz_targets/
    ├── markdown_reader.rs
    ├── html_reader.rs
    └── pdf_reader.rs

benches/
├── parsing.rs
└── emitting.rs
```

### Test Utilities Needed
- `assert_ast_eq`: Compare document structure ignoring spans
- `assert_text_eq`: Compare extracted text content
- `extract_text`: Get all text content from document
- `ArbitraryDocument`: Generate random valid documents
- `diff_documents`: Show structural differences

## Success Criteria

- All parsers survive 24h+ of fuzz testing without panics
- 100% of CommonMark spec examples pass
- Roundtrip tests pass for all format combinations
- No performance regressions > 10% in CI
- Edge case coverage for all categories above
