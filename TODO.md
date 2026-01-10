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

- [ ] **Implement ParseOptions**
  - [x] `embed_resources`: Actually embed images/resources
  - [ ] `preserve_source_info`: Track source spans

- [ ] **Implement EmitOptions**
  - [x] `pretty`: Pretty-print output (HTML writer)
  - [ ] `use_source_info`: Use original formatting hints

## Priority 3: Additional Readers

- [x] **LaTeX reader** (basic support for sections, lists, verbatim, math)
- [ ] **Org-mode reader** (currently write-only)

## Future

- [ ] Transforms crate (`rescribe-transforms`)
- [ ] DOCX reader/writer
- [ ] PDF reader
- [ ] Pandoc JSON compatibility layer
