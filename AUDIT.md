# Rescribe Format Audit Tracker

Systematic quality review of all readers and writers.

## Audit Criteria

Each format is evaluated on:

| Criteria | Description |
|----------|-------------|
| **Parsing** | Handles valid input correctly |
| **Edge Cases** | Handles malformed/unusual input gracefully |
| **Completeness** | Supports all format features (or documents gaps) |
| **Fidelity** | Emits warnings when losing information |
| **Tests** | Has adequate test coverage |
| **Roundtrip** | parse → emit → parse produces equivalent result |

## Status Legend

- `[ ]` Not reviewed
- `[~]` Reviewed, has issues
- `[x]` Reviewed, acceptable quality

---

## Core Formats (Priority 1)

### Markdown

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[x]` | Good coverage. TOML frontmatter, nested YAML (dot notation), arrays as PropValue::List. Tree-sitter preserves source info for roundtripping. |
| Writer | `[x]` | Excellent source info preservation. Definition list syntax, comprehensive escaping, table alignment support. |

### HTML

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### LaTeX

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Org-mode

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### DOCX

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### ODT

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## Secondary Formats (Priority 2)

### CommonMark

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### GFM (GitHub Flavored Markdown)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Markdown Strict

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### MultiMarkdown

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Djot

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### reStructuredText

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### AsciiDoc

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Typst

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### EPUB

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### PDF

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | Text extraction only |
| Writer | N/A | |

### Plaintext

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

---

## Data/Bibliography Formats

### BibTeX

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### BibLaTeX

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### CSL-JSON

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### RIS

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### EndNote XML

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### CSV

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### TSV

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## Office Formats

### XLSX

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### PPTX

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Jupyter Notebook (ipynb)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### RTF

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## XML/SGML Formats

### DocBook

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### JATS

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### TEI

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### OPML

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### FB2 (FictionBook)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## Wiki Formats

### MediaWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### DokuWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### TikiWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### TWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### XWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### VimWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### ZimWiki

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Creole

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## Technical Documentation Formats

### Man pages

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Texinfo

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### POD (Plain Old Documentation)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Haddock

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## Presentation Formats (Write-only)

### reveal.js

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### Slidy

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### S5

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### DZSlides

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### Slideous

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### Beamer

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

---

## Other Markup Formats

### Textile

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Muse

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### txt2tags (t2t)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Jira

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### BBCode

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Markua (Leanpub)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Fountain (Screenplay)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### ANSI

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

---

## Specialized Formats

### Native (rescribe IR)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | `[ ]` | |
| Writer | `[ ]` | |

### Chunked HTML

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### ConTeXt

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### ms (groff)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

### ICML (InDesign)

| Component | Status | Notes |
|-----------|--------|-------|
| Reader | N/A | |
| Writer | `[ ]` | |

---

## Summary

| Category | Readers | Writers | Reviewed |
|----------|---------|---------|----------|
| Core | 6 | 6 | 2/12 |
| Secondary | 10 | 11 | 0/21 |
| Data/Bib | 5 | 5 | 0/10 |
| Office | 4 | 4 | 0/8 |
| XML/SGML | 5 | 5 | 0/10 |
| Wiki | 8 | 8 | 0/16 |
| Tech Docs | 4 | 4 | 0/8 |
| Presentation | 0 | 6 | 0/6 |
| Other Markup | 9 | 9 | 0/18 |
| Specialized | 1 | 4 | 0/5 |
| **Total** | **52** | **62** | **2/114** |

---

## Audit Log

| Date | Format | Reviewer | Findings |
|------|--------|----------|----------|
| 2026-01-23 | Markdown Reader | Claude | TOML frontmatter not parsed, nested YAML ignored, arrays flattened |
| 2026-01-23 | Markdown Writer | Claude | Definition list syntax wrong, escape incomplete, table alignment lost |
| 2026-01-23 | Markdown Reader | Claude | FIXED: Added TOML frontmatter, nested YAML with dot notation, arrays as PropValue::List |
| 2026-01-23 | Markdown Writer | Claude | FIXED: Definition list syntax, comprehensive escaping, table alignment |
