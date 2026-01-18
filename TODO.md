# Rescribe MVP Backlog

## Priority 1: Usability

- [x] **CLI tool** (`rescribe-cli` crate)
  - `rescribe convert input.md -o output.html`
  - `rescribe convert input.md --to latex` (stdout)
  - Auto-detect input format, explicit output format
  - `--list-formats` to show available readers/writers

- [x] **Metadata handling**
  - YAML frontmatter parsing in Markdown reader
  - HTML `<meta>` tag extraction in HTML reader
  - Populate `Document.metadata` field
  - Emit metadata in writers where applicable

- [x] **Resource embedding**
  - Wire up `ParseOptions::embed_resources`
  - Populate `Document.resources` in readers
  - Emit embedded resources (data URIs, inline) in writers
  - At minimum: HTML→HTML roundtrip with images

## Priority 2: Options

- [x] **Implement ParseOptions**
  - [x] `embed_resources`: Actually embed images/resources
  - [x] `preserve_source_info`: Track source spans and formatting style hints

- [x] **Implement EmitOptions**
  - [x] `pretty`: Pretty-print output (HTML writer)
  - [x] `use_source_info`: Use original formatting hints (heading style, markers, fences)

## Priority 3: Additional Readers

- [x] **LaTeX reader** (basic support for sections, lists, verbatim, math)
- [x] **Org-mode reader** (handwritten parser with headings, lists, emphasis, code blocks, metadata)

## Future

- [x] Transforms crate (`rescribe-transforms`) - ShiftHeadings, StripEmpty, MergeText, UnwrapSingleChild, Pipeline
- [x] Pandoc JSON compatibility layer (`rescribe-read-pandoc-json`, `rescribe-write-pandoc-json`)
- [ ] DOCX reader/writer - **BLOCKED**: waiting on `ooxml` crate (see ~/git/ooxml)
- [x] PDF reader (text extraction via pdf-extract, with fidelity warnings about structural loss)
