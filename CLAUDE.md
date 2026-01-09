# CLAUDE.md

Behavioral rules for Claude Code in the rescribe repository.

## Project Overview

rescribe is a universal document conversion library, inspired by Pandoc but with:
- Open node kinds (not fixed enum)
- Property bags for extensibility
- Fidelity tracking (know what was lost)
- Embedded resource handling
- Roundtrip-friendly design

## Architecture

```
crates/
├── rescribe-core/       # Core IR: Document, Node, Properties, Resource
├── rescribe-markdown/   # Markdown parser/emitter (planned)
├── rescribe-html/       # HTML parser/emitter (planned)
├── rescribe-transforms/ # Standard transformers (planned)
└── rescribe-cli/        # CLI tool (planned)
```

## Key Types

- `Document` - Root container with content, resources, metadata
- `Node` - Tree node with kind, properties, children
- `NodeKind` - Open string type for node classification
- `Properties` - Key-value bag for node attributes
- `Resource` - Embedded binary (images, fonts, etc.)
- `ConversionResult<T>` - Result with fidelity warnings

## Traits

- `Parser` - Parse bytes → Document
- `Emitter` - Document → bytes
- `Transformer` - Document → Document

## Property Namespaces

- Semantic: `level`, `url`, `language`, etc.
- Style: `style:font`, `style:color`, etc.
- Layout: `layout:page_break`, `layout:float`, etc.
- Format-specific: `html:class`, `latex:env`, `docx:style`, etc.

## Development

```bash
nix develop        # Enter dev shell
cargo test         # Run tests
cargo clippy       # Lint
cd docs && bun dev # Local docs
```

## Conventions

- Crate names: `rescribe-{name}` (no org prefix)
- Node kinds: lowercase with underscores (`code_block`)
- Format-specific kinds: `{format}:{name}` (`latex:math`)
- Properties: lowercase, colons for namespacing
