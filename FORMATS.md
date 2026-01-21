# Format Support Roadmap

Tracking Pandoc format parity for rescribe. Formats organized by implementation status and priority.

## Currently Implemented

| Format | Read | Write | Notes |
|--------|:----:|:-----:|-------|
| Markdown (CommonMark) | ✅ | ✅ | Via pulldown-cmark |
| HTML | ✅ | ✅ | Via html5ever |
| LaTeX | ✅ | ✅ | Basic support |
| Org-mode | ✅ | ✅ | Basic support |
| PDF | ✅ | - | Via pdf-extract (text only) |
| DOCX | ✅ | ✅ | Via ooxml-wml |
| Pandoc JSON | ✅ | ✅ | For Pandoc interop |
| Plain text | - | ✅ | Simple text extraction |

## Priority 1: High Value Formats

Popular formats with significant user demand.

| Format | Read | Write | Complexity | Notes |
|--------|:----:|:-----:|:----------:|-------|
| GFM (GitHub Markdown) | ✅ | ✅ | Low | Tables, task lists, strikethrough via pulldown-cmark |
| EPUB | ✅ | ✅ | Medium | Via epub/epub-builder crates |
| ODT (OpenDocument) | - | - | Medium | Similar to DOCX; can extend ooxml-opc |
| reStructuredText | ✅ | ✅ | Medium | Python ecosystem standard; handwritten parser |
| AsciiDoc | ✅ | ✅ | Medium | Popular in technical docs; handwritten parser |
| RTF | - | - | Medium | Legacy but still common |
| Typst | ✅ | ✅ | Medium | Modern LaTeX alternative, growing fast |
| PPTX | - | - | Medium | Waiting on ooxml-pml |
| XLSX | ✅ | - | Low | Via ooxml-sml |

## Priority 2: Technical/Academic Formats

Important for scholarly and technical writing.

| Format | Read | Write | Complexity | Notes |
|--------|:----:|:-----:|:----------:|-------|
| DocBook | ✅ | ✅ | Medium | XML-based; well-specified |
| JATS | ✅ | ✅ | Medium | Journal articles; XML-based |
| TEI | ✅ | ✅ | Medium | Digital humanities; XML-based |
| BibTeX/BibLaTeX | ✅ | ✅ | Low | Via biblatex crate |
| CSL JSON | ✅ | ✅ | Low | Citation Style Language |
| Jupyter (ipynb) | ✅ | ✅ | Low | JSON format; implemented |
| man/mdoc | - | - | Medium | Unix manual pages |
| Texinfo | - | - | Medium | GNU documentation |
| ConTeXt | - | - | High | TeX variant |
| Beamer | - | - | Medium | LaTeX slides; extend LaTeX writer |

## Priority 3: Wiki Formats

Various wiki markup languages.

| Format | Read | Write | Complexity | Notes |
|--------|:----:|:-----:|:----------:|-------|
| MediaWiki | ✅ | ✅ | Medium | Wikipedia syntax |
| DokuWiki | ✅ | ✅ | Low | Simple wiki syntax |
| Jira | - | - | Low | Atlassian markup |
| Creole | - | - | Low | Wiki standard |
| Textile | - | - | Low | Ruby ecosystem |
| TikiWiki | - | - | Low | |
| TWiki | - | - | Low | |
| XWiki | - | - | Low | |
| ZimWiki | - | - | Low | Desktop wiki |
| VimWiki | - | - | Low | Vim plugin format |

## Priority 4: Presentation Formats

Slide/presentation outputs.

| Format | Read | Write | Complexity | Notes |
|--------|:----:|:-----:|:----------:|-------|
| reveal.js | - | - | Low | HTML + metadata |
| Slidy | - | - | Low | W3C HTML slides |
| Slideous | - | - | Low | HTML slides |
| S5 | - | - | Low | HTML slides |
| dzslides | - | - | Low | HTML slides |

## Priority 5: Niche/Legacy Formats

Lower priority but good for completeness.

| Format | Read | Write | Complexity | Notes |
|--------|:----:|:-----:|:----------:|-------|
| FB2 (FictionBook) | - | - | Medium | Russian ebook format |
| OPML | ✅ | ✅ | Low | Outline format; via quick-xml |
| Haddock | - | - | Low | Haskell docs |
| Muse | - | - | Low | Emacs Muse |
| t2t (txt2tags) | - | - | Low | |
| pod | - | - | Low | Perl docs |
| ICML | - | - | High | Adobe InCopy |
| Markua | - | - | Low | Leanpub format |
| Djot | ✅ | ✅ | Low | Pandoc author's new format; via jotdown |
| BITS | - | - | Medium | Book Interchange Tag Suite |
| ANSI | - | ✅ | Low | Terminal colored output |
| BBCode variants | - | - | Low | Forum markup |

## Implementation Strategy

### Phase 1: Leverage Existing Work
- Wire up XLSX reading from ooxml-sml
- Add GFM extensions to Markdown
- Add Beamer output to LaTeX writer

### Phase 2: ZIP-based Formats
- EPUB (XHTML in ZIP)
- ODT (XML in ZIP, similar to DOCX)

### Phase 3: XML Formats
- DocBook, JATS, TEI share similar structure
- Can build common XML utilities

### Phase 4: Wiki Formats
- Many share similar patterns
- Could build a wiki-markup abstraction layer

### Phase 5: Remaining Formats
- Based on user demand
- Community contributions welcome

## Rust Ecosystem Libraries

Potentially useful crates:

| Crate | Purpose |
|-------|---------|
| `ooxml-wml` | DOCX (done) |
| `ooxml-sml` | XLSX reading |
| `ooxml-pml` | PPTX (in development) |
| `epub` | EPUB read/write |
| `quick-xml` | XML parsing (already used) |
| `comrak` | GFM-compatible Markdown |
| `asciidoc-parser` | AsciiDoc parsing |
| `syntect` | Syntax highlighting |

## Contributing

Format implementations welcome! See individual format sections for complexity estimates. Start with "Low" complexity formats for easier contributions.

When implementing a new format:
1. Create `rescribe-read-{format}` and/or `rescribe-write-{format}` crate
2. Add feature flag to main `rescribe` crate
3. Wire up in CLI
4. Add tests with real-world documents
5. Update this file
