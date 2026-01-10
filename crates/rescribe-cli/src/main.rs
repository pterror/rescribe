//! Rescribe CLI - Universal document converter.

use clap::{Parser, Subcommand};
use rescribe::{Document, html, latex, markdown, org, plaintext};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rescribe")]
#[command(author, version, about = "Universal document converter", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a document from one format to another
    Convert {
        /// Input file (use - for stdin)
        input: PathBuf,

        /// Output file (use - for stdout, or omit to use stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Input format (auto-detected from extension if not specified)
        #[arg(short, long)]
        from: Option<Format>,

        /// Output format (required if output is stdout or has no extension)
        #[arg(short, long)]
        to: Option<Format>,
    },

    /// List available formats
    Formats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Format {
    Markdown,
    Html,
    Latex,
    Org,
    Plaintext,
}

impl Format {
    fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "md" | "markdown" => Some(Format::Markdown),
            "html" | "htm" => Some(Format::Html),
            "tex" | "latex" => Some(Format::Latex),
            "org" => Some(Format::Org),
            "txt" | "text" => Some(Format::Plaintext),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Format::Markdown => "markdown",
            Format::Html => "html",
            Format::Latex => "latex",
            Format::Org => "org",
            Format::Plaintext => "plaintext",
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        match self {
            Format::Markdown => &["md", "markdown"],
            Format::Html => &["html", "htm"],
            Format::Latex => &["tex", "latex"],
            Format::Org => &["org"],
            Format::Plaintext => &["txt", "text"],
        }
    }

    fn can_read(&self) -> bool {
        matches!(self, Format::Markdown | Format::Html)
    }

    fn can_write(&self) -> bool {
        true // All formats can be written
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert {
            input,
            output,
            from,
            to,
        } => {
            convert(input, output, from, to)?;
        }
        Commands::Formats => {
            list_formats();
        }
    }

    Ok(())
}

fn convert(
    input: PathBuf,
    output: Option<PathBuf>,
    from: Option<Format>,
    to: Option<Format>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Determine input format
    let input_format = from
        .or_else(|| {
            if input.as_os_str() == "-" {
                None
            } else {
                input
                    .extension()
                    .and_then(|e| e.to_str())
                    .and_then(Format::from_extension)
            }
        })
        .ok_or("Cannot determine input format. Use --from to specify.")?;

    if !input_format.can_read() {
        return Err(format!("No reader available for {} format", input_format.name()).into());
    }

    // Determine output format
    let output_format = to
        .or_else(|| {
            output.as_ref().and_then(|p| {
                if p.as_os_str() == "-" {
                    None
                } else {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .and_then(Format::from_extension)
                }
            })
        })
        .ok_or("Cannot determine output format. Use --to to specify.")?;

    // Read input
    let input_text = if input.as_os_str() == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        fs::read_to_string(&input)?
    };

    // Parse
    let doc = parse(&input_text, input_format)?;

    // Emit
    let output_bytes = emit(&doc, output_format)?;

    // Write output
    match output {
        Some(path) if path.as_os_str() != "-" => {
            fs::write(&path, &output_bytes)?;
        }
        _ => {
            io::stdout().write_all(&output_bytes)?;
        }
    }

    Ok(())
}

fn parse(input: &str, format: Format) -> Result<Document, Box<dyn std::error::Error>> {
    let result = match format {
        Format::Markdown => markdown::parse(input)?,
        Format::Html => html::parse(input)?,
        Format::Latex | Format::Org | Format::Plaintext => {
            return Err(format!("No reader for {} format", format.name()).into());
        }
    };

    // Report warnings to stderr
    for warning in &result.warnings {
        eprintln!("warning: {}", warning.message);
    }

    Ok(result.value)
}

fn emit(doc: &Document, format: Format) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let result = match format {
        Format::Markdown => markdown::emit(doc)?,
        Format::Html => html::emit(doc)?,
        Format::Latex => latex::emit(doc)?,
        Format::Org => org::emit(doc)?,
        Format::Plaintext => plaintext::emit(doc)?,
    };

    // Report warnings to stderr
    for warning in &result.warnings {
        eprintln!("warning: {}", warning.message);
    }

    Ok(result.value)
}

fn list_formats() {
    println!("Available formats:\n");
    println!("  {:12} {:6} {:6}  EXTENSIONS", "FORMAT", "READ", "WRITE");
    println!("  {:12} {:6} {:6}  ----------", "------", "----", "-----");

    let formats = [
        Format::Markdown,
        Format::Html,
        Format::Latex,
        Format::Org,
        Format::Plaintext,
    ];

    for fmt in formats {
        let read = if fmt.can_read() { "yes" } else { "-" };
        let write = if fmt.can_write() { "yes" } else { "-" };
        let exts = fmt.extensions().join(", ");
        println!("  {:12} {:6} {:6}  {}", fmt.name(), read, write, exts);
    }
}
