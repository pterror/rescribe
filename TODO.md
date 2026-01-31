# Rescribe Backlog

## Blocked

- [ ] Pre-commit hook blocks all commits — `ooxml-wml` git dependency (rev `3aa50763`) has broken import (`types::ns` no longer exists). Clippy fails on it. Need to update the ooxml pin or patch the dep.
- [ ] CLAUDE.md and hook changes staged but uncommitted (auto-format hook + workflow guidance) — commit once clippy passes

## Completed

- [x] CLI tool (`rescribe-cli`)
- [x] Metadata handling (YAML frontmatter, HTML meta tags)
- [x] Resource embedding (images, data URIs)
- [x] ParseOptions / EmitOptions implementation
- [x] Transforms crate (ShiftHeadings, StripEmpty, MergeText, etc.)
- [x] Pandoc JSON compatibility layer
- [x] DOCX reader/writer (via ooxml crate)
- [x] PDF reader (text extraction)
- [x] 54 readers, 64 writers - comprehensive format coverage

## Priority 1: Quality Audit

- [ ] **Review existing readers** - check for:
  - Edge cases and malformed input handling
  - Completeness of element support
  - Proper fidelity warnings when losing information
  - Test coverage

- [ ] **Review existing writers** - check for:
  - Output validity (well-formed HTML/XML/etc.)
  - Roundtrip accuracy (parse → emit → parse)
  - Escaping and special character handling
  - Missing node type handlers

- [ ] **Core format deep-dive** (highest priority):
  - Markdown (all variants)
  - HTML
  - LaTeX
  - DOCX/ODT
  - Org-mode

## Priority 2: Infrastructure

- [ ] **Roundtrip tests** - automated format A → B → A comparison
- [ ] **Pandoc fixture tests** - validate against Pandoc test suite
- [ ] **Fuzz testing** - catch crashes on malformed input
- [ ] **Documentation** - API docs, format support matrix

## Someday/Maybe: Niche Formats

Low priority formats that could be added later if there's demand:

- [ ] Gemtext (Gemini protocol markup)
- [ ] Mermaid (diagram markup)
- [ ] PlantUML (UML diagrams)
- [ ] GraphViz DOT (graph descriptions)
- [ ] PHP Markdown Extra
- [ ] Setext (original lightweight markup)
- [ ] troff/nroff variants
- [ ] DITA (technical documentation)
- [ ] Confluence wiki markup
- [ ] Notion export format
- [ ] Roam Research export
- [ ] Logseq export
