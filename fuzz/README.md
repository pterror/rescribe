# Fuzz Testing

This directory contains fuzz targets for rescribe's readers.

## Requirements

Fuzz testing requires **nightly Rust** due to sanitizer features.

### Using Nix (recommended)

```bash
# Enter the fuzz shell with nightly Rust
nix develop .#fuzz

# Then run fuzz commands normally
cargo fuzz build
cargo fuzz run fuzz_markdown_reader
```

### Using rustup

```bash
rustup default nightly
# or
rustup run nightly cargo fuzz <command>
```

## Available Targets

### Reader Targets (should never panic)
- `fuzz_markdown_reader` - Markdown parser
- `fuzz_html_reader` - HTML parser
- `fuzz_latex_reader` - LaTeX parser
- `fuzz_org_reader` - Org-mode parser
- `fuzz_pdf_reader` - PDF parser (binary input)
- `fuzz_pandoc_json_reader` - Pandoc JSON parser

### Roundtrip Targets (text content preservation)
- `fuzz_markdown_roundtrip` - Parse → emit → parse, verify text preserved
- `fuzz_html_roundtrip` - Parse → emit → parse, verify text preserved

## Usage

```bash
# Build all fuzz targets
cargo fuzz build

# Run a specific target
cargo fuzz run fuzz_markdown_reader

# Run with timeout (seconds)
cargo fuzz run fuzz_markdown_reader -- -max_total_time=60

# Run with specific corpus
cargo fuzz run fuzz_markdown_reader corpus/markdown/

# List all targets
cargo fuzz list
```

## Corpus

Fuzzer will create corpus directories automatically. You can seed them with
interesting inputs:

```bash
mkdir -p corpus/fuzz_markdown_reader
cp ~/git/pandoc/test/*.txt corpus/fuzz_markdown_reader/
```

## Crashes

Crashes are saved to `artifacts/fuzz_<target>/`. To reproduce:

```bash
cargo fuzz run fuzz_markdown_reader artifacts/fuzz_markdown_reader/crash-<hash>
```
