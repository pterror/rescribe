# Introduction

**rescribe** is a universal document conversion library, inspired by Pandoc but designed to address its limitations.

## Why rescribe?

Document conversion is a common need: Markdown to PDF, DOCX to HTML, etc. Current solutions make tradeoffs that lose information or limit interoperability.

### The Problem with Pandoc

Pandoc is the de facto standard for document conversion, but it has fundamental limitations:

| Issue | Description |
|-------|-------------|
| **Lossy by design** | AST is "least common denominator" - format-specific features lost on parse |
| **No layout layer** | AST is purely semantic - no page breaks, columns, positioning |
| **Poor roundtrip fidelity** | A→B→A loses information (DOCX styles, HTML classes, LaTeX macros) |
| **References-only media** | Images are paths, not embedded - no unified resource handling |
| **Fixed schema** | Adding element types requires changing Pandoc itself |

### rescribe's Approach

1. **Property bags over fixed schemas** - Elements carry extensible properties, not fixed fields
2. **Layered representation** - Separate semantic, style, and layout concerns
3. **Fidelity tracking** - Know what was lost, warn about it
4. **Embedded resources** - First-class handling of images, fonts, data
5. **Roundtrip-friendly** - Preserve source format info for better reconstruction

## Quick Example

```rust
use rescribe::{Document, Parser, Emitter};
use rescribe_markdown::MarkdownParser;
use rescribe_html::HtmlEmitter;

let parser = MarkdownParser::new();
let emitter = HtmlEmitter::new();

// Parse markdown to IR
let result = parser.parse(markdown_bytes, &ParseOptions::default())?;

// Check for fidelity warnings
for warning in &result.warnings {
    eprintln!("Warning: {}", warning.message);
}

// Emit to HTML
let output = emitter.emit(&result.value, &EmitOptions::default())?;
```

## Project Status

rescribe is in early development. The core IR types are being stabilized.

### Planned Format Support

**Tier 1: Pure Rust**
- Markdown (CommonMark, GFM)
- HTML
- Plain text
- JSON (serialized IR)

**Tier 2: Pure Rust (complex)**
- PDF (emit via typst/printpdf)
- EPUB
- LaTeX

**Tier 3: External tools**
- DOCX, XLSX, PPTX (via LibreOffice)
- ODT, ODS, ODP
